//! Concrete COM snapshots are usable independently of widget construction or Rust event enums.
// Link this executable's runtime explicitly, including on MSVC without section GC.
extern crate neocogi_com_host as _;

use neocogi_com::{ComBox, INcStringComPtrExt, NcStatus, NC_E_DROPPED, NC_E_INVALID_ARG, NC_E_NOINTERFACE};
use neocogi_ui_contracts::events::{IEvent, IEventAbi};
use neocogi_ui_contracts::abi::core::*;
use neocogi_ui_contracts::abi::nodes::*;
use neocogi_ui_contracts::events as snapshot;

/// Activation carries occurrence and ancestry only; no copied display text exists.
#[test]
fn activation_is_an_exclusive_marker() {
    let event = snapshot::activated().unwrap();
    let view = event.query_ref::<IActivatedEventAbi>().unwrap();
    assert_eq!(view.as_raw().object, event.as_raw().object);
    assert_eq!(IActivatedEventAbi::INTERFACE_DESCRIPTOR.method_count, 0);
    let observer = event.query_weak::<IActivatedEventAbi>().unwrap();
    assert_eq!(event.query_weak::<ITextChangedEventAbi>().unwrap_err(), NC_E_NOINTERFACE);
    assert!(observer.is_alive());
    drop(event);
    assert!(!observer.is_alive());
}

/// Immutable text getters share one captured string and can outlive the event.
#[test]
fn text_submission_retains_its_snapshot_independently_of_the_producer() {
    let mut input = String::from("Save α");
    let event = snapshot::text_submitted(&input).unwrap();
    input.clear();
    let typed = event.query_weak::<ITextSubmittedEventAbi>().unwrap();
    let text = typed.text().unwrap();
    assert_eq!(text.to_string().unwrap(), "Save α");
    assert!(text.as_com_ref().same_object(typed.text().unwrap().as_com_ref()));
    drop(event);
    assert_eq!(typed.text().unwrap_err(), NC_E_DROPPED);
    assert_eq!(text.to_string().unwrap(), "Save α");
}

/// Separate interfaces distinguish changes from submissions, including empty text.
#[test]
fn text_change_and_submission_are_distinct_typed_events() {
    let changed = snapshot::text_changed("éclair", 2).unwrap();
    let typed = changed.query_weak::<ITextChangedEventAbi>().unwrap();
    assert_eq!(typed.text().unwrap().to_string().unwrap(), "éclair");
    assert_eq!(typed.cursor(), Ok(2));
    assert_eq!(changed.query_weak::<ITextSubmittedEventAbi>().unwrap_err(), NC_E_NOINTERFACE);
    for text in ["line\ntwo", ""] {
        let submitted = snapshot::text_submitted(text).unwrap();
        assert_eq!(
            submitted.query_weak::<ITextSubmittedEventAbi>().unwrap().text().unwrap().to_string().unwrap(),
            text
        );
        assert_eq!(submitted.query_weak::<ITextChangedEventAbi>().unwrap_err(), NC_E_NOINTERFACE);
    }
    assert_eq!(snapshot::text_changed("é", 1).unwrap_err(), NC_E_INVALID_ARG);
    assert_eq!(snapshot::text_changed("x", 2).unwrap_err(), NC_E_INVALID_ARG);
    assert_eq!(snapshot::text_changed("", u64::MAX).unwrap_err(), NC_E_INVALID_ARG);
    assert_eq!(
        snapshot::text_changed("", 0).unwrap().query_weak::<ITextChangedEventAbi>().unwrap().cursor(),
        Ok(0)
    );
}

/// Producers reject invalid domain values before publishing an event owner.
#[test]
fn scalar_events_keep_domain_invariants() {
    let checkbox = snapshot::checkbox_changed(true).unwrap();
    assert_eq!(checkbox.query_weak::<ICheckboxChangedEventAbi>().unwrap().checked(), Ok(true));
    let numeric = snapshot::numeric_changed(0.75).unwrap();
    assert_eq!(numeric.query_weak::<INumericChangedEventAbi>().unwrap().value(), Ok(0.75));
    let scroll = snapshot::scrollbar_changed(42).unwrap();
    assert_eq!(scroll.query_weak::<IScrollbarChangedEventAbi>().unwrap().offset(), Ok(42));
    let split = snapshot::split_changed(73).unwrap();
    assert_eq!(split.query_weak::<ISplitChangedEventAbi>().unwrap().position(), Ok(73));
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(snapshot::numeric_changed(value).unwrap_err(), NC_E_INVALID_ARG);
    }
    assert_eq!(snapshot::scrollbar_changed(-1).unwrap_err(), NC_E_INVALID_ARG);
    assert_eq!(snapshot::split_changed(-1).unwrap_err(), NC_E_INVALID_ARG);
}

#[neocogi_com::com_object(interfaces(neocogi_ui_contracts::abi::nodes::INumericChangedEvent), ownership = exclusive)]
/// A foreign provider can reject a getter even after successful QueryInterface.
struct RejectedNumeric;
impl IEvent for RejectedNumeric {}
impl INumericChangedEvent for RejectedNumeric {
    /// Preserve the provider status for the caller to handle.
    fn value(&self) -> Result<f32, NcStatus> {
        Err(NC_E_INVALID_ARG)
    }
}

