//! Concrete COM snapshots are usable independently of widget construction or Rust event enums.
use neocogi_com::{ComBox, INcStringComPtrExt, NcStatus, NC_E_DROPPED, NC_E_INVALID_ARG};
use neocogi_com::events::{IEvent, IEventAbi, downcast_event};
use neocogi_ui_core_contracts::*;
use neocogi_ui_nodes_contracts::*;
use neocogi_ui::events as snapshot;

/// Text storage is copied once and its shared getter result outlives the event.
#[test]
fn activation_is_an_exclusive_immutable_snapshot() {
    let mut label = String::from("Save α");
    let event = snapshot::activated(&label).unwrap();
    label.clear();
    let typed = downcast_event::<IActivatedEventAbi>(&event).unwrap().unwrap();
    let text = typed.label().unwrap();
    assert_eq!(text.to_string().unwrap(), "Save α");
    assert!(text.as_com_ref().same_object(typed.label().unwrap().as_com_ref()));
    assert!(downcast_event::<ITextChangedEventAbi>(&event).unwrap().is_none());
    drop(event);
    assert_eq!(typed.label().unwrap_err(), NC_E_DROPPED);
    assert_eq!(text.to_string().unwrap(), "Save α");
}

/// Separate interfaces distinguish changes from submissions, including empty text.
#[test]
fn text_change_and_submission_are_distinct_typed_events() {
    let changed = snapshot::text_changed("éclair", 2).unwrap();
    let typed = downcast_event::<ITextChangedEventAbi>(&changed).unwrap().unwrap();
    assert_eq!(typed.text().unwrap().to_string().unwrap(), "éclair");
    assert_eq!(typed.cursor(), Ok(2));
    assert!(downcast_event::<ITextSubmittedEventAbi>(&changed).unwrap().is_none());
    for text in ["line\ntwo", ""] {
        let submitted = snapshot::text_submitted(text).unwrap();
        assert_eq!(
            downcast_event::<ITextSubmittedEventAbi>(&submitted)
                .unwrap()
                .unwrap()
                .text()
                .unwrap()
                .to_string()
                .unwrap(),
            text
        );
        assert!(downcast_event::<ITextChangedEventAbi>(&submitted).unwrap().is_none());
    }
    assert_eq!(snapshot::text_changed("é", 1).unwrap_err(), NC_E_INVALID_ARG);
    assert_eq!(snapshot::text_changed("x", 2).unwrap_err(), NC_E_INVALID_ARG);
    assert_eq!(snapshot::text_changed("", u64::MAX).unwrap_err(), NC_E_INVALID_ARG);
    assert_eq!(
        downcast_event::<ITextChangedEventAbi>(&snapshot::text_changed("", 0).unwrap())
            .unwrap()
            .unwrap()
            .cursor(),
        Ok(0)
    );
}

/// Producers reject invalid domain values before publishing an event owner.
#[test]
fn scalar_events_keep_domain_invariants() {
    let checkbox = snapshot::checkbox_changed(true).unwrap();
    assert_eq!(downcast_event::<ICheckboxChangedEventAbi>(&checkbox).unwrap().unwrap().checked(), Ok(true));
    let numeric = snapshot::numeric_changed(0.75).unwrap();
    assert_eq!(downcast_event::<INumericChangedEventAbi>(&numeric).unwrap().unwrap().value(), Ok(0.75));
    let scroll = snapshot::scrollbar_changed(42).unwrap();
    assert_eq!(downcast_event::<IScrollbarChangedEventAbi>(&scroll).unwrap().unwrap().offset(), Ok(42));
    let split = snapshot::split_changed(73).unwrap();
    assert_eq!(downcast_event::<ISplitChangedEventAbi>(&split).unwrap().unwrap().position(), Ok(73));
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(snapshot::numeric_changed(value).unwrap_err(), NC_E_INVALID_ARG);
    }
    assert_eq!(snapshot::scrollbar_changed(-1).unwrap_err(), NC_E_INVALID_ARG);
    assert_eq!(snapshot::split_changed(-1).unwrap_err(), NC_E_INVALID_ARG);
}

#[neocogi_com::com_object(interfaces(neocogi_ui_nodes_contracts::INumericChangedEvent), ownership = exclusive)]
/// A foreign provider can reject a getter even after successful QueryInterface.
struct RejectedNumeric;
impl IEvent for RejectedNumeric {}
impl INumericChangedEvent for RejectedNumeric {
    /// Preserve the provider status for the caller to handle.
    fn value(&self) -> Result<f32, NcStatus> {
        Err(NC_E_INVALID_ARG)
    }
}

