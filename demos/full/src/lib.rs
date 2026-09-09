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

/// Runs the complete demo through the same facade on native and browser deployments.
pub fn run() -> Result<(), neocogi_demo_host::HostError> {
    app::run()
}
