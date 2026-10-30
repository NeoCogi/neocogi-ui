//! Actual generated painter calls capture scene-widget output without a native window.
use crate::graphics::*;
use neocogi_com::{com_object};
use std::{cell::RefCell, rc::Rc};

/// Shared observations outlive the exclusive COM recorder invocation in the test.
#[derive(Default)]
pub(crate) struct Output {
    /// Item allocations remain full-sized even when the scene fits a smaller visible area.
    pub(crate) items: Vec<Rect>,
    /// Copied grid geometry contains no borrowed ABI storage.
    pub(crate) vertices: Vec<ColoredVertex>,
}
#[com_object(interfaces(IPainter))]
/// Minimal concrete recorder for the three operations issued by the scene widgets.
struct Recorder {
    /// The COM object and its test caller share observations; no borrow crosses a COM call.
    output: Rc<RefCell<Output>>,
}
impl IPainter for Recorder {
    /// This semantic-only fixture must never silently accept generic GPU output.
    fn begin_render_view(
        &mut self,
        _: PaintScope,
        _: ComRef<'_, neocogi_render_core::abi::IRenderResourcesAbi>,
    ) -> Result<neocogi_com::ComBox<neocogi_render_core::abi::IRenderViewAbi>, NcStatus> {
        panic!("unexpected GPU view")
    }

    /// Copies the full allocation without retaining a native or scene borrow.
    fn item(&mut self, _: PaintScope, rect: Rect, _: ComRef<'_, IRenderItemAbi>) -> Result<(), NcStatus> {
        self.output.borrow_mut().items.push(rect);
        Ok(())
    }
    /// Copies grid output while the generated ABI input slice is live.
    fn colored_triangles(&mut self, _: PaintScope, vertices: NcCopyFromBufferOffset<'_, ColoredVertex>) -> Result<(), NcStatus> {
        self.output.borrow_mut().vertices.extend_from_slice(vertices.as_slice());
        Ok(())
    }
    /// Accepts the grid background; geometry assertions inspect its overlaid lines.
    fn fill(&mut self, _: PaintScope, _: Rect, _: u32) -> Result<(), NcStatus> {
        Ok(())
    }
    /// Rejects an unexpected operation so the fixture cannot silently omit widget output.
    fn stroke_line(&mut self, _: PaintScope, _: PaintPoint, _: PaintPoint, _: f32, _: u32) -> Result<(), NcStatus> {
        panic!("unexpected stroke")
    }
    /// Rejects an unexpected operation so the fixture cannot silently omit widget output.
    fn fill_polygon(&mut self, _: PaintScope, _: NcCopyFromBufferOffset<'_, PaintPoint>, _: u32) -> Result<(), NcStatus> {
        panic!("unexpected polygon")
    }
    /// Rejects an unexpected operation so the fixture cannot silently omit widget output.
    fn triangles(&mut self, _: PaintScope, _: NcCopyFromBufferOffset<'_, RenderVertex>) -> Result<(), NcStatus> {
        panic!("unexpected atlas triangles")
    }
    /// Rejects an unexpected operation so the fixture cannot silently omit widget output.
    fn texture(&mut self, _: PaintScope, _: Rect, _: TextureRegion, _: u32, _: ComRef<'_, ITextureAbi>) -> Result<(), NcStatus> {
        panic!("unexpected texture")
    }
    /// Rejects an unexpected operation so the fixture cannot silently omit widget output.
    fn text(&mut self, _: PaintScope, _: Rect, _: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        panic!("unexpected text")
    }
    /// Rejects an unexpected operation so the fixture cannot silently omit widget output.
    fn frame(&mut self, _: PaintScope, _: Rect, _: WidgetFrame, _: WidgetVisualState, _: bool) -> Result<Rect, NcStatus> {
        panic!("unexpected frame")
    }
    /// Rejects an unexpected operation so the fixture cannot silently omit widget output.
    fn icon(&mut self, _: PaintScope, _: PaintIcon, _: Rect, _: u32) -> Result<(), NcStatus> {
        panic!("unexpected icon")
    }
    /// Rejects an unexpected operation so the fixture cannot silently omit widget output.
    fn text_run(&mut self, _: PaintScope, _: PaintFont, _: Point, _: u32, _: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        panic!("unexpected text run")
    }
}

/// Owns the generated painter interface independently of its captured observations.
pub(crate) fn recorder() -> (ComPtr<IPainterAbi>, Rc<RefCell<Output>>) {
    let output = Rc::new(RefCell::new(Output::default()));
    let recorder = Recorder::from_value(Recorder { output: output.clone() }).query().unwrap();
    (recorder, output)
}

/// Valid copied widget state with an explicit allocation and independently visible client.
pub(crate) fn update_state(rect: Rect, clip: Rect) -> WidgetUpdate {
    WidgetUpdate {
        in_hover_root: true,
        rect,
        clip,
        screen: rect,
        hovered: true,
        focused: true,
        clicked: false,
        active: false,
        action: WidgetAction::None,
        mouse_buttons: 0,
        modifiers: 0,
    }
}
