//! Dynamically linked runtime ownership over the authoritative retained traversal.

use crate::boundary::{local_rect, text};
use crate::ui_node::{HostState, Observation, WidgetHost};
use neocogi_ui_core_contracts::*;
use neocogi_com::{ComPtr, ComRef, IUnknown, NcCopyFromBufferOffset, NcStatus, NC_E_INVALID_ARG, com_object};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use crate::ui_node::paint_service::{Metrics, PaintService};
use crate::{Dimensioni, runtime_context::RuntimeContext};
pub(crate) mod choices;
mod menus;
mod windows;
use windows::NativeSurface;

/// Imports immutable resources and creates a private GFX compositor before publishing a UI.
///
/// The factory can be shared across runtimes, but each successful call must return a fresh
/// compositor initialized from `source`. Keeping that mutable capability inside the runtime
/// prevents another UI or caller from replacing its resources or interfering with a frame.
/// The UI depends only on the GFX contract in shared, monolithic, and WebAssembly deployments.
pub fn create_runtime_from_assets(
    source: ComRef<'_, IAssetBundleAbi>,
    gfx: ComRef<'_, neocogi_gfx_contracts::GfxPluginFactoryAbi>,
) -> Result<ComPtr<UiRuntimeAbi>, NcStatus> {
    // Reject invalid UI metadata before asking GFX to allocate any mutable frame state.
    let assets = crate::assets::ImportedAssets::read(source, None)?;
    // GFX imports the same immutable source during construction. Do not reinstall it into
    // a caller-provided compositor: that object could already belong to another runtime.
    // A factory error drops the local import without publishing a partially initialized UI.
    let compositor = gfx.create_compositor(source)?;
    let runtime = Runtime {
        storage: create_storage(assets, compositor),
    };
    Ok(Runtime::from_value(runtime).query().expect("runtime interface"))
}

/// Moves the runtime's private compositor into shared UI storage without exposing its capability.
/// GFX owns graphics execution; only this runtime selects its resource revisions and frames.
pub(crate) fn create_storage(assets: crate::assets::ImportedAssets, compositor: ComPtr<neocogi_gfx_contracts::GfxCompositorAbi>) -> Rc<RuntimeStorage> {
    let bundle = assets.bundle().clone();
    let state = Rc::new(HostState::default());
    let host: ComPtr<IWidgetHostAbi> = WidgetHost::from_value(WidgetHost { state: state.clone() }).query().expect("host interface");
    let resources = Rc::new(RefCell::new(assets));
    let metrics = Metrics::from_value(Metrics { resources: resources.clone() })
        .query()
        .expect("metrics interface");
    let factory = Rc::new(crate::ui_node::node_factory::NodeFactory {
        layout: crate::ui_node::layout_service::LayoutService::new(resources.clone(), state.clone()),
        attaching: Cell::new(false),
        state: state.clone(),
        host,
        metrics,
        resources: resources.clone(),
        painter: PaintService::new(resources.clone()),
        children: RefCell::default(),
    });
    *state.factory.borrow_mut() = Rc::downgrade(&factory);
    let construction: ComPtr<IConstructionHostAbi> = ConstructionHost::from_value(ConstructionHost { host: factory.host.clone() })
        .query()
        .expect("construction host declares its callable contract");
    Rc::new(RuntimeStorage {
        context: RefCell::new(Some(crate::runtime_context::RuntimeContext::new(compositor, bundle))),
        mounts: RefCell::new(Vec::new()),
        factory,
        construction,
        dimensions: Cell::new(Size::default()),
        pending: Cell::new(true),
        updating: Cell::new(false),
        menus: RefCell::new(Vec::new()),
        dispatch_widgets: RefCell::new(Vec::new()),
    })
}

#[com_object(interfaces(IConstructionHost))]
/// Runtime-owned construction projection containing no concrete node implementation.
struct ConstructionHost {
    /// Canonical non-owning widget authority returned to independently deployed node providers.
    host: ComPtr<IWidgetHostAbi>,
}

