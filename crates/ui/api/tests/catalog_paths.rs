//! UI catalog paths survive real form export and generated COM calls.
use std::path::Path;
/// Builds an IDL owner, discovers its COM catalog, then compiles a downstream form.
/// The fixture lives in a temporary workspace so its raw module and Cargo dependency
/// names are real Rust resolution boundaries, independent of test-crate imports.
#[test]
fn raw_identifier_catalog_paths_compile_in_exported_forms() {
    use std::{fs, process::Command};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap();
    let project = tempfile::tempdir().unwrap();
    let directory = project.path();
    fs::create_dir_all(directory.join("contracts/src/bin")).unwrap();
    fs::create_dir_all(directory.join("contracts/idl/example")).unwrap();
    fs::create_dir_all(directory.join("app/src")).unwrap();
    fs::write(directory.join("Cargo.toml"), "[workspace]\nmembers=['contracts','app']\nresolver='3'\n").unwrap();
    let dependencies = format!(
        "[dependencies]\nneocogi-com={{path={:?},features=['runtime']}}\nneocogi-com-runtime={{path={:?}}}\nneocogi-ui={{path={:?}}}\nneocogi-ui-document={{path={:?}}}\n",
        root.join("crates/com/api"),
        root.join("crates/com/runtime"),
        root.join("crates/ui/api"),
        root.join("crates/ui/document"),
    );
    fs::write(directory.join("contracts/Cargo.toml"), format!(
        "[package]\nname='path-contracts'\nversion='0.0.0'\nedition='2024'\nlinks='nc_contract_6578616d706c652e7061746873'\n{dependencies}neocogi-ui-core-contracts={{path={:?}}}\nserde_json='1'\n[build-dependencies]\nneocogi-idl={{path={:?}}}\n",
        root.join("crates/ui/core-contracts"), root.join("crates/com/idl"),
    )).unwrap();
    fs::write(
        directory.join("contracts/build.rs"),
        r#"
        /// Publishes generated declarations under a Rust raw-identifier module.
        fn main() {
            neocogi_idl::cargo::Module::new("idl", "example/paths.ncidl")
                .rust_path("path_contracts::r#type").build().unwrap();
        }
    "#,
    )
    .unwrap();
    fs::write(
        directory.join("contracts/idl/example/paths.ncidl"),
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
            include!(concat!(env!("OUT_DIR"), "/example_paths.include.rs"));
            neocogi_ui::ui_event_sources! { CounterAbi => [ICounterEventAbi]; }
            #[neocogi_com::com_object(interfaces(ICounterEvent), ownership = exclusive)]
            struct Snapshot(CounterEvent);
            impl neocogi_ui::events::IEvent for Snapshot {}
            impl ICounterEvent for Snapshot {
                fn value(&self) -> Result<CounterEvent, neocogi_com::NcStatus> { Ok(self.0) }
            }
            /// Creates the immutable event directly; form generation has no event adapters.
            pub fn changed(value: CounterEvent) -> Result<neocogi_com::ComBox<neocogi_ui::events::IEventAbi>, neocogi_com::NcStatus> {
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
