//! Builds the canonical UI contract module from its Rust-like IDL source.

/// Generates Rust bindings and the fingerprint-qualified native description.
fn main() {
    // The normalized source path defines the stable `neocogi.ui` module name.
    neocogi_idl_build::Module::directory("idl", "neocogi/ui")
        .build()
        .expect("failed to build neocogi.ui");
}
