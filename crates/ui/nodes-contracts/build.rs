//! Generates the independent built-in node contract and native description.

/// Compiles all NCIDL parts under the stable `neocogi.ui.nodes` module identity.
fn main() {
    neocogi_idl::cargo::Module::directory("idl", "neocogi/ui/nodes")
        .build()
        .expect("failed to build neocogi.ui.nodes");
}
