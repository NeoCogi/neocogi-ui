//! Runtime/window ownership with a temporary adapter over existing retained traversal.
use super::*;
use super::{
    button::Button,
    paint::{Metrics, PaintService},
    render::Backend,
};
use crate::prelude::{
    AtlasHandle, Dimensioni, Node, WidgetOption, KeyboardBehavior, KeyboardAction, WidgetUpdateCtx, WidgetPaintCtx, UiInputEvent, ControlRole, Constraints,
    Skin, Key, KeyEvent, KeyState, Modifiers, MouseButton,
};
use crate::{Context, TypedWidgetHandle, LeafWidget, Widget, Change};
mod windows;
use windows::NativeSurface;

/// Creates a runtime without registering individual ephemeral widgets as COM classes.
pub fn create_runtime(atlas: AtlasHandle) -> ComPtr<IUiRuntimeAbi> {
    let state = Rc::new(HostState::default());
    let host: ComPtr<IWidgetHostAbi> = WidgetHost::from_value(WidgetHost { state: state.clone() }).query().expect("host interface");
    let metrics = Metrics::from_value(Metrics {
        skin: Skin::from_atlas(&atlas),
        atlas: atlas.clone(),
    })
    .query()
    .expect("metrics interface");
    let backend = Backend::new(atlas, state.clone());
    let target = backend.target.clone();
    Runtime::from_value(Runtime {
        storage: Rc::new(RuntimeStorage {
            context: RefCell::new(Context::new(backend)),
            mounts: RefCell::new(Vec::new()),
            state,
            host,
            metrics,
            painter: PaintService::new(),
            target,
            dimensions: Cell::new(Size::default()),
            pending: Cell::new(true),
        }),
    })
    .query()
    .expect("runtime interface")
}

#[com_class(interfaces(IUiRuntime, IUiFactory, IWindowManager))]
/// One identity owns the runtime, its widget factory and window-management policy.
struct Runtime {
    /// Window handles use Weak references so retaining them cannot form a tree cycle.
    storage: Rc<RuntimeStorage>,
}

/// All authoritative runtime state is private to the UI plugin.
struct RuntimeStorage {
    /// Existing retained engine, temporarily bridged until COM node ownership replaces its handles.
    context: RefCell<Context<Backend>>,
    /// Mounted root capabilities and legacy invalidation hooks, removed with the node migration.
    mounts: RefCell<Vec<Mount>>,
    /// Shared lifetime/phase state contains no references back to this allocation.
    state: Rc<HostState>,
    /// Canonical runtime owner identity checked before attachment.
    host: ComPtr<IWidgetHostAbi>,
    /// Immutable resource service cached once rather than allocated during measurement.
    metrics: ComPtr<ITextMetricsAbi>,
    /// Reusable owned recorder shared by sequential widget paint calls.
    painter: PaintService,
    /// Renderer is retained only during explicit synchronous submission.
    target: Rc<RefCell<Option<ComPtr<IRendererAbi>>>>,
    /// Last successfully synchronized drawable dimensions.
    dimensions: Cell<Size>,
    /// Structural/setter changes require update before rendering.
    pending: Cell<bool>,
}

/// Temporary association between an actual COM widget and an existing retained root.
struct Mount {
    /// Native identity remains in the manager; this record carries no duplicate parent edges.
    surface: NativeSurface,
    /// Manager-owned COM surface identity authenticates parent capabilities without exposing IDs.
    identity: ComPtr<IUnknown>,
    /// Actual common widget object; no mirrored label or semantic state.
    widget: ComPtr<IWidgetAbi>,
    /// Cached lifecycle interface avoids repeated QueryInterface during teardown.
    lifetime: ComPtr<IWidgetLifetimeAbi>,
    /// Weak invalidation hook for the temporary retained adapter.
    bridge: TypedWidgetHandle<Bridge>,
}

