//! Builds the optional UI asset-authoring contract and its external core graph.

/// Generates Rust bindings and the fingerprint-qualified native description.
fn main() {
    // Cargo supplies the exact `neocogi.ui.core` IDL graph through the dependency
    // package, so generation never searches another crate's source directory.
    neocogi_idl_build::Module::directory("idl", "neocogi/ui/assets")
        .build()
        .expect("failed to build neocogi.ui.assets");
}
