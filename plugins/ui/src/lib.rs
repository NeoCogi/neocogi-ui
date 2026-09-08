//! UI assembly: one export packages runtime, window manager, widgets and layout.
//!
//! Applications supply immutable resource bundles; this assembly contains no demo assets or parsers.
//! The implementation library itself has no dynamic entrypoint.
#![deny(missing_docs)]
nc_com::plugin! {
    plugin_id(auto_guid = "microui.plugin.ui"),
    name = "microui.plugin.ui",
    version = "0.9.0-alpha.0",
    classes = [microui_ui::com::Runtime],
}

// Only the final native assembly publishes the dynamic entrypoint.
#[cfg(feature = "dynamic-plugin")]
nc_com::export_plugin!();
