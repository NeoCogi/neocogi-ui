//! Independently compiled DLL proving Cargo compositions share one runtime authority.
#[path = "../interface.rs"]
mod interface;
use neocogi_native_runtime_link as _;
use interface::{IRegistryProbe, IRegistryProbeAbi};
use neocogi_com::{ComPtr, NcStatus, Runtime, com_object};

#[com_object(interface::IRegistryProbe)]
/// State-free probe; all observed registration belongs to the shared runtime.
struct Probe;

impl IRegistryProbe for Probe {
    fn catalog_len(&self) -> Result<u32, NcStatus> {
        Ok(Runtime::global().plugins()?.len() as u32)
    }
    fn publish_marker(&self) -> Result<(), NcStatus> {
        // SAFETY: marker metadata is static in this DLL, which the runtime pinned
        // when it registered the probe before the host could acquire this object.
        unsafe { Runtime::global().register_plugin_descriptor(marker::plugin_descriptor()) }
    }
}

#[neocogi_com::plugin(name = "nc.tests.cargo.runtime-probe")]
/// Acquires an ordinary service, without an IDL factory construction role.
fn acquire() -> Result<ComPtr<IRegistryProbeAbi>, NcStatus> {
    Probe::from_value(Probe).query()
}

/// Process-lived marker metadata using the probe's exact acquisition contract.
mod marker {
    /// Static descriptor is pinned together with this DLL's root registration.
    static DESCRIPTOR: neocogi_com::NcPluginDescriptor = neocogi_com::NcPluginDescriptor {
        plugin_id: neocogi_com::guid_from_qualified_name("nc.tests.cargo.marker"),
        qualified_name: neocogi_com::NcStaticUtf8::from_static("nc.tests.cargo.marker"),
        ..*super::plugin_descriptor()
    };
    /// Lends marker metadata without acquiring an object.
    pub(super) fn plugin_descriptor() -> &'static neocogi_com::NcPluginDescriptor {
        &DESCRIPTOR
    }
}
