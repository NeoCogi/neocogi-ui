//! Execute compile contracts with an explicit SDK feature selection.
//!
//! Trybuild discovers parent features from Cargo's legacy fingerprint layout. Current
//! nightly Cargo places test binaries under build outputs, so that inference misses
//! feature-gated APIs. This independent fixture declares its UI dependency directly;
//! default-feature and disabled-service tests remain independent of the UI fixture.
use std::{fs, path::Path, process::Command};

/// Runs the exact success/rejection cases from package-owned sources, including in archives.
#[test]
fn public_api_contracts() {
    let package = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture = tempfile::tempdir().unwrap();
    fs::create_dir(fixture.path().join("src")).unwrap();
    fs::create_dir_all(fixture.path().join("tests/ui")).unwrap();
    for entry in fs::read_dir(package.join("tests/ui")).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            fs::copy(entry.path(), fixture.path().join("tests/ui").join(entry.file_name())).unwrap();
        }
    }
    fs::write(fixture.path().join("src/lib.rs"), include_str!("compile_cases.rs")).unwrap();
    fs::write(fixture.path().join("Cargo.toml"), format!(
        "[package]\nname='ui-compile-contracts'\nversion='0.0.0'\nedition='2024'\n[workspace]\n[dependencies]\nneocogi-ui={{path={package:?},default-features=false,features=['ui']}}\ntrybuild='=1.0.116'\n"
    )).unwrap();
    let output = Command::new("cargo")
        .args(["test", "--offline", "--manifest-path"])
        .arg(fixture.path().join("Cargo.toml"))
        .env(
            "CARGO_TARGET_DIR",
            std::env::var_os("CARGO_TARGET_DIR").unwrap_or_else(|| package.join("../../target").into_os_string()),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
