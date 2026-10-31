//! Generic borrowed clipboard capabilities never imply shareable ownership.
use super::*;
use neocogi_ui::abi::core::{ITextClipboard, ITextClipboardAbi, IClipboardCompletionAbi};

/// Independent observation of a clipboard allocation's concrete destruction.
struct Lifetime(Rc<Cell<u32>>);
impl Drop for Lifetime {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

/// Generates the same clipboard contract under both lifetime policies.
macro_rules! clipboard {
    ($name:ident, $ownership:ident) => {
        #[com_object(interfaces(ITextClipboard), ownership = $ownership)]
        struct $name {
            /// Retained by the value only, never by expired weak storage.
            _lifetime: Lifetime,
        }
        impl ITextClipboard for $name {
            fn read(&self, _: ComRef<'_, IClipboardCompletionAbi>) -> Result<(), NcStatus> {
                Err(NC_E_INVALID_ARG)
            }
            fn write(&self, _: NcCopyFromBufferOffset<'_, u8>, _: ComRef<'_, IClipboardCompletionAbi>) -> Result<(), NcStatus> {
                Err(NC_E_INVALID_ARG)
            }
        }
    };
}
clipboard!(SharedClipboard, shared);
clipboard!(ExclusiveClipboard, exclusive);

/// Actual generated UiRuntime calls reject exclusive replacement before changing the service slot.
#[test]
fn clipboard_capture_rejects_exclusive_borrows_without_replacing_shared_service() {
    let mut runtime = runtime();
    let host = runtime.construction_host().unwrap().host().unwrap();
    let old_drops = Rc::new(Cell::new(0));
    let old = SharedClipboard::from_value(SharedClipboard { _lifetime: Lifetime(old_drops.clone()) })
        .query::<ITextClipboardAbi>()
        .unwrap();
    let old_weak = old.downgrade().unwrap();
    runtime.set_clipboard(old.as_com_ref()).unwrap();
    drop(old);
    assert_eq!(old_drops.get(), 0);

    let rejected_drops = Rc::new(Cell::new(0));
    let rejected = ExclusiveClipboard::into_box(ExclusiveClipboard {
        _lifetime: Lifetime(rejected_drops.clone()),
    })
    .query::<ITextClipboardAbi>()
    .unwrap();
    let rejected_weak = rejected.downgrade();
    assert_eq!(runtime.set_clipboard(rejected.as_com_ref()), Err(NC_E_INVALID_ARG));
    assert!(rejected_weak.is_alive());
    assert_eq!(rejected_drops.get(), 0);
    assert_eq!(old_drops.get(), 0);
    assert!(
        host.clipboard()
            .unwrap()
            .as_com_ref()
            .same_object(old_weak.upgrade().unwrap().unwrap().as_com_ref())
    );
    drop(rejected);
    assert_eq!(rejected_drops.get(), 1);
    assert!(!rejected_weak.is_alive());

    let next_drops = Rc::new(Cell::new(0));
    let next = SharedClipboard::from_value(SharedClipboard { _lifetime: Lifetime(next_drops.clone()) })
        .query::<ITextClipboardAbi>()
        .unwrap();
    runtime.set_clipboard(next.as_com_ref()).unwrap();
    assert_eq!(old_drops.get(), 1);
    assert!(old_weak.upgrade().unwrap().is_none());
    drop(next);
    assert_eq!(next_drops.get(), 0);
    runtime.close().unwrap();
    drop((host, runtime));
    assert_eq!(next_drops.get(), 1);
}

/// Foreign retention can call another projection before acquiring its new reference.
/// Keep this fixture separate from generated providers, whose AddRef is count-only.
mod callback {
    use super::*;
    use neocogi_com::{ComInterface, ComRaw, IUnknownVTable, WeakComPtr};
    use neocogi_com::lifetime::{self, NcClassLifetime, NcWeakControl};
    use neocogi_ui::abi::core::{IClipboardCompletion, ITextClipboardVTable};
    use std::ffi::c_void;

    /// Only a weak runtime link is allowed: installation must not create an owner cycle.
    enum Action {
        /// Publish another service while the outgoing old service is being retained.
        Replace(WeakComPtr<UiRuntimeAbi>, ComPtr<ITextClipboardAbi>),
        /// Remove the installed service as part of terminal shutdown.
        Close(WeakComPtr<UiRuntimeAbi>),
    }

