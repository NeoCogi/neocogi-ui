//! Typed activation of the consolidated UI library's independently owned services.
use crate::abi::{core::UiPluginFactoryAbi, nodes::NodesPluginFactoryAbi, library::UiLibraryFactoryAbi};
use crate::{gfx::abi::GfxPluginFactoryAbi, appearance::skin::SkinPluginFactoryAbi};
use neocogi_com::{ComPtr, NcGuid, NcStatus, Runtime, guid_from_qualified_name};

/// Sole implementation identity for linked and dynamically loaded UI libraries.
pub const LIBRARY_PLUGIN_ID: NcGuid = guid_from_qualified_name("neocogi.ui");

/// Acquires the canonical root; registration remains the final application's responsibility.
pub fn library(runtime: Runtime) -> Result<ComPtr<UiLibraryFactoryAbi>, NcStatus> {
    runtime.acquire_plugin_root(LIBRARY_PLUGIN_ID)
}
/// Acquires a retained-runtime factory, or NC_E_NOT_FOUND when UI implementation is disabled.
pub fn ui(runtime: Runtime) -> Result<ComPtr<UiPluginFactoryAbi>, NcStatus> {
    library(runtime)?.ui()
}
/// Acquires a built-in-node factory, or NC_E_NOT_FOUND when UI implementation is disabled.
pub fn nodes(runtime: Runtime) -> Result<ComPtr<NodesPluginFactoryAbi>, NcStatus> {
    library(runtime)?.nodes()
}
/// Acquires a graphics factory, or NC_E_NOT_FOUND when graphics implementation is disabled.
pub fn gfx(runtime: Runtime) -> Result<ComPtr<GfxPluginFactoryAbi>, NcStatus> {
    library(runtime)?.gfx()
}
/// Acquires an appearance factory, or NC_E_NOT_FOUND when appearance implementation is disabled.
pub fn skin(runtime: Runtime) -> Result<ComPtr<SkinPluginFactoryAbi>, NcStatus> {
    library(runtime)?.skin()
}
