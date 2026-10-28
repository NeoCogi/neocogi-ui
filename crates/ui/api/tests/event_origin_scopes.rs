//! Owner scopes capture facts now and restore callers on every synchronous exit.
// Link this executable's runtime explicitly, including on MSVC without section GC.
extern crate neocogi_com_host as _;

use neocogi_com::{ComBox, ComPtr, INcStringComPtrExt, IUnknown, NcStatus, NC_E_CLOSED, NC_E_INVALID_ARG};
use neocogi_ui::events::{channel, activated, text_submitted, EventOrigin, EventOriginOwner, IEventAbi, IEventReceiverAbi};

/// Uses real exclusive event objects as sender identities without retaining their values.
fn sender() -> (ComBox<IEventAbi>, EventOrigin) {
    let owner = activated().unwrap();
    let origin = EventOrigin::Widget(owner.downgrade().into_unknown());
    (owner, origin)
}

/// Confirms both stack restoration and the source captured through the production channel ABI.
fn post(owner: &EventOriginOwner, queue: &ComPtr<IEventReceiverAbi>, label: &str) {
    owner.post(queue.as_com_ref(), text_submitted(label).unwrap()).unwrap();
}

#[test]
fn nested_scopes_capture_in_post_order_and_restore_after_errors() {
    let scope = EventOriginOwner::new();
    let (outer, outer_origin) = sender();
    let (inner, inner_origin) = sender();
    let (queue, inbox) = channel();
    let result: Result<(), NcStatus> = scope
        .with_origin(outer_origin, || {
            post(&scope, &queue, "before");
            let nested = scope
                .with_origin(inner_origin, || {
                    post(&scope, &queue, "inside");
                    Err::<(), _>(NC_E_CLOSED)
                })
                .unwrap();
            assert_eq!(nested, Err(NC_E_CLOSED));
            post(&scope, &queue, "after");
            Err(NC_E_CLOSED)
        })
        .unwrap();
    assert_eq!(result, Err(NC_E_CLOSED));
    assert!(matches!(scope.current(), Err(NC_E_INVALID_ARG)));
    for (expected, label) in [(&outer, "before"), (&inner, "inside"), (&outer, "after")] {
        let message = inbox.try_recv().unwrap();
        assert!(message.origin.widget::<IUnknown>().unwrap().unwrap().same_object(&expected.downgrade()));
        assert_eq!(
            message
                .query::<neocogi_ui::ITextSubmittedEventAbi>()
                .unwrap()
                .unwrap()
                .text()
                .unwrap()
                .to_string()
                .unwrap(),
            label
        );
    }
    drop(outer);
    drop(inner);
    assert!(inbox.try_recv().is_err());
}

#[test]
fn idle_and_rejected_posts_return_the_same_event_owner() {
    let scope = EventOriginOwner::new();
    let (queue, inbox) = channel();
    let event = activated().unwrap();
    let allocation = event.as_raw();
    let (status, event) = scope.post(queue.as_com_ref(), event).unwrap_err();
    assert_eq!(status, NC_E_INVALID_ARG);
    assert_eq!(event.as_raw(), allocation);
    assert!(inbox.try_recv().is_err());
    let (_source, origin) = sender();
    drop(inbox);
    let (status, event) = scope.with_origin(origin, || scope.post(queue.as_com_ref(), event)).unwrap().unwrap_err();
    assert_eq!(status, NC_E_CLOSED);
    assert_eq!(event.as_raw(), allocation);
    assert!(matches!(scope.current(), Err(NC_E_INVALID_ARG)));
}

#[test]
fn unwind_removes_inner_and_outer_frames() {
    let scope = EventOriginOwner::new();
    let (_source, origin) = sender();
    let (_nested, inner) = sender();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        scope
            .with_origin(origin, || scope.with_origin(inner, || panic!("dispatch failed")).unwrap())
            .unwrap()
    }));
    assert!(result.is_err());
    assert!(matches!(scope.current(), Err(NC_E_INVALID_ARG)));
}