impl IConstructionHost for ConstructionHost {
    /// Returns the exact host identity used by root attachment and child collection validation.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        self.host.check_open()?;
        Ok(self.host.clone())
    }
}

#[com_object(UiRuntime)]
/// One identity owns retained traversal, core construction authority and window-management policy.
pub struct Runtime {
    /// Window handles use Weak references so retaining them cannot form a tree cycle.
    pub(crate) storage: Rc<RuntimeStorage>,
}

/// All authoritative runtime state is private to the UI plugin.
pub(crate) struct RuntimeStorage {
    /// Idle engine slot; transactions take ownership so no RefCell borrow spans a COM call.
    context: RefCell<Option<RuntimeContext>>,
    /// Surface capabilities authenticate parents; the forest alone owns widget nodes.
    mounts: RefCell<Vec<SurfaceMount>>,
    /// Shared construction services retain no root forest or owning runtime.
    factory: Rc<crate::ui_node::node_factory::NodeFactory>,
    /// Core-only construction endpoint queried by independent node provider plugins.
    construction: ComPtr<IConstructionHostAbi>,
    /// Last successfully synchronized drawable dimensions.
    dimensions: Cell<Size>,
    /// Structural/setter changes require update before rendering.
    pending: Cell<bool>,
    /// Spans the full input transaction while individual phases return the context for callback mutations.
    updating: Cell<bool>,
    /// Weak declaration authentication retains no unused menu tree or application state.
    menus: RefCell<Vec<(usize, std::rc::Weak<menus::Declaration>)>>,
    /// Reusable descendant-first widget snapshot within one update; no handles survive a frame.
    /// A callback may edit topology, so no RefCell borrow spans its COM call.
    dispatch_widgets: RefCell<Vec<ComPtr<IWidgetAbi>>>,
}

/// Temporary association between an actual COM widget and an existing retained root.
struct SurfaceMount {
    /// Native identity remains in the manager; this record carries no duplicate parent edges.
    surface: NativeSurface,
    /// Manager-owned COM surface identity authenticates parent capabilities without exposing IDs.
    identity: ComPtr<IUnknown>,
}

impl RuntimeStorage {
    /// Rebuilds this update's delivery order after topology changes, reusing scratch capacity.
    fn refresh_dispatch_widgets(&self) -> Result<(), NcStatus> {
        if self.factory.state.dispatch_order_changed.get() {
            let context = self.context()?;
            let mut drivers = self.dispatch_widgets.borrow_mut();
            drivers.clear();
            context.window_manager.collect_dispatch_widgets(&mut drivers)?;
            self.factory.state.dispatch_order_changed.set(false);
        }
        Ok(())
    }

    /// Rejects all operations after logical closure.
    fn open(&self) -> Result<(), NcStatus> {
        if self.factory.state.closed.get() { Err(UI_E_CLOSED) } else { Ok(()) }
    }
    /// Takes exclusive engine ownership while allowing foreign calls to reenter safely.
    fn context(&self) -> Result<ContextTransaction<'_>, NcStatus> {
        let value = self.context.try_borrow_mut().map_err(|_| UI_E_BUSY)?.take().ok_or(UI_E_BUSY)?;
        Ok(ContextTransaction { slot: &self.context, value: Some(value) })
    }

    /// Removes surface projections whose authoritative forest entries were destroyed.
    fn prune_mounts(&self, manager: &crate::window_manager::WindowManager) {
        self.mounts.borrow_mut().retain(|mount| mount.surface.is_alive(manager));
    }

    /// Drains setter changes throughout the forest, including hidden subtrees.
    fn collect_changes(&self, context: &mut RuntimeContext) -> Result<(), NcStatus> {
        self.prune_mounts(&context.window_manager);
        let _phase = Observation::begin(&self.factory.state);
        self.factory.state.error.set(None);
        match context.window_manager.collect_widget_changes() {
            Ok(true) => self.pending.set(true),
            Ok(false) => {}
            Err(error) => {
                self.pending.set(true);
                return Err(error);
            }
        }
        if let Some(error) = self.factory.state.error.take() {
            self.pending.set(true);
            return Err(error);
        }
        Ok(())
    }
}

