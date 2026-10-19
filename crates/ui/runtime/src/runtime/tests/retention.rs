//! Generic borrowed clipboard capabilities never imply shareable ownership.
use super::*;
use neocogi_ui_core_contracts::{ITextClipboard, ITextClipboardAbi, IClipboardCompletionAbi};

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
