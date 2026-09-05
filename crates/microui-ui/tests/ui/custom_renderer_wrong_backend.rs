//! A callback typed for backend B must not enter a Context backed by A.

use microui_ui::Context;
use microui_ui::render::{CustomRenderArgs, RendererBackend};

fn register_for_wrong_backend<A, B, F>(context: &mut Context<A>, callback: F)
where
    A: RendererBackend,
    B: RendererBackend,
    F: for<'frame> FnMut(&mut B::Frame<'frame>, CustomRenderArgs) + 'static,
{
    context.register_custom_renderer(callback).unwrap();
}

fn main() {}