impl Drop for RuntimeStorage {
    /// Final release closes retained handles even without an explicit application close.
    fn drop(&mut self) {
        self.factory.state.closed.set(true);
        // Remove the forest first, then finish deferred widget close without
        // holding any traversal or RefCell borrow across a foreign call.
        drop(self.context.get_mut().take());
        let _ = self.factory.state.finish_widget_closure();
        self.dispatch_widgets.get_mut().clear();
        // No future runtime boundary exists after final destruction. A caller
        // may still own a borrowed widget, but the closed host rejects its work.
        // Do not retain that widget in a host -> widget -> host reference cycle.
        self.factory.state.release_pending_closure();
    }
}

impl IInputSink for Runtime {
    /// Validates before enqueueing and owns the copied text after this synchronous call.
    fn input(&self, event: UiInput, text: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        let storage = &self.storage;
        storage.open()?;
        let mut context = storage.context()?;
        crate::ui_node::input_contract::enqueue(&mut context.window_manager, event, text)?;
        storage.pending.set(true);
        Ok(())
    }
}

impl IChoiceMenus for Runtime {
    /// Creates a reusable core-owned choice popup without constructing a concrete header widget.
    fn create_choice_menu(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        menu_text: NcCopyFromBufferOffset<'_, u8>,
        entries: NcCopyFromBufferOffset<'_, ChoiceMenuEntry>,
        selection: ChoiceSelection,
        events: neocogi_com::NcValueRef<'_, IChoiceMenuEventBinding>,
    ) -> Result<ComPtr<IChoiceMenuAbi>, NcStatus> {
        choices::create_choice_menu(&self.storage, parent, menu_text, entries, selection, events.to_owned()?.into_queue())
    }
}

impl UiRuntime for Runtime {
    /// Publishes a platform service only through the runtime's exclusive mutation boundary.
    fn set_clipboard(&mut self, clipboard: ComRef<'_, neocogi_ui_core_contracts::ITextClipboardAbi>) -> Result<(), NcStatus> {
        self.storage.factory.host.check_mutation()?;
        let _context = self.storage.context()?;
        let previous = self.storage.factory.state.clipboard.replace(Some(clipboard.add_ref()));
        // Foreign destruction cannot run while a shared service slot is borrowed.
        drop(previous);
        Ok(())
    }

