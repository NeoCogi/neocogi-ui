//! Retained origin and UV assertions from the retired native texture-clipping example.
use super::{RenderEvent, recording_backend, test_atlas};
use crate::*;
use crate::render::FrameInfo;
use std::cell::Cell;

/// Draws an external texture between two atlas operations under a local clip.
struct Probe {
    /// Texture owned by the fixture renderer.
    texture: TextureId,
    /// Atlas-owned tile on both sides of the texture boundary.
    white: IconId,
    /// Records retained content origin so assertions cover local-to-screen translation.
    origin: Rc<Cell<Option<Recti>>>,
}
impl Widget for Probe {
    fn widget_opt(&self) -> &WidgetOption {
        &WidgetOption::NO_INTERACT
    }
    /// The clip trims all four texture edges without changing its UV projection.
    fn paint(&mut self, ctx: &mut WidgetPaintCtx<'_>) {
        self.origin.set(Some(ctx.screen_content_rect()));
        let white = color(255, 255, 255, 255);
        let mut painter = ctx.painter();
        painter.icon(self.white, rect(0, 0, 4, 4), white);
        painter.with_clip(rect(10, 12, 8, 6), |painter| painter.image(self.texture, rect(6, 9, 16, 12), white));
        painter.icon(self.white, rect(30, 0, 4, 4), white);
    }
}
impl LeafWidget for Probe {
    /// Keeps preferred size independent of the exact screen-space window origin.
    fn measure(&self, _skin: &Skin, _atlas: &AtlasHandle, _limits: Constraints) -> Dimensioni {
        Dimensioni::new(64, 64)
    }
}

/// Preserves the old standalone smoke assertions inside the normal unit-test gate.
#[test]
fn retained_texture_clipping_preserves_origin_uvs_and_painter_order() {
    let atlas = test_atlas();
    let white = atlas.white_icon();
    let (backend, log) = recording_backend(atlas);
    let mut context = Context::new(backend);
    let texture = context.try_load_image_rgba(16, 12, &[255; 16 * 12 * 4]).unwrap();
    let origin = Rc::new(Cell::new(None));
    let node = Node::widget(Probe { texture, white, origin: origin.clone() });
    let window = context.ui().create_window(Window::new("clipping", rect(0, 0, 64, 64), node));
    context
        .ui()
        .set_window_options(&window, WindowOption::NO_TITLE | WindowOption::NO_CLOSE | WindowOption::NO_RESIZE)
        .unwrap();
    // Transparent manager background leaves only the probe's three ordered operations.
    let mut skin = context.skin().clone();
    skin.apply_flat_palette(FlatPalette {
        window_background: color(0, 0, 0, 0),
        ..FlatPalette::default()
    });
    context.set_skin(skin);
    log.clear();
    let size = Dimensioni::new(64, 64);
    context.update_ui(size);
    context.frame(FrameInfo::try_new(size, color(0, 0, 0, 255)).unwrap()).render_ui().unwrap();
    let events = log.snapshot();
    let draws: Vec<_> = events
        .iter()
        .filter(|event| matches!(event, RenderEvent::AtlasQuad(_) | RenderEvent::ExternalTexture { .. }))
        .collect();
    assert_eq!(draws.len(), 3);
    assert!(matches!(draws[0], RenderEvent::AtlasQuad(_)));
    assert!(matches!(draws[2], RenderEvent::AtlasQuad(_)));
    let RenderEvent::ExternalTexture { id, vertices } = draws[1] else {
        panic!("texture must stay between atlas operations");
    };
    assert_eq!(*id, texture);
    let content = origin.get().expect("retained probe was painted");
    let x = content.x as f32 + 10.0;
    let y = content.y as f32 + 12.0;
    assert_eq!(vertices.map(|vertex| vertex.position), [[x, y], [x + 8.0, y], [x + 8.0, y + 6.0], [x, y + 6.0]]);
    assert_eq!(vertices[0].tex_coord, [0.25, 0.25]);
    assert_eq!(vertices[2].tex_coord, [0.75, 0.75]);
}
