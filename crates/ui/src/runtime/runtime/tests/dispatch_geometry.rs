//! Private content publishes into the owning inline widget state.
use neocogi_com::{ComRef, NcStatus};
use neocogi_ui::{DispatchingLeafContent, LeafContent, LeafDispatchContext, Painter};
use neocogi_ui::abi::core::*;
use std::{
    cell::Cell,
    rc::Rc,
    sync::mpsc::{Receiver, TryRecvError},
};

/// One inline model whose width changes only through its private receiver.
struct WidthLeaf {
    /// Authoritative preferred width committed by the private callback.
    width: i32,
    /// Handler outcomes cover successful work, idle-with-mutation and failure.
    messages: Receiver<(bool, Result<bool, NcStatus>)>,
    /// Test observations do not own widget or receiver lifetimes.
    clicks: Rc<Cell<usize>>,
    /// Counts real layout calls separately from ordinary update calls.
    measures: Rc<Cell<usize>>,
}
impl LeafContent for WidthLeaf {
    /// Reads the concrete preferred width through the production layout interface.
    fn measure(&self, _: LayoutLimits, _: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        self.measures.set(self.measures.get() + 1);
        Ok(Size { width: self.width, height: 20 })
    }
    /// Ordinary updates make no geometry edits and record only routed activation.
    fn update(&mut self, state: WidgetUpdate, _: WidgetEvent, _: &str) -> Result<GeometryChange, NcStatus> {
        if state.clicked {
            self.clicks.set(self.clicks.get() + 1);
        }
        Ok(GeometryChange::NoChange)
    }
    /// Records one ordinary primitive without affecting geometry.
    fn paint(&mut self, state: WidgetPaint, painter: &mut Painter<'_>) -> Result<(), NcStatus> {
        painter.fill(state.rect, 0xff112233)
    }
}
impl DispatchingLeafContent for WidthLeaf {
    /// Commits width before outcome; publication uses the owning sticky state.
    fn dispatch_next(&mut self, context: &mut LeafDispatchContext<'_>) -> Result<bool, NcStatus> {
        match self.messages.try_recv() {
            Ok((publish, result)) => {
                self.width = 120;
                if publish {
                    context.geometry_changed();
                }
                result
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => Ok(false),
        }
    }
}

/// Geometry committed by a private handler precedes hit testing, even on false/error.
#[test]
fn private_content_geometry_is_collected_before_queued_input() {
    for (publish, result) in [
        (true, Ok(true)),
        (true, Ok(false)),
        (true, Err(neocogi_com::NC_E_INVALID_ARG)),
        // A partial failure also exercises the wrapper's conservative fallback.
        (false, Err(neocogi_com::NC_E_INVALID_ARG)),
    ] {
        let atlas = neocogi_ui::support::test_support::test_atlas();
        let assets = neocogi_ui::support::test_support::assets::export_bundle(&neocogi_ui::support::SkinBundle::from_atlas(atlas)).unwrap();
        let gfx = neocogi_ui::support::test_support::gfx_factory(crate::runtime::test_support::host_runtime());
        let runtime = crate::runtime::create_runtime_from_assets(assets.as_com_ref(), gfx.as_com_ref()).unwrap();
        let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime).unwrap();
        let (send, messages) = std::sync::mpsc::channel();
        let clicks = Rc::new(Cell::new(0));
        let measures = Rc::new(Cell::new(0));
        let leaf = ui
            .widgets()
            .dispatching_leaf(
                WidthLeaf {
                    width: 40,
                    messages,
                    clicks: clicks.clone(),
                    measures: measures.clone(),
                },
                WidgetOptions::NONE,
            )
            .unwrap();
        let window = ui.window("leaf geometry", Rect { x: 0, y: 0, width: 40, height: 20 }, leaf).unwrap();
        window
            .set_options(WindowOptions::AUTO_SIZE | WindowOptions::NO_TITLE | WindowOptions::NO_PADDING | WindowOptions::NO_RESIZE)
            .unwrap();
        let size = Size { width: 640, height: 480 };
        ui.update(size).unwrap();
        assert_eq!(window.snapshot().unwrap().rect.width, 40);
        let initial_measures = measures.get();
        send.send((publish, result)).unwrap();
        // This point is inside the privately committed new width, outside the old width.
        ui.input(
            UiInput::Down {
                position: Point { x: 100, y: 10 },
                buttons: 1,
            },
            "",
        )
        .unwrap();
        if let Err(error) = result {
            assert_eq!(ui.update(size), Err(error));
            assert_eq!(clicks.get(), 0, "failed dispatch preserves queued input for retry");
        }
        ui.update(size).unwrap();
        assert_eq!(window.snapshot().unwrap().rect.width, 120);
        assert_eq!(clicks.get(), 1, "new width must govern the first queued input");
        assert!(measures.get() > initial_measures, "committed geometry must be remeasured");
    }
}
