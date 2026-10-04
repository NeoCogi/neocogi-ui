//! The demo's one queue object posts snapshots without application callbacks.
use super::{Action, NumericTarget};
use neocogi_com::WeakComPtr;
use neocogi_ui::*;

/// Ordinary application-owned construction and command identities.
/// Only &mut Inputs registers controls; no second COM borrow gate protects this data.
pub(crate) struct Inputs {
    /// Common receiver projection shared directly with every observed factory.
    pub(super) queue: neocogi_com::ComPtr<neocogi_com::events::IEventReceiverAbi>,
    /// Weak identities keep action lookup independent of labels and address reuse.
    pub(super) buttons: Vec<(neocogi_com::WeakComBox<ButtonAbi>, Action)>,
    /// Menu entries retain only their explicit application command.
    pub(super) menus: Vec<(WeakComPtr<IMenuItemAbi>, Action)>,
    /// Color and metric targets are data rather than closure specializations.
    pub(super) sliders: Vec<(neocogi_com::WeakComBox<SliderAbi>, NumericTarget)>,
}
impl Inputs {
    /// Constructs the sole queue before creating any eventful widgets.
    pub(super) fn new(queue: neocogi_com::ComPtr<neocogi_com::events::IEventReceiverAbi>) -> Self {
        Self {
            queue,
            buttons: Vec::new(),
            menus: Vec::new(),
            sliders: Vec::new(),
        }
    }
}
