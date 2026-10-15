//! Native linking consumes Cargo's resolved runtime artifact; no compilation or graph discovery.

/// Adds the one shared runtime's directory for the final native executable.
/// Cargo also uses native search directories when running executables/tests.
fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("wasm32") {
        let directory = std::env::var_os("CARGO_CDYLIB_DIR_COM_LIBRARY").expect("dynamic hosts require Cargo's com-library artifact dependency");
        println!("cargo::rustc-link-search=native={}", std::path::Path::new(&directory).display());
    }
}
