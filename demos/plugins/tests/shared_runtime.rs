//! Cargo builds the probe independently and the host calls it only through the C ABI.
#![cfg(not(target_arch = "wasm32"))]
#[allow(dead_code, reason = "the host uses ABI clients; only the separate DLL implements this trait")]
#[path = "runtime-probe/interface.rs"]
mod interface;
use interface::IRegistryProbeAbi;
use neocogi_com::{Runtime, guid_from_qualified_name};

/// Registration observed/changed inside the DLL must be the host's same process catalog.
#[test]
fn cargo_host_and_plugin_share_one_runtime_catalog() {
    // Cargo adds native dependency directories to its loader environment. The
    // child must also start directly, before main, without that hidden support.
    #[cfg(unix)]
    if std::env::var_os("NEOCOGI_DIRECT_RUNTIME_TEST").is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "cargo_host_and_plugin_share_one_runtime_catalog", "--nocapture"])
            .env("NEOCOGI_DIRECT_RUNTIME_TEST", "1")
            .env_remove("LD_LIBRARY_PATH")
            .env_remove("DYLD_LIBRARY_PATH")
            .env_remove("DYLD_FALLBACK_LIBRARY_PATH")
            .output()
            .unwrap();
        assert!(output.status.success(), "direct host startup/ABI calls failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        return;
    }
    neocogi_demo_plugins::register();
    let runtime = Runtime::global();
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
    neocogi_demo_plugins::register();
    assert_eq!(runtime.plugins().unwrap().len(), count);
    assert!(count >= 4, "production composition must register its actual providers");
    // SAFETY: Cargo supplies this trusted, independently compiled test DLL.
    let id = unsafe { runtime.load_plugin_library(std::path::Path::new(env!("CARGO_CDYLIB_FILE_RUNTIME_PROBE_nc_cargo_runtime_probe"))) };
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
