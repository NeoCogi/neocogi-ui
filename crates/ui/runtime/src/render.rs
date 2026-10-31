//! Core-owned replay from shared recorded operations into the GFX compositor.
pub use neocogi_ui_support::render::{
    Color, FrameError, FrameInfo, FrameInfoError, RendererBackend, RendererFrame, TextureError, Vertex, color, NinePatch, NinePatchCell, NinePatchCells,
    NinePatchContent, NinePatchImage, SliceInsets, Painter,
};
// Recording arenas remain engine-owned implementation details, not a second native authoring API.
pub(crate) use neocogi_ui_support::render::{display_list, geometry, contract_color};
pub(crate) use display_list::DisplayList;
pub(crate) mod compositor;
