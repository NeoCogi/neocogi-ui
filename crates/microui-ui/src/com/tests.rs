//! Boundary regressions use the real runtime and deliberately retaining/reentrant components.
use super::*;
use nc_com::{NcCopyToBufferOffset, INcStringComPtrExt};

/// Positive drawable size shared by the deterministic headless fixtures.
const SIZE: Size = Size { width: 640, height: 400 };
/// Logical window allocation with room for retained chrome and the test leaf.
const RECT: Rect = Rect { x: 20, y: 20, width: 400, height: 200 };

/// Idle still performs the complete setter/resize synchronization transaction.
fn idle() -> PlatformEvent {
    PlatformEvent {
        kind: PlatformEventKind::NONE,
        x: 0,
        y: 0,
    }
}

/// Creates the production runtime with a small deterministic atlas.
fn runtime() -> ComPtr<IUiRuntimeAbi> {
    create_runtime(crate::test_support::test_atlas())
}

/// Creates a concrete button without bypassing its production factory.
fn button(runtime: &ComPtr<IUiRuntimeAbi>) -> ComPtr<IButtonAbi> {
    runtime.query::<IUiFactoryAbi>().unwrap().create_button().unwrap()
}

/// Mounts the common capability through the production manager's transactional boundary.
fn mount(runtime: &ComPtr<IUiRuntimeAbi>, root: &ComPtr<IWidgetAbi>) -> Result<ComPtr<IWindowAbi>, NcStatus> {
    runtime
        .query::<IWindowManagerAbi>()?
        .create_window(NcCopyFromBufferOffset::new(0, b"test"), RECT, root.as_com_ref())
}

/// Observable state retained by a fake backend independently of its COM allocation.
#[derive(Default)]
struct RenderProbe {
    /// Complete renderer invocations, including deliberately failing submissions.
    calls: Cell<u32>,
    /// Old retained stream/token used to prove that expiry survives later reuse.
    streams: RefCell<Vec<(ComPtr<IRenderStreamAbi>, u64)>>,
    /// Test-controlled explicit renderer failure.
    fail: Cell<bool>,
}

#[com_class(interfaces(IRenderer))]
/// Typed backend records real final vertices and retains the stream maliciously but safely.
struct Renderer {
    /// Shared observations owned by the test, with no borrowed stack reference in COM.
    probe: Rc<RenderProbe>,
}

impl IRenderer for Renderer {
    /// Copies bulk geometry during its valid scope and retains authority for later rejection tests.
    fn render(&self, info: RenderInfo, stream: ComRef<'_, IRenderStreamAbi>) -> Result<(), NcStatus> {
        let mut vertices = vec![RenderVertex::default(); info.vertex_count as usize];
        assert_eq!(
            stream.vertices(info.invocation, NcCopyToBufferOffset::new(0, &mut vertices))?,
            info.vertex_count
        );
        assert!(vertices.len() >= 6);
        assert!(vertices.iter().all(|v| v.x.is_finite() && v.y.is_finite()));
        self.probe.calls.set(self.probe.calls.get() + 1);
        self.probe.streams.borrow_mut().push((stream.add_ref(), info.invocation));
        if self.probe.fail.get() { Err(UI_E_RENDER) } else { Ok(()) }
    }
}

/// Returns the COM renderer and independently observable probe state.
fn renderer() -> (ComPtr<IRendererAbi>, Rc<RenderProbe>) {
    let probe = Rc::new(RenderProbe::default());
    (Renderer::from_value(Renderer { probe: probe.clone() }).query().unwrap(), probe)
}

/// Wrong owners and duplicate attachment fail without consuming the legitimate mount opportunity.
#[test]
fn mount_validation_is_transactional_and_closure_outlives_tree_storage() {
    let first = runtime();
    let second = runtime();
    let button = button(&first);
    let node = button.query().unwrap();
    assert_eq!(mount(&second, &node).unwrap_err(), UI_E_OWNER);
    let window = mount(&first, &node).unwrap();
    assert_eq!(mount(&first, &node).unwrap_err(), UI_E_OWNER);
    assert_eq!(button.set_label(NcCopyFromBufferOffset::new(1, b"bad")), Err(NC_E_INVALID_ARG));
    assert_eq!(button.set_label(NcCopyFromBufferOffset::new(0, &[0xff])), Err(NC_E_INVALID_ARG));
    assert_eq!(button.label().unwrap().to_string().unwrap(), "");
    window.close().unwrap();
    assert_eq!(button.set_label(NcCopyFromBufferOffset::new(0, b"closed")), Err(UI_E_CLOSED));
    let unmounted = self::button(&first);
    drop(first);
    assert_eq!(unmounted.label().unwrap_err(), UI_E_CLOSED);
    assert_eq!(window.set_rect(RECT), Err(UI_E_CLOSED));
    second.update(SIZE, idle()).unwrap();
}

