//! Builds the canonical core UI contract module from its Rust-like IDL parts.

/// Generates Rust bindings and the fingerprint-qualified native description.
fn main() {
    // The normalized source directory defines the stable
    // `neocogi.ui.core` module name independently of its part filenames.
    neocogi_idl_build::Module::directory("idl", "neocogi/ui/core")
        .build()
        .expect("failed to build neocogi.ui.core");
}
