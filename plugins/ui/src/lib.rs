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
        microui_ui::com::CheckboxObject,
        microui_ui::com::ScrollbarObject,
        microui_ui::com::ComboObject,
        microui_ui::com::TextboxObject,
        microui_ui::com::TextBlockObject,
        microui_ui::com::TextAreaObject,
        microui_ui::com::SliderObject,
        microui_ui::com::NumberObject,
        microui_ui::com::ColorSwatchObject,
        microui_ui::com::ListItemObject,
        microui_ui::com::ListBoxObject,
        microui_ui::com::LinearObject,
        microui_ui::com::GridObject,
        microui_ui::com::ScrollAreaObject,
        microui_ui::com::DisclosureObject,
    ],
}

// Only the final native assembly publishes the dynamic entrypoint.
#[cfg(feature = "dynamic-plugin")]
nc_com::export_plugin!();
