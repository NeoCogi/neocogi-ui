//! Runtime/window ownership with a temporary adapter over existing retained traversal.
use super::*;
use super::{
    button::Button,
    paint::{Metrics, PaintService},
    render::Backend,
};
use crate::prelude::{AtlasHandle, Dimensioni};
use crate::Context;
mod dialogs;
mod menus;
mod windows;
use windows::NativeSurface;

/// Creates a runtime without registering individual ephemeral widgets as COM classes.
pub fn create_runtime(atlas: AtlasHandle) -> ComPtr<IUiRuntimeAbi> {
    let bundle = crate::SkinBundle::from_atlas(atlas);
    let source = crate::assets::export_bundle(&bundle).expect("native test/source bundle is valid");
    create_runtime_from_assets(source.as_com_ref()).expect("exported native bundle imports")
}

/// Imports application-supplied resources before constructing any live runtime or widget service.
pub fn create_runtime_from_assets(source: ComRef<'_, IAssetBundleAbi>) -> Result<ComPtr<IUiRuntimeAbi>, NcStatus> {
    let assets = crate::assets::ImportedAssets::read(source, None)?;
    Ok(Runtime::from_value(Runtime { storage: create_storage(assets) })
        .query()
        .expect("runtime interface"))
}

/// Creates the one authoritative storage allocation used by the runtime and its weak controllers.
fn create_storage(assets: crate::assets::ImportedAssets) -> Rc<RuntimeStorage> {
    let bundle = assets.bundle().clone();
    let atlas = bundle.atlas().clone();
    let state = Rc::new(HostState::default());
    let host: ComPtr<IWidgetHostAbi> = WidgetHost::from_value(WidgetHost { state: state.clone() }).query().expect("host interface");
    let resources = super::widget::Resources::new(assets, state.clone());
    let metrics = Metrics::from_value(Metrics { resources: resources.clone() })
        .query()
        .expect("metrics interface");
    let backend = Backend::new(atlas, state.clone(), resources.clone());
    let target = backend.target.clone();
    let factory = Rc::new(super::factory::NodeFactory {
        layout: super::layout::LayoutService::new(resources.clone(), state.clone()),
        attaching: Cell::new(false),
        state: state.clone(),
        host,
        metrics,
        resources,
        painter: PaintService::new(),
        children: RefCell::default(),
    });
    *state.factory.borrow_mut() = Rc::downgrade(&factory);
    Rc::new(RuntimeStorage {
        context: RefCell::new(Some(Context::from_bundle(backend, bundle))),
        mounts: RefCell::new(Vec::new()),
        factory,
        target,
        dimensions: Cell::new(Size::default()),
        pending: Cell::new(true),
        updating: Cell::new(false),
        menus: RefCell::new(Vec::new()),
        dialogs: RefCell::new(Vec::new()),
    })
}

#[com_class(interfaces(IUiRuntime, IUiFactory, IWindowManager, IMenus, IFileDialogs, IUiStyle))]
/// One identity owns the runtime, its widget factory and window-management policy.
struct Runtime {
    /// Window handles use Weak references so retaining them cannot form a tree cycle.
    storage: Rc<RuntimeStorage>,
}

/// All authoritative runtime state is private to the UI plugin.
struct RuntimeStorage {
    /// Idle engine slot; transactions take ownership so no RefCell borrow spans a COM call.
    context: RefCell<Option<Context<Backend>>>,
    /// Surface capabilities authenticate parents; the forest alone owns widget nodes.
    mounts: RefCell<Vec<Mount>>,
    /// Shared construction services retain no root forest or owning runtime.
    factory: Rc<super::factory::NodeFactory>,
    /// Renderer is retained only during explicit synchronous submission.
    target: Rc<RefCell<Option<Rc<ComPtr<IRendererAbi>>>>>,
    /// Last successfully synchronized drawable dimensions.
    dimensions: Cell<Size>,
    /// Structural/setter changes require update before rendering.
    pending: Cell<bool>,
    /// Spans the full input transaction while individual phases return the context for callback mutations.
    updating: Cell<bool>,
    /// Weak declaration authentication retains no unused menu tree or application state.
    menus: RefCell<Vec<(usize, std::rc::Weak<menus::Declaration>)>>,
    /// Standard controllers own only weak runtime references, so runtime ownership keeps active modals functional without a cycle.
    dialogs: RefCell<Vec<Rc<dialogs::Dialog>>>,
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
    fn collect_changes(&self, context: &mut Context<Backend>) -> Result<(), NcStatus> {
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
        for dialog in self.dialogs.get_mut().iter() {
            dialog.retire();
        }
        // Dropping the owned Context then closes every node in its authoritative forest.
    }
}