/// Missing projections and errors from supported getters retain their exact status.
#[test]
fn foreign_event_getter_errors_remain_visible() {
    let event: ComBox<IEventAbi> = RejectedNumeric::into_box(RejectedNumeric).query().unwrap();
    assert_eq!(event.query_weak::<INumericChangedEventAbi>().unwrap().value(), Err(NC_E_INVALID_ARG));
    assert_eq!(event.query_weak::<ICheckboxChangedEventAbi>().unwrap_err(), NC_E_NOINTERFACE);
}

/// Every lifecycle fact has its own interface and shares the window marker ancestry.
#[test]
fn service_events_keep_lifecycle_and_selection_facts() {
    let rect = Rect { x: -5, y: 12, width: 90, height: 70 };
    macro_rules! geometry {
        ($construct:ident, $interface:ty) => {{
            let event = snapshot::$construct(rect).unwrap();
            assert_eq!(event.query_weak::<$interface>().unwrap().rect(), Ok(rect));
            event.query_weak::<IWindowEventAbi>().unwrap();
            assert_eq!(event.query_weak::<IWindowHiddenEventAbi>().unwrap_err(), NC_E_NOINTERFACE);
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
            event.query_weak::<$interface>().unwrap();
            event.query_weak::<IWindowEventAbi>().unwrap();
            assert_eq!(event.query_weak::<IWindowGeometryChangedEventAbi>().unwrap_err(), NC_E_NOINTERFACE);
        }};
    }
    marker!(window_close_requested, IWindowCloseRequestedEventAbi);
    marker!(window_hidden, IWindowHiddenEventAbi);
    marker!(window_destroyed, IWindowDestroyedEventAbi);
    let popup = snapshot::popup_dismissed().unwrap();
    popup.query_weak::<IPopupDismissedEventAbi>().unwrap();
    assert_eq!(popup.query_weak::<IWindowEventAbi>().unwrap_err(), NC_E_NOINTERFACE);
    for selection in [ChoiceSelection::None, ChoiceSelection::Selected(0), ChoiceSelection::Selected(u64::MAX)] {
        let event = snapshot::choice_changed(selection).unwrap();
        assert_eq!(event.query_weak::<IChoiceChangedEventAbi>().unwrap().selection(), Ok(selection));
    }
}

/// Accepted native paths and cancellation remain distinct after the producer is gone.
#[test]
fn file_dialog_events_preserve_native_paths_and_cancellation() {
    let original = std::path::Path::new("directory/example file.txt");
    let path = neocogi_ui_contracts::NativePath::new(original);
    let accepted = snapshot::file_accepted(path.clone().into_interface()).unwrap();
    drop(path);
    let typed = accepted.query_weak::<IFileAcceptedEventAbi>().unwrap();
    assert_eq!(neocogi_ui_contracts::path::decode(typed.path().unwrap().as_com_ref()).unwrap(), original);
    assert_eq!(accepted.query_weak::<IFileCancelledEventAbi>().unwrap_err(), NC_E_NOINTERFACE);
    let cancelled = snapshot::file_cancelled().unwrap();
    cancelled.query_weak::<IFileCancelledEventAbi>().unwrap();
    assert_eq!(cancelled.query_weak::<IFileAcceptedEventAbi>().unwrap_err(), NC_E_NOINTERFACE);
}

/// The production channel moves the same allocation and restores it on rejection.
#[test]
fn common_channel_retains_exclusive_owner_and_returns_it_when_disconnected() {
    use neocogi_com::{NcValueRef, NC_E_CLOSED};
    use neocogi_ui_contracts::events::{channel, EventOrigin, IEventReceiverAbi};
    let (source, _source_inbox) = channel();
    let origin = EventOrigin::Service(source.downgrade().unwrap().into_unknown());
    let (mut queue, inbox) = channel();
    let event = snapshot::text_submitted("captured").unwrap();
    let allocation = event.as_raw();
    let projection = event.query_weak::<ITextSubmittedEventAbi>().unwrap();
    queue.post(NcValueRef::new(&origin).unwrap(), event).unwrap();
    drop(source);
    let message = inbox.try_recv().unwrap();
    assert_eq!(message.event.as_raw(), allocation);
    assert!(message.origin.service::<IEventReceiverAbi>().unwrap().unwrap().upgrade().unwrap().is_none());
    assert_eq!(projection.text().unwrap().to_string().unwrap(), "captured");
    drop(inbox);
    let (status, returned) = queue.post(NcValueRef::new(&origin).unwrap(), message.event).unwrap_err();
    assert_eq!(status, NC_E_CLOSED);
    assert_eq!(returned.as_raw(), allocation);
    assert_eq!(projection.text().unwrap().to_string().unwrap(), "captured");
    drop(returned);
    assert_eq!(projection.text().unwrap_err(), NC_E_DROPPED);
}