impl RuntimeStorage {
    /// Rejects all operations after logical closure.
    fn open(&self) -> Result<(), NcStatus> {
        if self.state.closed.get() { Err(UI_E_CLOSED) } else { Ok(()) }
    }
    /// Closes capabilities removed by any manager subtree operation, including descendants.
    fn prune_mounts(&self) -> Result<(), NcStatus> {
        let removed: Vec<_> = self.mounts.borrow_mut().extract_if(.., |mount| !mount.bridge.is_alive()).collect();
        let mut failure = None;
        for mount in removed {
            if let Err(error) = mount.lifetime.close() {
                failure.get_or_insert(error);
            }
        }
        failure.map_or(Ok(()), Err)
    }
    /// Drains sticky setter dirtiness before any parent can return cached measurement.
    fn collect_changes(&self) -> Result<(), NcStatus> {
        // Snapshot capabilities first: external implementations may reenter, so no mounts borrow
        // survives a call. The Context borrow owned by the transaction rejects such reentry.
        self.prune_mounts()?;
        let widgets: Vec<_> = self.mounts.borrow().iter().map(|mount| (mount.widget.clone(), mount.bridge.clone())).collect();
        for (widget, bridge) in widgets {
            if widget.take_change()? == GeometryChange::CHANGED {
                bridge.try_update(|_| ()).ok_or(UI_E_CLOSED)?;
                self.pending.set(true);
            }
        }
        Ok(())
    }
    /// Closes every mounted capability after the caller has secured exclusive traversal access.
    fn close_mounts(&self) -> Result<(), NcStatus> {
        let mounts = std::mem::take(&mut *self.mounts.borrow_mut());
        let mut failure = None;
        for mount in mounts {
            if let Err(error) = mount.lifetime.close() {
                failure.get_or_insert(error);
            }
        }
        failure.map_or(Ok(()), Err)
    }
}

impl Drop for RuntimeStorage {
    /// Final release closes retained handles even without an explicit application close.
    fn drop(&mut self) {
        self.state.closed.set(true);
        let _ = self.close_mounts();
    }
}

impl IUiFactory for Runtime {
    /// Creates one unmounted button whose host has no reference back to this runtime.
    fn create_button(&self) -> Result<ComPtr<IButtonAbi>, NcStatus> {
        self.storage.host.check_mutation()?;
        Ok(Button::create(self.storage.host.clone()))
    }
}

impl IUiRuntime for Runtime {
    /// Processes a normalized event using the existing retained router and layout algorithms.
    fn update(&self, size: Size, event: PlatformEvent) -> Result<(), NcStatus> {
        let storage = &self.storage;
        storage.open()?;
        if size.width <= 0 || size.height <= 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let mut context = storage.context.try_borrow_mut().map_err(|_| UI_E_BUSY)?;
        storage.collect_changes()?;
        match event.kind {
            PlatformEventKind::NONE | PlatformEventKind::RESIZE | PlatformEventKind::QUIT => {}
            PlatformEventKind::POINTER_MOVE => context.mousemove(event.x, event.y),
            PlatformEventKind::POINTER_DOWN => context.mousedown(event.x, event.y, MouseButton::LEFT),
            PlatformEventKind::POINTER_UP => context.mouseup(event.x, event.y, MouseButton::LEFT),
            PlatformEventKind::TAB | PlatformEventKind::ACTIVATE => context.key(KeyEvent {
                key: if event.kind == PlatformEventKind::TAB { Key::Tab } else { Key::Enter },
                state: KeyState::Pressed,
                modifiers: Modifiers::empty(),
                repeat: false,
            }),
            _ => return Err(NC_E_INVALID_ARG),
        }
        storage.state.error.set(None);
        context.update_ui(Dimensioni::new(size.width, size.height));
        if let Some(error) = storage.state.error.take() {
            storage.pending.set(true);
            return Err(error);
        }
        storage.dimensions.set(size);
        storage.pending.set(false);
        Ok(())
    }
    /// Keeps the renderer reference and stream authority within one synchronous submission.
    fn render(&self, size: Size, renderer: ComRef<'_, IRendererAbi>) -> Result<(), NcStatus> {
        let storage = &self.storage;
        storage.open()?;
        let mut context = storage.context.try_borrow_mut().map_err(|_| UI_E_BUSY)?;
        storage.collect_changes()?;
        if storage.pending.get() || storage.dimensions.get() != size {
            return Err(UI_E_UPDATE_REQUIRED);
        }
        let info =
            crate::render::FrameInfo::try_new(Dimensioni::new(size.width, size.height), super::render::color(0xff302820)).map_err(|_| NC_E_INVALID_ARG)?;
        // The complete submission is observational, including calls into the native renderer.
        let _phase = Observation::begin(&storage.state);
        storage.state.error.set(None);
        *storage.target.borrow_mut() = Some(renderer.add_ref());
        let result = context.frame(info).render_ui();
        storage.target.borrow_mut().take();
        if let Some(error) = storage.state.error.take() {
            return Err(error);
        }
        result.map_err(|_| UI_E_RENDER)
    }
    /// Removes every root while leaving independent retained COM handles safely closed.
    fn close(&self) -> Result<(), NcStatus> {
        let storage = &self.storage;
        if storage.state.closed.get() {
            return Ok(());
        }
        let mut context = storage.context.try_borrow_mut().map_err(|_| UI_E_BUSY)?;
        let surfaces: Vec<_> = storage.mounts.borrow().iter().map(|m| m.surface.clone()).collect();
        storage.state.closed.set(true);
        for surface in surfaces {
            surface.close(&mut context.window_manager)?;
        }
        storage.close_mounts()
    }
}

