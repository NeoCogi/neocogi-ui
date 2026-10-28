//! UI admission validates ordinary IDL methods rather than extending the compiler language.
#[path = "../../../../scripts/com_dependencies.rs"]
mod com_dependencies;
#[path = "support/event_support.rs"]
mod event_support;
use std::{fs, path::Path, process::Command};

/// Compiles legal COM contracts, then proves that only valid snapshots enter UI catalogs.
#[test]
fn ui_admits_snapshot_contracts_and_merges_their_value_graphs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap();
    let module = neocogi_idl::parse_module(
        Path::new("proof/admission.ncidl"),
        r#"
        extern neocogi.ui.core; use neocogi.ui.core::IEvent;
        #[version(1)] struct Detail { enabled: bool }
        #[version(1)] enum Value { Empty, Captured(Detail) }
        #[version(1)] interface Good: IEvent { fn value(&self) -> Result<Value>; }
        #[version(1)] interface Unrelated { fn value(&self) -> Result<u32>; }
        #[version(1)] interface Mutable: IEvent { fn value(&mut self) -> Result<u32>; }
        #[version(1)] interface Parameter: IEvent { fn value(&self, index: u32) -> Result<u32>; }
        #[version(1)] interface Transfer: IEvent { fn take(&self) -> Result<ComBox<IEvent>>; }
        #[version(1)] interface Left: IEvent { fn value(&self) -> Result<u32>; }
        #[version(1)] interface Right: IEvent { fn value(&self) -> Result<bool>; }
        #[version(1)] interface Ambiguous: Left + Right {}
        #[version(1)] interface Base<T>: IEvent { fn value(&self) -> Result<T>; }
        #[version(1)] interface First: Base<u32> {}
        #[version(1)] interface Second: Base<bool> {}
        #[version(1)] interface GenericAmbiguous: First + Second {}
        #[version(1)] interface Source {}
        #[version(1)] interface Calls {
            fn replace(&mut self, text: &str, data: &[bool], output: &mut [u32], object: ComBox<IEvent>) -> Result<()>;
        }
    "#,
    )
    .unwrap();
    let graph = event_support::resolve(vec![module]).unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::create_dir(project.path().join("src")).unwrap();
    fs::write(project.path().join("Cargo.toml"), format!(
        "[package]\nname='ui-admission-proof'\nversion='0.0.0'\nedition='2024'\n[workspace]\n[dependencies]\n{}neocogi-ui={{path={:?}}}\nneocogi-ui-design={{path={:?}}}\n",
        com_dependencies::dependency("neocogi-com", &["std", "macros"]), root.join("crates/ui/api"), root.join("crates/ui/design"),
    )).unwrap();
    fs::write(
        project.path().join("src/contract.rs"),
        neocogi_idl::generate::generate_rust_with_paths(&graph, "proof.admission", &event_support::paths()).unwrap(),
    )
    .unwrap();
    fs::write(project.path().join("src/lib.rs"), include_str!("fixtures/event_metadata.rs")).unwrap();
    let output = Command::new("cargo")
        .args(["test", "--offline", "--manifest-path"])
        .arg(project.path().join("Cargo.toml"))
        .env(
            "CARGO_TARGET_DIR",
            std::env::var_os("CARGO_TARGET_DIR").unwrap_or_else(|| root.join("target").into()),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("3 passed"));
}
