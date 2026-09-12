//! Builds the optional UI asset-authoring contract and its imported UI graph.

/// Generates Rust bindings and the fingerprint-qualified native description.
fn main() {
    // Cargo supplies the exact `neocogi.ui` IDL graph through the dependency
    // package, so generation never searches another crate's source directory.
    neocogi_idl_build::Module::new("idl", "neocogi/ui_assets.ncidl")
        .build()
        .expect("failed to build neocogi.ui_assets");
}
