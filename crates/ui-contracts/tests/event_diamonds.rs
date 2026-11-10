//! Canonical getter ancestry must survive local generic projections and catalog transport.
#[path = "package-inputs/com_dependencies.rs"]
mod com_dependencies;
#[path = "support/event_support.rs"]
mod event_support;
use neocogi_idl::{AbiType, Module, parse_module};
use std::{fs, path::Path, process::Command};

/// Parses a fixture under the module identity used by its generated imports.
fn module(path: &str, source: &str) -> Module {
    parse_module(Path::new(path), source).unwrap()
}

/// Compiles independent local projections, calls their real exclusive COM getters,
/// and admits/round-trips the same generated event metadata through the catalog.
#[test]
fn shared_plain_and_generic_ancestors_publish_getters_once() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let graph = event_support::resolve(vec![
        module(
            "proof/contracts.ncidl",
            r#"
            extern neocogi.ui.core; use neocogi.ui.core::IEvent;
            #[version(1)] struct Detail { value: u32 }
            #[version(1)] interface Snapshot<T>: IEvent { fn value(&self) -> Result<T>; }
            #[version(1)] interface Plain: IEvent { fn value(&self) -> Result<Detail>; }
        "#,
        ),
        module(
            "proof/left.ncidl",
            r#"
            extern proof.contracts; use proof.contracts::{Snapshot, Detail, Plain};
            #[version(1)] interface Left: Snapshot<Detail> { fn left(&self) -> Result<bool>; }
            #[version(1)] interface ScalarLeft: Snapshot<u32> {}
            #[version(1)] interface PlainLeft: Plain { fn left(&self) -> Result<bool>; }
        "#,
        ),
        module(
            "proof/right.ncidl",
            r#"
            extern proof.contracts; use proof.contracts::{Snapshot, Detail, Plain};
            #[version(1)] interface Right: Snapshot<Detail> { fn right(&self) -> Result<bool>; }
            #[version(1)] interface ScalarRight: Snapshot<u32> {}
            #[version(1)] interface PlainRight: Plain { fn right(&self) -> Result<bool>; }
        "#,
        ),
        module(
            "proof/source.ncidl",
            r#"
            extern proof.left; extern proof.right;
            use proof.left::{Left, ScalarLeft, PlainLeft};
            use proof.right::{Right, ScalarRight, PlainRight};
            #[version(1)] interface Fact: Left + Right { fn revision(&self) -> Result<u32>; }
            #[version(1)] interface ScalarFact: ScalarLeft + ScalarRight {}
            #[version(1)] interface PlainFact: PlainLeft + PlainRight { fn revision(&self) -> Result<u32>; }
            #[version(1)] interface Source {}
        "#,
        ),
    ])
    .unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::create_dir(project.path().join("src")).unwrap();
    fs::write(
        project.path().join("Cargo.toml"),
        format!(
            "[package]\nname='event-diamonds-proof'\nversion='0.0.0'\nedition='2024'\n[workspace]\n[dependencies]\n{}neocogi-ui-contracts={{path={:?}}}\n",
            com_dependencies::dependency("neocogi-com", &["std", "macros"]),
            root.join("crates/ui-contracts"),
        ),
    )
    .unwrap();
    let mut paths = event_support::paths();
    paths.insert("neocogi.ui.core", "neocogi_ui_contracts::abi::core").unwrap();
    for name in ["contracts", "left", "right", "source"] {
        paths.insert(&format!("proof.{name}"), &format!("event_diamonds_proof::{name}")).unwrap();
    }
    for name in ["contracts", "left", "right", "source"] {
        let owner = format!("proof.{name}");
        let mut generated = neocogi_idl::generate::generate_rust_with_paths(&graph, &owner, &paths).unwrap();
        if matches!(name, "left" | "right") {
            // Aliases select the actual closed declarations; their mangled Rust
            // names are backend details and must not define the test's contract.
            for declaration in &graph.module(&owner).unwrap().source.declarations {
                let Some(instance) = &declaration.specialization else { continue };
                let alias = match instance.arguments.as_slice() {
                    [AbiType::Scalar(value)] if value == "u32" => "ScalarSnapshot",
                    [AbiType::Named(_)] => "DetailSnapshot",
                    arguments => panic!("unexpected fixture specialization: {arguments:?}"),
                };
                generated.push_str(&format!(
                    "\npub use {} as {alias};\npub use {}Abi as {alias}Abi;\n",
                    declaration.name, declaration.name
                ));
            }
        }
        fs::write(project.path().join(format!("src/{name}.rs")), generated).unwrap();
    }
    fs::write(project.path().join("src/lib.rs"), include_str!("fixtures/event_diamonds.rs")).unwrap();
    let output = Command::new("cargo")
        .args(["test", "--offline", "--manifest-path"])
        .arg(project.path().join("Cargo.toml"))
        .env(
            "CARGO_TARGET_DIR",
            std::env::var_os("CARGO_TARGET_DIR").unwrap_or_else(|| root.join("target").into_os_string()),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("3 passed"),
        "all compiled diamond boundary tests must execute"
    );
}
