//! Generate the reusable example contract against the same packaged UI interfaces.
fn main() {
    let request = neocogi_ui_build::sources(&[])
        .expect("prepare UI sources")
        .include_root("schemas")
        .module("neocogi.demo", "neocogi_ui_samples::abi");
    neocogi_ui_build::build(request).expect("generate sample bindings");
}
