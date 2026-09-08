//! UI assembly: one export packages runtime, window manager, widgets and layout.
//!
//! Applications supply immutable resource bundles; this assembly contains no demo assets or parsers.
//! The implementation library itself has no dynamic entrypoint.
#![deny(missing_docs)]
nc_com::plugin! {
    plugin_id(auto_guid = "microui.plugin.ui"),
    name = "microui.plugin.ui",
    version = "0.9.0-alpha.0",
    classes = [
        microui_ui::com::Runtime,
        microui_ui::com::UiFactoryObject,
        microui_ui::com::Button,
        microui_ui::com::Checkbox,
        microui_ui::com::Scrollbar,
        microui_ui::com::Combo,
        microui_ui::com::Textbox,
        microui_ui::com::TextBlock,
        microui_ui::com::TextAreaObject,
        microui_ui::com::Slider,
        microui_ui::com::Number,
        microui_ui::com::ColorSwatch,
        microui_ui::com::ListItem,
        microui_ui::com::ListBox,
        microui_ui::com::Linear,
        microui_ui::com::Grid,
        microui_ui::com::ScrollAreaObject,
        microui_ui::com::Disclosure,
    ],
}

// Only the final native assembly publishes the dynamic entrypoint.
#[cfg(feature = "dynamic-plugin")]
nc_com::export_plugin!();
