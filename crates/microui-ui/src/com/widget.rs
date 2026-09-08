//! Shared phase execution for concrete built-in COM classes.
//!
//! Concrete widget types own their semantic fields directly. [`WidgetRuntime`] contains only the
//! common host, lifecycle, option, resource, and rendering machinery required by every class.
//! [`WidgetCore`] remains temporarily for widgets that have not yet moved their fields onto their
//! COM allocation; it is deleted after the incremental migration completes.
use super::*;
use crate::math::RectExt;
use microui_contracts::widget::WidgetState;
use crate::{LeafWidget, WidgetOption, WidgetUpdateCtx, WidgetPaintCtx, Constraints, AvailableSpace};
use crate::render::{DisplayList, Renderer};

/// Private semantic algorithms reused by each statically known built-in class.
pub(crate) trait WidgetAlgorithm: crate::Widget {
    /// Ends semantic event production on logical close; states without event sources need no work.
    fn close_events(&mut self) {}
    /// Adds concrete state-dependent routing policy to the common caller-authored options.
    fn effective_options(&self, options: WidgetOption) -> WidgetOption {
        options
    }
}

/// One installed revision shared by runtime traversal and its retained services.
pub(super) struct Resources {
    /// Clone an owned snapshot before foreign calls; the slot never lends a RefCell guard to a component.
    current: RefCell<Rc<crate::assets::ImportedAssets>>,
}
impl Resources {
    /// Creates the one runtime resource slot from the already validated, resolved bundle.
    pub(super) fn new(assets: crate::assets::ImportedAssets) -> Rc<Self> {
        Rc::new(Self { current: RefCell::new(Rc::new(assets)) })
    }
    /// Publishes prepared local state; foreign final releases occur after the slot borrow ends.
    pub(super) fn install(&self, revision: Rc<crate::assets::ImportedAssets>) {
        let old = self.current.replace(revision);
        drop(old);
    }
    /// Retains one immutable revision with no borrow surviving into layout, paint or foreign calls.
    pub(super) fn current(&self) -> Rc<crate::assets::ImportedAssets> {
        self.current.borrow().clone()
    }
}

/// One built-in widget's imported resources and reusable local paint workspace.
struct WidgetRevision {
    /// Locally decoded immutable bundle selected by the host's current revision.
    assets: Rc<crate::assets::ImportedAssets>,
    /// Reusable local workspace protected by the COM object's exclusive paint receiver.
    scratch: PaintScratch,
}

impl WidgetRevision {
    /// Imports a host revision and prepares the local geometry recorder before publication.
    fn read(source: ComRef<'_, IAssetBundleAbi>, previous: Option<&crate::assets::ImportedAssets>) -> Result<Self, NcStatus> {
        let assets = Rc::new(crate::assets::ImportedAssets::read(source, previous)?);
        let atlas = assets.bundle().atlas().clone();
        let (collector, storage) = super::render::Backend::recording();
        Ok(Self {
            assets,
            scratch: PaintScratch {
                list: DisplayList::new(),
                renderer: Renderer::from_atlas(collector, atlas),
                storage,
            },
        })
    }
}

/// Host-backed resource cache owned by one independently constructible widget.
struct WidgetResources {
    /// Current imported revision, mutated only through an exclusive widget receiver.
    current: WidgetRevision,
}

impl WidgetResources {
    /// Imports the host's current immutable bundle during widget construction.
    fn new(host: ComRef<'_, IWidgetHostAbi>) -> Result<Self, NcStatus> {
        let source = host.assets()?;
        Ok(Self {
            current: WidgetRevision::read(source.as_com_ref(), None)?,
        })
    }

    /// Refreshes only when the host publishes a distinct immutable bundle object.
    fn current(&mut self, host: ComRef<'_, IWidgetHostAbi>) -> Result<&mut WidgetRevision, NcStatus> {
        let source = host.assets()?;
        if self.current.assets.source().as_ptr() != source.as_ptr() {
            self.current = WidgetRevision::read(source.as_com_ref(), Some(&self.current.assets))?;
        }
        Ok(&mut self.current)
    }

    /// Retains the current immutable resource revision after completing any required refresh.
    fn snapshot(&mut self, host: ComRef<'_, IWidgetHostAbi>) -> Result<Rc<crate::assets::ImportedAssets>, NcStatus> {
        // Returning an owned revision ends the cache borrow before semantic code receives control.
        Ok(self.current(host)?.assets.clone())
    }
}

/// Common non-semantic state embedded directly in every concrete widget COM allocation.
///
/// This helper deliberately has no type parameter and cannot contain a widget implementation. It
/// owns only cross-cutting COM lifecycle and host-backed rendering resources. Concrete structs such
/// as `Button` keep their label, font, interaction state, and event sources as ordinary fields.
pub(crate) struct WidgetRuntime {
    /// Shared logical lifetime, attachment state, options, and sticky geometry invalidation.
    pub(crate) state: WidgetState,
    /// Per-object imported-resource cache and reusable rendering backend.
    resources: WidgetResources,
    /// Keeps the real host runtime alive for native-only unit-test fixtures.
    #[cfg(test)]
    test_owner: Option<ComPtr<UiRuntimeAbi>>,
}

