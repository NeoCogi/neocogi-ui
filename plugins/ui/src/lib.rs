//! UI assembly: one export packages runtime, window manager, widgets and layout.
//!
//! Applications supply immutable resource bundles; this assembly contains no demo assets or parsers.
//! The implementation library itself has no dynamic entrypoint.
#![deny(missing_docs)]
use microui_contracts::*;
use nc_com::{ComPtr, ComRef, NcStatus, com_class};

#[com_class(class_id(UI_PROVIDER_CLASS), name = "microui.UiProvider", version = "0.9.0-alpha.0",
    constructor(args = none), interfaces(IUiProvider))]
/// Stateless bootstrap; each created runtime owns independent UI state.
pub struct UiProvider;

impl IUiProvider for UiProvider {
    /// Installs one validated default bundle in a new retained runtime.
    fn create_runtime(&self, assets: ComRef<'_, IAssetBundleAbi>) -> Result<ComPtr<IUiRuntimeAbi>, NcStatus> {
        microui_ui::com::create_runtime_from_assets(assets)
    }
}

nc_com::plugin! {
    plugin_id(auto_guid = "microui.plugin.ui"),
    name = "microui.plugin.ui",
    version = "0.9.0-alpha.0",
    classes = [UiProvider],
}

// Only the final native assembly publishes the dynamic entrypoint.
#[cfg(feature = "dynamic-plugin")]
nc_com::export_plugin!();
