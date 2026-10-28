//! Full demo application using the static facade and concrete local COM widgets.
mod app;
mod camera;
mod falloff;
mod graphics;
mod grid;
mod mesh;
mod obj_loader;
mod painter_demo;
mod polymesh;
mod style_editor;
mod view3d;

#[cfg(test)]
mod viewport_test_support;

/// Runs the complete demo through the same facade on native and browser deployments.
pub fn run(runtime: neocogi_com::Runtime) -> Result<(), neocogi_demo_host::HostError> {
    app::run(runtime)
}

/// Initializes the selected implementation owned by this test executable.
#[cfg(test)]
pub(crate) fn host_runtime() -> neocogi_com::Runtime {
    // SAFETY: this explicit host owns the complete immutable process-lived graph.
    unsafe { neocogi_com::Runtime::__from_host_api(neocogi_com_host::initialize_process()) }.expect("compatible test host")
}
