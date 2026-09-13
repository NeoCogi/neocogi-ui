//! Generates typed bindings and one native description for `neocogi.gfx`.

/// Resolves the checked-in module directory and delegates deterministic generation.
fn main() {
    // The directory is one ABI module even though declarations are grouped by
    // portable bundles, decoding, construction, diagnostics, and activation.
    neocogi_idl_build::Module::directory("idl", "neocogi/gfx")
        .build()
        .expect("failed to build neocogi.gfx");
}
