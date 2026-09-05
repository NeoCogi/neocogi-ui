//! A live logical frame must prevent overlapping mutation through its borrowed Context.

use microui_ui::Context;
use microui_ui::render::{FrameInfo, RendererBackend};

fn mutate_during_frame<B: RendererBackend>(context: &mut Context<B>, info: FrameInfo) {
    let frame = context.frame(info);
    context.mousemove(10, 20);
    drop(frame);
}

fn main() {}
