//! Real UI source graph shared by generated-code event regressions.
use std::path::Path;
use neocogi_idl::{Module, ResolvedModules, ResolutionError};

/// Loads each contract owner, preserving its complete authored ABI fingerprint.
pub fn resolve(mut consumers: Vec<Module>) -> Result<ResolvedModules, ResolutionError> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for (owner, path) in [
        ("crates/render/contracts", "neocogi/renderer"),
        ("crates/platform/contracts", "neocogi/platform"),
        ("crates/gfx/contracts", "neocogi/gfx"),
        ("crates/ui/core-contracts", "neocogi/ui/core"),
    ] {
        consumers.push(
            neocogi_idl::cargo::ModuleSources::load(&root.join(owner).join("idl"), Path::new(path))
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
        ("neocogi.renderer", "neocogi_renderer_contracts"),
        ("neocogi.platform", "neocogi_platform_contracts"),
        ("neocogi.gfx", "neocogi_gfx_contracts"),
        ("neocogi.ui.core", "neocogi_ui_core_contracts"),
    ] {
        paths.insert(logical, rust).unwrap();
    }
    paths
}