/// Temporary leaf adapter: derived geometry stays in Node, semantics in the COM widget.
struct Bridge {
    /// Actual common widget capability, also retained by the application proxy.
    widget: ComPtr<IWidgetAbi>,
    /// Cached preferred-size capability.
    layout: ComPtr<ILeafLayoutAbi>,
    /// Cached semantic activation; the slice currently mounts button-like leaves.
    activation: ComPtr<IActivatableAbi>,
    /// Owned immutable resource metrics.
    metrics: ComPtr<ITextMetricsAbi>,
    /// Owned scope-checked recorder, never a wrapper around WidgetPaintCtx.
    painter: PaintService,
    /// Error/phase state without a pointer back into the retained tree.
    host: Rc<HostState>,
    /// Temporary legacy routing configuration, removed by common-widget contracts.
    opt: WidgetOption,
}

impl Widget for Bridge {
    /// Uses the existing generic frame and hit-target policy.
    fn widget_opt(&self) -> &WidgetOption {
        &self.opt
    }
    /// Declares button keyboard behavior to the existing shared input router.
    fn keyboard_behavior(&self) -> KeyboardBehavior {
        KeyboardBehavior::TAB_STOP | KeyboardBehavior::ACTIVATE_ENTER | KeyboardBehavior::ACTIVATE_SPACE
    }
    /// Lets the shared retained traversal draw the same theme-owned outer frame.
    fn frame_appearance_role(&self) -> crate::FrameRole {
        crate::FrameRole::Control(ControlRole::Button)
    }
    /// Sends activation to the concrete COM widget without a producer borrow or callback.
    fn update(&mut self, ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) -> Change {
        if ctx.action(input, self.keyboard_behavior()) == Some(KeyboardAction::Activate) {
            match self.activation.activate() {
                Ok(change) => return Change::from(change == GeometryChange::CHANGED),
                Err(error) => self.host.fail(error),
            }
        }
        Change::NoChange
    }
    /// Starts an observational scope and records/replays complete owned commands.
    fn paint(&mut self, ctx: &mut WidgetPaintCtx<'_>) {
        let _phase = Observation::begin(&self.host);
        if let Err(error) = self.painter.paint(&self.widget, ctx) {
            self.host.fail(error);
        }
    }
}

impl LeafWidget for Bridge {
    /// Preserves finite/unbounded constraints across the temporary ABI conversion.
    fn measure(&self, _skin: &Skin, _atlas: &AtlasHandle, constraints: Constraints) -> Dimensioni {
        let _phase = Observation::begin(&self.host);
        let bounds = LayoutLimits {
            width: axis_limit(constraints.width),
            height: axis_limit(constraints.height),
        };
        match self.layout.measure(bounds, self.metrics.as_com_ref()) {
            Ok(size) if size.width >= 0 && size.height >= 0 => Dimensioni::new(size.width, size.height),
            result => {
                self.host.fail(result.err().unwrap_or(NC_E_INVALID_ARG));
                Dimensioni::default()
            }
        }
    }
}

/// Converts one legacy axis without losing the difference between zero and unbounded.
fn axis_limit(space: crate::AvailableSpace) -> AxisLimit {
    match space.bound() {
        Some(extent) => AxisLimit { kind: LimitKind::FINITE, extent },
        None => AxisLimit { kind: LimitKind::UNBOUNDED, extent: 0 },
    }
}