    /// Returns the cached core authority used to authenticate independently bound node factories.
    fn construction_host(&self) -> Result<ComPtr<IConstructionHostAbi>, NcStatus> {
        self.storage.open()?;
        Ok(self.storage.construction.clone())
    }
    /// Exposes an owned immutable revision without borrowing mutable tree state.
    fn assets(&self) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        self.storage.open()?;
        Ok(self.storage.factory.resources.borrow().source().add_ref())
    }
    /// Uses exclusive runtime access to prepare CPU state and the GPU upload before publication.
    fn install_assets(&mut self, source: ComRef<'_, IAssetBundleAbi>, renderer: ComRef<'_, IRendererAbi>) -> Result<(), NcStatus> {
        let storage = &self.storage;
        storage.factory.host.check_mutation()?;
        // Dispatch returns the context specifically to permit application edits. The active
        // observation phase and exclusive context slot still reject layout/paint/upload reentry.
        let mut context = storage.context()?;
        let _phase = Observation::begin(&storage.factory.state);
        let next = {
            let previous = storage.factory.resources.borrow();
            if previous.source().as_raw().object == source.as_raw().object {
                None
            } else {
                Some(crate::assets::ImportedAssets::read(source, Some(&previous))?)
            }
        };
        let Some(next) = next else {
            // The UI revision is already current. Only the explicitly supplied renderer may
            // need to switch back from another UI's atlas; do not invalidate unchanged geometry.
            let resources = storage.factory.resources.borrow();
            return renderer.prepare_atlas(resources.image());
        };
        renderer.prepare_atlas(next.image())?;
        context.install_compositor_assets(source)?;
        // ImportedAssets has already proved every original identity survives. The COM backend's
        // atlas replacement is a local assignment; this commit performs no further foreign calls.
        context.set_skin_bundle(next.bundle()).map_err(|_| NC_E_INVALID_ARG)?;
        storage.factory.resources.replace(next);
        storage.pending.set(true);
        Ok(())
    }
    /// Exposes only committed geometry; pending setters or input hide stale IME placement.
    fn text_input_area(&self) -> Result<TextInputArea, NcStatus> {
        self.storage.open()?;
        let mut context = self.storage.context()?;
        self.storage.collect_changes(&mut context)?;
        let area = if self.storage.pending.get() { None } else { context.text_input_area() };
        Ok(match area {
            Some(area) => TextInputArea::Area(Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: area.height,
            }),
            None => TextInputArea::None,
        })
    }

    /// Polling and eventful updates execute the same concrete phase sequence.
    fn update(&self, size: Size) -> Result<(), NcStatus> {
        self.update_cycle(size, None)
    }
    /// Lends the dispatcher only after returning owned UI context storage to its idle slot.
    fn update_with_events(&self, size: Size, dispatcher: ComRef<'_, IApplicationDispatcherAbi>) -> Result<(), NcStatus> {
        self.update_cycle(size, Some(dispatcher))
    }
    /// Keeps the renderer reference and stream authority within one synchronous submission.
    fn render(&self, size: Size, renderer: ComRef<'_, IRendererAbi>) -> Result<(), NcStatus> {
        let storage = &self.storage;
        storage.open()?;
        if storage.updating.get() {
            return Err(UI_E_BUSY);
        }
        let mut context = storage.context()?;
        storage.collect_changes(&mut context)?;
        if storage.pending.get() || storage.dimensions.get() != size {
            return Err(UI_E_UPDATE_REQUIRED);
        }
        // The complete submission is observational, including calls into the native renderer.
        let _phase = Observation::begin(&storage.factory.state);
        storage.factory.state.error.set(None);
        let painted = context.paint(size);
        let boundary_error = storage.factory.state.error.take();
        if let Err(error) = painted.and_then(|()| boundary_error.map_or(Ok(()), Err)) {
            // A foreign widget can fail after recording commands. Discard those owned commands
            // before entering GFX so a failed observational phase cannot publish a partial frame.
            context.discard_paint();
            return Err(error);
        }
        // Only a successful paint phase may cross the GFX submission boundary. Native renderer
        // diagnostics retain backend detail while the public UI operation reports UI_E_RENDER.
        context.submit(size, 0xff302820, renderer).map_err(|_| UI_E_RENDER)
    }
    /// Removes every root while leaving independent retained COM handles safely closed.
    fn close(&self) -> Result<(), NcStatus> {
        let storage = &self.storage;
        if storage.factory.state.closed.get() {
            // Closing again may finish a widget close previously blocked by a
            // caller's local widget guard. Never reenter an active handler.
            return match storage.updating.get() {
                true => Ok(()),
                false => storage.factory.state.finish_widget_closure(),
            };
        }
        let mut context = storage.context()?;
        let surfaces: Vec<_> = storage.mounts.borrow().iter().map(|m| m.surface.clone()).collect();
        storage.factory.state.closed.set(true);
        let clipboard = storage.factory.state.clipboard.take();
        drop(clipboard);
        storage.factory.state.error.set(None);
        for surface in surfaces {
            surface.close(&mut context.window_manager)?;
        }
        storage.mounts.borrow_mut().clear();
        context.clear_choice_menus();
        drop(context);
        let views = storage.factory.state.error.take().map_or(Ok(()), Err);
        let owners = match storage.updating.get() {
            true => Ok(()), // The active event handler must return before its own cleanup.
            false => storage.factory.state.finish_widget_closure(),
        };
        views.and(owners)
    }
}

