//! An active backend frame must retain the backend's exclusive mutable borrow.

use neocogi_ui::runtime::render::{FrameInfo, RendererBackend};

fn acquire_twice<B: RendererBackend>(backend: &mut B, info: FrameInfo) {
    let first = backend.frame(info).unwrap();
    let second = backend.frame(info).unwrap();
    drop((first, second));
}

fn main() {}
