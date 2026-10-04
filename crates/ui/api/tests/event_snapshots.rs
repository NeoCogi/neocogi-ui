//! Concrete event interfaces remain callable independently of widgets and native enums.
use neocogi_com::{ComBox, INcStringComPtrExt, NcStatus, NC_E_DROPPED, NC_E_INVALID_ARG};
use neocogi_com::events::{EventPayload, IEvent, IEventAbi, downcast_event};
use neocogi_ui_core_contracts::*;
use neocogi_ui_nodes_contracts::*;

/// Activation owns its text and only the event owner controls projection lifetime.
#[test]
fn activation_is_an_exclusive_immutable_snapshot() {
    let event = ButtonEvent::Activated(Activation { label: "Save α".into() }).into_event().unwrap();
    let typed = downcast_event::<IActivatedEventAbi>(&event).unwrap().unwrap();
    assert_eq!(typed.label().unwrap().to_string().unwrap(), "Save α");
    assert!(downcast_event::<ITextChangedEventAbi>(&event).unwrap().is_none());
    assert!(matches!(ButtonEvent::from_event(&event).unwrap(), Some(ButtonEvent::Activated(value)) if value.label == "Save α"));
    drop(event);
    assert_eq!(typed.label().unwrap_err(), NC_E_DROPPED);
}

/// Text change and submission expose distinct capabilities instead of sentinel fields.
#[test]
fn text_change_and_submission_are_distinct_typed_events() {
    let changed = TextboxEvent::Changed(TextChanged { text: "éclair".into(), cursor: 2 }).into_event().unwrap();
    let typed = downcast_event::<ITextChangedEventAbi>(&changed).unwrap().unwrap();
    assert_eq!(typed.text().unwrap().to_string().unwrap(), "éclair");
    assert_eq!(typed.cursor(), Ok(2));
    assert!(downcast_event::<ITextSubmittedEventAbi>(&changed).unwrap().is_none());
    let submitted = TextAreaEvent::Submitted(TextSubmitted { text: "line\ntwo".into() }).into_event().unwrap();
    assert_eq!(
        downcast_event::<ITextSubmittedEventAbi>(&submitted)
            .unwrap()
            .unwrap()
            .text()
            .unwrap()
            .to_string()
            .unwrap(),
        "line\ntwo"
    );
    assert!(TextChanged::from_event(&submitted).unwrap().is_none());
    assert!(matches!(TextAreaEvent::from_event(&changed).unwrap(), Some(TextAreaEvent::Changed(value)) if value.cursor == 2));
    assert_eq!(TextChanged { text: "é".into(), cursor: 1 }.into_event().unwrap_err(), NC_E_INVALID_ARG);
    assert_eq!(TextChanged { text: "x".into(), cursor: 2 }.into_event().unwrap_err(), NC_E_INVALID_ARG);
}

/// All scalar interfaces expose their committed field with the same validation as native callers.
#[test]
fn scalar_events_keep_domain_invariants() {
    let checkbox = CheckboxEvent::Changed(CheckboxChanged { checked: true }).into_event().unwrap();
    assert_eq!(downcast_event::<ICheckboxChangedEventAbi>(&checkbox).unwrap().unwrap().checked(), Ok(true));
    let slider = SliderEvent::Changed(NumericChanged { value: 0.75 }).into_event().unwrap();
    assert_eq!(downcast_event::<INumericChangedEventAbi>(&slider).unwrap().unwrap().value(), Ok(0.75));
    assert!(matches!(NumberEvent::from_event(&slider).unwrap(), Some(NumberEvent::Changed(value)) if value.value == 0.75));
    let scroll = ScrollbarEvent::Changed(ScrollbarChanged { offset: 42 }).into_event().unwrap();
    assert_eq!(downcast_event::<IScrollbarChangedEventAbi>(&scroll).unwrap().unwrap().offset(), Ok(42));
    let split = SplitViewEvent::Changed(SplitChanged { position: 73 }).into_event().unwrap();
    assert_eq!(downcast_event::<ISplitChangedEventAbi>(&split).unwrap().unwrap().position(), Ok(73));
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(NumericChanged { value }.into_event().unwrap_err(), NC_E_INVALID_ARG);
    }
    assert_eq!(ScrollbarChanged { offset: -1 }.into_event().unwrap_err(), NC_E_INVALID_ARG);
    assert_eq!(SplitChanged { position: -1 }.into_event().unwrap_err(), NC_E_INVALID_ARG);
}

#[neocogi_com::com_object(interfaces(neocogi_ui_nodes_contracts::INumericChangedEvent), ownership = exclusive)]
/// A foreign provider can return ABI-valid floats outside the event's semantic domain.
struct InvalidNumeric;
impl IEvent for InvalidNumeric {}
impl INumericChangedEvent for InvalidNumeric {
    /// NaN is representable on the wire but must not enter native numeric handlers.
    fn value(&self) -> Result<f32, NcStatus> {
        Ok(f32::NAN)
    }
}