    /// Observes value and allocation lifetime independently of any strong clipboard owner.
    #[derive(Default)]
    struct Probe {
        /// Destruction must happen exactly once, after the returned snapshot is released.
        drops: Cell<u32>,
        /// A real weak lease retains bookkeeping until the assertions finish.
        frees: Cell<u32>,
        /// Proves a returned capability still dispatches to the original live provider.
        reads: Cell<u32>,
        /// One successful nested mutation is required before the outgoing count increases.
        actions: Cell<u32>,
    }

    /// A valid foreign COM allocation, with its common lifetime header at offset zero.
    #[repr(C)]
    struct Clipboard {
        /// Paired permanently with CLASS and its two canonical interface tables.
        control: NcWeakControl,
        /// Interior mutability is necessary because foreign AddRef has no mutable receiver.
        /// Taking the action before reentry also makes all nested retention count-only.
        action: Cell<Option<Action>>,
        /// Bookkeeping survives value destruction through the explicit weak lease.
        probe: Rc<Probe>,
    }

    /// Executes before retention, so the runtime's local snapshot must pin the old owner.
    unsafe extern "C" fn retain(object: *mut c_void) -> u32 {
        // SAFETY: only CLASS's live Clipboard allocation is paired with this table.
        let clipboard = unsafe { &*object.cast::<Clipboard>() };
        let action = clipboard.action.take();
        let probe = clipboard.probe.clone();
        let outcome = match action {
            Some(Action::Replace(runtime, next)) => runtime.upgrade().and_then(|runtime| {
                let mut runtime = runtime.ok_or(UI_E_CLOSED)?;
                runtime.set_clipboard(next.as_com_ref())
            }),
            Some(Action::Close(runtime)) => runtime.upgrade().and_then(|runtime| runtime.ok_or(UI_E_CLOSED)?.close()),
            None => {
                // SAFETY: an ordinary call retains the live incoming owner directly.
                return unsafe { lifetime::nc_strong_retain(object) };
            }
        };
        // Never unwind through the C ABI or touch a destroyed application value. The
        // child process reports either contract failure without attempting resurrection.
        if outcome.is_err() || probe.drops.get() != 0 {
            std::process::abort();
        }
        probe.actions.set(probe.actions.get() + 1);
        // SAFETY: the selected local runtime snapshot still owns the live provider.
        unsafe { lifetime::nc_strong_retain(object) }
    }

    /// Ends application lifetime; only inert bookkeeping remains until final weak release.
    unsafe extern "C" fn destroy(control: *mut NcWeakControl) {
        // SAFETY: the class destroys this initialized allocation once at strong zero.
        let clipboard = unsafe { &*control.cast::<Clipboard>() };
        drop(clipboard.action.take());
        clipboard.probe.drops.set(clipboard.probe.drops.get() + 1);
    }

    /// Frees the original allocation exactly once, after all strong and weak ownership.
    unsafe extern "C" fn deallocate(control: *mut NcWeakControl) {
        // SAFETY: the final weak release uniquely owns the original Box allocation.
        let clipboard = unsafe { Box::from_raw(control.cast::<Clipboard>()) };
        clipboard.probe.frees.set(clipboard.probe.frees.get() + 1);
    }

    /// Immediate failure is a valid clipboard result and requires no retained completion.
    unsafe extern "C" fn read(object: *mut c_void, _: ComRaw<IClipboardCompletionAbi>) -> NcStatus {
        // SAFETY: the generated client calls this only through a live owned capability.
        let clipboard = unsafe { &*object.cast::<Clipboard>() };
        clipboard.probe.reads.set(clipboard.probe.reads.get() + 1);
        NC_E_INVALID_ARG
    }

    /// This fixture supports no asynchronous writes and never keeps borrowed inputs.
    unsafe extern "C" fn write(_: *mut c_void, _: u64, _: u64, _: *const u8, _: ComRaw<IClipboardCompletionAbi>) -> NcStatus {
        NC_E_INVALID_ARG
    }

    /// Canonical root projection uses the ordinary published lifetime implementation.
    static ROOT: IUnknownVTable = IUnknownVTable {
        interface_id: &IUnknown::INTERFACE_ID,
        query_interface: lifetime::nc_query_interface,
        add_ref: lifetime::nc_strong_retain,
        release: lifetime::nc_strong_release,
        downgrade: Some(lifetime::nc_downgrade),
    };
    /// Immutable clipboard table uses the exact generated ABI, with foreign retention.
    static TABLE: ITextClipboardVTable = ITextClipboardVTable {
        unknown: IUnknownVTable {
            interface_id: &ITextClipboardAbi::INTERFACE_ID,
            query_interface: lifetime::nc_query_interface,
            add_ref: retain,
            release: lifetime::nc_strong_release,
            downgrade: Some(lifetime::nc_downgrade),
        },
        read,
        write,
    };
    /// Both projections are permanently listed; no generated vtable is patched or spoofed.
    static CLASS: NcClassLifetime = NcClassLifetime {
        destroy_value: destroy,
        deallocate,
        interfaces: [&ROOT as *const IUnknownVTable, &TABLE as *const ITextClipboardVTable as *const IUnknownVTable].as_ptr(),
        interface_count: 2,
    };