/// Owns one synchronous transaction instead of lending runtime storage across COM.
struct ContextTransaction<'a> {
    /// Empty while calls are active; reentry sees BUSY instead of a borrow panic.
    slot: &'a RefCell<Option<RuntimeContext>>,
    /// Owned engine restored on every return path, including error unwinding.
    value: Option<RuntimeContext>,
}

impl core::ops::Deref for ContextTransaction<'_> {
    type Target = RuntimeContext;
    /// Borrows the engine owned by this transaction, never its empty storage slot.
    fn deref(&self) -> &Self::Target {
        self.value.as_ref().expect("live transaction owns its context")
    }
}

impl core::ops::DerefMut for ContextTransaction<'_> {
    /// Allows local algorithms to mutate the owned engine without a RefCell guard.
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value.as_mut().expect("live transaction owns its context")
    }
}

impl Drop for ContextTransaction<'_> {
    /// Reentry never receives the slot, so it is empty and unborrowed here.
    fn drop(&mut self) {
        *self.slot.borrow_mut() = self.value.take();
    }
}

/// Owns the entire input-cycle exclusion without owning or borrowing the context during application callbacks.
struct UpdateGuard<'a>(
    /// Reset on success, plugin failure, callback failure or explicit runtime closure.
    &'a Cell<bool>,
);
impl Drop for UpdateGuard<'_> {
    /// A recoverable failure never leaves future runtime updates permanently blocked.
    fn drop(&mut self) {
        self.0.set(false);
    }
}
impl Runtime {
    /// Executes owned phases so callback edits use ordinary COM window/topology APIs with no borrowed runtime wrapper.
    fn update_cycle(&self, size: Size, dispatcher: Option<ComRef<'_, IApplicationDispatcherAbi>>) -> Result<(), NcStatus> {
        let storage = &self.storage;
        storage.open()?;
        if size.width <= 0 || size.height <= 0 {
            return Err(NC_E_INVALID_ARG);
        }
        if storage.updating.replace(true) {
            return Err(UI_E_BUSY);
        }
        let _guard = UpdateGuard(&storage.updating);
        let result = self.run_update_cycle(size, dispatcher);
        // Every recoverable exit finalizes already removed widgets, including
        // input/layout failures before the next ordinary dispatch boundary.
        let context_available = storage.context.borrow().is_some();
        let retired = match context_available {
            true => storage.factory.state.finish_widget_closure(),
            false => Ok(()), // A rejected reentry must not run cleanup inside its caller's traversal.
        };
        // The body marks pending only after acquiring the context. A rejected
        // reentrant call during paint must not invalidate its caller's commit.
        if retired.is_err() {
            storage.pending.set(true);
        }
        // Preserve scratch capacity, not widget ownership, between updates.
        // Retaining this snapshot across frames would prolong a removed leaf's
        // allocation after its final application handle is released.
        storage.dispatch_widgets.borrow_mut().clear();
        storage.factory.state.dispatch_order_changed.set(true);
        result.and(retired)
    }

    /// Performs one guarded update; the outer operation always drains terminal cleanup.
    fn run_update_cycle(&self, size: Size, dispatcher: Option<ComRef<'_, IApplicationDispatcherAbi>>) -> Result<(), NcStatus> {
        let storage = &self.storage;
        let dimensions = Dimensioni::new(size.width, size.height);
        {
            let mut context = storage.context()?;
            // A rejected call during paint/another context transaction must not
            // invalidate the caller's already committed frame.
            storage.pending.set(true);
            storage.collect_changes(&mut context)?;
            context.window_manager.begin_update_cycle(dimensions)?;
            context.window_manager.event_result()?;
            storage.phase_result()?;
        }
        // All retained traversal ownership is back in storage before entering the
        // application. A handler can remove/create windows or edit ordinary topology.
        let mut remaining = EVENT_DISPATCH_LIMIT;
        self.dispatch_boundary(dispatcher, dimensions, &mut remaining)?;
        let mut first = true;
        loop {
            let updated = {
                storage.open()?;
                let mut context = storage.context()?;
                let updated = context.window_manager.update_cycle_step(first)?;
                context.window_manager.event_result()?;
                storage.phase_result()?;
                updated
            };
            first = false;
            if !updated {
                break;
            }
            self.dispatch_boundary(dispatcher, dimensions, &mut remaining)?;
        }
        storage.context()?.window_manager.finish_update_cycle(dimensions);
        storage.dimensions.set(size);
        storage.pending.set(false);
        Ok(())
    }

