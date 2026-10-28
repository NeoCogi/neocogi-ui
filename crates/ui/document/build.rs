//! Generates the local receiver regression directly from shared IDL.
fn main() {
    for module in neocogi_idl::cargo::SharedSources::new("schemas")
        .module("nc.tests.capability_retainer", "neocogi_ui_document::receiver_fixture")
        .build()
        .expect("receiver fixture bindings")
    {
        println!("cargo::rustc-env=IDL_RETAINER_BINDINGS={}", module.include.display());
    }
}
