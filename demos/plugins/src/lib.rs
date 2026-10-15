//! Cargo-owned native demo composition. No provider Rust library is linked here.
//!
//! Artifact paths refer to this build tree. This is development startup, not an
//! installation manifest. The single runtime is linked by the adjacent build
//! script using only Cargo's already-resolved native search directory.
#![deny(missing_docs)]

/// Loads trusted Cargo artifacts in their explicit provider order.
/// The shared runtime validates metadata, dependencies and process ownership.
pub fn register() {
    let paths = [
        env!("CARGO_CDYLIB_FILE_UI_LIBRARY_nc_ui"),
        env!("CARGO_CDYLIB_FILE_NODES_LIBRARY_nc_ui_nodes"),
        env!("CARGO_CDYLIB_FILE_RENDER_LIBRARY_nc_render"),
        env!("CARGO_CDYLIB_FILE_GFX_LIBRARY_nc_gfx"),
        #[cfg(feature = "extension")]
        env!("CARGO_CDYLIB_FILE_EXTENSION_LIBRARY_nc_ui_extension"),
        #[cfg(feature = "skin")]
        env!("CARGO_CDYLIB_FILE_SKIN_LIBRARY_nc_skin"),
    ];
    let runtime = neocogi_com::Runtime::global();
    for path in paths {
        // SAFETY: these are trusted provider artifacts built from this Cargo graph.
        // Runtime registration validates descriptors and retains code for process lifetime.
        unsafe {
            runtime.load_plugin_library(std::path::Path::new(path));
        }
    }
}