    /// Settles descendants and layout before consuming one application message.
    /// Every topology change invalidates the snapshot before another widget can be dispatched.
    fn dispatch_boundary(
        &self,
        dispatcher: Option<ComRef<'_, IApplicationDispatcherAbi>>,
        dimensions: Dimensioni,
        remaining: &mut u32,
    ) -> Result<(), NcStatus> {
        loop {
            // Exhaustion stops before another dequeue. Reaching the exact ceiling also
            // returns LIMIT: proving quiescence would require inspecting foreign queues.
            if *remaining == 0 {
                return Err(UI_E_EVENT_LIMIT);
            }
            self.storage.layout_boundary(dimensions)?;
            self.storage.factory.state.finish_widget_closure()?;
            let reconciled = {
                let mut context = self.storage.context()?;
                context.reconcile_choice_menus()?
            };
            self.storage.refresh_dispatch_widgets()?;
            let mut progressed = reconciled;
            if reconciled {
                spend_dispatch_budget(remaining)?;
            }
            let mut index = 0;
            loop {
                // Retain only this call's widget; release snapshot access before
                // its handler can change the tree or remove this same object.
                let widget = self.storage.dispatch_widgets.borrow().get(index).cloned();
                let Some(mut widget) = widget else { break };
                // One message per visit lets removal take effect before this
                // widget receives any successor from its private channel.
                let handled = widget.dispatch_next();
                let retired = self.storage.factory.state.finish_widget_closure();
                let handled = handled?;
                retired?;
                if handled || self.storage.factory.state.dispatch_order_changed.get() {
                    progressed = true;
                    // Topology-changing callbacks must also count when they report no message.
                    // Otherwise a misbehaving widget could force unlimited rebuilds in one update.
                    spend_dispatch_budget(remaining)?;
                    // Reconsider descendants after every delivered message.
                    // No later cached widget may run against outdated topology.
                    break;
                }
                self.storage.open()?;
                index += 1;
            }
            self.storage.phase_result()?;
            if progressed {
                continue;
            }
            // One app message may create child work. Settle it before the next
            // application visit; callbacks never retain a traversal borrow.
            let delivered = match dispatcher {
                Some(mut dispatcher) => dispatcher.dispatch_next(),
                None => Ok(false),
            };
            let retired = self.storage.factory.state.finish_widget_closure();
            let delivered = delivered?;
            retired?;
            self.storage.open()?;
            self.storage.phase_result()?;
            match delivered {
                true => spend_dispatch_budget(remaining)?,
                false => return Ok(()),
            }
        }
    }
}

/// Charges one completed unit of work without dequeuing or probing the next one.
/// The ceiling is conservative: even an exactly exhausted, now-empty update must retry.
fn spend_dispatch_budget(remaining: &mut u32) -> Result<(), NcStatus> {
    *remaining -= 1;
    match *remaining {
        0 => Err(UI_E_EVENT_LIMIT),
        _ => Ok(()),
    }
}
impl RuntimeStorage {
    /// Returns the first component failure before any subsequent collection can clear the phase latch.
    fn phase_result(&self) -> Result<(), NcStatus> {
        self.factory.state.error.take().map_or(Ok(()), Err)
    }
    /// Collects callback edits and publishes layout before any subsequent input can be routed.
    fn layout_boundary(&self, dimensions: Dimensioni) -> Result<(), NcStatus> {
        self.open()?;
        let mut context = self.context()?;
        self.collect_changes(&mut context)?;
        context.window_manager.layout_update_cycle(dimensions)?;
        context.window_manager.event_result()?;
        self.phase_result()
    }
}

#[cfg(test)]
mod tests;
