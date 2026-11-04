//! Real generated factory calls prove that event ownership starts at construction.
#[path = "package-inputs/com_dependencies.rs"]
mod com_dependencies;
#[path = "support/event_support.rs"]
mod event_support;
use neocogi_idl::{parse_module};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// Locates the checked-in generic COM contracts independently of the test cwd.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

/// Resolves one independent consumer against the exact shipped library templates.
fn resolve(source: &str) -> Result<neocogi_idl::ResolvedModules, neocogi_idl::ModuleGraphError> {
    event_support::resolve(vec![parse_module(Path::new("proof/construction.ncidl"), source).unwrap()])
}

/// Emits only after constructor contracts and their public names have resolved.
fn generate(source: &str) -> Result<String, neocogi_idl::GenerateError> {
    generate_rust(&resolve(source).unwrap(), "proof.construction")
}

/// Compiles generated values, methods and receiver metadata, then exercises
/// ownership through checked COM factory/producer calls rather than Rust shortcuts.
#[test]
fn constructor_owns_one_permanent_sink_and_publishes_its_reachable_types() {
    let source = r#"
        extern neocogi.ui.core; use neocogi.ui.core::{EventBinding, IEvent};
        // These local declarations must not capture support names in generated
        // event binding conversions, method results, or catalog metadata.
        #[version(1)] struct Option { marker: u32 }
        #[version(1)] struct Some { marker: u32 }
        #[version(1)] struct None { marker: u32 }
        #[version(1)] struct Changed { text: String, enabled: bool, raw_0: u8, budget: u32 }
        #[version(1)] interface IChangedEvent: IEvent {
            fn text(&self) -> Result<ComPtr<INcString>>;
            fn enabled(&self) -> Result<bool>;
            fn raw_0(&self) -> Result<u8>;
            fn budget(&self) -> Result<u32>;
        }
        #[version(1)] interface ISubmittedEvent: IEvent {}
        #[version(1)] interface Base {}
        #[version(1)] interface Counter: Base {
            fn emit(&mut self, value: &Changed) -> Result<()>;
            fn submit(&mut self) -> Result<()>;
            fn close(&mut self) -> Result<()>;
        }
        #[version(1)] #[threading(shared)] interface SharedOrigin {}
        #[version(1)] interface SharedFactory {
            fn create(&self, events: &EventBinding) -> Result<ComPtr<SharedOrigin>>;
        }
        #[version(1)] interface Factory {
            fn create(&self, reject: bool, events: &EventBinding) -> Result<ComPtr<Counter>>;
        }
    "#;
    let project = tempfile::tempdir().unwrap();
    fs::create_dir(project.path().join("src")).unwrap();
    fs::write(
        project.path().join("Cargo.toml"),
        format!(
            "[package]\nname='construction-events-proof'\nversion='0.0.0'\nedition='2024'\n[workspace]\n[dependencies]\n{}neocogi-ui={{path={:?}}}\nneocogi-ui-design={{path={:?}}}\n",
            com_dependencies::dependency("neocogi-com", &["std", "macros"]), root().join("crates/ui/api"), root().join("crates/ui/design")
        ),
    )
    .unwrap();
    fs::write(project.path().join("src/contract.rs"), generate(source).unwrap()).unwrap();
    fs::write(project.path().join("src/lib.rs"), include_str!("fixtures/construction_events.rs")).unwrap();
    let result = Command::new("cargo")
        .args(["test", "--offline", "--manifest-path"])
        .arg(project.path().join("Cargo.toml"))
        .env(
            "CARGO_TARGET_DIR",
            std::env::var_os("CARGO_TARGET_DIR").unwrap_or_else(|| root().join("target").into_os_string()),
        )
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    // An independently compiled caller cannot reopen either the public product
    // or its private producer helper. Runtime replacement rejection alone would
    // leave the unwanted attachment protocol available to future consumers.
    fs::write(
        project.path().join("src/lib.rs"),
        r#"
            pub mod contract;
            use contract::*;
            use neocogi_com::ComPtr;
            use neocogi_ui::events::IEventReceiverAbi;
            pub fn cannot_attach(
                mut product: ComPtr<CounterAbi>,
                mut port: neocogi_ui::events::EventPort,
                queue: ComPtr<IEventReceiverAbi>,
            ) {
                product.attach(queue.as_com_ref());
                port.attach(queue.as_com_ref());
                IEventReceiverAbi::receiver(|_, _| Ok(()));
            }
        "#,
    )
    .unwrap();
    let result = Command::new("cargo")
        .args(["check", "--offline", "--manifest-path"])
        .arg(project.path().join("Cargo.toml"))
        .env(
            "CARGO_TARGET_DIR",
            std::env::var_os("CARGO_TARGET_DIR").unwrap_or_else(|| root().join("target").into_os_string()),
        )
        .output()
        .unwrap();
    let diagnostics = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success(), "late attachment unexpectedly compiled");
    assert_eq!(diagnostics.matches("no method named `attach`").count(), 2, "{diagnostics}");
    assert!(diagnostics.contains("named `receiver` found"), "{diagnostics}");
}

/// Standalone fixtures consume the shipped common receiver from its owning crate.
fn generate_rust(graph: &neocogi_idl::ResolvedModules, module: &str) -> Result<String, neocogi_idl::GenerateError> {
    let mut paths = event_support::paths();
    if graph.module("neocogi.ui.core").is_some() {
        paths.insert("neocogi.ui.core", "neocogi_ui::abi::core").unwrap();
    }
    neocogi_idl::generate::generate_rust_with_paths(graph, module, &paths)
}
