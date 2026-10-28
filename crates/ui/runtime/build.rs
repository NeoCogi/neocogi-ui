//! Compiles the runtime protocol and its independently owned persistence fixture.
#[path = "../../../scripts/idl_build.rs"]
mod shared_idl;
fn main() {
    shared_idl::build(shared_idl::sources("../../..", &["neocogi.ui.runtime.internal"]));
    // The saved-component regression needs only a mountable composition contract,
    // rather than the independent COM SDK's example objects and event transport.
    shared_idl::build(neocogi_idl::cargo::SharedSources::new("schemas").module("nc.tests.composition_root", "nc_ui::test_composition"));
}