/// Unsupported type is None; errors from a supported getter stay explicit.
#[test]
fn foreign_event_getter_errors_remain_visible() {
    let event: ComBox<IEventAbi> = RejectedNumeric::into_box(RejectedNumeric).query().unwrap();
    assert_eq!(
        downcast_event::<INumericChangedEventAbi>(&event).unwrap().unwrap().value(),
        Err(NC_E_INVALID_ARG)
    );
    assert!(downcast_event::<ICheckboxChangedEventAbi>(&event).unwrap().is_none());
}

/// Every lifecycle fact has its own interface and shares the window marker ancestry.
#[test]
fn service_events_keep_lifecycle_and_selection_facts() {
    let rect = Rect { x: -5, y: 12, width: 90, height: 70 };
    macro_rules! geometry {
        ($construct:ident, $interface:ty) => {{
            let event = snapshot::$construct(rect).unwrap();
            assert_eq!(downcast_event::<$interface>(&event).unwrap().unwrap().rect(), Ok(rect));
            assert!(downcast_event::<IWindowEventAbi>(&event).unwrap().is_some());
            assert!(downcast_event::<IWindowHiddenEventAbi>(&event).unwrap().is_none());
            assert_eq!(snapshot::$construct(Rect { width: -1, ..rect }).unwrap_err(), NC_E_INVALID_ARG);
            assert_eq!(snapshot::$construct(Rect { height: -1, ..rect }).unwrap_err(), NC_E_INVALID_ARG);
        }};
    }
    geometry!(window_geometry_changed, IWindowGeometryChangedEventAbi);
    geometry!(window_minimized, IWindowMinimizedEventAbi);
    geometry!(window_maximized, IWindowMaximizedEventAbi);
    geometry!(window_restored, IWindowRestoredEventAbi);
    macro_rules! marker {
        ($construct:ident, $interface:ty) => {{
            let event = snapshot::$construct().unwrap();
            assert!(downcast_event::<$interface>(&event).unwrap().is_some());
            assert!(downcast_event::<IWindowEventAbi>(&event).unwrap().is_some());
            assert!(downcast_event::<IWindowGeometryChangedEventAbi>(&event).unwrap().is_none());
        }};
    }
    marker!(window_close_requested, IWindowCloseRequestedEventAbi);
    marker!(window_hidden, IWindowHiddenEventAbi);
    marker!(window_destroyed, IWindowDestroyedEventAbi);
    let popup = snapshot::popup_dismissed().unwrap();
    assert!(downcast_event::<IPopupDismissedEventAbi>(&popup).unwrap().is_some());
    assert!(downcast_event::<IWindowEventAbi>(&popup).unwrap().is_none());
    for selection in [ChoiceSelection::None, ChoiceSelection::Selected(0), ChoiceSelection::Selected(u64::MAX)] {
        let event = snapshot::choice_changed(selection).unwrap();
        assert_eq!(downcast_event::<IChoiceChangedEventAbi>(&event).unwrap().unwrap().selection(), Ok(selection));
    }
}

/// Accepted native paths and cancellation remain distinct after the producer is gone.
#[test]
fn file_dialog_events_preserve_native_paths_and_cancellation() {
    let original = std::path::Path::new("directory/example file.txt");
    let path = neocogi_ui::NativePath::new(original);
    let accepted = snapshot::file_accepted(path.as_interface().add_ref()).unwrap();
    drop(path);
    let typed = downcast_event::<IFileAcceptedEventAbi>(&accepted).unwrap().unwrap();
    assert_eq!(neocogi_ui::path::decode(typed.path().unwrap().as_com_ref()).unwrap(), original);
    assert!(downcast_event::<IFileCancelledEventAbi>(&accepted).unwrap().is_none());
    let cancelled = snapshot::file_cancelled().unwrap();
    assert!(downcast_event::<IFileCancelledEventAbi>(&cancelled).unwrap().is_some());
    assert!(downcast_event::<IFileAcceptedEventAbi>(&cancelled).unwrap().is_none());
}

/// The production channel moves the same allocation and restores it on rejection.
#[test]
fn common_channel_retains_exclusive_owner_and_returns_it_when_disconnected() {
    use neocogi_com::{NcValueRef, NC_E_CLOSED};
    use neocogi_com::events::{channel, EventOrigin, IEventReceiverAbi};
    let (source, _source_inbox) = channel();
    let origin = EventOrigin::Service(source.downgrade().unwrap().into_unknown());
    let (mut queue, inbox) = channel();
    let event = snapshot::activated("captured").unwrap();
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
