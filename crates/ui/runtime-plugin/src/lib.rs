//! Deployable activation boundary for the retained core UI runtime.
//!
//! The implementation crate deliberately exports no plugin entrypoint. Keeping this thin root
//! separate lets the sibling built-in-nodes plugin reuse implementation support without embedding
//! a second loader symbol or pretending that one plugin is an ordinary dependency of another.

#![deny(missing_docs)]

pub use neocogi_ui_implementation::*;

/// Stateless process entry that creates independent retained UI runtimes.
#[neocogi_com::com_factory(neocogi_ui_core_contracts::UiPluginFactory)]
struct UiPluginFactoryObject;

impl neocogi_ui_core_contracts::UiPluginFactory for UiPluginFactoryObject {
    /// Imports the complete asset revision before publishing any runtime capability.
    fn create_runtime(
        &self,
        assets: neocogi_com::ComRef<'_, neocogi_ui_core_contracts::IAssetBundleAbi>,
        compositor: neocogi_com::ComRef<'_, neocogi_gfx_contracts::GfxCompositorAbi>,
    ) -> Result<neocogi_com::ComPtr<neocogi_ui_core_contracts::UiRuntimeAbi>, neocogi_com::NcStatus> {
        neocogi_ui_implementation::create_runtime_from_assets(assets, compositor)
    }
}

neocogi_com::factory_plugin! {
    name = "neocogi.ui.runtime",
    version = "0.9.0-alpha.0",
    contracts = [neocogi_ui_core_contracts::MODULE],
    factory = UiPluginFactoryObject,
}

// Static consumers call `factory_plugin_description` directly. Only a final dynamic artifact
// exposes the loader entrypoint, avoiding duplicate exports in monolithic applications.
#[cfg(feature = "dynamic-plugin")]
neocogi_com::export_factory_plugin!();
