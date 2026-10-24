//! Real UI source graph shared by generated-code event regressions.
use std::path::Path;
use neocogi_idl::{Module, ResolvedModules, ModuleGraphError};

/// Loads each contract owner, preserving its complete authored ABI fingerprint.
pub fn resolve(mut consumers: Vec<Module>) -> Result<ResolvedModules, ModuleGraphError> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for (source_root, path) in [
        ("crates/render/schemas", "neocogi/renderer"),
        ("crates/platform/schemas", "neocogi/platform"),
        ("crates/gfx/schemas", "neocogi/gfx"),
        ("crates/ui/schemas", "neocogi/ui/core"),
    ] {
        consumers.push(
            neocogi_idl::cargo::ModuleSources::load(&root.join(source_root), Path::new(path))
                .unwrap()
                .parse()
                .unwrap(),
        );
    }
    neocogi_idl::resolve_modules(consumers)
}

/// Maps actual supplying contracts even when a fixture imports only the UI root.
pub fn paths() -> neocogi_idl::generate::RustModulePaths {
    let mut paths = neocogi_idl::generate::RustModulePaths::default();
    for (logical, rust) in [
        ("neocogi.renderer", "neocogi_render_core::abi"),
        ("neocogi.platform", "neocogi_render_core::platform"),
        ("neocogi.gfx", "neocogi_gfx_support::abi"),
        ("neocogi.ui.core", "neocogi_ui::abi::core"),
    ] {
        paths.insert(logical, rust).unwrap();
    }
    paths
}
