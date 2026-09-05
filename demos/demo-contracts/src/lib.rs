//! Application-specific mesh contracts shared by the demo host and its co-packaged GPU renderer.
#![no_std]
#![deny(missing_docs)]
use microui_contracts::*;
use nc_com::{ComPtr, NcCopyFromBufferOffset, NcStatus, com_interface, com_struct};

#[com_struct]
#[derive(Debug, Default)]
/// Canonical copied 2-component floating-point vector for application scene data.
pub struct Float2 {
    /// Component 0, in native shader order.
    pub x: f32,
    /// Component 1, in native shader order.
    pub y: f32,
}
impl From<[f32; 2]> for Float2 {
    /// Converts a local array into a C-shaped value; arrays never occur in COM signatures.
    fn from(value: [f32; 2]) -> Self {
        Self { x: value[0], y: value[1] }
    }
}
impl Float2 {
    /// Copies components in shader order for local validation and math adapters.
    pub const fn values(self) -> [f32; 2] {
        [self.x, self.y]
    }
}

#[com_struct]
#[derive(Debug, Default)]
/// Canonical copied 3-component floating-point vector for application scene data.
pub struct Float3 {
    /// Component 0, in native shader order.
    pub x: f32,
    /// Component 1, in native shader order.
    pub y: f32,
    /// Component 2, in native shader order.
    pub z: f32,
}
impl From<[f32; 3]> for Float3 {
    /// Converts a local array into a C-shaped value; arrays never occur in COM signatures.
    fn from(value: [f32; 3]) -> Self {
        Self { x: value[0], y: value[1], z: value[2] }
    }
}
impl Float3 {
    /// Copies components in shader order for local validation and math adapters.
    pub const fn values(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }
}

#[com_struct]
#[derive(Debug, Default)]
/// Canonical copied 4-component floating-point vector for application scene data.
pub struct Float4 {
    /// Component 0, in native shader order.
    pub x: f32,
    /// Component 1, in native shader order.
    pub y: f32,
    /// Component 2, in native shader order.
    pub z: f32,
    /// Component 3, in native shader order.
    pub w: f32,
}
impl From<[f32; 4]> for Float4 {
    /// Converts a local array into a C-shaped value; arrays never occur in COM signatures.
    fn from(value: [f32; 4]) -> Self {
        Self {
            x: value[0],
            y: value[1],
            z: value[2],
            w: value[3],
        }
    }
}
impl Float4 {
    /// Copies components in shader order for local validation and math adapters.
    pub const fn values(self) -> [f32; 4] {
        [self.x, self.y, self.z, self.w]
    }
}

#[com_struct]
#[derive(Debug)]
/// Explicit column-major transform; each column has a stable four-float C layout.
pub struct Matrix4 {
    /// First column.
    pub c0: Float4,
    /// Second column.
    pub c1: Float4,
    /// Third column.
    pub c2: Float4,
    /// Fourth column.
    pub c3: Float4,
}
impl Matrix4 {
    /// Copies columns in the exact order consumed by the existing native shaders.
    pub fn values(self) -> [f32; 16] {
        let [a, b, c, d] = self.c0.values();
        let [e, f, g, h] = self.c1.values();
        let [i, j, k, l] = self.c2.values();
        let [m, n, o, p] = self.c3.values();
        [a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p]
    }
    /// Identity transform is useful for a scene rendered directly in clip coordinates.
    pub fn identity() -> Self {
        Self {
            c0: [1., 0., 0., 0.].into(),
            c1: [0., 1., 0., 0.].into(),
            c2: [0., 0., 1., 0.].into(),
            c3: [0., 0., 0., 1.].into(),
        }
    }
}

#[com_struct]
#[derive(Debug, Default)]
/// Plain vertex format shared by all native demo shaders.
pub struct MeshVertex {
    /// Object-space position; every component must be finite.
    pub position: Float3,
    /// Object-space normal; every component must be finite.
    pub normal: Float3,
    /// Procedural material coordinates; every component must be finite.
    pub uv: Float2,
}

#[com_struct]
#[derive(Debug)]
/// Column-major scene transforms, copied atomically as one camera/model update.
pub struct SceneTransforms {
    /// Projection-view-model transform applied to mesh positions.
    pub pvm: Matrix4,
    /// View-model transform applied to normals, matching the existing demo's shader convention.
    pub view_model: Matrix4,
}

#[com_interface(auto_guid = "microui.demo.MeshScene:v1")]
/// Editable mesh scene is itself a render item on one COM allocation.
pub trait IMeshScene: IRenderItem {
    /// Replaces the complete immutable mesh after validating finite vertices and in-range triangle indices.
    /// Offsets must be zero; limits are 4M vertices and 12M indices. Two empty buffers clear the mesh.
    fn set_mesh(&self, vertices: NcCopyFromBufferOffset<'_, MeshVertex>, indices: NcCopyFromBufferOffset<'_, u32>) -> Result<(), NcStatus>;
    /// Replaces both finite matrices together; scene edits affect subsequent render preparation only.
    fn set_transforms(&self, transforms: SceneTransforms) -> Result<(), NcStatus>;
}

#[com_interface(auto_guid = "microui.demo.Scenes:v1")]
/// Demo-specific factory exposed by the native window alongside its ordinary renderer interfaces.
pub trait IDemoScenes {
    /// Creates an empty scene with identity transforms; its resources retain this window/device.
    fn create_scene(&self) -> Result<ComPtr<IMeshSceneAbi>, NcStatus>;
}
