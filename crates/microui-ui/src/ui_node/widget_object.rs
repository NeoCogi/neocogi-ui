//! Shared phase execution for concrete built-in COM classes.
//!
//! Concrete widget types own their semantic fields directly. [`WidgetRuntime`] contains only the
//! common host, lifecycle, option, and resource machinery required by every class.
use crate::boundary::text;
use microui_contracts::*;
use nc_com::{ComPtr, NcCopyFromBufferOffset, NcStatus, NC_E_INVALID_ARG};
use std::{cell::RefCell, rc::Rc};
use microui_contracts::widget::WidgetState;
use crate::{AvailableSpace, WidgetOption};

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
pub(crate) type SharedAssets = Rc<RefCell<crate::assets::ImportedAssets>>;

/// Common non-semantic state embedded directly in every concrete widget COM allocation.
///
/// This helper deliberately has no type parameter and cannot contain a widget implementation. It
/// owns only cross-cutting COM lifecycle and host-backed rendering resources. Concrete structs such
/// as `Button` keep their label, font, interaction state, and event sources as ordinary fields.
pub(crate) struct WidgetRuntime {
    /// Shared logical lifetime, attachment state, options, and sticky geometry invalidation.
    pub(crate) state: WidgetState,
    /// The one imported asset value used by this widget's semantic phases.
    resources: SharedAssets,
    /// Direct class activation polls its foreign host; runtime-factory products observe shared publication.
    refresh_resources: bool,
    /// Keeps the real host runtime alive for native-only unit-test fixtures.
    #[cfg(test)]
    test_owner: Option<ComPtr<UiRuntimeAbi>>,
}

impl WidgetRuntime {
    /// Binds a newly constructed concrete widget to its host without storing semantic state.
    pub(crate) fn new(host: ComPtr<IWidgetHostAbi>, options: WidgetOption) -> Result<Self, NcStatus> {
        host.check_mutation()?;
        let source = host.assets()?;
        let resources = Rc::new(RefCell::new(crate::assets::ImportedAssets::read(source.as_com_ref(), None)?));
        Ok(Self {
            state: WidgetState::new(host, WidgetOptions { bits: options.bits() }),
            resources,
            refresh_resources: true,
            #[cfg(test)]
            test_owner: None,
        })
    }

    /// Binds a factory product directly to the runtime's already imported asset value.
    pub(crate) fn from_shared(host: ComPtr<IWidgetHostAbi>, options: WidgetOption, resources: SharedAssets) -> Result<Self, NcStatus> {
        host.check_mutation()?;
        Ok(Self {
            state: WidgetState::new(host, WidgetOptions { bits: options.bits() }),
            resources,
            refresh_resources: false,
            #[cfg(test)]
            test_owner: None,
        })
    }

    /// Selects host-owned import or an explicitly shared runtime value for composite construction.
    pub(crate) fn for_resources(
        host: ComPtr<IWidgetHostAbi>,
        options: WidgetOption,
        resources: Option<&SharedAssets>,
    ) -> Result<Self, NcStatus> {
        match resources {
            Some(resources) => Self::from_shared(host, options, resources.clone()),
            None => Self::new(host, options),
        }
    }

