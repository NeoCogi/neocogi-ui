//! Browser stack configuration; resource compilation belongs to demo-assets host build dependencies.
/// The larger stack supports the full retained demo and its nested layout workloads in wasm.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        println!("cargo:rustc-link-arg=-zstack-size=2097152");
    }
}
