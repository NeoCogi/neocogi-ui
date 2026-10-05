//! Public walkthroughs run against real providers instead of copied test examples.

use std::{fs, path::Path, process::Command};

#[path = "../../../../api/docs/button_events.rs"]
mod button_events;

/// The exact rustdoc source must mount, deliver, and expire its weak event origin.
#[test]
fn documented_button_walkthrough_activates_and_drops_queued_origin() {
    let ui = nc_ui_nodes::bind_builtin_nodes(super::runtime()).unwrap();
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

/// The exact reload/dispatch blocks must compile with newly generated fields and
/// handle live snapshots while rejecting a queued fact from a replaced form.
#[test]
fn documented_form_reload_and_event_dispatch_execute() {
    let readme = include_str!("../../../../document/README.md");
    let document = neocogi_ui_document::Document::new("README events");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::create_dir(project.path().join("src")).unwrap();
    fs::write(
        project.path().join("Cargo.toml"),
        format!(
            "[package]\nname='documented-form-events'\nversion='0.0.0'\nedition='2024'\n[workspace]\n[dependencies]\n\
         neocogi-com={{path={:?},features=['static-runtime']}}\n\
         neocogi-ui={{path={:?}}}\n\
         neocogi-ui-document={{path={:?}}}\n\
         neocogi-ui-runtime={{path={:?}}}\n\
         neocogi-ui-nodes={{path={:?}}}\n\
         neocogi-ui-support={{path={:?},features=['test-support']}}\n",
            root.join("crates/com/api"),
            root.join("crates/ui/api"),
            root.join("crates/ui/document"),
            root.join("crates/ui/runtime"),
            root.join("crates/ui/nodes"),
            root.join("crates/ui/support"),
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
    fs::write(project.path().join("src/lib.rs"), implementation).unwrap();
    let output = Command::new("cargo")
        .args(["test", "--offline", "--manifest-path"])
        .arg(project.path().join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", root.join("target/documentation-proof"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("test tests::documented_reload ... ok"),
        "the public blocks must execute, not just compile"
    );
}