/// Setter and resize invalidation are independent of user activation's NoChange result.
#[test]
fn rendering_requires_synchronized_geometry_and_streams_expire_after_success_and_failure() {
    let runtime = runtime();
    let button = button(&runtime);
    let node = button.query().unwrap();
    let _window = mount(&runtime, &node).unwrap();
    let (renderer, probe) = renderer();
    assert_eq!(runtime.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    runtime.update(SIZE, idle()).unwrap();
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    button.set_label(NcCopyFromBufferOffset::new(0, b"longer")).unwrap();
    assert_eq!(runtime.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    assert_eq!(probe.calls.get(), 1);
    runtime.update(SIZE, idle()).unwrap();
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    button.set_label(NcCopyFromBufferOffset::new(0, b"longer")).unwrap();
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    let resized = Size { width: 800, height: 480 };
    assert_eq!(runtime.render(resized, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    runtime.update(resized, idle()).unwrap();
    probe.fail.set(true);
    assert_eq!(runtime.render(resized, renderer.as_com_ref()), Err(UI_E_RENDER));
    for (stream, scope) in probe.streams.borrow().iter() {
        assert_eq!(stream.vertices(*scope, NcCopyToBufferOffset::new(0, &mut [])), Err(UI_E_EXPIRED_SCOPE));
    }
}

/// A component can retain COM services; the runtime must enforce phase and scope at runtime.
#[derive(Default)]
struct WidgetProbe {
    /// Paint calls retain these values beyond their valid invocation.
    painters: RefCell<Vec<(ComPtr<IPainterAbi>, PaintScope)>>,
    /// Number of actual preferred measurement calls after cache handling.
    measures: Cell<u32>,
    /// Injected paint failure must prevent the complete backend submission.
    fail_paint: Cell<bool>,
}

#[com_class(interfaces(IWidget, ILeafLayout, IActivatable, IWidgetLifetime))]
/// Independently implemented leaf delegates lifetime only to a factory-owned fixture object.
struct RetainingWidget {
    /// Gives the custom implementation the same runtime ownership identity and mount-once rules.
    lifetime: ComPtr<IWidgetLifetimeAbi>,
    /// Actual concrete setter used to test observational mutation rejection.
    button: ComPtr<IButtonAbi>,
    /// Retained runtime deliberately tries to reenter during measurement and painting.
    runtime: ComPtr<IUiRuntimeAbi>,
    /// Test state is owned and survives every COM call.
    probe: Rc<WidgetProbe>,
}

impl IWidgetLifetime for RetainingWidget {
    /// Returns the shared owner without exposing runtime storage.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        self.lifetime.host()
    }
    /// Delegates mount-once state to its independent lifetime fixture.
    fn mount(&self) -> Result<(), NcStatus> {
        self.lifetime.mount()
    }
    /// Delegates logical closure to its lifetime fixture.
    fn close(&self) -> Result<(), NcStatus> {
        self.lifetime.close()
    }
}
impl IActivatable for RetainingWidget {
    /// This custom leaf has no activation-dependent geometry.
    fn activate(&self) -> Result<GeometryChange, NcStatus> {
        Ok(GeometryChange::NO_CHANGE)
    }
}
impl ILeafLayout for RetainingWidget {
    /// Checks reentry and mutation rejection while returning deterministic preferred geometry.
    fn measure(&self, _bounds: LayoutLimits, _metrics: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        self.probe.measures.set(self.probe.measures.get() + 1);
        assert_eq!(self.runtime.update(SIZE, idle()), Err(UI_E_BUSY));
        assert_eq!(self.button.set_label(NcCopyFromBufferOffset::new(0, b"forbidden")), Err(UI_E_BUSY));
        Ok(Size { width: 100, height: 30 })
    }
}
impl IWidget for RetainingWidget {
    /// No setter changes are needed by this deliberate retention fixture.
    fn take_change(&self) -> Result<GeometryChange, NcStatus> {
        Ok(GeometryChange::NO_CHANGE)
    }
    /// Retains authority and probes old tokens while a later scope is active.
    fn paint(&self, scope: PaintScope, rect: Rect, painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
        assert_eq!(self.runtime.update(SIZE, idle()), Err(UI_E_BUSY));
        assert_eq!(self.button.set_label(NcCopyFromBufferOffset::new(0, b"forbidden")), Err(UI_E_BUSY));
        for (old, expired) in self.probe.painters.borrow().iter() {
            assert_eq!(old.fill(*expired, rect, 0), Err(UI_E_EXPIRED_SCOPE));
        }
        painter.fill(scope, rect, 0xff00ff00)?;
        self.probe.painters.borrow_mut().push((painter.add_ref(), scope));
        if self.probe.fail_paint.get() { Err(NC_E_INVALID_ARG) } else { Ok(()) }
    }
}

/// Retained scope authority expires, reentry is rejected, and paint failure never reaches the GPU.
#[test]
fn external_widget_cannot_reenter_mutate_during_observation_or_reuse_a_paint_scope() {
    let runtime = runtime();
    let button = button(&runtime);
    let probe = Rc::new(WidgetProbe::default());
    let widget = RetainingWidget::from_value(RetainingWidget {
        lifetime: button.query().unwrap(),
        button,
        runtime: runtime.clone(),
        probe: probe.clone(),
    });
    let window = mount(&runtime, &widget.query().unwrap()).unwrap();
    let (renderer, backend) = renderer();
    runtime.update(SIZE, idle()).unwrap();
    let measured = probe.measures.get();
    runtime.update(SIZE, idle()).unwrap();
    assert_eq!(probe.measures.get(), measured);
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    probe.fail_paint.set(true);
    assert_eq!(runtime.render(SIZE, renderer.as_com_ref()), Err(NC_E_INVALID_ARG));
    assert_eq!(backend.calls.get(), 2);
    window.close().unwrap();
    runtime.close().unwrap();
    for (painter, scope) in probe.painters.borrow().iter() {
        assert_eq!(painter.fill(*scope, RECT, 0), Err(UI_E_EXPIRED_SCOPE));
    }
}
