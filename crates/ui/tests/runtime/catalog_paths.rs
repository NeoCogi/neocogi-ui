//! UI catalog paths survive real form export and generated COM calls.
#[path = "../package-inputs/com_dependencies.rs"]
mod com_dependencies;
use std::path::Path;
/// Builds an IDL owner, discovers its COM catalog, then compiles a downstream form.
/// The fixture lives in a temporary workspace so its raw module and Cargo dependency
/// names are real Rust resolution boundaries, independent of test-crate imports.
#[test]
fn raw_identifier_catalog_paths_compile_in_exported_forms() {
    use std::{fs, process::Command};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let project = tempfile::tempdir().unwrap();
    let directory = project.path();
    fs::create_dir_all(directory.join("contracts/src/bin")).unwrap();
    fs::create_dir_all(directory.join("idl/example")).unwrap();
    fs::create_dir_all(directory.join("app/src")).unwrap();
    fs::write(directory.join("Cargo.toml"), "[workspace]\nmembers=['contracts','app']\nresolver='3'\n").unwrap();
    // The provider and application share source, not a Cargo graph owner.
    for (source_root, namespace) in [
        (Path::new(env!("OUT_DIR")).join("neocogi-shared-sources"), "neocogi/audio"),
        (Path::new(env!("OUT_DIR")).join("neocogi-shared-sources"), "neocogi/renderer"),
        (Path::new(env!("OUT_DIR")).join("neocogi-shared-sources"), "neocogi/platform"),
        (Path::new(env!("OUT_DIR")).join("neocogi-ui-sources"), "neocogi/gfx"),
        (Path::new(env!("OUT_DIR")).join("neocogi-ui-sources"), "neocogi/ui/core"),
    ] {
        let module = neocogi_idl::cargo::ModuleSources::load(&source_root, Path::new(namespace)).unwrap();
        for (relative, text) in module.parts() {
            let source = directory.join("idl").join(relative);
            fs::create_dir_all(source.parent().unwrap()).unwrap();
            fs::write(source, text).unwrap();
        }
    }
    let dependencies = format!(
        "[dependencies]\n{}{}neocogi-ui-contracts={{path={:?}}}\nneocogi-ui={{path={:?},features=['document']}}\n",
        com_dependencies::dependency("neocogi-com", &["std", "macros", "runtime"]),
        com_dependencies::dependency("neocogi-com-host", &[]),
        root.join("crates/ui-contracts"),
        root.join("crates/ui"),
    );
    fs::write(
        directory.join("contracts/Cargo.toml"),
        format!(
            "[package]\nname='path-contracts'\nversion='0.0.0'\nedition='2024'\n{dependencies}serde_json='1'\n[build-dependencies]\n{}",
            com_dependencies::dependency("neocogi-idl", &[]),
        ),
    )
    .unwrap();
    fs::write(
        directory.join("contracts/build.rs"),
        r#"
        /// Publishes generated declarations under a Rust raw-identifier module.
        fn main() {
            for module in neocogi_idl::cargo::SharedSources::new("../idl")
                .module("example.paths", "path_contracts::r#type")
                .import("neocogi.ui.core", "neocogi_ui_contracts::abi::core")
                .import("neocogi.audio", "neocogi_platform_contracts::audio")
                .import("neocogi.renderer", "neocogi_platform_contracts::render::abi")
                .import("neocogi.platform", "neocogi_platform_contracts::render::platform")
                .import("neocogi.gfx", "neocogi_ui_contracts::gfx::abi")
                .build().unwrap() {
                println!("cargo::rustc-env=PATH_BINDINGS={}", module.include.display());
            }
        }
    "#,
    )
    .unwrap();
    fs::write(
        directory.join("idl/example/paths.ncidl"),
        r#"
        extern neocogi.ui.core;
        use neocogi.ui.core::{EventBinding, IEvent};
        use neocogi.ui.core::IWidget;
        /// Nested getter value exposes the real generated type across modules.
        #[version(1)] struct CounterChanged {
            /// Count read from the immutable event by application code.
            value: u32
        }
        /// One complete typed stream, with nested and unit variants.
        #[version(1)] enum CounterEvent {
            /// Carries the new count.
            Changed(CounterChanged),
            /// Restores the application count to zero.
            Reset
        }
        /// Immutable copied native view exposed through an ordinary event getter.
        #[version(1)] interface ICounterEvent: IEvent { fn value(&self) -> Result<CounterEvent>; }
        /// Concrete projection whose Rust module location uses a raw identifier.
        #[version(1)] interface CounterFactory { fn create(&self, events: &EventBinding) -> Result<ComBox<Counter>>; }
        #[version(1)] interface Counter: IWidget {}
    "#,
    )
    .unwrap();
    fs::write(
        directory.join("contracts/src/lib.rs"),
        r#"
        /// Real public module named with a Rust keyword, separate from its IDL identity.
        pub mod r#type {
            include!(env!("PATH_BINDINGS"));
            neocogi_ui_contracts::ui_event_sources! { CounterAbi => [ICounterEventAbi]; }
            #[neocogi_com::com_object(interfaces(ICounterEvent), ownership = exclusive)]
            struct Snapshot(CounterEvent);
            impl neocogi_ui_contracts::events::IEvent for Snapshot {}
            impl ICounterEvent for Snapshot {
                fn value(&self) -> Result<CounterEvent, neocogi_com::NcStatus> { Ok(self.0) }
            }
            /// Creates the immutable event directly; form generation has no event adapters.
            pub fn changed(value: CounterEvent) -> Result<neocogi_com::ComBox<neocogi_ui_contracts::events::IEventAbi>, neocogi_com::NcStatus> {
                Snapshot::into_box(Snapshot(value)).query().map_err(|(status, _)| status)
            }
        }
    "#,
    )
    .unwrap();
    fs::write(directory.join("contracts/src/bin/catalog.rs"), include_str!("fixtures/rust_paths/catalog.rs")).unwrap();
    fs::write(
        directory.join("app/Cargo.toml"),
        format!("[package]\nname='path-app'\nversion='0.0.0'\nedition='2024'\n{dependencies}path-contracts={{path='../contracts'}}\n",),
    )
    .unwrap();
    fs::write(directory.join("app/src/lib.rs"), include_str!("fixtures/rust_paths/application.rs")).unwrap();
    for arguments in [
        vec!["run", "--offline", "-p", "path-contracts", "--bin", "catalog", "--", "app/src"],
        vec!["test", "--offline", "-p", "path-app"],
    ] {
        let output = Command::new("cargo")
            .args(&arguments)
            .current_dir(directory)
            .env(
                "CARGO_TARGET_DIR",
                std::env::var_os("CARGO_TARGET_DIR").unwrap_or_else(|| root.join("target").into_os_string()),
            )
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{arguments:?}\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
