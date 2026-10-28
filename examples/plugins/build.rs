//! Generates this consumer's probe bindings from shared source.
fn main() {
    for module in neocogi_idl::cargo::SharedSources::new("../schemas")
        .module("nc.tests.registry_probe", "probe_bindings::interface")
        .build()
        .expect("probe bindings")
    {
        println!("cargo::rustc-env=IDL_REGISTRY_PROBE={}", module.include.display());
    }
}
