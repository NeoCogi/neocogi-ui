//! Builds the demo extension contract module and its exact UI dependency.

/// Generates Rust bindings and the fingerprint-qualified native description.
fn main() {
    // The build helper obtains the external `neocogi.ui.core` graph through Cargo
    // metadata rather than reaching into the dependency crate's source tree.
    neocogi_idl_build::Module::new("idl", "neocogi/demo.ncidl")
        .build()
        .expect("failed to build neocogi.demo");
}