    /// Creates a fully valid host-bound runtime for the native algorithm test harness.
    #[cfg(test)]
    pub(crate) fn for_test(options: WidgetOption) -> Self {
        // Native tests exercise the same concrete structs without mounting them through COM. A real
        // runtime supplies their host and assets, while this test-only owner keeps its weak factory
        // reachable for the complete lifetime of the widget value.
        let owner = crate::runtime::create_runtime(crate::test_support::test_atlas());
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

    /// Returns the widget's single shared value after refreshing an independently activated class.
    pub(crate) fn resources(&mut self) -> Result<SharedAssets, NcStatus> {
        if self.refresh_resources {
            let source = self.state.host().assets()?;
            if self.resources.borrow().source().as_ptr() != source.as_ptr() {
                let replacement = {
                    let current = self.resources.borrow();
                    crate::assets::ImportedAssets::read(source.as_com_ref(), Some(&current))?
                };
                *self.resources.borrow_mut() = replacement;
            }
        }
        Ok(self.resources.clone())
    }

    /// Resolves one UTF-8 font name against this widget's exact current resource revision.
    pub(crate) fn named_font(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<crate::FontRef, NcStatus> {
        self.state.writable()?;
        let name = text(input)?;
        let assets = self.resources()?;
        let font = assets.borrow().original().font_id(name).ok_or(nc_com::NC_E_NOT_FOUND)?;
        Ok(crate::FontRef::Named(font))
    }

    /// Resolves one UTF-8 icon name against this widget's exact current resource revision.
    pub(crate) fn named_icon(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<crate::IconRef, NcStatus> {
        self.state.writable()?;
        let name = text(input)?;
        let assets = self.resources()?;
        let icon = assets.borrow().original().icon_id(name).ok_or(nc_com::NC_E_NOT_FOUND)?;
        Ok(crate::IconRef::Named(icon))
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
pub(crate) fn set_direct_font_role<S: HostedWidget + FontState>(widget: &mut S, role: TextFontRole) -> Result<(), NcStatus> {
    widget.runtime().state.writable()?;
    let font = crate::FontRef::Role(local_font_role(role)?);
    if widget.font() != &font {
        widget.set_font(font);
        widget.runtime_mut().state.record_change(GeometryChange::CHANGED);
    }
    Ok(())
}

/// Resolves and commits one named font against the concrete widget's current host resources.
pub(crate) fn set_direct_named_font<S: HostedWidget + FontState>(
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
pub(crate) fn set_direct_icon_role<S: HostedWidget + IconState>(widget: &mut S, icon: ContentIcon) -> Result<(), NcStatus> {
    widget.runtime().state.writable()?;
    if widget.set_icon(local_content_icon(icon)?) {
        widget.runtime_mut().state.record_change(GeometryChange::CHANGED);
    }
    Ok(())
}

/// Resolves and commits one named icon against the concrete widget's current host resources.
pub(crate) fn set_direct_named_icon<S: HostedWidget + IconState>(
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
pub(crate) fn available(axis: AxisLimit) -> Result<AvailableSpace, NcStatus> {
    axis.validate()?;
    Ok(if axis.kind == LimitKind::FINITE {
        AvailableSpace::Bounded(axis.extent)
    } else {
        AvailableSpace::Unbounded
    })
}

/// Converts the callable text-wrapping vocabulary into the concrete layout policy.
pub(crate) fn local_wrap(wrap: TextWrapping) -> Result<crate::TextWrap, NcStatus> {
    match wrap {
        TextWrapping::NONE => Ok(crate::TextWrap::None),
        TextWrapping::WORD => Ok(crate::TextWrap::Word),
        _ => Err(nc_com::NC_E_INVALID_ARG),
    }
}

/// Implements the shared COM lifecycle and update/paint phases for one concrete widget type.
///
/// The generated methods access only the concrete object's [`WidgetRuntime`] field through
/// [`HostedWidget`]. Semantic update and paint execute directly on `$name`; there is no contained
/// algorithm object, erased payload, or secondary widget identity.
macro_rules! direct_widget_phase_impl {
    ($name:ty) => {
        impl microui_contracts::IWidget for $name {
            /// Returns the runtime authority without retaining a tree or parent.
            fn host(&self) -> Result<nc_com::ComPtr<microui_contracts::IWidgetHostAbi>, nc_com::NcStatus> {
                let runtime = <$name as crate::ui_node::widget_object::HostedWidget>::runtime(self);
                Ok(runtime.state.host().add_ref())
            }

            /// Claims this concrete allocation's only attachment after host validation.
            fn mount(&mut self) -> Result<(), nc_com::NcStatus> {
                <$name as crate::ui_node::widget_object::HostedWidget>::runtime_mut(self).state.mount()
            }

            /// Closes the concrete widget's event sources exactly once.
            fn close(&mut self) -> Result<(), nc_com::NcStatus> {
                let closed = <$name as crate::ui_node::widget_object::HostedWidget>::runtime_mut(self).state.close()?;
                if closed {
                    <$name as crate::ui_node::widget_object::HostedWidget>::close_contents(self)?;
                }
                Ok(())
            }

            /// Copies routing and frame policy from the concrete widget and its common options.
            fn properties(&self) -> Result<microui_contracts::WidgetProperties, nc_com::NcStatus> {
                let runtime = <$name as crate::ui_node::widget_object::HostedWidget>::runtime(self);
                let options = runtime.state.options()?;
                Ok(microui_contracts::WidgetProperties {
                    options: microui_contracts::WidgetOptions {
                        bits: <$name as crate::ui_node::widget_object::WidgetAlgorithm>::effective_options(
                            self,
                            crate::WidgetOption::from_bits_retain(options.bits),
                        )
                        .bits(),
                    },
                    keyboard: crate::Widget::keyboard_behavior(self).bits(),
                    frame: crate::ui_node::frame_contract(crate::Widget::frame_appearance_role(self)),
                })
            }

            /// Replaces the single common option value after complete validation.
            fn set_options(&mut self, options: microui_contracts::WidgetOptions) -> Result<(), nc_com::NcStatus> {
                options.validate()?;
                <$name as crate::ui_node::widget_object::HostedWidget>::runtime_mut(self).state.set_options(options)
            }

            /// Runs copied input directly against this concrete widget's semantic fields.
            fn update(
                &mut self,
                state: microui_contracts::WidgetUpdate,
                event: microui_contracts::WidgetEventAbi,
                text: nc_com::NcCopyFromBufferOffset<'_, u8>,
            ) -> Result<microui_contracts::GeometryChange, nc_com::NcStatus> {
                <$name as crate::ui_node::widget_object::HostedWidget>::runtime(self).state.writable()?;
                state.validate()?;
                let input = crate::ui_node::input_contract::local_event(event, text)?;
                let assets = <$name as crate::ui_node::widget_object::HostedWidget>::runtime_mut(self).resources()?;
                let assets = assets.borrow();
                let mut ctx = crate::WidgetUpdateCtx::new_with_content_geometry(
                    crate::boundary::local_rect(state.screen),
                    crate::math::RectExt::translated(
                        crate::boundary::local_rect(state.clip),
                        crate::Vec2i::new(state.screen.x, state.screen.y),
                    ),
                    assets.bundle().skin(),
                    assets.bundle().atlas(),
                    state.in_hover_root,
                    state.hovered,
                    state.focused,
                    state.clicked,
                    state.active,
                    crate::MouseButton::from_bits(u32::from(state.mouse_buttons)).ok_or(nc_com::NC_E_INVALID_ARG)?,
                    crate::Modifiers::from_bits(state.modifiers).ok_or(nc_com::NC_E_INVALID_ARG)?,
                );
                Ok(match crate::Widget::update(self, &mut ctx, input.as_ref()) {
                    crate::Change::Changed => microui_contracts::GeometryChange::CHANGED,
                    crate::Change::NoChange => microui_contracts::GeometryChange::NO_CHANGE,
                })
            }

            /// Drains sticky common and explicitly shared semantic geometry changes together.
            fn take_change(&mut self) -> Result<microui_contracts::GeometryChange, nc_com::NcStatus> {
                if crate::Widget::take_shared_change(self) {
                    <$name as crate::ui_node::widget_object::HostedWidget>::runtime_mut(self)
                        .state
                        .record_change(microui_contracts::GeometryChange::CHANGED);
                }
                <$name as crate::ui_node::widget_object::HostedWidget>::runtime_mut(self).state.take_change()
            }

            /// Paints semantic fields directly into the runtime-owned scoped frame context.
            fn paint(
                &mut self,
                scope: microui_contracts::PaintScope,
                state: microui_contracts::WidgetPaint,
                painter: nc_com::ComRef<'_, microui_contracts::IPainterAbi>,
            ) -> Result<(), nc_com::NcStatus> {
                state.validate()?;
                // Reading options performs the one lifecycle check required by
                // the complete paint invocation. Repeating it before the same
                // immutable state read would cross the host COM boundary twice.
                let options = <$name as crate::ui_node::widget_object::HostedWidget>::runtime(self).state.options()?;
                let assets = <$name as crate::ui_node::widget_object::HostedWidget>::runtime_mut(self).resources()?;
                let assets = assets.borrow();
                let mut ctx = crate::WidgetPaintCtx::new_scoped(
                    crate::boundary::local_rect(state.rect),
                    crate::boundary::local_rect(state.clip),
                    assets.bundle().skin(),
                    assets.bundle().atlas(),
                    state.enabled,
                    state.hovered,
                    state.focused,
                    state.clicked,
                    state.active,
                    state.window_active,
                    crate::WidgetOption::from_bits_retain(options.bits),
                    scope,
                    painter,
                );
                crate::Widget::paint(self, &mut ctx);
                ctx.finish()
            }
        }
    };
}

pub(crate) use direct_widget_phase_impl;

/// Implements intrinsic COM measurement directly on one concrete leaf widget.
macro_rules! direct_leaf_layout_impl {
    ($name:ty) => {
        impl microui_contracts::ILeafLayout for $name {
            /// Measures the concrete semantic fields against one owned host-resource snapshot.
            fn measure(
                &mut self,
                bounds: microui_contracts::LayoutLimits,
                _metrics: nc_com::ComRef<'_, microui_contracts::ITextMetricsAbi>,
            ) -> Result<microui_contracts::Size, nc_com::NcStatus> {
                <$name as crate::ui_node::widget_object::HostedWidget>::runtime(self).state.readable()?;
                let constraints = crate::Constraints::new(
                    crate::ui_node::widget_object::available(bounds.width)?,
                    crate::ui_node::widget_object::available(bounds.height)?,
                );
                let assets = <$name as crate::ui_node::widget_object::HostedWidget>::runtime_mut(self).resources()?;
                let assets = assets.borrow();
                let size = crate::LeafWidget::measure(
                    self,
                    assets.bundle().skin(),
                    assets.bundle().atlas(),
                    constraints,
                );
                Ok(microui_contracts::Size {
                    width: size.width.max(0),
                    height: size.height.max(0),
                })
            }
        }
    };
}

pub(crate) use direct_leaf_layout_impl;

/// Implements scoped layout directly on a concrete container allocation.
macro_rules! direct_container_layout_impl {
    ($name:ty) => {
        impl microui_contracts::IContainerLayout for $name {
            /// Returns the read-only view paired with this concrete container's private writer.
            fn children(&self) -> Result<nc_com::ComPtr<microui_contracts::IChildrenAbi>, nc_com::NcStatus> {
                <$name as crate::ui_node::widget_object::HostedWidget>::runtime(self).state.readable()?;
                self.editor.view()
            }

            /// Measures the concrete container against the caller's scoped child service.
            fn measure(
                &mut self,
                scope: microui_contracts::LayoutScope,
                bounds: microui_contracts::LayoutLimits,
                service: nc_com::ComRef<'_, microui_contracts::ILayoutServiceAbi>,
            ) -> Result<microui_contracts::Size, nc_com::NcStatus> {
                <$name as crate::ui_node::widget_object::HostedWidget>::runtime(self).state.readable()?;
                let constraints = crate::ui_node::layout_service::constraints(bounds)?;
                let layout = crate::ui_node::layout_service::ScopedLayout::new(scope, service)?;
                let assets = <$name as crate::ui_node::widget_object::HostedWidget>::runtime_mut(self).resources()?;
                let assets = assets.borrow();
                let mut ctx = crate::MeasureCtx::scoped(assets.bundle().skin(), assets.bundle().atlas(), layout);
                let size = crate::ContainerWidget::measure(self, &mut ctx, constraints);
                ctx.result()?;
                Ok(microui_contracts::Size {
                    width: size.width.max(0),
                    height: size.height.max(0),
                })
            }

            /// Places the concrete container and returns only its copied child geometry.
            fn place(
                &mut self,
                scope: microui_contracts::LayoutScope,
                rect: microui_contracts::Rect,
                service: nc_com::ComRef<'_, microui_contracts::ILayoutServiceAbi>,
            ) -> Result<microui_contracts::ContainerGeometry, nc_com::NcStatus> {
                <$name as crate::ui_node::widget_object::HostedWidget>::runtime(self).state.readable()?;
                if rect.width < 0 || rect.height < 0 {
                    return Err(nc_com::NC_E_INVALID_ARG);
                }
                let layout = crate::ui_node::layout_service::ScopedLayout::new(scope, service)?;
                let assets = <$name as crate::ui_node::widget_object::HostedWidget>::runtime_mut(self).resources()?;
                let assets = assets.borrow();
                let mut ctx = crate::ContainerLayoutCtx::scoped(
                    assets.bundle().skin(),
                    assets.bundle().atlas(),
                    crate::boundary::local_rect(rect),
                    layout,
                );
                crate::ContainerWidget::place(self, &mut ctx, crate::boundary::local_rect(rect));
                ctx.result()
            }

            /// Applies the concrete container's post-target event filtering policy.
            fn accepts_event(
                &self,
                event: microui_contracts::WidgetEventAbi,
                text: nc_com::NcCopyFromBufferOffset<'_, u8>,
            ) -> Result<bool, nc_com::NcStatus> {
                <$name as crate::ui_node::widget_object::HostedWidget>::runtime(self).state.readable()?;
                let event = crate::ui_node::input_contract::local_event(event, text)?;
                Ok(event.is_some_and(|event| crate::ContainerWidget::accepts_event(self, &event)))
            }
        }
    };
}

pub(crate) use direct_container_layout_impl;


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
        impl microui_contracts::IScrolling for $name {
            /// Reads one coherent snapshot from the scalar model shared with both scrollbar children.
            fn info(&self) -> Result<microui_contracts::ScrollInfo, nc_com::NcStatus> {
                <$name as crate::ui_node::widget_object::HostedWidget>::runtime(self).state.readable()?;
                Ok(self.model.com_info())
            }

            /// Records a sticky nonnegative request for the next concrete placement.
            fn set_offset(&mut self, offset: microui_contracts::Point) -> Result<(), nc_com::NcStatus> {
                <$name as crate::ui_node::widget_object::HostedWidget>::runtime(self).state.writable()?;
                self.model.set_offset(crate::Vec2i::new(offset.x, offset.y));
                Ok(())
            }

            /// Changes wheel and scrollbar participation through the shared scalar model.
            fn set_enabled(&mut self, enabled: bool) -> Result<(), nc_com::NcStatus> {
                <$name as crate::ui_node::widget_object::HostedWidget>::runtime(self).state.writable()?;
                self.model.set_enabled(enabled);
                Ok(())
            }

            /// Preserves an end request until placement installs the freshly measured range.
            fn scroll_to_end(&mut self) -> Result<(), nc_com::NcStatus> {
                <$name as crate::ui_node::widget_object::HostedWidget>::runtime(self).state.writable()?;
                self.model.scroll_to_end();
                Ok(())
            }

            /// Validates one content-local rectangle before changing either requested axis.
            fn reveal(&mut self, rect: microui_contracts::Rect) -> Result<(), nc_com::NcStatus> {
                <$name as crate::ui_node::widget_object::HostedWidget>::runtime(self).state.writable()?;
                if rect.x < 0 || rect.y < 0 || rect.width < 0 || rect.height < 0 {
                    return Err(nc_com::NC_E_INVALID_ARG);
                }
                self.model.reveal(crate::boundary::local_rect(rect));
                Ok(())
            }
        }
    };
}

pub(crate) use direct_scrolling_impl;


/// Resolves one concrete font-role vocabulary for both widget and compact menu presentation.
pub(crate) fn local_font_role(role: TextFontRole) -> Result<crate::FontRole, NcStatus> {
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