impl IUiFactory for Runtime {
    /// Supplies the same host used by built-ins without allocating a dummy widget or retaining the UI tree.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        self.storage.open()?;
        Ok(self.storage.factory.host.clone())
    }
    /// Constructs the complete semantic combo header with the same host and immutable resources as other leaves.
    fn create_combo(&self) -> Result<ComPtr<IComboAbi>, NcStatus> {
        self.storage.factory.host.check_mutation()?;
        Ok(super::combo::Combo::create(
            self.storage.factory.host.clone(),
            self.storage.factory.resources.clone(),
            crate::Combo::from(crate::ComboParameters::new()),
        ))
    }

    /// Validates complete text and wrapping before constructing the fixed scroll/editor composition.
    fn create_text_area(&self, input: NcCopyFromBufferOffset<'_, u8>, wrapping: TextWrapping, scrolling: bool) -> Result<ComPtr<ITextAreaAbi>, NcStatus> {
        super::text_area::TextArea::create(
            self.storage.factory.host.clone(),
            self.storage.factory.resources.clone(),
            text(input)?,
            super::leaves::local_wrap(wrapping)?,
            scrolling,
        )
    }
    /// Validates text and presentation before constructing either fixed structural child.
    fn create_disclosure(&self, label: NcCopyFromBufferOffset<'_, u8>, style: DisclosureStyle, expanded: bool) -> Result<ComPtr<IDisclosureAbi>, NcStatus> {
        super::disclosure::Disclosure::create(
            self.storage.factory.host.clone(),
            self.storage.factory.resources.clone(),
            text(label)?,
            style,
            expanded,
        )
    }
    /// Composes real bars and a transform container around the validated application content root.
    fn create_scroll_area(&self, content: ComRef<'_, IWidgetAbi>, enabled: bool) -> Result<ComPtr<IScrollAreaAbi>, NcStatus> {
        super::scroll::ScrollArea::create(self.storage.factory.host.clone(), self.storage.factory.resources.clone(), content, enabled)
    }
    /// Validates both policies before creating the container's single COM allocation and host-owned child store.
    fn create_linear(&self, direction: FlowDirection, cross: CrossSizeAbi) -> Result<ComPtr<ILinearAbi>, NcStatus> {
        self.storage.factory.host.check_mutation()?;
        let direction = super::containers::local_direction(direction)?;
        let cross = super::containers::local_cross(cross)?;
        super::containers::Linear::create(
            self.storage.factory.host.clone(),
            self.storage.factory.resources.clone(),
            crate::Linear::com_new(direction, cross),
        )
    }
    /// Validates complete typed track sequences before allocating any live Grid state.
    fn create_grid(
        &self,
        columns: NcCopyFromBufferOffset<'_, LayoutTrackAbi>,
        rows: NcCopyFromBufferOffset<'_, LayoutTrackAbi>,
    ) -> Result<ComPtr<IGridAbi>, NcStatus> {
        self.storage.factory.host.check_mutation()?;
        let columns = super::containers::local_tracks(columns)?;
        let rows = super::containers::local_tracks(rows)?;
        super::containers::Grid::create(
            self.storage.factory.host.clone(),
            self.storage.factory.resources.clone(),
            crate::Grid::com_new(columns, rows),
        )
    }
    /// Validates complete checkbox initialization before allocating its COM identity.
    fn create_checkbox(&self, checked: bool, label: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<ICheckboxAbi>, NcStatus> {
        self.storage.factory.host.check_mutation()?;
        let parameters = crate::CheckboxParameters::new(text(label)?, checked);
        Ok(super::leaves::Checkbox::create(
            self.storage.factory.host.clone(),
            self.storage.factory.resources.clone(),
            crate::Checkbox::from(parameters),
        ))
    }
    /// Validates complete textbox initialization before allocating its COM identity.
    fn create_textbox(&self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<ITextboxAbi>, NcStatus> {
        self.storage.factory.host.check_mutation()?;
        let parameters = crate::TextboxParameters::new(text(input)?);
        Ok(super::leaves::Textbox::create(
            self.storage.factory.host.clone(),
            self.storage.factory.resources.clone(),
            crate::Textbox::from(parameters),
        ))
    }
    /// Validates complete text block initialization before allocating its COM identity.
    fn create_text_block(&self, input: NcCopyFromBufferOffset<'_, u8>, wrapping: TextWrapping) -> Result<ComPtr<ITextBlockAbi>, NcStatus> {
        self.storage.factory.host.check_mutation()?;
        let parameters = crate::TextBlockParameters::with_wrap(text(input)?, super::leaves::local_wrap(wrapping)?);
        Ok(super::leaves::TextBlock::create(
            self.storage.factory.host.clone(),
            self.storage.factory.resources.clone(),
            crate::TextBlock::from(parameters),
        ))
    }
    /// Validates complete slider initialization before allocating its COM identity.
    fn create_slider(&self, options: SliderOptions) -> Result<ComPtr<ISliderAbi>, NcStatus> {
        self.storage.factory.host.check_mutation()?;
        let parameters = crate::SliderParameters::with_opt(
            options.value,
            options.low,
            options.high,
            options.step,
            crate::DecimalPrecision::try_from(usize::from(options.precision)).map_err(|_| NC_E_INVALID_ARG)?,
            crate::WidgetOption::FRAME,
        )
        .map_err(|_| NC_E_INVALID_ARG)?;
        Ok(super::leaves::Slider::create(
            self.storage.factory.host.clone(),
            self.storage.factory.resources.clone(),
            crate::Slider::from(parameters),
        ))
    }
    /// Validates complete number initialization before allocating its COM identity.
    fn create_number(&self, options: NumberOptions) -> Result<ComPtr<INumberAbi>, NcStatus> {
        self.storage.factory.host.check_mutation()?;
        let parameters = crate::NumberParameters::new(
            options.value,
            options.step,
            crate::DecimalPrecision::try_from(usize::from(options.precision)).map_err(|_| NC_E_INVALID_ARG)?,
        )
        .map_err(|_| NC_E_INVALID_ARG)?;
        Ok(super::leaves::Number::create(
            self.storage.factory.host.clone(),
            self.storage.factory.resources.clone(),
            crate::Number::from(parameters),
        ))
    }
    /// Validates complete color swatch initialization before allocating its COM identity.
    fn create_color_swatch(&self, rgba: u32, label: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<IColorSwatchAbi>, NcStatus> {
        self.storage.factory.host.check_mutation()?;
        let parameters = crate::ColorSwatchParameters::new(super::render::color(rgba)).label(text(label)?);
        Ok(super::leaves::ColorSwatch::create(
            self.storage.factory.host.clone(),
            self.storage.factory.resources.clone(),
            crate::ColorSwatch::from(parameters),
        ))
    }
    /// Validates complete list item initialization before allocating its COM identity.
    fn create_list_item(&self, label: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<IListItemAbi>, NcStatus> {
        self.storage.factory.host.check_mutation()?;
        let parameters = crate::ListItemParameters::new(text(label)?);
        Ok(super::leaves::ListItem::create(
            self.storage.factory.host.clone(),
            self.storage.factory.resources.clone(),
            crate::ListItem::from(parameters),
        ))
    }
    /// Validates complete list box initialization before allocating its COM identity.
    fn create_list_box(&self, label: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<IListBoxAbi>, NcStatus> {
        self.storage.factory.host.check_mutation()?;
        let parameters = crate::ListBoxParameters::new(text(label)?, None);
        Ok(super::leaves::ListBox::create(
            self.storage.factory.host.clone(),
            self.storage.factory.resources.clone(),
            crate::ListBox::from(parameters),
        ))
    }
    /// Validates the axis and complete range before constructing a scrollbar.
    fn create_scrollbar(&self, range: ScrollRange) -> Result<ComPtr<IScrollbarAbi>, NcStatus> {
        self.storage.factory.host.check_mutation()?;
        let axis = match range.axis {
            ScrollAxis::HORIZONTAL => crate::ScrollbarAxis::Horizontal,
            ScrollAxis::VERTICAL => crate::ScrollbarAxis::Vertical,
            _ => return Err(NC_E_INVALID_ARG),
        };
        if range.viewport < 0 || range.content < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let parameters = crate::ScrollbarParameters::new(axis).range(range.viewport, range.content, range.offset);
        Ok(super::leaves::Scrollbar::create(
            self.storage.factory.host.clone(),
            self.storage.factory.resources.clone(),
            crate::Scrollbar::from(parameters),
        ))
    }

    /// Creates one unmounted button whose host has no reference back to this runtime.
    fn create_button(&self) -> Result<ComPtr<IButtonAbi>, NcStatus> {
        self.storage.factory.host.check_mutation()?;
        Ok(Button::create(
            self.storage.factory.host.clone(),
            self.storage.factory.resources.clone(),
            crate::Button::from(crate::ButtonParameters::new("")),
        ))
    }
}

impl IInputSink for Runtime {
    /// Validates before enqueueing and owns the copied text after this synchronous call.
    fn input(&self, event: UiInputAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        let storage = &self.storage;
        storage.open()?;
        let mut context = storage.context()?;
        super::input::enqueue(&mut context, event, text)?;
        storage.pending.set(true);
        Ok(())
    }

}

impl IUiStyle for Runtime {
    /// Takes one immutable resource snapshot; derivation never borrows or mutates the retained tree.
    fn derive_style(&self, metrics: AssetMetrics, palette: PaletteEditAbi) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        self.storage.open()?;
        self.storage.factory.resources.current().assets.derive_style(metrics, palette)
    }
}

impl IUiRuntime for Runtime {
    /// Exposes an owned immutable revision without borrowing mutable tree state.
    fn assets(&self) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        self.storage.open()?;
        Ok(self.storage.factory.resources.current().assets.source().add_ref())
    }
    /// Prepares all CPU state and the fallible GPU upload before publishing a shared revision.
    fn install_assets(&self, source: ComRef<'_, IAssetBundleAbi>, renderer: ComRef<'_, IRendererAbi>) -> Result<(), NcStatus> {
        let storage = &self.storage;
        storage.factory.host.check_mutation()?;
        if storage.updating.get() {
            return Err(UI_E_BUSY);
        }
        let mut context = storage.context()?;
        let _phase = Observation::begin(&storage.factory.state);
        let previous = storage.factory.resources.current();
        if previous.assets.source().as_ptr() == source.as_ptr() {
            // The UI revision is already current. Only the explicitly supplied renderer may
            // need to switch back from another UI's atlas; do not invalidate unchanged geometry.
            return renderer.prepare_assets(source);
        }
        let assets = crate::assets::ImportedAssets::read(source, Some(&previous.assets))?;
        let next = super::widget::ResourceRevision::new(assets, storage.factory.state.clone());
        renderer.prepare_assets(source)?;
        // ImportedAssets has already proved every original identity survives. The COM backend's
        // atlas replacement is a local assignment; this commit performs no further foreign calls.
        context.set_skin_bundle(next.assets.bundle()).map_err(|_| NC_E_INVALID_ARG)?;
        storage.factory.resources.install(next);
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
            Some(area) => TextInputArea::Area(Rect { x: area.x, y: area.y, width: area.width, height: area.height }),
            None => TextInputArea::None,
        }.into())
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
        let info =
            crate::render::FrameInfo::try_new(Dimensioni::new(size.width, size.height), super::render::color(0xff302820)).map_err(|_| NC_E_INVALID_ARG)?;
        // The complete submission is observational, including calls into the native renderer.
        let _phase = Observation::begin(&storage.factory.state);
        storage.factory.state.error.set(None);
        let renderer = Rc::new(renderer.add_ref());
        *storage.target.borrow_mut() = Some(renderer);
        let result = context.frame(info).render_ui();
        let renderer = storage.target.borrow_mut().take();
        drop(renderer);
        if let Some(error) = storage.factory.state.error.take() {
            return Err(error);
        }
        result.map_err(|_| UI_E_RENDER)
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
        for dialog in storage.dialog_snapshot() {
            dialog.retire();
        }
        storage.factory.state.error.take().map_or(Ok(()), Err)
    }
}

