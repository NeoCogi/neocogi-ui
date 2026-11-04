//! Generate this consumer's bindings from packaged, subsystem-owned interface sources.
use neocogi_ui_build::{Module, build, sources};
fn main() {
    sources(&[Module::UiRuntimeInternal]).and_then(build).expect("generate UI bindings");
    neocogi_ui_build::build(neocogi_idl::cargo::SharedSources::new("schemas").module("nc.tests.composition_root", "nc_ui::test_composition"))
        .expect("generate composition fixture");
}
