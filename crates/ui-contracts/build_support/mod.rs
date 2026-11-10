//! Build-only source ownership for NeoCogi UI, GFX and Skin contracts.
//!
//! Shared declarations are embedded from this package's one authored tree. Each
//! consumer reconstructs its own inputs and owns its generated bindings; no runtime
//! contract library or neighboring checkout supplies build output or authored files.
#![deny(missing_docs)]

use std::{
    fs,
    path::{Path, PathBuf},
};
pub use neocogi_idl::cargo::{BuildError, BuildOutput, SharedSources};

/// A shared namespace with an established ordinary Rust definition owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Module {
    /// Typed root of the consolidated UI library.
    Library,
    /// Portable graphics bundles, construction and composition contracts.
    Gfx,
    /// Appearance and theme compilation contracts.
    Skin,
    /// Retained UI runtime, widget, event, window and menu contracts.
    UiCore,
    /// Built-in widget and layout construction contracts.
    UiNodes,
    /// Authored document lookup and custom component contracts.
    UiDocument,
    /// Private choice-menu protocol emitted only into the retained runtime.
    UiRuntimeInternal,
}
impl Module {
    /// Dotted authored namespace; independent of package download locations.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Library => "neocogi.ui.library",
            Self::Gfx => "neocogi.gfx",
            Self::Skin => "neocogi.skin",
            Self::UiCore => "neocogi.ui.core",
            Self::UiNodes => "neocogi.ui.nodes",
            Self::UiDocument => "neocogi.ui.document",
            Self::UiRuntimeInternal => "neocogi.ui.runtime.internal",
        }
    }
    /// Ordinary definition owner used for unselected reachable imports.
    const fn rust_path(self) -> &'static str {
        match self {
            Self::Library => "neocogi_ui_contracts::abi::library",
            Self::Gfx => "neocogi_ui_contracts::gfx::abi",
            Self::Skin => "neocogi_ui_contracts::appearance::skin",
            Self::UiCore => "neocogi_ui_contracts::abi::core",
            Self::UiNodes => "neocogi_ui_contracts::abi::nodes",
            Self::UiDocument => "neocogi_ui_contracts::abi::document",
            Self::UiRuntimeInternal => "neocogi_ui::runtime::choice_menu::bindings",
        }
    }
}