/// Owns one synchronous transaction instead of lending runtime storage across COM.
struct ContextTransaction<'a> {
    /// Empty while calls are active; reentry sees BUSY instead of a borrow panic.
    slot: &'a RefCell<Option<Context<Backend>>>,
    /// Owned engine restored on every return path, including error unwinding.
    value: Option<Context<Backend>>,
}

impl core::ops::Deref for ContextTransaction<'_> {
    type Target = Context<Backend>;
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
            storage.phase_result()?;
        }
        // All retained traversal ownership is back in storage before entering the
        // application. A handler can remove/create windows or edit ordinary topology.
        if self.dispatch_boundary(dispatcher)? {
            storage.layout_boundary(dimensions)?;
        }
        let mut first = true;
        loop {
            let updated = {
                storage.open()?;
                let mut context = storage.context()?;
                let updated = context.window_manager.update_cycle_step(first);
                storage.phase_result()?;
                updated
            };
            first = false;
            if !updated {
                break;
            }
            self.dispatch_boundary(dispatcher)?;
            storage.layout_boundary(dimensions)?;
        }
        storage.context()?.window_manager.finish_update_cycle(dimensions);
        storage.dimensions.set(size);
        storage.pending.set(false);
        Ok(())
    }

    /// Runs one finite application sweep without retaining its borrowed capability or any context transaction.
    fn dispatch_boundary(&self, dispatcher: Option<ComRef<'_, IEventDispatcherAbi>>) -> Result<bool, NcStatus> {
        let mut delivered = 0;
        loop {
            let before = delivered;
            // Internal standard components and application callbacks share one
            // finite boundary. Neither owns a registry borrow across the other.
            for dialog in self.storage.dialog_snapshot() {
                let remaining = EVENT_DISPATCH_LIMIT - delivered;
                let count = dialog.dispatch(remaining)?;
                if count > remaining {
                    return Err(NC_E_INVALID_ARG);
                }
                delivered += count;
                self.storage.open()?;
            }
            if let Some(dispatcher) = dispatcher {
                let remaining = EVENT_DISPATCH_LIMIT - delivered;
                let count = dispatcher.dispatch(remaining)?;
                if count > remaining {
                    return Err(NC_E_INVALID_ARG);
                }
                delivered += count;
            }
            self.storage.open()?;
            self.storage.phase_result()?;
            if before == delivered {
                break;
            }
        }
        Ok(delivered != 0)
    }
}
impl RuntimeStorage {
    /// Takes a pure local Rc snapshot and retires closed controllers outside the short registry borrow.
    fn dialog_snapshot(&self) -> Vec<Rc<dialogs::Dialog>> {
        let mut retired = Vec::new();
        let snapshot = {
            let mut dialogs = self.dialogs.borrow_mut();
            let mut index = 0;
            while index < dialogs.len() {
                if dialogs[index].finished() {
                    retired.push(dialogs.remove(index));
                } else {
                    index += 1;
                }
            }
            dialogs.clone()
        };
        for dialog in &retired {
            dialog.retire();
        }
        drop(retired);
        snapshot
    }

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
        self.phase_result()
    }
}