    #[com_object(interfaces(IClipboardCompletion))]
    /// Valid completion capability for checking dispatch through the returned clipboard.
    struct Completion;
    impl IClipboardCompletion for Completion {
        fn complete(&self, _: i32, _: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
            Ok(())
        }
    }

    /// Arms the provider only after setup, leaving the installed slot as its sole strong owner.
    fn scenario(close: bool) {
        let mut runtime = runtime();
        let host = runtime.construction_host().unwrap().host().unwrap();
        assert_eq!(host.clipboard().unwrap_err(), neocogi_com::NC_E_NOT_FOUND);
        let probe = Rc::new(Probe::default());
        let allocation = Box::into_raw(Box::new(Clipboard {
            control: NcWeakControl::new(&CLASS),
            action: Cell::new(None),
            probe: probe.clone(),
        }));
        // SAFETY: original strong ownership is transferred exactly once, paired with
        // the immutable clipboard table listed in the initialized common header.
        let clipboard = unsafe {
            ComPtr::<ITextClipboardAbi>::from_raw_owned(ComRaw {
                object: allocation.cast(),
                vtable: &TABLE,
            })
        }
        .unwrap();
        let weak = clipboard.downgrade().unwrap();
        runtime.set_clipboard(clipboard.as_com_ref()).unwrap();
        let next_drops = Rc::new(Cell::new(0));
        let next = SharedClipboard::from_value(SharedClipboard { _lifetime: Lifetime(next_drops.clone()) })
            .query::<ITextClipboardAbi>()
            .unwrap();
        let next_weak = next.downgrade().unwrap();
        let action = if close {
            drop(next);
            Action::Close(runtime.downgrade().unwrap())
        } else {
            Action::Replace(runtime.downgrade().unwrap(), next)
        };
        drop(clipboard);
        // SAFETY: the installed slot is the only strong C owner; the explicit weak
        // additionally keeps allocation bookkeeping addressable until final assertions.
        unsafe { (*allocation).action.set(Some(action)) };
        let snapshot = host.clipboard().unwrap();
        assert_eq!(probe.actions.get(), 1);
        assert_eq!(probe.drops.get(), 0);
        let completion = Completion::from_value(Completion).query::<IClipboardCompletionAbi>().unwrap();
        assert_eq!(snapshot.read(completion.as_com_ref()), Err(NC_E_INVALID_ARG));
        assert_eq!(probe.reads.get(), 1);
        if close {
            assert_eq!(host.clipboard().unwrap_err(), UI_E_CLOSED);
            assert_eq!(runtime.update(SIZE), Err(UI_E_CLOSED));
        } else {
            let installed = host.clipboard().unwrap();
            assert!(!snapshot.as_com_ref().same_object(installed.as_com_ref()));
            assert!(installed.as_com_ref().same_object(next_weak.upgrade().unwrap().unwrap().as_com_ref()));
            assert_eq!(next_drops.get(), 0);
        }
        drop(snapshot);
        assert_eq!(probe.drops.get(), 1);
        assert_eq!(probe.frees.get(), 0);
        assert!(weak.upgrade().unwrap().is_none());
        drop(weak);
        assert_eq!(probe.frees.get(), 1);
        runtime.close().unwrap();
        runtime.close().unwrap();
        assert_eq!(next_drops.get(), 1);
    }

    /// A pre-fix RefCell panic crosses C and aborts, so each scenario needs isolation.
    #[test]
    fn pre_retain_reentry_preserves_the_selected_clipboard() {
        const CHILD: &str = "NEOCOGI_TEST_CLIPBOARD_REENTRY";
        if let Some(case) = std::env::var_os(CHILD) {
            scenario(case == "close");
            return;
        }
        for case in ["replace", "close"] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "runtime::tests::retention::callback::pre_retain_reentry_preserves_the_selected_clipboard",
                    "--nocapture",
                ])
                .env(CHILD, case)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{case}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}
