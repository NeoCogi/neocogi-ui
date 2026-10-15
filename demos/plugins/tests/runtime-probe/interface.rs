//! Fixed test-only C ABI shared by independently compiled host and probe DLL.
use neocogi_com::{NcStatus, com_interface};

#[com_interface(auto_guid = "nc.tests.CargoRegistryProbe:v1", assigned_signature = b"00000000000000000000000000000092")]
/// Observes and changes runtime registration from inside a separately loaded DLL.
pub trait IRegistryProbe {
    /// Reads the process catalog through the DLL's facade.
    fn catalog_len(&self) -> Result<u32, NcStatus>;
    /// Registers a process-lived descriptor from inside the DLL.
    fn publish_marker(&self) -> Result<(), NcStatus>;
}
