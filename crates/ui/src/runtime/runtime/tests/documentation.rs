//! Public walkthroughs run against real providers instead of copied test examples.
#[path = "../../../../tests/package-inputs/com_dependencies.rs"]
mod com_dependencies;

use std::{fs, path::Path, process::Command};

#[path = "../../../../tests/package-inputs/button_events.rs"]
mod button_events;

/// The exact rustdoc source must mount, deliver, and expire its weak event origin.
#[test]
fn documented_button_walkthrough_activates_and_drops_queued_origin() {
    let ui = neocogi_ui::nodes::bind_builtin_nodes(super::runtime()).unwrap();
    button_events::walkthrough(&ui, super::SIZE).unwrap();
}

/// Extracts exactly one marked README block without duplicating its authored code.
fn example<'a>(document: &'a str, label: &str) -> &'a str {
    let marker = format!("<!-- checked-example: {label} -->");
    assert_eq!(document.matches(&marker).count(), 1, "one {label} example must remain documented");
    let after = document.split_once(&marker).unwrap().1.trim_start();
    assert!(after.starts_with("```"), "{label} marker must immediately precede its code block");
    let (_, body) = after.split_once('\n').unwrap();
    body.split_once("\n```").expect("example has a closing fence").0
}

/// Root onboarding and reload/dispatch blocks compile with newly generated fields.
/// They execute real input, owned events, weak-origin expiry and stale-form filtering.
#[test]
fn documented_onboarding_reload_and_event_dispatch_execute() {
    let readme = include_str!("../../../../tests/package-inputs/document.md");
    let document = neocogi_ui::document::Document::new("README events");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::create_dir(project.path().join("src")).unwrap();
    fs::write(
        project.path().join("Cargo.toml"),
        format!(
            "[package]\nname='documented-form-events'\nversion='0.0.0'\nedition='2024'\n[workspace]\n[dependencies]\n{}{}neocogi-ui={{path={:?},features=['test-support','document']}}\nneocogi-ui-contracts={{path={:?}}}\n",
            com_dependencies::dependency("neocogi-com", &["std", "macros", "runtime"]),
            com_dependencies::dependency("neocogi-com-host", &[]),
            root.join("crates/ui"), root.join("crates/ui-contracts"),
        ),
    )
    .unwrap();
    let mut implementation = format!(
        "{}\n#[cfg(test)]\n#[allow(dead_code)]\nmod generated {{\n{}\n}}\n",
        include_str!("fixtures/document_readme.rs"),
        document.rust_bindings().unwrap()
    );
    // Repeating the public blocks exercises reload and stale-generation filtering
    // without a second copy of their implementation in the fixture.
    for label in ["document-reload", "document-dispatch"] {
        let marker = format!("/* checked-example: {label} */");
        assert!(implementation.contains(&marker), "fixture must exercise {label}");
        implementation = implementation.replace(&marker, example(readme, label));
    }
    let mut onboarding = include_str!("fixtures/application_readme.rs").to_owned();
    let root_readme = include_str!("../../../../tests/package-inputs/application.md");
    for label in ["application-install", "application-events", "application-update"] {
        let marker = format!("/* checked-example: {label} */");
        assert!(onboarding.contains(&marker), "fixture must exercise {label}");
        onboarding = onboarding.replace(&marker, example(root_readme, label));
    }
    implementation.push_str("\n#[cfg(test)]\nmod application_readme {\n");
    implementation.push_str(&onboarding);
    implementation.push_str("\n}\n");
    fs::write(project.path().join("src/lib.rs"), implementation).unwrap();
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
        String::from_utf8_lossy(&output.stdout).contains("test application_readme::documented_application_onboarding ... ok"),
        "the root onboarding blocks must execute, not just compile"
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("test tests::documented_reload ... ok"),
        "the public blocks must execute, not just compile"
    );
}
