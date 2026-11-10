//! Exercise the real service root under each independently selected production feature set.
use neocogi_ui::*;
use neocogi_ui_contracts::abi::library::UiLibraryFactoryAbi;
use neocogi_com::{ComInterface, IUnknown};

/// The real bootstrap and generated getters agree with every selected feature set.
#[test]
fn one_descriptor_exposes_exactly_the_selected_services() {
    let runtime = host_runtime();
    let descriptor = plugin_descriptor();
    assert_eq!(descriptor.plugin_id, services::LIBRARY_PLUGIN_ID);
    assert_eq!(descriptor.dependency_count, 0);
    assert_eq!(descriptor.root_factory, UiLibraryFactoryAbi::INTERFACE_ID);
    // SAFETY: the linked descriptor and its implementation remain mapped for process lifetime.
    unsafe { runtime.register_plugin_descriptor(descriptor) }.unwrap();
    let root = services::library(runtime).unwrap();
    for (name, result, enabled) in [
        ("ui", root.ui().map(drop), cfg!(feature = "ui")),
        ("nodes", root.nodes().map(drop), cfg!(feature = "ui")),
        ("gfx", root.gfx().map(drop), cfg!(feature = "gfx")),
        ("skin", root.skin().map(drop), cfg!(feature = "skin")),
    ] {
        assert_eq!(result, if enabled { Ok(()) } else { Err(neocogi_com::NC_E_NOT_FOUND) }, "{name}");
    }
    if cfg!(feature = "gfx") {
        assert_eq!(
            root.gfx().unwrap().asset_loader().map(drop),
            if cfg!(feature = "builder") {
                Ok(())
            } else {
                Err(neocogi_com::NC_E_NOT_FOUND)
            },
            "the optional atlas builder follows its own feature inside the enabled GFX service"
        );
    }
    root.query::<IUnknown>().unwrap();
}

/// A service is an independent owner; releasing the bootstrap root cannot revoke it.
#[cfg(feature = "gfx")]
#[test]
fn graphics_factory_outlives_its_library_root() {
    let runtime = host_runtime();
    // SAFETY: this static descriptor and its code remain linked for process lifetime.
    unsafe {
        runtime.register_plugin_descriptor(plugin_descriptor()).unwrap();
    }
    let root = services::library(runtime).unwrap();
    let factory = root.gfx().unwrap();
    let other = root.gfx().unwrap();
    assert!(!factory.as_com_ref().same_object(other.as_com_ref()));
    drop(root);
    factory.asset_publisher().unwrap();
}

/// One process-owned runtime supplies both production bootstrap and returned factories.
fn host_runtime() -> neocogi_com::Runtime {
    // SAFETY: this final test executable links one process-lived COM host implementation.
    unsafe { neocogi_com::Runtime::__from_host_api(neocogi_com_host::initialize_process()) }.unwrap()
}
