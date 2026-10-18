//! Factories finish event wiring before exposing normally allocated capabilities.
use super::*;
use neocogi_com::events::{EventBinding, channel};

/// All window placements deliver immediate closure with the exact public source,
/// even without a widget update supplying an ambient event origin.
#[test]
fn fresh_window_capabilities_deliver_direct_closure_with_their_own_identity() {
    let runtime = runtime();
    let manager: ComPtr<IWindowManagerAbi> = runtime.query().unwrap();
    let (queue, messages) = channel();
    let binding = EventBinding::Observed(queue);
    let root = manager.create_window(
        NcCopyFromBufferOffset::new(0, b"root"), RECT,
        button(&runtime).query().unwrap(), neocogi_com::NcValueRef::new(&binding).unwrap(),
    ).unwrap();
    let child = manager.create_child(
        root.as_com_ref(), NcCopyFromBufferOffset::new(0, b"child"), RECT,
        button(&runtime).query().unwrap(), neocogi_com::NcValueRef::new(&binding).unwrap(),
    ).unwrap();
    let dialog = manager.create_dialog(
        root.as_com_ref(), NcCopyFromBufferOffset::new(0, b"dialog"), RECT,
        button(&runtime).query().unwrap(), neocogi_com::NcValueRef::new(&binding).unwrap(),
    ).unwrap();
    assert!(messages.try_recv().is_err(), "creation must not call the receiver");
    for window in [dialog, child, root] {
        let expected = window.downgrade().unwrap();
        window.close().unwrap();
        let message = messages.try_recv().expect("close delivers synchronously");
        let source = message.origin.service::<IWindowAbi>().unwrap().unwrap();
        assert!(source.same_object(&expected));
        assert!(message.downcast::<IWindowDestroyedEventAbi>().unwrap().is_some());
        drop(window);
        assert!(source.upgrade().unwrap().is_none(), "queued facts must not retain their source");
    }
    assert!(messages.try_recv().is_err());
}

/// Popup dismissal uses its separately connected public identity; queued facts
/// remain valid after native removal and final COM owner release.
#[test]
fn fresh_popup_connects_dismissal_before_the_first_application_call() {
    let runtime = runtime();
    let manager: ComPtr<IWindowManagerAbi> = runtime.query().unwrap();
    let root = mount(&runtime, button(&runtime).query().unwrap()).unwrap();
    let (queue, messages) = channel();
    let binding = EventBinding::Observed(queue);
    let mut popup = manager.create_popup(
        root.as_com_ref(), NcCopyFromBufferOffset::new(0, b"popup"),
        button(&runtime).query().unwrap(), neocogi_com::NcValueRef::new(&binding).unwrap(),
    ).unwrap();
    assert!(messages.try_recv().is_err());
    runtime.update(SIZE).unwrap();
    popup.show_at(RECT).unwrap();
    popup.hide().unwrap();
    let message = messages.try_recv().expect("hide delivers synchronously");
    let source = message.origin.service::<IPopupAbi>().unwrap().unwrap();
    assert!(source.same_object(&popup.downgrade().unwrap()));
    assert!(message.downcast::<IPopupDismissedEventAbi>().unwrap().is_some());
    popup.close().unwrap();
    drop(popup);
    assert!(source.upgrade().unwrap().is_none());
    assert!(messages.try_recv().is_err());
    root.close().unwrap();
}
