//! Generates typed bindings and one native description for `neocogi.skin`.

/// Resolves the checked-in module directory and delegates deterministic generation.
fn main() {
    // The module owns appearance authoring and theme selection without importing
    // decoder, renderer, or UI-tree implementation details.
    neocogi_idl_build::Module::directory("idl", "neocogi/skin")
        .build()
        .expect("failed to build neocogi.skin");
}
