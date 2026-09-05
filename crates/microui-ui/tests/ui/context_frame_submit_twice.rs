//! A logical frame must not remain usable after its consuming submission call.

use microui_ui::Context;
use microui_ui::render::{FrameInfo, RendererBackend};

fn submit_twice<B: RendererBackend>(context: &mut Context<B>, info: FrameInfo) {
    let frame = context.frame(info);
    frame.render_ui().unwrap();
    frame.render_ui().unwrap();
}

fn main() {}
