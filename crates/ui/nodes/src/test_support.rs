//! Fixtures keep production core authority alive beside unchanged concrete node values.
pub(crate) use neocogi_ui_support::test_support::*;
/// Direct algorithm ownership uses the normal host-backed production constructors.
pub(crate) mod algorithm;

/// Assembles production UI and GFX objects from deterministic canonical resources.
/// The fixture adds no alternate routing, traversal, layout, or compositor implementation.
pub(crate) fn create_runtime(atlas: crate::AtlasHandle) -> neocogi_com::ComPtr<neocogi_ui_core_contracts::UiRuntimeAbi> {
    let source = assets::export_bundle(&crate::SkinBundle::from_atlas(atlas)).expect("valid fixture resources");
    let gfx = neocogi_ui_support::test_support::gfx_factory();
    nc_ui::create_runtime_from_assets(source.as_com_ref(), gfx.as_com_ref()).expect("production core runtime")
}
