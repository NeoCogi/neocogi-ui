//! Compiles private and local test protocols from the shared source tree.
#[path = "../../../scripts/idl_build.rs"]
mod shared_idl;
fn main() {
    let request = shared_idl::sources("../../..", &["neocogi.ui.runtime.internal"]).module("neocogi.idl_example", "nc_ui::test_example");
    shared_idl::build(request);
}
