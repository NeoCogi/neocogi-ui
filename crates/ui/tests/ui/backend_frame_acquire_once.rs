//! One backend frame may be acquired, used, and dropped before another borrow begins.

use neocogi_ui::runtime::render::{FrameInfo, RendererBackend, RendererFrame};

#[allow(dead_code)]
fn acquire_once<B: RendererBackend>(backend: &mut B, info: FrameInfo) {
    let frame = backend.frame(info).expect("the fixture characterizes ownership after successful acquisition");
    drop(frame);
    // Abandonment releases the borrow; a fresh frame can be explicitly completed.
    backend.frame(info).unwrap().finish().unwrap();
}

fn main() {}
