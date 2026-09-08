//! Shared phase execution for concrete built-in COM classes.
//!
//! Concrete widget types own their semantic fields directly. [`WidgetRuntime`] contains only the
//! common host, lifecycle, option, resource, and rendering machinery required by every class.
use super::*;
use microui_contracts::widget::WidgetState;
use crate::{AvailableSpace, WidgetOption};
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

/// One current imported asset value shared by the runtime services that observe installation.
pub(super) type SharedAssets = Rc<RefCell<crate::assets::ImportedAssets>>;

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

/// Converts one checked contract axis without treating finite zero as unbounded.
pub(super) fn available(axis: AxisLimit) -> Result<AvailableSpace, NcStatus> {
    axis.validate()?;
    Ok(if axis.kind == LimitKind::FINITE {
        AvailableSpace::Bounded(axis.extent)
    } else {
        AvailableSpace::Unbounded
    })
}

/// Reusable local rendering backend for one independently constructible widget.
struct PaintScratch {
    /// The same text/nine-patch/clipping implementation used by retained painting.
    renderer: Renderer<super::render::Backend>,
    /// Final local atlas geometry produced by the collector.
    storage: Rc<super::render::Storage>,
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

/// Implements scoped layout directly on a concrete container allocation.
macro_rules! direct_container_layout_impl {
    ($name:ty) => {
        impl IContainerLayout for $name {
            /// Returns the read-only view paired with this concrete container's private writer.
            fn children(&self) -> Result<ComPtr<IChildrenAbi>, NcStatus> {
                <$name as super::widget::HostedWidget>::runtime(self).state.readable()?;
                self.editor.view()
            }

            /// Measures the concrete container against the caller's scoped child service.
            fn measure(
                &mut self,
                scope: LayoutScope,
                bounds: LayoutLimits,
                service: ComRef<'_, ILayoutServiceAbi>,
            ) -> Result<Size, NcStatus> {
                <$name as super::widget::HostedWidget>::runtime(self).state.readable()?;
                let constraints = super::layout::constraints(bounds)?;
                let layout = super::layout::ScopedLayout::new(scope, service)?;
                let assets = <$name as super::widget::HostedWidget>::runtime_mut(self).resources()?;
                let mut ctx = crate::MeasureCtx::scoped(assets.bundle().skin(), assets.bundle().atlas(), layout);
                let size = crate::ContainerWidget::measure(self, &mut ctx, constraints);
                ctx.result()?;
                Ok(Size {
                    width: size.width.max(0),
                    height: size.height.max(0),
                })
            }

            /// Places the concrete container and returns only its copied child geometry.
            fn place(
                &mut self,
                scope: LayoutScope,
                rect: Rect,
                service: ComRef<'_, ILayoutServiceAbi>,
            ) -> Result<ContainerGeometry, NcStatus> {
                <$name as super::widget::HostedWidget>::runtime(self).state.readable()?;
                if rect.width < 0 || rect.height < 0 {
                    return Err(NC_E_INVALID_ARG);
                }
                let layout = super::layout::ScopedLayout::new(scope, service)?;
                let assets = <$name as super::widget::HostedWidget>::runtime_mut(self).resources()?;
                let mut ctx = crate::ContainerLayoutCtx::scoped(
                    assets.bundle().skin(),
                    assets.bundle().atlas(),
                    local_rect(rect),
                    layout,
                );
                crate::ContainerWidget::place(self, &mut ctx, local_rect(rect));
                ctx.result()
            }

            /// Applies the concrete container's post-target event filtering policy.
            fn accepts_event(
                &self,
                event: WidgetEventAbi,
                text: NcCopyFromBufferOffset<'_, u8>,
            ) -> Result<bool, NcStatus> {
                <$name as super::widget::HostedWidget>::runtime(self).state.readable()?;
                let event = super::input::local_event(event, text)?;
                Ok(event.is_some_and(|event| crate::ContainerWidget::accepts_event(self, &event)))
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

/// Static access to the adjacent visual stored by a button or list row.
pub(crate) trait IconState: WidgetAlgorithm {
    /// Replaces the visual while retaining label/font state; reports an actual visual change.
    fn set_icon(&mut self, icon: Option<crate::IconRef>) -> bool;
}

/// Implements shared scrolling calls directly on a concrete fixed-composition class.
macro_rules! direct_scrolling_impl {
    ($name:ty) => {
        impl IScrolling for $name {
            /// Reads one coherent snapshot from the scalar model shared with both scrollbar children.
            fn info(&self) -> Result<ScrollInfo, NcStatus> {
                <$name as super::widget::HostedWidget>::runtime(self).state.readable()?;
                Ok(self.model.com_info())
            }

            /// Records a sticky nonnegative request for the next concrete placement.
            fn set_offset(&mut self, offset: Point) -> Result<(), NcStatus> {
                <$name as super::widget::HostedWidget>::runtime(self).state.writable()?;
                self.model.set_offset(crate::Vec2i::new(offset.x, offset.y));
                Ok(())
            }

            /// Changes wheel and scrollbar participation through the shared scalar model.
            fn set_enabled(&mut self, enabled: bool) -> Result<(), NcStatus> {
                <$name as super::widget::HostedWidget>::runtime(self).state.writable()?;
                self.model.set_enabled(enabled);
                Ok(())
            }

            /// Preserves an end request until placement installs the freshly measured range.
            fn scroll_to_end(&mut self) -> Result<(), NcStatus> {
                <$name as super::widget::HostedWidget>::runtime(self).state.writable()?;
                self.model.scroll_to_end();
                Ok(())
            }

            /// Validates one content-local rectangle before changing either requested axis.
            fn reveal(&mut self, rect: Rect) -> Result<(), NcStatus> {
                <$name as super::widget::HostedWidget>::runtime(self).state.writable()?;
                if rect.x < 0 || rect.y < 0 || rect.width < 0 || rect.height < 0 {
                    return Err(NC_E_INVALID_ARG);
                }
                self.model.reveal(local_rect(rect));
                Ok(())
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