/// Canonical authored bytes survive relocation or removal of Cargo's downloaded source tree.
const SOURCES: &[(&str, &[u8])] = &[
    (
        "neocogi/ui/library/services.ncidl",
        include_bytes!("../schemas/neocogi/ui/library/services.ncidl"),
    ),
    ("neocogi/gfx/building.ncidl", include_bytes!("../schemas/neocogi/gfx/building.ncidl")),
    ("neocogi/gfx/compositor.ncidl", include_bytes!("../schemas/neocogi/gfx/compositor.ncidl")),
    ("neocogi/gfx/dependencies.ncidl", include_bytes!("../schemas/neocogi/gfx/dependencies.ncidl")),
    ("neocogi/gfx/diagnostics.ncidl", include_bytes!("../schemas/neocogi/gfx/diagnostics.ncidl")),
    ("neocogi/gfx/plugin.ncidl", include_bytes!("../schemas/neocogi/gfx/plugin.ncidl")),
    ("neocogi/gfx/resources.ncidl", include_bytes!("../schemas/neocogi/gfx/resources.ncidl")),
    ("neocogi/gfx/textures.ncidl", include_bytes!("../schemas/neocogi/gfx/textures.ncidl")),
    ("neocogi/skin/dependencies.ncidl", include_bytes!("../schemas/neocogi/skin/dependencies.ncidl")),
    ("neocogi/skin/diagnostics.ncidl", include_bytes!("../schemas/neocogi/skin/diagnostics.ncidl")),
    ("neocogi/skin/plugin.ncidl", include_bytes!("../schemas/neocogi/skin/plugin.ncidl")),
    ("neocogi/skin/themes.ncidl", include_bytes!("../schemas/neocogi/skin/themes.ncidl")),
    (
        "neocogi/ui/core/dependencies.ncidl",
        include_bytes!("../schemas/neocogi/ui/core/dependencies.ncidl"),
    ),
    ("neocogi/ui/core/dispatch.ncidl", include_bytes!("../schemas/neocogi/ui/core/dispatch.ncidl")),
    ("neocogi/ui/core/events.ncidl", include_bytes!("../schemas/neocogi/ui/core/events.ncidl")),
    ("neocogi/ui/core/geometry.ncidl", include_bytes!("../schemas/neocogi/ui/core/geometry.ncidl")),
    (
        "neocogi/ui/core/input_events.ncidl",
        include_bytes!("../schemas/neocogi/ui/core/input_events.ncidl"),
    ),
    ("neocogi/ui/core/layout.ncidl", include_bytes!("../schemas/neocogi/ui/core/layout.ncidl")),
    (
        "neocogi/ui/core/menus_platform.ncidl",
        include_bytes!("../schemas/neocogi/ui/core/menus_platform.ncidl"),
    ),
    ("neocogi/ui/core/plugin.ncidl", include_bytes!("../schemas/neocogi/ui/core/plugin.ncidl")),
    ("neocogi/ui/core/rendering.ncidl", include_bytes!("../schemas/neocogi/ui/core/rendering.ncidl")),
    ("neocogi/ui/core/runtime.ncidl", include_bytes!("../schemas/neocogi/ui/core/runtime.ncidl")),
    (
        "neocogi/ui/core/semantic_events.ncidl",
        include_bytes!("../schemas/neocogi/ui/core/semantic_events.ncidl"),
    ),
    ("neocogi/ui/core/widgets.ncidl", include_bytes!("../schemas/neocogi/ui/core/widgets.ncidl")),
    (
        "neocogi/ui/document/component.ncidl",
        include_bytes!("../schemas/neocogi/ui/document/component.ncidl"),
    ),
    (
        "neocogi/ui/nodes/dependencies.ncidl",
        include_bytes!("../schemas/neocogi/ui/nodes/dependencies.ncidl"),
    ),
    ("neocogi/ui/nodes/events.ncidl", include_bytes!("../schemas/neocogi/ui/nodes/events.ncidl")),
    ("neocogi/ui/nodes/factory.ncidl", include_bytes!("../schemas/neocogi/ui/nodes/factory.ncidl")),
    ("neocogi/ui/nodes/layout.ncidl", include_bytes!("../schemas/neocogi/ui/nodes/layout.ncidl")),
    (
        "neocogi/ui/nodes/semantic_events.ncidl",
        include_bytes!("../schemas/neocogi/ui/nodes/semantic_events.ncidl"),
    ),
    ("neocogi/ui/nodes/widgets.ncidl", include_bytes!("../schemas/neocogi/ui/nodes/widgets.ncidl")),
    (
        "neocogi/ui/runtime/internal/choice_menu.ncidl",
        include_bytes!("../schemas/neocogi/ui/runtime/internal/choice_menu.ncidl"),
    ),
];

/// Supplies operation and path context before any binding generation takes place.
fn io_error(operation: &'static str, path: &Path, source: std::io::Error) -> BuildError {
    BuildError::Io { operation, path: path.to_owned(), source }
}

/// Prunes only this helper's derived subtree; stale declarations cannot reenter resolution.
fn prune(root: &Path, directory: &Path) -> Result<(), BuildError> {
    for entry in fs::read_dir(directory).map_err(|error| io_error("read UI sources", directory, error))? {
        let entry = entry.map_err(|error| io_error("read UI source entry", directory, error))?;
        let path = entry.path();
        let kind = entry.file_type().map_err(|error| io_error("inspect UI source", &path, error))?;
        if kind.is_dir() {
            prune(root, &path)?;
        } else if !kind.is_file() || !SOURCES.iter().any(|(name, _)| root.join(name) == path) {
            fs::remove_file(&path).map_err(|error| io_error("remove obsolete UI source", &path, error))?;
        }
    }
    Ok(())
}

