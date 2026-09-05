//! UI assembly: one export packages runtime, window manager, widgets and layout.
//!
//! The demo default atlas is temporary packaging input until the assets milestone.
//! The implementation library itself has no dynamic entrypoint.
#![deny(missing_docs)]
use microui_contracts::*;
use nc_com::{ComPtr, NcStatus, com_class};

#[com_class(class_id(UI_PROVIDER_CLASS), name = "microui.UiProvider", version = "0.9.0-alpha.0",
    constructor(args = none), interfaces(IUiProvider))]
/// Stateless bootstrap; each created runtime owns independent UI state.
pub struct UiProvider;

impl IUiProvider for UiProvider {
    /// Installs one validated default bundle in a new retained runtime.
    fn create_runtime(&self) -> Result<ComPtr<IUiRuntimeAbi>, NcStatus> {
        Ok(microui_ui::com::create_runtime(microui_redux_demo_assets::load_atlas()))
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
