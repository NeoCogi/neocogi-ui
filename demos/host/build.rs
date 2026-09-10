//! Reports the native package-build requirement before an otherwise opaque linker failure.

/// Static builds own the runtime; dynamic native packages supply its linker path.
fn main() {
    println!("cargo:rerun-if-env-changed=NC_COM_RUNTIME_LINK_DIR");
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("wasm32")
        && std::env::var_os("CARGO_FEATURE_MONOLITH").is_none()
        && std::env::var_os("NC_COM_RUNTIME_LINK_DIR").is_none_or(|value| value.is_empty())
    {
        // Keep cargo check and custom linker setups usable. The package driver supplies this
        // path after building the runtime, then stages the plugins and loader-relative executable.
        println!(
            "cargo:warning=Native demos require a complete COM package. Use cargo neocogi run --assembly <name> --variant <glow|vulkan|wgpu>. See docs/COM_NATIVE.md for details."
        );
    }
}
