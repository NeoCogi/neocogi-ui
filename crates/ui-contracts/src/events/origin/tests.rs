//! Final weak cleanup is foreign code even after its application value is dead.
use super::*;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use neocogi_com::{IUnknown, IUnknownVTable, NcWeakRaw, WeakComPtr};
use neocogi_com::lifetime::{self, NcClassLifetime, NcWeakControl};

/// Allocation-only observation outlives the deliberately destroyed application payload.
#[derive(Default)]
struct Observation {
    /// Destruction must precede allocation cleanup.
    destroyed: Cell<bool>,
    /// Every final observer frees its provider allocation exactly once.
    frees: Cell<usize>,
    /// The provider catches locally so its C callback never unwinds.
    panicked: Cell<bool>,
}
/// Valid common-header allocation with independent allocation-lifetime bookkeeping.
#[repr(C)]
struct Allocation {
    /// Retained by the expired weak origin until scope rollback.
    control: NcWeakControl,
    /// Reentered during final weak release, after the application value is dead.
    owner: Rc<EventOriginOwner>,
    /// Expected visibility of an enclosing scope during the callback.
    enclosing: bool,
    /// Test assertions remain observable after the allocation is freed.
    observation: Rc<Observation>,
}
/// Marks only the value lifetime dead; explicit weak ownership retains storage.
unsafe extern "C" fn destroy(control: *mut NcWeakControl) {
    // SAFETY: this class is paired only with the matching initialized Allocation.
    let allocation = unsafe { &*control.cast::<Allocation>() };
    allocation.observation.destroyed.set(true);
}
/// Reenters the owner before freeing storage, honoring the non-unwinding ABI.
unsafe extern "C" fn deallocate(control: *mut NcWeakControl) {
    // SAFETY: final weak ownership uniquely releases this original Box allocation.
    let allocation = unsafe { Box::from_raw(control.cast::<Allocation>()) };
    let result = catch_unwind(AssertUnwindSafe(|| {
        assert!(allocation.observation.destroyed.get());
        assert_eq!(allocation.owner.current().is_ok(), allocation.enclosing);
    }));
    allocation.observation.panicked.set(result.is_err());
    allocation.observation.frees.set(allocation.observation.frees.get() + 1);
}
/// Ordinary common-header entrypoints establish a fully valid weak allocation.
static ROOT: IUnknownVTable = IUnknownVTable {
    interface_id: &IUnknown::INTERFACE_ID,
    query_interface: lifetime::nc_query_interface,
    add_ref: lifetime::nc_strong_retain,
    release: lifetime::nc_strong_release,
    downgrade: Some(lifetime::nc_downgrade),
};
/// Callback pairing stays permanent after the sole application owner is destroyed.
static CLASS: NcClassLifetime = NcClassLifetime {
    destroy_value: destroy,
    deallocate,
    interfaces: [&ROOT as *const IUnknownVTable].as_ptr(),
    interface_count: 1,
};

/// Creates an expired final weak token without malformed storage or self-destruction.
fn expired(owner: &Rc<EventOriginOwner>, enclosing: bool) -> (EventOrigin, Rc<Observation>) {
    let observation = Rc::new(Observation::default());
    let allocation = Box::into_raw(Box::new(Allocation {
        control: NcWeakControl::new(&CLASS),
        owner: owner.clone(),
        enclosing,
        observation: observation.clone(),
    }));
    let control = allocation.cast::<NcWeakControl>();
    // SAFETY: exactly one explicit weak is retained before releasing the original
    // strong. The initialized header/storage therefore remain live for this token.
    let weak = unsafe {
        lifetime::nc_weak_retain(control);
        lifetime::nc_strong_release(control.cast());
        WeakComPtr::<IUnknown>::from_raw_owned(NcWeakRaw { control }).unwrap()
    };
    assert!(observation.destroyed.get());
    (EventOrigin::Service(weak), observation)
}

/// Idle and scoped cleanup must both run callbacks without a stack borrow.
#[test]
fn final_weak_cleanup_can_reenter_after_normal_scope_or_direct_release() {
    for scoped in [false, true] {
        let owner = Rc::new(EventOriginOwner::new());
        let (origin, observed) = expired(&owner, false);
        if scoped {
            assert_eq!(owner.with_origin(origin, || 73), Ok(73));
        } else {
            drop(origin);
        }
        assert_eq!(observed.frees.get(), 1);
        assert!(!observed.panicked.get());
        assert!(matches!(owner.current(), Err(NC_E_INVALID_ARG)));
    }
}

/// Error/unwind rollback exposes the enclosing sender before inner foreign cleanup.
#[test]
fn nested_scope_cleanup_restores_outer_origin_on_error_and_unwind() {
    for unwind in [false, true] {
        let owner = Rc::new(EventOriginOwner::new());
        let (outer, outer_observed) = expired(&owner, false);
        let (inner, inner_observed) = expired(&owner, true);
        owner
            .with_origin(outer, || {
                assert!(owner.current().is_ok());
                if unwind {
                    let result = catch_unwind(AssertUnwindSafe(|| owner.with_origin(inner, || panic!("dispatch failure"))));
                    assert!(result.is_err());
                } else {
                    assert_eq!(owner.with_origin(inner, || Err::<(), _>(NC_E_INVALID_ARG)), Ok(Err(NC_E_INVALID_ARG)));
                }
                assert!(owner.current().is_ok());
                assert_eq!(inner_observed.frees.get(), 1);
                assert!(!inner_observed.panicked.get());
                assert_eq!(outer_observed.frees.get(), 0);
            })
            .unwrap();
        assert_eq!(outer_observed.frees.get(), 1);
        assert!(!outer_observed.panicked.get());
        assert!(matches!(owner.current(), Err(NC_E_INVALID_ARG)));
    }
}
