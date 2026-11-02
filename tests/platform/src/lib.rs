//! UI/GFX integration acceptance against the platform's public contracts and explicit diagnostics.
#[cfg(all(test, target_arch = "wasm32"))]
mod browser_input;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod native;

/// This test executable explicitly owns its process runtime.
#[cfg(test)]
fn host_runtime() -> neocogi_com::Runtime {
    // SAFETY: The executable links the immutable process-lived host implementation exactly once.
    unsafe { neocogi_com::Runtime::__from_host_api(neocogi_com_host::initialize_process()) }.expect("compatible host")
}
