//! Wide rectangles cross the real importer and generic renderer without overflow or skipping.
use super::*;

#[com_object(interfaces(IRenderStream))]
/// One authenticated view, with no texture or atlas vertices to obscure the graphics result.
pub(super) struct ViewStream {
    /// Canonical immutable root and generic commands survive the generated stream calls.
    pub(super) blocks: Vec<ComBox<IRenderViewBlockAbi>>,
}
impl IRenderStream for ViewStream {
    /// Borrows the only sealed contribution through the exclusive weak protocol.
    fn view(&self, _: u64, index: u32) -> Result<neocogi_com::WeakComBox<IRenderViewBlockAbi>, NcStatus> {
        self.blocks.get(index as usize).map(ComBox::downgrade).ok_or(NC_E_INVALID_ARG)
    }
}
