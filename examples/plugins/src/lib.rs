//! Cargo-owned native demo composition. No provider Rust library is linked here.
//!
//! Cargo paths are consumed only while staging build outputs. Runtime startup
//! loads selected filenames from the lib directory beside the executable.
#![deny(missing_docs)]

/// Loads trusted Cargo artifacts in their explicit provider order.
/// The supplied host validates metadata, dependencies and process ownership.
pub fn register(runtime: neocogi_com::Runtime) -> std::io::Result<()> {
    let executable = std::env::current_exe()?;
    let directory = executable
        .parent()
        .ok_or_else(|| std::io::Error::other("executable has no parent"))?
        .join("lib");
    let filenames = [
        neocogi_ui_native_artifacts::stage_library!("CARGO_CDYLIB_FILE_UI_LIBRARY_nc_ui"),
        neocogi_ui_native_artifacts::stage_library!("CARGO_CDYLIB_FILE_NODES_LIBRARY_nc_ui_nodes"),
        neocogi_ui_native_artifacts::stage_library!("CARGO_CDYLIB_FILE_PLATFORM_LIBRARY_nc_platform"),
        neocogi_ui_native_artifacts::stage_library!("CARGO_CDYLIB_FILE_GFX_LIBRARY_nc_gfx"),
        #[cfg(feature = "extension")]
        neocogi_ui_native_artifacts::stage_library!("CARGO_CDYLIB_FILE_EXTENSION_LIBRARY_nc_ui_extension"),
        #[cfg(feature = "skin")]
        neocogi_ui_native_artifacts::stage_library!("CARGO_CDYLIB_FILE_SKIN_LIBRARY_nc_skin"),
    ];
    for filename in filenames {
        // SAFETY: the application ships trusted Cargo-built providers in its lib directory.
        // Runtime registration validates descriptors and retains code for process lifetime.
        unsafe {
            runtime.load_plugin_library(directory.join(filename));
        }
    }
    Ok(())
}
