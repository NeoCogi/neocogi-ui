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

/// Imports matching resources and a GFX-owned compositor before publishing a live UI runtime.
pub fn create_runtime_from_assets(
    source: ComRef<'_, IAssetBundleAbi>,
    mut compositor: ComRef<'_, neocogi_gfx_contracts::GfxCompositorAbi>,
) -> Result<ComPtr<UiRuntimeAbi>, NcStatus> {
    let assets = crate::assets::ImportedAssets::read(source, None)?;
    // Reinstalling the source authenticates that the compositor and UI use the same catalog.
    compositor.install_assets(source)?;
    let runtime = Runtime {
        storage: create_storage(assets, compositor.add_ref()),
    };
    Ok(Runtime::from_value(runtime).query().expect("runtime interface"))
}

/// Creates shared runtime storage while GFX retains exclusive ownership of graphics execution.
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
    mounts: RefCell<Vec<Mount>>,
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
}

/// Temporary association between an actual COM widget and an existing retained root.
struct Mount {
    /// Native identity remains in the manager; this record carries no duplicate parent edges.
    surface: NativeSurface,
    /// Manager-owned COM surface identity authenticates parent capabilities without exposing IDs.
    identity: ComPtr<IUnknown>,
}

impl RuntimeStorage {
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
        if context.window_manager.collect_widget_changes() {
            self.pending.set(true);
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
        // Dropping the owned Context then closes every node in its authoritative forest.
    }
}

impl IInputSink for Runtime {
    /// Validates before enqueueing and owns the copied text after this synchronous call.
    fn input(&self, event: UiInputAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
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
        entries: NcCopyFromBufferOffset<'_, ChoiceMenuEntryAbi>,
        selection: ChoiceSelectionAbi,
    ) -> Result<ComPtr<IChoiceMenuAbi>, NcStatus> {
        choices::create_choice_menu(&self.storage, parent, menu_text, entries, selection)
    }
}

impl UiRuntime for Runtime {
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
            if previous.source().as_ptr() == source.as_ptr() {
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
    fn text_input_area(&self) -> Result<TextInputAreaAbi, NcStatus> {
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
        }
        .into())
    }

    /// Polling and eventful updates execute the same concrete phase sequence.
    fn update(&self, size: Size) -> Result<(), NcStatus> {
        self.update_cycle(size, None)
    }
    /// Lends the dispatcher only after returning owned UI context storage to its idle slot.
    fn update_with_events(&self, size: Size, dispatcher: ComRef<'_, IEventDispatcherAbi>) -> Result<(), NcStatus> {
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
        context.paint(size)?;
        if let Some(error) = storage.factory.state.error.take() {
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
            return Ok(());
        }
        let mut context = storage.context()?;
        let surfaces: Vec<_> = storage.mounts.borrow().iter().map(|m| m.surface.clone()).collect();
        storage.factory.state.closed.set(true);
        storage.factory.state.error.set(None);
        for surface in surfaces {
            surface.close(&mut context.window_manager)?;
        }
        storage.mounts.borrow_mut().clear();
        context.clear_choice_menus();
        storage.factory.state.error.take().map_or(Ok(()), Err)
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
    fn update_cycle(&self, size: Size, dispatcher: Option<ComRef<'_, IEventDispatcherAbi>>) -> Result<(), NcStatus> {
        let storage = &self.storage;
        storage.open()?;
        if size.width <= 0 || size.height <= 0 {
            return Err(NC_E_INVALID_ARG);
        }
        if storage.updating.replace(true) {
            return Err(UI_E_BUSY);
        }
        let _guard = UpdateGuard(&storage.updating);
        let dimensions = Dimensioni::new(size.width, size.height);
        {
            let mut context = storage.context()?;
            // A rejected call during paint/another context transaction must not
            // invalidate the caller's already committed frame.
            storage.pending.set(true);
            storage.collect_changes(&mut context)?;
            context.window_manager.begin_update_cycle(dimensions);
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
                let updated = context.window_manager.update_cycle_step(first);
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
    /// Every visit rediscovers mounted drivers; removed tickets are retired immediately.
    fn dispatch_boundary(&self, dispatcher: Option<ComRef<'_, IEventDispatcherAbi>>, dimensions: Dimensioni, remaining: &mut u32) -> Result<(), NcStatus> {
        loop {
            // Layout itself may post messages, so it precedes the next queue visit.
            // No context transaction survives any component or application callback.
            self.storage.layout_boundary(dimensions)?;
            let (reconciled, component_drivers) = {
                let mut context = self.storage.context()?;
                (context.reconcile_choice_menus()?, context.window_manager.component_event_drivers()?)
            };
            let before = *remaining;
            if reconciled {
                *remaining = remaining.checked_sub(1).ok_or(neocogi_com_events::EVENT_E_LIMIT)?;
            }
            for component in component_drivers {
                let count = component.dispatch(*remaining)?;
                *remaining = remaining.checked_sub(count).ok_or(NC_E_INVALID_ARG)?;
                self.storage.open()?;
            }
            self.storage.phase_result()?;
            if before != *remaining {
                continue;
            }
            // One app message may create child work. Settle that work before the
            // next app message, preserving FIFO without exposing a public pump.
            let count = match dispatcher {
                Some(mut dispatcher) => dispatcher.dispatch((*remaining).min(1))?,
                None => 0,
            };
            if count > 1 {
                return Err(NC_E_INVALID_ARG);
            }
            *remaining = remaining.checked_sub(count).ok_or(NC_E_INVALID_ARG)?;
            self.storage.open()?;
            self.storage.phase_result()?;
            if count == 0 {
                return Ok(());
            }
        }
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
        context.window_manager.layout_update_cycle(dimensions);
        context.window_manager.event_result()?;
        self.phase_result()
    }
}

#[cfg(test)]
mod tests;