impl WidgetRuntime {
    /// Binds a newly constructed concrete widget to its host without storing semantic state.
    pub(crate) fn new(host: ComPtr<IWidgetHostAbi>, options: WidgetOption) -> Result<Self, NcStatus> {
        host.check_mutation()?;
        let resources = WidgetResources::new(host.as_com_ref())?;
        Ok(Self {
            state: WidgetState::new(host, WidgetOptions { bits: options.bits() }),
            resources,
            #[cfg(test)]
            test_owner: None,
        })
    }

    /// Creates a fully valid host-bound runtime for the native algorithm test harness.
    #[cfg(test)]
    pub(crate) fn for_test(options: WidgetOption) -> Self {
        // Native tests exercise the same concrete structs without mounting them through COM. A real
        // runtime supplies their host and assets, while this test-only owner keeps its weak factory
        // reachable for the complete lifetime of the widget value.
        let owner = super::runtime::create_runtime(crate::test_support::test_atlas());
        let factory = owner.factory().expect("test UI runtime exposes its factory");
        let host = factory.host().expect("test UI factory exposes its widget host");
        let mut runtime = Self::new(host, options).expect("test widget runtime imports valid assets");
        runtime.test_owner = Some(owner);
        runtime
    }

    /// Creates test-only common state together with a valid host-owned child writer.
    #[cfg(test)]
    pub(crate) fn for_test_container(options: WidgetOption) -> (Self, ComPtr<IChildrenEditorAbi>) {
        // Container algorithm tests still use the native tree harness. Supplying a real writer keeps
        // the concrete type structurally identical to its dynamically constructed COM allocation.
        let runtime = Self::for_test(options);
        let editor = runtime
            .state
            .host()
            .create_children()
            .expect("test widget host creates a child collection");
        (runtime, editor)
    }

    /// Returns an owned resource snapshot so no cache borrow crosses semantic widget execution.
    pub(super) fn resources(&mut self) -> Result<Rc<crate::assets::ImportedAssets>, NcStatus> {
        self.resources.snapshot(self.state.host())
    }

