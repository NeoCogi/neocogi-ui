//! Generate all canonical public namespaces into this contract owner's OUT_DIR.
#[path = "build_support/mod.rs"]
mod build_support;
/// Generate each canonical namespace once; imports resolve to this same owner.
fn main() {
    use build_support::{Module, build, sources};
    sources(&[Module::Gfx, Module::Skin, Module::UiCore, Module::UiNodes, Module::UiDocument, Module::Library])
        .and_then(build)
        .expect("generate canonical UI contracts");
}
