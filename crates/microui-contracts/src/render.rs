//! Ordered bulk frames and owned renderer resources; native GPU APIs stay inside their plugin.
use crate::{Rect, Size};
use nc_com::{ComPtr, ComRef, INcStringAbi, NcCopyFromBufferOffset, NcCopyToBufferOffset, NcStatus, com_enum, com_interface, com_struct};

/// Shared allocation ceiling for one complete frame, checked before reserving receiver storage.
pub const MAX_RENDER_VERTICES: u32 = 16 * 1024 * 1024;
/// Shared ceiling for ordered records; adjacent atlas triangles should occupy one batch.
pub const MAX_RENDER_BATCHES: u32 = 1024 * 1024;

#[com_enum(tag = u8)]
#[derive(Clone, Copy, Debug)]
/// Ordered draw record. Vertex ranges must partition the entire vertex buffer without gaps or overlap.
pub enum RenderBatch {
    /// Atlas-backed triangles already clipped and tessellated by the UI.
    Atlas {
        /// First vertex, equal to the end of the preceding vertex-bearing batch.
        first: u32,
        /// Positive vertex count divisible by three.
        count: u32,
    },
    /// Four clipped vertices in top-left, top-right, bottom-right, bottom-left order.
    Texture {
        /// First of exactly four vertices; stream.texture(batch_index) supplies the owned resource.
        first: u32,
    },
    /// Backend-specific work; stream.item(batch_index) supplies an owned capability from that renderer.
    Item {
        /// Positive, unclipped content rectangle in drawable pixels.
        rect: Rect,
        /// Positive intersection of content, widget clip and drawable viewport.
        clip: Rect,
    },
}

#[com_interface(auto_guid = "microui.Texture:v1")]
/// Immutable RGBA image whose final release destroys its native allocation with its owning device alive.
pub trait ITexture {
    /// Positive immutable pixel dimensions, independent of its destination rectangle.
    fn size(&self) -> Result<Size, NcStatus>;
}

#[com_interface(auto_guid = "microui.RenderItem:v1")]
/// Opaque owned application GPU work, authenticated by the renderer that created it.
/// Application-specific derived interfaces edit concrete scene data; this interface exposes no native handles.
pub trait IRenderItem {}

#[com_interface(auto_guid = "microui.TextureFactory:v1")]
/// Optional renderer capability for immutable external images.
pub trait ITextureFactory {
    /// Uploads exactly width*height*4 RGBA bytes at offset zero, publishing only after a successful upload.
    fn create_texture(&self, size: Size, pixels: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<ITextureAbi>, NcStatus>;
}

#[com_interface(auto_guid = "microui.RenderDiagnostics:v1")]
/// Owned diagnostic snapshots preserve backend details without exposing backend-dependent errors.
pub trait IRenderDiagnostics {
    /// Returns the most recent failed GPU operation's detail, or empty text before a failure.
    fn diagnostic(&self) -> Result<ComPtr<INcStringAbi>, NcStatus>;
}

#[com_struct]
#[derive(Debug, Default)]
/// Finite floating-point position used by widget-local line and polygon drawing.
pub struct PaintPoint {
    /// Horizontal widget-local coordinate in pixels.
    pub x: f32,
    /// Vertical widget-local coordinate in pixels.
    pub y: f32,
}

#[com_struct]
#[derive(Debug, Default)]
/// Local solid vertex; applications never need the atlas's private white-pixel coordinates.
pub struct ColoredVertex {
    /// Horizontal widget-local position in pixels.
    pub x: f32,
    /// Vertical widget-local position in pixels.
    pub y: f32,
    /// Red in the least significant byte, then green, blue and alpha.
    pub rgba: u32,
}

#[com_struct]
#[derive(Debug, Default)]
/// Final textured vertex; tessellation and clipping already happened in the UI.
pub struct RenderVertex {
    /// Horizontal position in drawable pixels.
    pub x: f32,
    /// Vertical position in drawable pixels.
    pub y: f32,
    /// Normalized horizontal texture coordinate.
    pub u: f32,
    /// Normalized vertical texture coordinate.
    pub v: f32,
    /// Color packed as red in the least significant byte, then green, blue, alpha.
    pub rgba: u32,
}

#[com_struct]
/// Metadata for one synchronous renderer submission.
pub struct RenderInfo {
    /// Positive drawable dimensions.
    pub size: Size,
    /// Clear color in the same byte order as RenderVertex::rgba.
    pub clear: u32,
    /// Unique authority to read this frame's stream; expires when render returns.
    pub invocation: u64,
    /// Exact vertex count: atlas batches contain triples and texture batches contain four vertices.
    pub vertex_count: u32,
    /// Exact number of ordered batch records.
    pub batch_count: u32,
}

#[com_interface(auto_guid = "microui.RenderStream:v1")]
/// Owned reusable frame storage, readable only during its active submission.
pub trait IRenderStream {
    /// Copies a bounded range into caller-owned storage; offsets count vertices.
    fn vertices(&self, invocation: u64, output: NcCopyToBufferOffset<'_, RenderVertex>) -> Result<u32, NcStatus>;
    /// Copies ordered records; offsets count batches, and no output record contains a borrowed pointer.
    fn batches(&self, invocation: u64, output: NcCopyToBufferOffset<'_, RenderBatchAbi>) -> Result<u32, NcStatus>;
    /// Retains the texture used by this batch; another batch kind or invalid index is rejected.
    fn texture(&self, invocation: u64, batch: u32) -> Result<ComPtr<ITextureAbi>, NcStatus>;
    /// Retains the custom item used by this batch; another batch kind or invalid index is rejected.
    fn item(&self, invocation: u64, batch: u32) -> Result<ComPtr<IRenderItemAbi>, NcStatus>;
}

#[com_interface(auto_guid = "microui.Renderer:v1")]
/// GPU resources and presentation belong to this object's native window.
pub trait IRenderer {
    /// Imports and transactionally uploads a complete immutable atlas. Repeated preparation of
    /// the identical bundle is a cheap no-op; failure leaves the previous native atlas usable.
    fn prepare_assets(&self, assets: ComRef<'_, crate::IAssetBundleAbi>) -> Result<(), NcStatus>;
    /// Executes one complete frame synchronously; failure must not present it.
    fn render(&self, info: RenderInfo, assets: ComRef<'_, crate::IAssetBundleAbi>, stream: ComRef<'_, IRenderStreamAbi>) -> Result<(), NcStatus>;
}