    /// Resolves one UTF-8 font name against this widget's exact current resource revision.
    pub(super) fn named_font(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<crate::FontRef, NcStatus> {
        self.state.writable()?;
        let name = text(input)?;
        let assets = self.resources()?;
        let font = assets.original().font_id(name).ok_or(nc_com::NC_E_NOT_FOUND)?;
        Ok(crate::FontRef::Named(font))
    }

    /// Resolves one UTF-8 icon name against this widget's exact current resource revision.
    pub(super) fn named_icon(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<crate::IconRef, NcStatus> {
        self.state.writable()?;
        let name = text(input)?;
        let assets = self.resources()?;
        let icon = assets.original().icon_id(name).ok_or(nc_com::NC_E_NOT_FOUND)?;
        Ok(crate::IconRef::Named(icon))
    }

    /// Renders one locally recorded display list and forwards its copied operations to the caller.
    pub(super) fn render(
        &mut self,
        scope: PaintScope,
        size: crate::Dimensioni,
        list: &mut DisplayList,
        painter: ComRef<'_, IPainterAbi>,
    ) -> Result<(), NcStatus> {
        let revision = self.resources.current(self.state.host())?;
        let info = crate::render::FrameInfo::try_new(size, crate::color(0, 0, 0, 0)).map_err(|_| NC_E_INVALID_ARG)?;
        revision.scratch.renderer.render(info, list).map_err(|_| UI_E_RENDER)?;
        revision.scratch.storage.paint(scope, painter)
    }
}

/// Gives shared COM phase plumbing access to a concrete widget's non-semantic runtime field.
///
/// This is a statically dispatched internal Rust trait, not a COM interface or an alternate widget
/// representation. Each implementation returns the one `WidgetRuntime` physically stored beside
/// that widget's concrete fields.
pub(crate) trait HostedWidget: WidgetAlgorithm {
    /// Borrows common state for observational lifecycle checks.
    fn runtime(&self) -> &WidgetRuntime;
    /// Borrows common state exclusively for mutation, resource refresh, or rendering.
    fn runtime_mut(&mut self) -> &mut WidgetRuntime;
    /// Closes resources owned specifically by this widget after its common lifetime ends.
    fn close_contents(&mut self) -> Result<(), NcStatus> {
        // Leaf controls own only their semantic event sources. Containers override this hook to
        // close their child writer without making the common runtime aware of topology.
        self.close_events();
        Ok(())
    }
}

/// Commits one validated semantic font role on a concrete widget allocation.
pub(super) fn set_direct_font_role<S: HostedWidget + FontState>(widget: &mut S, role: TextFontRole) -> Result<(), NcStatus> {
    widget.runtime().state.writable()?;
    let font = crate::FontRef::Role(local_font_role(role)?);
    if widget.font() != &font {
        widget.set_font(font);
        widget.runtime_mut().state.record_change(GeometryChange::CHANGED);
    }
    Ok(())
}

/// Resolves and commits one named font against the concrete widget's current host resources.
pub(super) fn set_direct_named_font<S: HostedWidget + FontState>(
    widget: &mut S,
    name: NcCopyFromBufferOffset<'_, u8>,
) -> Result<(), NcStatus> {
    let font = widget.runtime_mut().named_font(name)?;
    if widget.font() != &font {
        widget.set_font(font);
        widget.runtime_mut().state.record_change(GeometryChange::CHANGED);
    }
    Ok(())
}

/// Commits one validated semantic icon on a concrete widget allocation.
pub(super) fn set_direct_icon_role<S: HostedWidget + IconState>(widget: &mut S, icon: ContentIcon) -> Result<(), NcStatus> {
    widget.runtime().state.writable()?;
    if widget.set_icon(local_content_icon(icon)?) {
        widget.runtime_mut().state.record_change(GeometryChange::CHANGED);
    }
    Ok(())
}

/// Resolves and commits one named icon against the concrete widget's current host resources.
pub(super) fn set_direct_named_icon<S: HostedWidget + IconState>(
    widget: &mut S,
    name: NcCopyFromBufferOffset<'_, u8>,
) -> Result<(), NcStatus> {
    let icon = widget.runtime_mut().named_icon(name)?;
    if widget.set_icon(Some(icon)) {
        widget.runtime_mut().state.record_change(GeometryChange::CHANGED);
    }
    Ok(())
}

/// Inline common state plus the one concrete semantic state of a COM widget.
pub(super) struct WidgetCore<S: WidgetAlgorithm> {
    /// Shared lifecycle, options and pending setter invalidation.
    pub(super) state: WidgetState,
    /// Concrete algorithm state; no Any or dynamically selected Rust widget.
    pub(super) core: S,
    /// Host-backed resource cache contains no runtime implementation pointer or type erasure.
    resources: WidgetResources,
}

impl<S: WidgetAlgorithm> WidgetCore<S> {
    /// Closes ordinary sources with the logical widget, before final COM release drops its semantic state.
    pub(super) fn close(&mut self) -> Result<(), NcStatus> {
        if self.state.close()? {
            self.core.close_events();
        }
        Ok(())
    }

    /// Obtains the native typed endpoint under local state access, then releases that access before retaining the foreign sink.
    pub(super) fn subscribe<E: super::events::ComEvent>(&mut self, sink: ComRef<'_, E::Sink>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>
    where
        S: crate::TypedWidget<E>,
    {
        self.state.writable()?;
        let endpoint = self.core.event();
        super::events::subscribe(endpoint, sink)
    }

    /// Composes an unmounted semantic value into its final COM allocation.
    pub(super) fn new(host: ComPtr<IWidgetHostAbi>, options: WidgetOption, core: S) -> Result<Self, NcStatus> {
        host.check_mutation()?;
        let resources = WidgetResources::new(host.as_com_ref())?;
        let options = WidgetOptions { bits: options.bits() };
        Ok(Self {
            state: WidgetState::new(host, options),
            core,
            resources,
        })
    }

    /// Combines common setter markers with the composite's explicitly shared semantic notifications.
    pub(super) fn take_change(&mut self) -> Result<GeometryChange, NcStatus> {
        self.state.readable()?;
        if self.core.take_shared_change() {
            self.state.record_change(GeometryChange::CHANGED);
        }
        self.state.take_change()
    }

    /// Refreshes routing policy from the same state that handles update and paint.
    pub(super) fn properties(&self) -> Result<WidgetProperties, NcStatus> {
        let options = self.state.options()?;
        Ok(WidgetProperties {
            options: WidgetOptions {
                bits: self.core.effective_options(WidgetOption::from_bits_retain(options.bits)).bits(),
            },
            keyboard: self.core.keyboard_behavior().bits(),
            frame: super::node::frame_contract(self.core.frame_appearance_role()),
        })
    }

    /// Updates the sole option authority while rejecting reentry into an active algorithm.
    pub(super) fn set_options(&mut self, options: WidgetOptions) -> Result<(), NcStatus> {
        self.state.writable()?;
        options.validate()?;
        self.state.set_options(options)
    }

    /// Reconstructs a local update context from copied input and owned resources.
    pub(super) fn update(&mut self, state: WidgetUpdate, event: WidgetEventAbi, bytes: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
        self.state.writable()?;
        state.validate()?;
        let input = super::input::local_event(event, bytes)?;
        let resources = self.resources.current(self.state.host())?;
        let mut ctx = WidgetUpdateCtx::new_with_content_geometry(
            local_rect(state.screen),
            local_rect(state.clip).translated(crate::Vec2i::new(state.screen.x, state.screen.y)),
            resources.assets.bundle().skin(),
            resources.assets.bundle().atlas(),
            state.in_hover_root,
            state.hovered,
            state.focused,
            state.clicked,
            state.active,
            crate::MouseButton::from_bits(u32::from(state.mouse_buttons)).ok_or(NC_E_INVALID_ARG)?,
            crate::Modifiers::from_bits(state.modifiers).ok_or(NC_E_INVALID_ARG)?,
        );
        let change = self.core.update(&mut ctx, input.as_ref());
        Ok(match change {
            crate::Change::Changed => GeometryChange::CHANGED,
            crate::Change::NoChange => GeometryChange::NO_CHANGE,
        })
    }

    /// Reuses the existing preferred-size algorithm with exact finite/unbounded bounds.
    pub(super) fn measure(&mut self, bounds: LayoutLimits) -> Result<Size, NcStatus>
    where
        S: LeafWidget,
    {
        self.state.readable()?;
        let constraints = Constraints::new(available(bounds.width)?, available(bounds.height)?);
        let resources = self.resources.current(self.state.host())?;
        let size = self
            .core
            .measure(resources.assets.bundle().skin(), resources.assets.bundle().atlas(), constraints);
        Ok(Size {
            width: size.width.max(0),
            height: size.height.max(0),
        })
    }

    /// Runs private paint algorithms before calling the provided foreign recorder.
    pub(super) fn paint(&mut self, scope: PaintScope, state: WidgetPaint, painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
        self.state.readable()?;
        state.validate()?;
        let options = self.state.options()?;
        let resources = self.resources.current(self.state.host())?;
        {
            let scratch = &mut resources.scratch;
            scratch.list.clear();
            let mut ctx = WidgetPaintCtx::new_with_content_geometry(
                local_rect(state.rect),
                &mut scratch.list,
                local_rect(state.clip),
                resources.assets.bundle().skin(),
                resources.assets.bundle().atlas(),
                state.enabled,
                state.hovered,
                state.focused,
                state.clicked,
                state.active,
                state.window_active,
                WidgetOption::from_bits_retain(options.bits),
            );
            self.core.paint(&mut ctx);
            let size = crate::Dimensioni::new(state.rect.width.max(1), state.rect.height.max(1));
            let info = crate::render::FrameInfo::try_new(size, crate::color(0, 0, 0, 0)).map_err(|_| NC_E_INVALID_ARG)?;
            scratch.renderer.render(info, &mut scratch.list).map_err(|_| UI_E_RENDER)?;
        }
        resources.scratch.storage.paint(scope, painter)
    }
}

impl<S: WidgetAlgorithm + crate::ContainerWidget> WidgetCore<S> {
    /// Runs the existing concrete container algorithm through validated indexed COM measurements.
    pub(super) fn measure_container(&mut self, scope: LayoutScope, bounds: LayoutLimits, service: ComRef<'_, ILayoutServiceAbi>) -> Result<Size, NcStatus> {
        self.state.readable()?;
        let constraints = super::layout::constraints(bounds)?;
        let layout = super::layout::ScopedLayout::new(scope, service)?;
        let resources = self.resources.current(self.state.host())?;
        let mut ctx = crate::MeasureCtx::scoped(resources.assets.bundle().skin(), resources.assets.bundle().atlas(), layout);
        let size = crate::ContainerWidget::measure(&self.core, &mut ctx, constraints);
        ctx.result()?;
        Ok(Size {
            width: size.width.max(0),
            height: size.height.max(0),
        })
    }

    /// Runs exact placement with owned semantic state, returning one complete copied result.
    pub(super) fn place_container(&mut self, scope: LayoutScope, rect: Rect, service: ComRef<'_, ILayoutServiceAbi>) -> Result<ContainerGeometry, NcStatus> {
        self.state.readable()?;
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let layout = super::layout::ScopedLayout::new(scope, service)?;
        let resources = self.resources.current(self.state.host())?;
        let mut ctx = crate::ContainerLayoutCtx::scoped(resources.assets.bundle().skin(), resources.assets.bundle().atlas(), local_rect(rect), layout);
        self.core.place(&mut ctx, local_rect(rect));
        ctx.result()
    }

    /// Reuses concrete post-target event filtering without exposing any semantic state.
    pub(super) fn accepts_event(&self, event: WidgetEventAbi, bytes: NcCopyFromBufferOffset<'_, u8>) -> Result<bool, NcStatus> {
        self.state.readable()?;
        let event = super::input::local_event(event, bytes)?;
        Ok(match event {
            Some(event) => self.core.accepts_event(&event),
            None => false,
        })
    }
}

/// Converts one checked contract axis without treating finite zero as unbounded.
pub(super) fn available(axis: AxisLimit) -> Result<AvailableSpace, NcStatus> {
    axis.validate()?;
    Ok(if axis.kind == LimitKind::FINITE {
        AvailableSpace::Bounded(axis.extent)
    } else {
        AvailableSpace::Unbounded
    })
}

/// One reused local display list and the ordinary UI geometry executor.
struct PaintScratch {
    /// Owned semantic commands cleared after each built-in invocation.
    list: DisplayList,
    /// The same text/nine-patch/clipping implementation used by retained painting.
    renderer: Renderer<super::render::Backend>,
    /// Final local atlas geometry produced by the collector.
    storage: Rc<super::render::Storage>,
}

/// Expands the identical COM lifecycle/phase plumbing for one concrete class.
/// Specialized editing implementations remain ordinary explicit Rust impl blocks.
macro_rules! widget_phase_impl {
    ($name:ident) => {
        impl IWidget for $name {
            /// Returns the runtime authority without retaining a tree or parent.
            fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
                Ok(self.widget.state.host().add_ref())
            }
            /// Claims the only attachment after runtime validation succeeds.
            fn mount(&mut self) -> Result<(), NcStatus> {
                self.widget.state.mount()
            }
            /// Closes semantic capabilities and the class's concrete event sources.
            fn close(&mut self) -> Result<(), NcStatus> {
                self.close_widget()
            }
            /// Copies policy from the same state used by update and painting.
            fn properties(&self) -> Result<WidgetProperties, NcStatus> {
                self.widget.properties()
            }
            /// Commits validated common and semantic presentation options together.
            fn set_options(&mut self, options: WidgetOptions) -> Result<(), NcStatus> {
                self.widget.set_options(options)
            }
            /// Reuses the concrete control's full pointer/key/text update algorithm.
            fn update(&mut self, state: WidgetUpdate, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
                self.update_widget(state, event, text)
            }
            /// Drains the sticky setter marker without erasing unconsumed changes.
            fn take_change(&mut self) -> Result<GeometryChange, NcStatus> {
                self.widget.take_change()
            }
            /// Records complete owned geometry before calling a foreign painter.
            fn paint(&mut self, scope: PaintScope, state: WidgetPaint, painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
                self.widget.paint(scope, state, painter)
            }
        }
    };
}

/// Adds only leaf geometry; common widget lifecycle and phases are shared with containers.
macro_rules! leaf_layout_impl {
    ($name:ident) => {
        impl ILeafLayout for $name {
            /// Measures with the resource revision owned by this runtime-bound class.
            fn measure(&mut self, bounds: LayoutLimits, _metrics: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
                self.widget.measure(bounds)
            }
        }
    };
}

/// Implements the shared COM lifecycle and update/paint phases for one concrete widget type.
///
/// The generated methods access only the concrete object's [`WidgetRuntime`] field through
/// [`HostedWidget`]. Semantic update and paint execute directly on `$name`; there is no contained
/// algorithm object, erased payload, or secondary widget identity.
macro_rules! direct_widget_phase_impl {
    ($name:ty) => {
        impl IWidget for $name {
            /// Returns the runtime authority without retaining a tree or parent.
            fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
                let runtime = <$name as super::widget::HostedWidget>::runtime(self);
                Ok(runtime.state.host().add_ref())
            }

            /// Claims this concrete allocation's only attachment after host validation.
            fn mount(&mut self) -> Result<(), NcStatus> {
                <$name as super::widget::HostedWidget>::runtime_mut(self).state.mount()
            }

            /// Closes the concrete widget's event sources exactly once.
            fn close(&mut self) -> Result<(), NcStatus> {
                let closed = <$name as super::widget::HostedWidget>::runtime_mut(self).state.close()?;
                if closed {
                    <$name as super::widget::HostedWidget>::close_contents(self)?;
                }
                Ok(())
            }

            /// Copies routing and frame policy from the concrete widget and its common options.
            fn properties(&self) -> Result<WidgetProperties, NcStatus> {
                let runtime = <$name as super::widget::HostedWidget>::runtime(self);
                let options = runtime.state.options()?;
                Ok(WidgetProperties {
                    options: WidgetOptions {
                        bits: <$name as super::widget::WidgetAlgorithm>::effective_options(
                            self,
                            crate::WidgetOption::from_bits_retain(options.bits),
                        )
                        .bits(),
                    },
                    keyboard: crate::Widget::keyboard_behavior(self).bits(),
                    frame: super::node::frame_contract(crate::Widget::frame_appearance_role(self)),
                })
            }

            /// Replaces the single common option value after complete validation.
            fn set_options(&mut self, options: WidgetOptions) -> Result<(), NcStatus> {
                options.validate()?;
                <$name as super::widget::HostedWidget>::runtime_mut(self).state.set_options(options)
            }

            /// Runs copied input directly against this concrete widget's semantic fields.
            fn update(
                &mut self,
                state: WidgetUpdate,
                event: WidgetEventAbi,
                text: NcCopyFromBufferOffset<'_, u8>,
            ) -> Result<GeometryChange, NcStatus> {
                <$name as super::widget::HostedWidget>::runtime(self).state.writable()?;
                state.validate()?;
                let input = super::input::local_event(event, text)?;
                let assets = <$name as super::widget::HostedWidget>::runtime_mut(self).resources()?;
                let mut ctx = crate::WidgetUpdateCtx::new_with_content_geometry(
                    local_rect(state.screen),
                    crate::math::RectExt::translated(
                        local_rect(state.clip),
                        crate::Vec2i::new(state.screen.x, state.screen.y),
                    ),
                    assets.bundle().skin(),
                    assets.bundle().atlas(),
                    state.in_hover_root,
                    state.hovered,
                    state.focused,
                    state.clicked,
                    state.active,
                    crate::MouseButton::from_bits(u32::from(state.mouse_buttons)).ok_or(NC_E_INVALID_ARG)?,
                    crate::Modifiers::from_bits(state.modifiers).ok_or(NC_E_INVALID_ARG)?,
                );
                Ok(match crate::Widget::update(self, &mut ctx, input.as_ref()) {
                    crate::Change::Changed => GeometryChange::CHANGED,
                    crate::Change::NoChange => GeometryChange::NO_CHANGE,
                })
            }

            /// Drains sticky common and explicitly shared semantic geometry changes together.
            fn take_change(&mut self) -> Result<GeometryChange, NcStatus> {
                <$name as super::widget::HostedWidget>::runtime(self).state.readable()?;
                if crate::Widget::take_shared_change(self) {
                    <$name as super::widget::HostedWidget>::runtime_mut(self)
                        .state
                        .record_change(GeometryChange::CHANGED);
                }
                <$name as super::widget::HostedWidget>::runtime_mut(self).state.take_change()
            }

            /// Paints directly from semantic fields, then submits the resulting local display list.
            fn paint(
                &mut self,
                scope: PaintScope,
                state: WidgetPaint,
                painter: ComRef<'_, IPainterAbi>,
            ) -> Result<(), NcStatus> {
                <$name as super::widget::HostedWidget>::runtime(self).state.readable()?;
                state.validate()?;
                let options = <$name as super::widget::HostedWidget>::runtime(self).state.options()?;
                let assets = <$name as super::widget::HostedWidget>::runtime_mut(self).resources()?;
                // The display list is phase-local so semantic painting can borrow the complete
                // concrete object immutably without an interior-mutable scratch field.
                let mut list = crate::render::DisplayList::new();
                let mut ctx = crate::WidgetPaintCtx::new_with_content_geometry(
                    local_rect(state.rect),
                    &mut list,
                    local_rect(state.clip),
                    assets.bundle().skin(),
                    assets.bundle().atlas(),
                    state.enabled,
                    state.hovered,
                    state.focused,
                    state.clicked,
                    state.active,
                    state.window_active,
                    crate::WidgetOption::from_bits_retain(options.bits),
                );
                crate::Widget::paint(self, &mut ctx);
                let size = crate::Dimensioni::new(state.rect.width.max(1), state.rect.height.max(1));
                <$name as super::widget::HostedWidget>::runtime_mut(self).render(scope, size, &mut list, painter)
            }
        }
    };
}

/// Implements intrinsic COM measurement directly on one concrete leaf widget.
macro_rules! direct_leaf_layout_impl {
    ($name:ty) => {
        impl ILeafLayout for $name {
            /// Measures the concrete semantic fields against one owned host-resource snapshot.
            fn measure(&mut self, bounds: LayoutLimits, _metrics: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
                <$name as super::widget::HostedWidget>::runtime(self).state.readable()?;
                let constraints = crate::Constraints::new(
                    super::widget::available(bounds.width)?,
                    super::widget::available(bounds.height)?,
                );
                let assets = <$name as super::widget::HostedWidget>::runtime_mut(self).resources()?;
                let size = crate::LeafWidget::measure(
                    self,
                    assets.bundle().skin(),
                    assets.bundle().atlas(),
                    constraints,
                );
                Ok(Size {
                    width: size.width.max(0),
                    height: size.height.max(0),
                })
            }
        }
    };
}

/// Declares one non-generic COM allocation with a statically known semantic state.
macro_rules! leaf_object {
    ($visibility:vis $name:ident, $core:ty, $($object:tt)+) => {
        #[com_object($($object)+)]
        /// One concrete built-in object, including its inherited widget state.
        $visibility struct $name {
            /// Inline semantic state and shared phase execution; no secondary widget owner.
            widget: super::widget::WidgetCore<$core>,
        }
        impl $name {
            /// Applies the complete existing semantic algorithm to copied input.
            fn update_widget(&mut self, state: WidgetUpdate, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
                self.widget.update(state, event, text)
            }
            /// Commits logical closure before final COM destruction.
            fn close_widget(&mut self) -> Result<(), NcStatus> { self.widget.close() }
        }
        widget_phase_impl!($name);
        leaf_layout_impl!($name);
    };
}

/// Preserves interface-only leaf allocations until their callable class family is migrated.
macro_rules! leaf_class {
    ($name:ident, $core:ty, $interface:ident, $abi:ty $(, $extra:ident)*) => {
        leaf_object!(pub(super) $name, $core, interfaces($interface, ILeafLayout $(, $extra)*));
        impl $name {
            /// Creates the unmounted interface-only allocation after input validation.
            pub(super) fn create(host: ComPtr<IWidgetHostAbi>, options: crate::WidgetOption, core: $core) -> Result<ComPtr<$abi>, NcStatus> {
                Ok(Self::from_value(Self {
                    widget: super::widget::WidgetCore::new(host, options, core)?,
                })
                .query()
                .expect("object declares its concrete editing interface"))
            }
        }
    };
}

/// Static typography access implemented only by semantic states which own a font.
pub(crate) trait FontState: WidgetAlgorithm {
    /// Borrows the actual stable font selection used by measurement and paint.
    fn font(&self) -> &crate::FontRef;
    /// Replaces that same selection after the resource owner validates it.
    fn set_font(&mut self, font: crate::FontRef);
}

impl<S: FontState> WidgetCore<S> {
    /// Resolves the finite semantic font vocabulary before committing any change.
    pub(super) fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus> {
        self.state.writable()?;
        let role = local_font_role(role)?;
        self.commit_font(crate::FontRef::Role(role))
    }
    /// Resolves a font once against the exact bundle, never by name per glyph.
    pub(super) fn set_named_font(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.state.writable()?;
        let name = text(input)?;
        let resources = self.resources.current(self.state.host())?;
        let font = resources.assets.original().font_id(name).ok_or(nc_com::NC_E_NOT_FOUND)?;
        if *self.core.font() != crate::FontRef::Named(font) {
            self.core.set_font(crate::FontRef::Named(font));
            self.state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Commits an already validated selection and preserves pending setter changes.
    fn commit_font(&mut self, font: crate::FontRef) -> Result<(), NcStatus> {
        if *self.core.font() != font {
            self.core.set_font(font);
            self.state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
}

/// Exposes shared typography on the same COM allocation as a concrete text control.
macro_rules! font_style_impl {
    ($name:ident) => {
        impl IFontStyle for $name {
            /// Validates a semantic role before changing the control's font selection.
            fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus> {
                self.widget.set_font_role(role)
            }
            /// Resolves and retains a stable named font identity at the setter boundary.
            fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
                self.widget.set_named_font(name)
            }
        }
    };
}

/// Static access to the adjacent visual stored by a button or list row.
pub(crate) trait IconState: WidgetAlgorithm {
    /// Replaces the visual while retaining label/font state; reports an actual visual change.
    fn set_icon(&mut self, icon: Option<crate::IconRef>) -> bool;
}

/// Shares only scoped container geometry; fixed composites need not expose mutable generic topology.
macro_rules! container_layout_impl {
    ($name:ident) => {
        impl IContainerLayout for $name {
            /// Returns the same lifetime-stable read-only identity as the editing base.
            fn children(&self) -> Result<ComPtr<IChildrenAbi>, NcStatus> {
                self.widget.state.readable()?;
                self.editor.view()
            }
            /// Runs the existing measurement algorithm through owned scoped child access.
            fn measure(&mut self, scope: LayoutScope, bounds: LayoutLimits, layout: ComRef<'_, ILayoutServiceAbi>) -> Result<Size, NcStatus> {
                self.widget.measure_container(scope, bounds, layout)
            }
            /// Publishes only copied geometry after the complete concrete placement algorithm succeeds.
            fn place(&mut self, scope: LayoutScope, rect: Rect, layout: ComRef<'_, ILayoutServiceAbi>) -> Result<ContainerGeometry, NcStatus> {
                self.widget.place_container(scope, rect, layout)
            }
            /// Retains each concrete container's state-dependent event policy.
            fn accepts_event(&self, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<bool, NcStatus> {
                self.widget.accepts_event(event, text)
            }
        }
    };
}

/// Common phase methods for fixed composites retain one editor without exposing generic topology edits.
macro_rules! fixed_phases {
    ($name:ident) => {
        impl $name {
            /// Rejects active-state reentry before logically closing this widget and its authoritative child store.
            fn close_widget(&mut self) -> Result<(), NcStatus> {
                if self.widget.state.close()? {
                    self.editor.close()?.finish()?;
                }
                Ok(())
            }
            /// Reuses the complete concrete update algorithm with copied input and owned state.
            fn update_widget(&mut self, state: WidgetUpdate, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
                self.widget.update(state, event, text)
            }
        }
        widget_phase_impl!($name);
        container_layout_impl!($name);
    };
}

/// Shares whole-text snapshots/replacement while retaining each control's line policy.
macro_rules! text_content {
    ($name:ident, $get:ident, $set:ident) => {
        impl ITextContent for $name {
            /// Returns an immutable snapshot without exposing the semantic String allocation.
            fn text(&self) -> Result<ComPtr<nc_com::INcStringAbi>, NcStatus> {
                self.widget.state.readable()?;
                Ok(nc_com::NcString::from_utf8_str(self.widget.core.$get()))
            }
            /// Applies the existing normalization/cursor policy, then compares committed text.
            fn set_text(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
                self.widget.state.writable()?;
                let input = text(input)?;
                // Compare normalized output, not raw CR/LF input. An editor may
                // still reset its cursor on a text-identical replacement.
                let previous = self.widget.core.$get().to_owned();
                self.widget.core.$set(input);
                if self.widget.core.$get() != previous {
                    self.widget.state.record_change(GeometryChange::CHANGED);
                }
                Ok(())
            }
        }
    };
}

/// Shares scrolling access for concrete composites while retaining their separate content-editing authority.
macro_rules! scrolling_impl {
    ($name:ident) => {
        impl IScrolling for $name {
            /// Reads actual axis requests and one shared committed viewport summary.
            fn info(&self) -> Result<ScrollInfo, NcStatus> {
                self.widget.state.readable()?;
                Ok(self.widget.core.com_info())
            }
            /// Records a sticky shared request so hidden/skipped updates cannot lose the needed placement.
            fn set_offset(&mut self, offset: Point) -> Result<(), NcStatus> {
                self.widget.state.writable()?;
                self.widget.core.set_offset(crate::Vec2i::new(offset.x, offset.y));
                Ok(())
            }
            /// Disabling clears ranges and requests synchronously; the next layout gates structural children.
            fn set_enabled(&mut self, enabled: bool) -> Result<(), NcStatus> {
                self.widget.state.writable()?;
                self.widget.core.set_scrolling_enabled(enabled);
                Ok(())
            }
            /// Preserves an above-old-range end request until fresh content geometry is measured.
            fn scroll_to_end(&mut self) -> Result<(), NcStatus> {
                self.widget.state.writable()?;
                self.widget.core.scroll_to_end();
                Ok(())
            }
            /// Validates the complete reveal rectangle before changing either shared axis request.
            fn reveal(&mut self, rect: Rect) -> Result<(), NcStatus> {
                self.widget.state.writable()?;
                if rect.x < 0 || rect.y < 0 || rect.width < 0 || rect.height < 0 {
                    return Err(NC_E_INVALID_ARG);
                }
                self.widget.core.scroll_rect_into_view(local_rect(rect));
                Ok(())
            }
        }
    };
}

/// Exposes the two existing independent editor ports on one concrete COM editing object.
macro_rules! text_subscription_impl {
    ($name:ident, $changed:ty, $submitted:ty) => {
        impl ITextEvents for $name {
            /// Returns the exclusive changed lease without retaining the callback on the producer.
            fn subscribe_changed(&mut self, sink: ComRef<'_, ITextChangedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
                self.widget.subscribe::<$changed>(sink)
            }
            /// Returns the independent submitted lease through the same typed queue transport.
            fn subscribe_submitted(&mut self, sink: ComRef<'_, ITextSubmittedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
                self.widget.subscribe::<$submitted>(sink)
            }
        }
    };
}

/// Resolves one concrete font-role vocabulary for both widget and compact menu presentation.
pub(super) fn local_font_role(role: TextFontRole) -> Result<crate::FontRole, NcStatus> {
    match role {
        TextFontRole::BODY => Ok(crate::FontRole::Body),
        TextFontRole::SMALL => Ok(crate::FontRole::Small),
        TextFontRole::TITLE => Ok(crate::FontRole::Title),
        TextFontRole::HEADING => Ok(crate::FontRole::Heading),
        TextFontRole::MONO => Ok(crate::FontRole::Mono),
        _ => Err(NC_E_INVALID_ARG),
    }
}

/// Converts the finite public content-icon vocabulary into concrete semantic state.
pub(super) fn local_content_icon(icon: ContentIcon) -> Result<Option<crate::IconRef>, NcStatus> {
    let role = match icon {
        ContentIcon::NONE => return Ok(None),
        ContentIcon::CLOSE => crate::IconRole::Close,
        ContentIcon::EXPAND => crate::IconRole::Expand,
        ContentIcon::COLLAPSE => crate::IconRole::Collapse,
        ContentIcon::CHECK => crate::IconRole::Check,
        ContentIcon::EXPAND_DOWN => crate::IconRole::ExpandDown,
        ContentIcon::OPEN_FOLDER => crate::IconRole::OpenFolder,
        ContentIcon::CLOSED_FOLDER => crate::IconRole::ClosedFolder,
        ContentIcon::FILE => crate::IconRole::File,
        _ => return Err(NC_E_INVALID_ARG),
    };
    Ok(Some(crate::IconRef::Role(role)))
}

/// Private shared editing hook over each algorithm's authoritative concrete image content.
pub(crate) trait ImageState {
    /// Receives only a validated owned image, with no caller buffer or native identifier.
    fn set_image(&mut self, image: Option<microui_redux::Image>, fit_width: bool) -> bool;
}
