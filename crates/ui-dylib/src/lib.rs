//! Final native loader symbol; all descriptors, factories and state belong to the static SDK.
#![deny(missing_docs)]

/// Exposes the selected UI implementation through the canonical discovery ABI.
///
/// # Safety
/// `out` must be null or aligned and writable for one descriptor pointer. The
/// loader retains this library while any reachable descriptor, vtable or object is used.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nc_get_plugin_descriptor_v1(out: *mut *const neocogi_com::NcPluginDescriptor) -> neocogi_com::NcStatus {
    // SAFETY: the caller supplies the output slot and this final library owns the
    // complete static descriptor graph and every reachable acquisition function.
    unsafe { neocogi_com::return_plugin_descriptor(out, neocogi_ui::plugin_descriptor()) }
}
