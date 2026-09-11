//! Application-specific mesh contracts shared by the demo host and its co-packaged GPU renderer.
#![no_std]
#![deny(missing_docs)]

/// Compile-checks the generated demo surface before it becomes the crate's
/// public API in the following migration commit.
#[allow(dead_code)]
mod generated_contracts {
    include!(concat!(env!("OUT_DIR"), "/neocogi_demo.rs"));
}
use neocogi_ui_contracts::*;
use neocogi_com::{ComPtr, NcCopyFromBufferOffset, NcStatus, com_interface, com_struct};
use rs_math3d::{Mat4f, Vec2f, Vec3f};

#[com_struct(c_struct)]
#[derive(Debug, Default)]
/// Plain vertex format shared by all native demo shaders.
pub struct MeshVertex {
    /// Object-space position; every component must be finite.
    pub position: Vec3f,
    /// Object-space normal; every component must be finite.
    pub normal: Vec3f,
    /// Procedural material coordinates; every component must be finite.
    pub uv: Vec2f,
}

#[com_struct(c_struct)]
#[derive(Debug)]
/// Column-major scene transforms, copied atomically as one camera/model update.
pub struct SceneTransforms {
    /// Projection-view-model transform applied to mesh positions.
    pub pvm: Mat4f,
    /// View-model transform applied to normals, matching the existing demo's shader convention.
    pub view_model: Mat4f,
}

#[com_interface(auto_guid = "neocogi.ui.demo.MeshScene:v1")]
/// Editable mesh scene is itself a render item on one COM allocation.
pub trait IMeshScene: IRenderItem {
    /// Replaces the complete immutable mesh after validating finite vertices and in-range triangle indices.
    /// Offsets must be zero; limits are 4M vertices and 12M indices. Two empty buffers clear the mesh.
    fn set_mesh(&self, vertices: NcCopyFromBufferOffset<'_, MeshVertex>, indices: NcCopyFromBufferOffset<'_, u32>) -> Result<(), NcStatus>;
    /// Replaces both finite matrices together; scene edits affect subsequent render preparation only.
    fn set_transforms(&self, transforms: SceneTransforms) -> Result<(), NcStatus>;
}

#[com_interface(auto_guid = "neocogi.ui.demo.Scenes:v1")]
/// Demo-specific factory exposed by the native window alongside its ordinary renderer interfaces.
pub trait IDemoScenes {
    /// Creates an empty scene with identity transforms; its resources retain this window/device.
    fn create_scene(&self) -> Result<ComPtr<IMeshSceneAbi>, NcStatus>;
}

/// Independent extension-plugin contracts, also used by the native acceptance host.
pub mod extension;