/// Native convenience decoding revalidates foreign getters instead of trusting construction.
#[test]
fn foreign_event_getters_are_validated() {
    let event: ComBox<IEventAbi> = InvalidNumeric::into_box(InvalidNumeric).query().unwrap();
    assert_eq!(NumericChanged::from_event(&event).unwrap_err(), NC_E_INVALID_ARG);
    assert_eq!(SliderEvent::from_event(&event).unwrap_err(), NC_E_INVALID_ARG);
    assert!(CheckboxEvent::from_event(&event).unwrap().is_none());
}

/// Copied lifecycle and selection values retain their exact variant and geometry.
#[test]
fn service_events_keep_lifecycle_and_selection_facts() {
    let rect = Rect { x: -5, y: 12, width: 90, height: 70 };
    let window = WindowEvent::GeometryChanged { rect }.into_event().unwrap();
    assert!(matches!(downcast_event::<IWindowEventAbi>(&window).unwrap().unwrap().value().unwrap(), WindowEvent::GeometryChanged { rect: got } if got == rect));
    assert_eq!(
        WindowEvent::Restored { rect: Rect { width: -1, ..rect } }.into_event().unwrap_err(),
        NC_E_INVALID_ARG
    );
    let popup = PopupDismissed::Dismissed.into_event().unwrap();
    assert_eq!(
        downcast_event::<IPopupDismissedEventAbi>(&popup).unwrap().unwrap().value(),
        Ok(PopupDismissed::Dismissed)
    );
    for selection in [ChoiceSelection::None, ChoiceSelection::Selected(0), ChoiceSelection::Selected(u64::MAX)] {
        let event = IChoiceMenuEvent::Changed(selection).into_event().unwrap();
        let restored = downcast_event::<IChoiceChangedEventAbi>(&event).unwrap().unwrap().selection().unwrap();
        assert_eq!(restored, selection);
    }
}

/// Accepted path identity and cancellation each have their own concrete COM interface.
#[test]
fn file_dialog_events_preserve_native_paths_and_cancellation() {
    let original = std::path::Path::new("directory/example file.txt");
    let path = neocogi_ui::NativePath::new(original);
    let accepted = IFileDialogEvent::Completed(FileDialogCompletion::Accepted { path: path.as_interface().add_ref() })
        .into_event()
        .unwrap();
    drop(path);
    let typed = downcast_event::<IFileAcceptedEventAbi>(&accepted).unwrap().unwrap();
    assert_eq!(neocogi_ui::path::decode(typed.path().unwrap().as_com_ref()).unwrap(), original);
    assert!(downcast_event::<IFileCancelledEventAbi>(&accepted).unwrap().is_none());
    assert!(matches!(
        IFileDialogEvent::from_event(&accepted).unwrap(),
        Some(IFileDialogEvent::Completed(FileDialogCompletion::Accepted { .. }))
    ));
    let cancelled = FileDialogCompletion::Cancelled.into_event().unwrap();
    assert!(downcast_event::<IFileCancelledEventAbi>(&cancelled).unwrap().is_some());
    assert!(downcast_event::<IFileAcceptedEventAbi>(&cancelled).unwrap().is_none());
    assert!(matches!(
        FileDialogCompletion::from_event(&cancelled).unwrap(),
        Some(FileDialogCompletion::Cancelled)
    ));
}

/// The production channel moves the same allocation and restores it on rejection.
#[test]
fn common_channel_retains_exclusive_owner_and_returns_it_when_disconnected() {
    use neocogi_com::{NcValueRef, NC_E_CLOSED};
    use neocogi_com::events::{channel, EventOrigin, IEventReceiverAbi};
    let (source, _source_inbox) = channel();
    let origin = EventOrigin::Service(source.downgrade().unwrap().into_unknown());
    let (mut queue, inbox) = channel();
    let event = Activation { label: "captured".into() }.into_event().unwrap();
    let allocation = event.as_raw();
    let projection = downcast_event::<IActivatedEventAbi>(&event).unwrap().unwrap();
    queue.post(NcValueRef::new(&origin).unwrap(), event).unwrap();
    drop(source);
    let message = inbox.try_recv().unwrap();
    assert_eq!(message.event.as_raw(), allocation);
    assert!(message.origin.service::<IEventReceiverAbi>().unwrap().unwrap().upgrade().unwrap().is_none());
    assert_eq!(projection.label().unwrap().to_string().unwrap(), "captured");
    drop(inbox);
    let (status, returned) = queue.post(NcValueRef::new(&origin).unwrap(), message.event).unwrap_err();
    assert_eq!(status, NC_E_CLOSED);
    assert_eq!(returned.as_raw(), allocation);
    assert_eq!(projection.label().unwrap().to_string().unwrap(), "captured");
    drop(returned);
    assert_eq!(projection.label().unwrap_err(), NC_E_DROPPED);
}
