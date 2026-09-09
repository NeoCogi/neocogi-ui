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
        microui_ui::Runtime,
        microui_ui::UiFactory,
        microui_ui::Button,
        microui_ui::Checkbox,
        microui_ui::Scrollbar,
        microui_ui::Combo,
        microui_ui::Textbox,
        microui_ui::TextBlock,
        microui_ui::TextArea,
        microui_ui::Slider,
        microui_ui::Number,
        microui_ui::ColorSwatch,
        microui_ui::ListItem,
        microui_ui::ListBox,
        microui_ui::Linear,
        microui_ui::Grid,
        microui_ui::ScrollArea,
        microui_ui::Disclosure,
    ],
}

// Only the final native assembly publishes the dynamic entrypoint.
#[cfg(feature = "dynamic-plugin")]
nc_com::export_plugin!();
