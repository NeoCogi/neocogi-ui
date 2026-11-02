//! Cargo builds the probe independently and the host calls it only through the C ABI.
#![cfg(not(target_arch = "wasm32"))]
extern crate self as probe_bindings;
#[allow(dead_code, reason = "the host uses ABI clients; only the separate DLL implements this trait")]
#[path = "runtime-probe/interface.rs"]
mod interface;
use interface::IRegistryProbeAbi;
use neocogi_com::guid_from_qualified_name;

use neocogi_render_core::platform::deployment as profile;

/// Compiler-only artifact copying leaves only this platform filename in the test host.
const PROBE: &str = neocogi_native_layout::stage_library!("CARGO_CDYLIB_FILE_RUNTIME_PROBE_nc_cargo_runtime_probe");

/// Calls discovery in the actual separately linked DLLs, including the null-output contract.
/// No implementation Rust dependency or copied fixture descriptor can satisfy this regression.
#[test]
fn final_libraries_export_their_real_descriptors() {
    let executable = std::env::current_exe().unwrap();
    let root = profile::output_root(&executable, std::env::var_os("NEOCOGI_NATIVE_OUTPUT_DIR").map(Into::into)).unwrap();
    let renderer = if cfg!(feature = "vulkan") {
        "neocogi.render.vulkan"
    } else if cfg!(feature = "wgpu") {
        "neocogi.render.wgpu"
    } else {
        "neocogi.render.glow"
    };
    let providers = [
        (
            neocogi_native_layout::stage_library!("CARGO_CDYLIB_FILE_UI_LIBRARY_nc_ui"),
            "neocogi.ui.runtime",
        ),
        (
            neocogi_native_layout::stage_library!("CARGO_CDYLIB_FILE_NODES_LIBRARY_nc_ui_nodes"),
            "neocogi.ui.nodes.runtime",
        ),
        (
            neocogi_native_layout::stage_library!("CARGO_CDYLIB_FILE_SHADER_LIBRARY_nc_shader_compiler"),
            "neocogi.shader.compiler",
        ),
        (neocogi_native_layout::stage_library!("CARGO_CDYLIB_FILE_RENDER_LIBRARY_nc_render"), renderer),
        (
            neocogi_native_layout::stage_library!("CARGO_CDYLIB_FILE_GFX_LIBRARY_nc_gfx"),
            "neocogi.gfx.runtime",
        ),
        #[cfg(feature = "skin")]
        (
            neocogi_native_layout::stage_library!("CARGO_CDYLIB_FILE_SKIN_LIBRARY_nc_skin"),
            "neocogi.skin.runtime",
        ),
        #[cfg(feature = "extension")]
        (
            neocogi_native_layout::stage_library!("CARGO_CDYLIB_FILE_EXTENSION_LIBRARY_nc_ui_extension"),
            "neocogi.example.ui-extension",
        ),
    ];
    for (filename, name) in providers {
        // SAFETY: trusted Cargo-built final roots; the library remains mapped until every
        // borrowed descriptor field has been inspected and no COM objects are acquired here.
        unsafe {
            let library = libloading::Library::new(root.join("lib").join(filename)).unwrap();
            let discovery = library
                .get::<neocogi_com::NcGetPluginDescriptor>(neocogi_com::NC_GET_PLUGIN_DESCRIPTOR_SYMBOL.as_bytes())
                .unwrap();
            assert_eq!(discovery(std::ptr::null_mut()), neocogi_com::NC_E_POINTER);
            let mut descriptor = std::ptr::null();
            assert_eq!(discovery(&mut descriptor), neocogi_com::NC_OK);
            assert!(!descriptor.is_null());
            assert_eq!((*descriptor).plugin_id, guid_from_qualified_name(name), "{filename}");
        }
    }
}

/// Registration observed/changed inside the DLL must be the host's same process catalog.
#[test]
fn cargo_host_and_plugin_share_one_runtime_catalog() {
    // Cargo adds native dependency directories to its loader environment. The
    // child must also start directly, before main, without that hidden support.
    if std::env::var_os("NEOCOGI_DIRECT_RUNTIME_TEST").is_none() {
        let executable = std::env::current_exe().unwrap();
        let root = profile::output_root(&executable, std::env::var_os("NEOCOGI_NATIVE_OUTPUT_DIR").map(Into::into)).unwrap();
        let temporary = tempfile::tempdir().unwrap();
        let before = temporary.path().join("before");
        let after = temporary.path().join("relocated");
        std::fs::create_dir_all(before.join("lib")).unwrap();
        let filename = executable.file_name().unwrap();
        std::fs::copy(&executable, before.join(filename)).unwrap();
        for entry in std::fs::read_dir(root.join("lib")).unwrap() {
            let source = entry.unwrap().path();
            if source.is_file() {
                std::fs::copy(&source, before.join("lib").join(source.file_name().unwrap())).unwrap();
            }
        }
        std::fs::rename(before, &after).unwrap();
        let output = std::process::Command::new(after.join(filename))
            .args(["--exact", "cargo_host_and_plugin_share_one_runtime_catalog", "--nocapture"])
            .current_dir(temporary.path())
            .env("NEOCOGI_DIRECT_RUNTIME_TEST", "1")
            .env_remove("LD_LIBRARY_PATH")
            .env_remove("DYLD_LIBRARY_PATH")
            .env_remove("DYLD_FALLBACK_LIBRARY_PATH")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "direct host startup/ABI calls failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    neocogi_demo_plugins::register(host_runtime()).unwrap();
    let runtime = host_runtime();
    let renderer = if cfg!(feature = "vulkan") {
        "neocogi.render.vulkan"
    } else if cfg!(feature = "wgpu") {
        "neocogi.render.wgpu"
    } else {
        "neocogi.render.glow"
    };
    let catalog = runtime.plugins().unwrap();
    assert!(catalog.iter().any(|plugin| plugin.plugin_id == guid_from_qualified_name(renderer)));
    assert_eq!(catalog.iter().filter(|plugin| plugin.qualified_name.starts_with("neocogi.render.")).count(), 1);
    let count = catalog.len();
    // Re-loading the exact Cargo artifacts must reuse the first registrations.
    neocogi_demo_plugins::register(host_runtime()).unwrap();
    assert_eq!(runtime.plugins().unwrap().len(), count);
    assert!(count >= 4, "production composition must register its actual providers");
    // SAFETY: Cargo supplies this trusted, independently compiled test DLL.
    let executable = std::env::current_exe().unwrap();
    let id = unsafe { runtime.load_plugin_library(executable.parent().unwrap().join("lib").join(PROBE)) };
    let probe = runtime.acquire_plugin_root::<IRegistryProbeAbi>(id).unwrap();
    assert_eq!(probe.catalog_len().unwrap() as usize, count + 1);
    probe.publish_marker().unwrap();
    let marker = guid_from_qualified_name("nc.tests.cargo.marker");
    assert!(runtime.plugin_info(marker).unwrap().is_some());
    assert_eq!(probe.catalog_len().unwrap() as usize, count + 2);
    drop(probe);
    // Dropping the acquired root does not unmap its code or static registration.
    let retained = runtime.acquire_plugin_root::<IRegistryProbeAbi>(marker).unwrap();
    assert_eq!(retained.catalog_len().unwrap() as usize, count + 2);
}

/// Initializes the selected implementation owned by this test executable.
#[cfg(test)]
fn host_runtime() -> neocogi_com::Runtime {
    // SAFETY: this explicit host owns the complete immutable process-lived graph.
    unsafe { neocogi_com::Runtime::__from_host_api(neocogi_com_host::initialize_process()) }.expect("compatible test host")
}
