//! An independent author generates a new interface against the archived shared contracts.
fn main() {
    let request = neocogi_ui_contracts::build::sources(&[])
        .expect("prepare published UI sources")
        .include_root("schemas")
        .module("release.probe", "neocogi_ui_package_consumer::contracts");
    neocogi_ui_contracts::build::build(request).expect("generate independent consumer bindings");
}
