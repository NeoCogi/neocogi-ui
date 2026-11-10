//! Emit implementation-private protocols and real ABI regression fixtures.
/// Generate private protocols and independent test ABI from package-contained sources.
fn main() {
    use neocogi_ui_contracts::build::{Module, build, sources};
    sources(&[Module::UiRuntimeInternal]).and_then(build).expect("generate private UI protocols");
    build(neocogi_idl::cargo::SharedSources::new("schemas/runtime").module("nc.tests.composition_root", "neocogi_ui::runtime::test_composition"))
        .expect("composition fixture");
    for module in neocogi_idl::cargo::SharedSources::new("schemas/document")
        .module("nc.tests.capability_retainer", "neocogi_ui::document::receiver_fixture")
        .build()
        .expect("receiver fixture")
    {
        println!("cargo::rustc-env=IDL_RETAINER_BINDINGS={}", module.include.display());
    }
    let samples = sources(&[])
        .expect("canonical sample imports")
        .include_root("tests/package-inputs/sample/schemas")
        .module("neocogi.demo", "neocogi_ui_samples::abi");
    build(samples).expect("independent sample contract fixture");
}
