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
impl neocogi_platform_contracts::render::abi::IRenderDriverBound for TemporaryTexture {
    fn driver_identity(&self) -> Result<neocogi_com::ComPtr<neocogi_platform_contracts::render::abi::IRenderDriverIdentityAbi>, neocogi_com::NcStatus> {
        Err(neocogi_com::NC_E_NOINTERFACE)
    }
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
        neocogi_gfx_support::testing::register_image(texture.as_com_ref(), Size { width: 8, height: 8 });
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
    assert_eq!(
        fixture.ui.render(VIEW, fixture.renderer.as_com_ref()).map_err(|error| error.status),
        Err(neocogi_com::NC_E_INVALID_ARG)
    );
    assert_eq!(fixture.snapshot().calls, 0);
    assert_eq!(attempt.borrow().releases, 1);
    assert_eq!(later.borrow().releases, 0);
    attempt.borrow_mut().reject = false;
    fixture.frame(VIEW).unwrap();
    assert_eq!(fixture.snapshot().calls, 1);
    assert_eq!(attempt.borrow().releases, 2);
    assert_eq!(later.borrow().releases, 1);
}

/// Client actions performed inside the production IWidget::paint callback.
#[derive(Copy, Clone)]
enum GpuPaintExit {
    /// Leave a healthy host-owned recording unfinished when its facade dies.
    Unfinished,
    /// Try to erase a command failure by explicit cancellation before destruction.
    RejectedCancel,
    /// Ignore a command failure and let the facade die.
    RejectedDrop,
    /// Retry begin after destroying unfinished work, then ignore its result.
    Retry,
    /// Deliberately abandon a healthy contribution.
    Cancel,
    /// Seal a valid contribution before dropping its facade.
    Finish,
}

/// An ordinary leaf receives only resource and painter capabilities, without host session access.
struct GpuPaint {
    /// Same-device factory; the test retains the exclusive domain controller.
    resources: ComPtr<neocogi_platform_contracts::render::abi::IRenderResourcesAbi>,
    /// Allows a later frame to succeed without replacing or remounting the leaf.
    exit: Rc<std::cell::Cell<GpuPaintExit>>,
}
impl LeafContent for GpuPaint {
    /// Keeps the real paint allocation positive and independent of the selected exit behavior.
    fn measure(&self, _: LayoutLimits, _: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        Ok(Size { width: 40, height: 20 })
    }
    /// The test changes paint behavior without invalidating its committed layout.
    fn update(&mut self, _: WidgetUpdate, _: WidgetEvent, _: &str) -> Result<GeometryChange, NcStatus> {
        Ok(GeometryChange::NoChange)
    }
    /// Return Ok deliberately: host harvesting must reject unfinished/rejected work independently.
    fn paint(&mut self, _: WidgetPaint, painter: &mut Painter<'_>) -> Result<(), NcStatus> {
        let mut view = painter.begin_render_view(self.resources.as_com_ref())?;
        view.clear_color(0xff112233)?;
        match self.exit.get() {
            GpuPaintExit::RejectedCancel | GpuPaintExit::RejectedDrop => {
                assert_eq!(view.clear_depth(f32::NAN), Err(neocogi_com::NC_E_INVALID_ARG));
                if matches!(self.exit.get(), GpuPaintExit::RejectedCancel) {
                    view.cancel()?;
                }
            }
            GpuPaintExit::Cancel => view.cancel()?,
            GpuPaintExit::Finish => view.finish()?,
            GpuPaintExit::Unfinished | GpuPaintExit::Retry => {}
        }
        drop(view);
        if matches!(self.exit.get(), GpuPaintExit::Retry) {
            assert_eq!(
                painter.begin_render_view(self.resources.as_com_ref()).err(),
                Some(neocogi_platform_contracts::render::abi::RENDER_E_STATE)
            );
            assert_eq!(
                painter.begin_render_view(self.resources.as_com_ref()).err(),
                Some(neocogi_platform_contracts::render::abi::RENDER_E_STATE)
            );
        }
        Ok(())
    }
}

/// Actual callback return, host harvest and compositor submission enforce one failure boundary.
#[test]
fn gpu_paint_failure_survives_facade_drop_cancel_and_retry_until_host_return() {
    for mode in [
        GpuPaintExit::Unfinished,
        GpuPaintExit::RejectedCancel,
        GpuPaintExit::RejectedDrop,
        GpuPaintExit::Retry,
    ] {
        let mut fixture = Headless::standard();
        let domain = neocogi_gfx_support::testing::client();
        let resources = domain.resources().unwrap();
        let exit = Rc::new(std::cell::Cell::new(mode));
        let leaf = fixture
            .ui
            .widgets()
            .leaf(
                GpuPaint {
                    resources: resources.clone(),
                    exit: exit.clone(),
                },
                WidgetOptions::NONE,
            )
            .unwrap();
        let _owner = window(&fixture.ui, "gpu", Rect { x: 10, y: 10, width: 120, height: 80 }, leaf);
        let later = Rc::new(RefCell::new(Attempt::default()));
        let later_leaf = fixture.ui.widgets().leaf(PartialPaint { attempt: later.clone() }, WidgetOptions::NONE).unwrap();
        let _later_owner = window(&fixture.ui, "later", Rect { x: 140, y: 10, width: 120, height: 80 }, later_leaf);
        fixture.ui.update(VIEW).unwrap();
        for _ in 0..2 {
            assert_eq!(
                fixture.ui.render(VIEW, fixture.renderer.as_com_ref()).map_err(|error| error.status),
                Err(neocogi_platform_contracts::render::abi::RENDER_E_STATE)
            );
            assert_eq!(fixture.snapshot().calls, 0);
            assert_eq!(later.borrow().releases, 0);
            assert_eq!(neocogi_gfx_support::testing::recording_usage(resources.as_com_ref()).unwrap(), (0, 0));
        }
        for success in [GpuPaintExit::Cancel, GpuPaintExit::Finish] {
            exit.set(success);
            fixture.frame(VIEW).unwrap();
        }
        assert_eq!(fixture.snapshot().calls, 2);
        assert_eq!(later.borrow().releases, 2);
        assert_eq!(neocogi_gfx_support::testing::recording_usage(resources.as_com_ref()).unwrap(), (0, 0));
    }
}