/// Reconstructs canonical inputs below an explicit consumer-owned output directory.
///
/// Only the `neocogi-ui-sources` child is owned and pruned. Unchanged files keep
/// their mtimes. Fixtures can use this operation without changing process-wide
/// environment variables; ordinary build scripts should use [`sources`].
pub fn materialize(output: &Path) -> Result<PathBuf, BuildError> {
    let root = output.join("neocogi-ui-sources");
    fs::create_dir_all(&root).map_err(|error| io_error("create UI source root", &root, error))?;
    prune(&root, &root)?;
    for (name, contents) in SOURCES {
        let path = root.join(name);
        fs::create_dir_all(path.parent().expect("fixed source name has a parent")).map_err(|error| io_error("create UI source namespace", &path, error))?;
        if fs::read(&path).ok().as_deref() != Some(*contents) {
            fs::write(&path, contents).map_err(|error| io_error("write UI source", &path, error))?;
        }
    }
    Ok(root)
}

/// Selects local emissions and imports for the UI-owned namespaces.
///
/// Platform sources come from the matching packaged Platform helper. Other reachable
/// namespaces use their ordinary contract libraries. Additional example namespaces
/// can be added with `include_root` and `module`; they never replace these owners.
/// The caller must supply Cargo's OUT_DIR and handle any source preparation error.
pub fn sources(local: &[Module]) -> Result<SharedSources, BuildError> {
    let output = std::env::var_os("OUT_DIR").ok_or(BuildError::MissingEnvironment("OUT_DIR"))?;
    let mut request = neocogi_platform_contracts::build::sources(&[])?.include_root(materialize(Path::new(&output))?);
    for module in [
        Module::Library,
        Module::Gfx,
        Module::Skin,
        Module::UiCore,
        Module::UiNodes,
        Module::UiDocument,
        Module::UiRuntimeInternal,
    ] {
        request = if local.contains(&module) {
            request.module(module.name(), module.rust_path())
        } else {
            request.import(module.name(), module.rust_path())
        };
    }
    Ok(request)
}

/// Generates into this consumer and publishes include paths only after complete success.
pub fn build(request: SharedSources) -> Result<Vec<BuildOutput>, BuildError> {
    let modules = request.build()?;
    for module in &modules {
        println!(
            "cargo::rustc-env=IDL_{}={}",
            module.module_name.replace('.', "_").to_uppercase(),
            module.include.display()
        );
    }
    Ok(modules)
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Warm reuse restores modified bytes, removes obsolete inputs and preserves all other output.
    #[test]
    fn owned_sources_are_complete_repaired_and_reusable() {
        let output = tempfile::tempdir().unwrap();
        fs::write(output.path().join("unrelated"), b"preserve me").unwrap();
        let root = materialize(output.path()).unwrap();
        for (name, bytes) in SOURCES {
            assert_eq!(fs::read(root.join(name)).unwrap(), *bytes);
        }
        let keep = root.join(SOURCES[0].0);
        let stamp = fs::metadata(&keep).unwrap().modified().unwrap();
        let changed = root.join(SOURCES[1].0);
        fs::write(&changed, b"changed").unwrap();
        let stale = root.join("neocogi/ui/core/obsolete.ncidl");
        fs::write(&stale, b"stale declaration").unwrap();
        assert_eq!(materialize(output.path()).unwrap(), root);
        assert_eq!(fs::metadata(keep).unwrap().modified().unwrap(), stamp);
        assert_eq!(fs::read(changed).unwrap(), SOURCES[1].1);
        assert!(!stale.exists());
        assert_eq!(fs::read(output.path().join("unrelated")).unwrap(), b"preserve me");
    }
    /// Cached helpers reconstruct every namespace independently after the first consumer disappears.
    #[test]
    fn separate_outputs_do_not_depend_on_an_earlier_consumers_files() {
        let first = tempfile::tempdir().unwrap();
        let first_root = materialize(first.path()).unwrap();
        first.close().unwrap();
        assert!(!first_root.exists());
        let second = tempfile::tempdir().unwrap();
        let root = materialize(second.path()).unwrap();
        for (name, _) in SOURCES {
            assert!(root.join(name).is_file());
        }
    }
}
