//! Generate this consumer's bindings from packaged, subsystem-owned interface sources.
use neocogi_ui_build::{Module, build, sources};
fn main() {
    sources(&[Module::UiCore, Module::UiNodes, Module::UiDocument])
        .and_then(build)
        .expect("generate UI bindings");
}
