//! Resource lifetime assertions cover partial paint and successful production submission.
use super::*;
use neocogi_com::{com_object, ComRef};

/// One shared callback observation contains both the failure switch and release count.
#[derive(Default)]
struct Attempt {
    /// Reject after recording one owned texture, leaving a partial list for runtime cleanup.
    reject: bool,
    /// Counts actual COM object destruction, independently of widget logical closure.
    releases: usize,
}

#[com_object(interfaces(ITexture))]
/// Per-paint texture survives only as long as the painter/compositor retain its capability.
struct TemporaryTexture {
    /// Drop publishes a copied lifecycle observation without keeping any UI capability alive.
    attempt: Rc<RefCell<Attempt>>,
}
impl ITexture for TemporaryTexture {
    /// Dimensions are immutable for this resource's complete lifetime.
    fn size(&self) -> Result<Size, NcStatus> {
        Ok(Size { width: 8, height: 8 })
    }
}
impl Drop for TemporaryTexture {
    /// Observes final capability release after every abandoned or completed frame.
    fn drop(&mut self) {
        self.attempt.borrow_mut().releases += 1;
    }
}

/// A real application leaf can fail after emitting commands through its expiring painter.
struct PartialPaint {
    /// The test changes only the next callback result; runtime and compositor behavior stay real.
    attempt: Rc<RefCell<Attempt>>,
}
impl LeafContent for PartialPaint {
    /// A stable intrinsic extent keeps geometry independent of the injected paint failure.
    fn measure(&self, _: LayoutLimits, _: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        Ok(Size { width: 40, height: 20 })
    }
    /// This leaf has no semantic update behavior and never invalidates committed measurement.
    fn update(&mut self, _: WidgetUpdate, _: WidgetEvent, _: &str) -> Result<GeometryChange, NcStatus> {
        Ok(GeometryChange::NoChange)
    }
    /// Creates a resource inside the callback so any surviving owner belongs to recorded work.
    fn paint(&mut self, state: WidgetPaint, painter: &mut Painter<'_>) -> Result<(), NcStatus> {
        let texture: ComPtr<ITextureAbi> = TemporaryTexture::from_value(TemporaryTexture { attempt: self.attempt.clone() })
            .query()
            .unwrap();
        painter.texture(state.rect, 0xffffffff, texture.as_com_ref())?;
        if self.attempt.borrow().reject {
            Err(neocogi_com::NC_E_INVALID_ARG)
        } else {
            Ok(())
        }
    }
}

/// Rejected paint releases partial resources before returning and never enters the renderer.
#[test]
fn partial_paint_releases_owned_commands_before_a_later_frame_succeeds() {
    let mut fixture = Headless::standard();
    let attempt = Rc::new(RefCell::new(Attempt { reject: true, releases: 0 }));
    let leaf = fixture
        .ui
        .widgets()
        .leaf(PartialPaint { attempt: attempt.clone() }, WidgetOptions::NONE)
        .unwrap();
    let _owner = window(&fixture.ui, "partial", Rect { x: 10, y: 10, width: 120, height: 80 }, leaf);
    // A later root must remain completely untouched after an earlier callback fails.
    let later = Rc::new(RefCell::new(Attempt::default()));
    let later_leaf = fixture.ui.widgets().leaf(PartialPaint { attempt: later.clone() }, WidgetOptions::NONE).unwrap();
    let _later_owner = window(&fixture.ui, "later", Rect { x: 140, y: 10, width: 120, height: 80 }, later_leaf);
    fixture.ui.update(VIEW).unwrap();
    assert_eq!(fixture.ui.render(VIEW, fixture.renderer.as_com_ref()), Err(neocogi_com::NC_E_INVALID_ARG));
    assert_eq!(fixture.snapshot().calls, 0);
    assert_eq!(attempt.borrow().releases, 1);
    assert_eq!(later.borrow().releases, 0);
    attempt.borrow_mut().reject = false;
    fixture.frame(VIEW).unwrap();
    assert_eq!(fixture.snapshot().calls, 1);
    assert_eq!(attempt.borrow().releases, 2);
    assert_eq!(later.borrow().releases, 1);
}
