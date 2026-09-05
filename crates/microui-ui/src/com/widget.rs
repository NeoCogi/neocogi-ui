//! Shared phase execution for concrete built-in COM classes.
//!
//! Each class stores its semantic state inline in WidgetCore<S>. The private generic
//! helper has no vtable, type erasure, secondary widget identity or tree ownership.
use super::*;
use crate::math::RectExt;
use microui_contracts::widget::WidgetState;
use crate::{AtlasHandle, Skin, LeafWidget, WidgetOption, WidgetUpdateCtx, WidgetPaintCtx, Constraints, AvailableSpace};
use crate::render::{DisplayList, Renderer};

/// Private semantic algorithms reused by each statically known built-in class.
pub(crate) trait WidgetAlgorithm: crate::Widget {
    /// Copies common options into the algorithm's existing presentation state.
    fn set_options(&mut self, options: WidgetOption);
    /// Adds concrete state-dependent routing policy to the common caller-authored options.
    fn effective_options(&self, options: WidgetOption) -> WidgetOption {
        options
    }
}

/// One resource revision shared by built-ins, metrics and geometry expansion.
pub(super) struct Resources {
    /// Immutable atlas allocation, shared without per-glyph COM calls.
    pub(super) atlas: AtlasHandle,
    /// Resolved skin for that same atlas revision.
    pub(super) skin: Skin,
    /// Idle reusable geometry workspace; taking it prevents reentrant recording.
    scratch: RefCell<Option<PaintScratch>>,
}

impl Resources {
    /// Creates one geometry executor for the entire runtime, not per widget.
    pub(super) fn new(atlas: AtlasHandle, host: Rc<HostState>) -> Rc<Self> {
        let (collector, storage) = super::render::Backend::recording(atlas.clone(), host);
        Rc::new(Self {
            skin: Skin::from_atlas(&atlas),
            atlas,
            scratch: RefCell::new(Some(PaintScratch {
                list: DisplayList::new(),
                renderer: Renderer::new(collector),
                storage,
            })),
        })
    }
}

/// Inline common state plus the one concrete semantic state of a COM widget.
pub(super) struct WidgetCore<S: WidgetAlgorithm> {
    /// Shared lifecycle, options and pending setter invalidation.
    pub(super) state: WidgetState,
    /// Concrete algorithm state; no Any or dynamically selected Rust widget.
    core: RefCell<Option<S>>,
    /// Resource ownership contains no reference back to a runtime or parent.
    resources: Rc<Resources>,
}

impl<S: WidgetAlgorithm> WidgetCore<S> {
    /// Takes the inline state so recursive component calls cannot encounter a held RefCell guard.
    pub(super) fn take(&self) -> Result<CoreAccess<'_, S>, NcStatus> {
        let value = self.core.try_borrow_mut().map_err(|_| UI_E_BUSY)?.take().ok_or(UI_E_BUSY)?;
        Ok(CoreAccess { slot: &self.core, value: Some(value) })
    }

    /// Composes an unmounted semantic value into its final COM allocation.
    pub(super) fn new(host: ComPtr<IWidgetHostAbi>, resources: Rc<Resources>, core: S) -> Self {
        let options = WidgetOptions { bits: core.widget_opt().bits() };
        Self {
            state: WidgetState::new(host, options),
            core: RefCell::new(Some(core)),
            resources,
        }
    }

    /// Combines common setter markers with the composite's explicitly shared semantic notifications.
    pub(super) fn take_change(&self) -> Result<GeometryChange, NcStatus> {
        self.state.readable()?;
        if self.take()?.take_shared_change() {
            self.state.record_change(GeometryChange::CHANGED);
        }
        self.state.take_change()
    }

    /// Refreshes routing policy from the same state that handles update and paint.
    pub(super) fn properties(&self) -> Result<WidgetProperties, NcStatus> {
        let options = self.state.options()?;
        let core = self.take()?;
        Ok(WidgetProperties {
            options: WidgetOptions {
                bits: core.effective_options(WidgetOption::from_bits_retain(options.bits)).bits(),
            },
            keyboard: core.keyboard_behavior().bits(),
            frame: super::node::frame_contract(core.frame_appearance_role()),
        })
    }

    /// Commits shared options and concrete presentation together after validation.
    pub(super) fn set_options(&self, options: WidgetOptions) -> Result<(), NcStatus> {
        self.state.writable()?;
        options.validate()?;
        let mut core = self.take()?;
        // The second state check is local host authority, not an app callback.
        // Commit it before the infallible algorithm assignment.
        self.state.set_options(options)?;
        core.set_options(WidgetOption::from_bits_retain(options.bits));
        Ok(())
    }

    /// Reconstructs a local update context from copied input and owned resources.
    pub(super) fn update(&self, state: WidgetUpdate, event: WidgetEventAbi, bytes: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
        self.state.writable()?;
        state.validate()?;
        let input = super::input::local_event(event, bytes)?;
        let mut ctx = WidgetUpdateCtx::new_with_content_geometry(
            local_rect(state.screen),
            local_rect(state.clip).translated(crate::Vec2i::new(state.screen.x, state.screen.y)),
            &self.resources.skin,
            &self.resources.atlas,
            state.in_hover_root,
            state.hovered,
            state.focused,
            state.clicked,
            state.active,
            crate::MouseButton::from_bits(u32::from(state.mouse_buttons)).ok_or(NC_E_INVALID_ARG)?,
            crate::Modifiers::from_bits(state.modifiers).ok_or(NC_E_INVALID_ARG)?,
        );
        let change = self.take()?.update(&mut ctx, input.as_ref());
        Ok(match change {
            crate::Change::Changed => GeometryChange::CHANGED,
            crate::Change::NoChange => GeometryChange::NO_CHANGE,
        })
    }

    /// Reuses the existing preferred-size algorithm with exact finite/unbounded bounds.
    pub(super) fn measure(&self, bounds: LayoutLimits) -> Result<Size, NcStatus>
    where
        S: LeafWidget,
    {
        self.state.readable()?;
        let constraints = Constraints::new(available(bounds.width)?, available(bounds.height)?);
        let size = self.take()?.measure(&self.resources.skin, &self.resources.atlas, constraints);
        Ok(Size {
            width: size.width.max(0),
            height: size.height.max(0),
        })
    }

    /// Runs private paint algorithms before calling the provided foreign recorder.
    pub(super) fn paint(&self, scope: PaintScope, state: WidgetPaint, painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
        self.state.readable()?;
        state.validate()?;
        let scratch = self.resources.scratch.try_borrow_mut().map_err(|_| UI_E_BUSY)?.take().ok_or(UI_E_BUSY)?;
        let mut recording = PaintRecording {
            resources: &self.resources,
            scratch: Some(scratch),
            vertices: Vec::new(),
        };
        {
            let scratch = recording.scratch.as_mut().expect("active recording owns its workspace");
            scratch.list.clear();
            let mut ctx = WidgetPaintCtx::new_with_content_geometry(
                local_rect(state.rect),
                &mut scratch.list,
                local_rect(state.clip),
                &self.resources.skin,
                &self.resources.atlas,
                state.enabled,
                state.hovered,
                state.focused,
                state.clicked,
                state.active,
                state.window_active,
            );
            self.take()?.paint(&mut ctx);
            let size = crate::Dimensioni::new(state.rect.width.max(1), state.rect.height.max(1));
            let info = crate::render::FrameInfo::try_new(size, crate::color(0, 0, 0, 0)).map_err(|_| NC_E_INVALID_ARG)?;
            scratch.renderer.render(info, &mut scratch.list).map_err(|_| UI_E_RENDER)?;
            recording.vertices = std::mem::take(&mut *scratch.storage.vertices.borrow_mut());
        }
        // All semantic and scratch RefCell guards have ended. Only owned storage
        // remains live if a custom recorder calls back into this object.
        painter.triangles(scope, NcCopyFromBufferOffset::new(0, &recording.vertices))
    }
}

/// One method temporarily owns its concrete state, returning it after success or any early error.
/// Reentry into that same semantic state reports BUSY; host/lifetime authority remains separate.
pub(super) struct CoreAccess<'a, S> {
    /// Vacant inline slot in the same COM allocation, never a secondary widget owner.
    slot: &'a RefCell<Option<S>>,
    /// Actual state moved for the duration of this call, including recursive child layout.
    value: Option<S>,
}
impl<S> std::ops::Deref for CoreAccess<'_, S> {
    type Target = S;
    /// Exposes the concrete local value, never a Rust dynamic widget interface.
    fn deref(&self) -> &S {
        self.value.as_ref().expect("active core access owns its state")
    }
}
impl<S> std::ops::DerefMut for CoreAccess<'_, S> {
    /// Mutable access stays inside the owning synchronous component method.
    fn deref_mut(&mut self) -> &mut S {
        self.value.as_mut().expect("active core access owns its state")
    }
}
impl<S> Drop for CoreAccess<'_, S> {
    /// Returns ownership without calling foreign code while the slot is borrowed.
    fn drop(&mut self) {
        *self.slot.borrow_mut() = self.value.take();
    }
}

impl<S: WidgetAlgorithm + crate::ContainerWidget> WidgetCore<S> {
    /// Runs the existing concrete container algorithm through validated indexed COM measurements.
    pub(super) fn measure_container(&self, scope: LayoutScope, bounds: LayoutLimits, service: ComRef<'_, ILayoutServiceAbi>) -> Result<Size, NcStatus> {
        self.state.readable()?;
        let constraints = super::layout::constraints(bounds)?;
        let core = self.take()?;
        let layout = super::layout::ScopedLayout::new(scope, service)?;
        let mut ctx = crate::MeasureCtx::scoped(&self.resources.skin, &self.resources.atlas, layout);
        let size = crate::ContainerWidget::measure(&*core, &mut ctx, constraints);
        ctx.result()?;
        Ok(Size {
            width: size.width.max(0),
            height: size.height.max(0),
        })
    }

    /// Runs exact placement with owned semantic state, returning one complete copied result.
    pub(super) fn place_container(&self, scope: LayoutScope, rect: Rect, service: ComRef<'_, ILayoutServiceAbi>) -> Result<ContainerGeometry, NcStatus> {
        self.state.readable()?;
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let mut core = self.take()?;
        let layout = super::layout::ScopedLayout::new(scope, service)?;
        let mut ctx = crate::ContainerLayoutCtx::scoped(&self.resources.skin, &self.resources.atlas, local_rect(rect), layout);
        core.place(&mut ctx, local_rect(rect));
        ctx.result()
    }

    /// Reuses concrete post-target event filtering without exposing any semantic state.
    pub(super) fn accepts_event(&self, event: WidgetEventAbi, bytes: NcCopyFromBufferOffset<'_, u8>) -> Result<bool, NcStatus> {
        self.state.readable()?;
        let event = super::input::local_event(event, bytes)?;
        let core = self.take()?;
        Ok(match event {
            Some(event) => core.accepts_event(&event),
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

/// Owned paint transaction that restores scratch capacity on every return path.
struct PaintRecording<'a> {
    /// Owner of the vacant workspace slot; contains no borrowed runtime context.
    resources: &'a Resources,
    /// Present until Drop returns the reusable workspace.
    scratch: Option<PaintScratch>,
    /// Detached final geometry, borrowed only for the synchronous bulk copy.
    vertices: Vec<RenderVertex>,
}

impl Drop for PaintRecording<'_> {
    /// Restores both workspace and vertex capacity after success or failure.
    fn drop(&mut self) {
        let scratch = self.scratch.take().expect("recording owns scratch until drop");
        if !self.vertices.is_empty() || self.vertices.capacity() != 0 {
            self.vertices.clear();
            *scratch.storage.vertices.borrow_mut() = std::mem::take(&mut self.vertices);
        }
        *self.resources.scratch.borrow_mut() = Some(scratch);
    }
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
            fn mount(&self) -> Result<(), NcStatus> {
                self.widget.state.mount()
            }
            /// Closes semantic capabilities and the class's concrete event sources.
            fn close(&self) -> Result<(), NcStatus> {
                self.close_widget()
            }
            /// Copies policy from the same state used by update and painting.
            fn properties(&self) -> Result<WidgetProperties, NcStatus> {
                self.widget.properties()
            }
            /// Commits validated common and semantic presentation options together.
            fn set_options(&self, options: WidgetOptions) -> Result<(), NcStatus> {
                self.widget.set_options(options)
            }
            /// Reuses the concrete control's full pointer/key/text update algorithm.
            fn update(&self, state: WidgetUpdate, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
                self.update_widget(state, event, text)
            }
            /// Drains the sticky setter marker without erasing unconsumed changes.
            fn take_change(&self) -> Result<GeometryChange, NcStatus> {
                self.widget.take_change()
            }
            /// Records complete owned geometry before calling a foreign painter.
            fn paint(&self, scope: PaintScope, state: WidgetPaint, painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
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
            fn measure(&self, bounds: LayoutLimits, _metrics: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
                self.widget.measure(bounds)
            }
        }
    };
}

/// Declares one non-generic COM allocation with a statically known semantic state.
macro_rules! leaf_class {
    ($name:ident, $core:ty, $interface:ident, $abi:ty $(, $extra:ident)*) => {
        #[com_class(interfaces($interface, ILeafLayout $(, $extra)*))]
        /// One concrete built-in object, including its inherited widget state.
        pub(super) struct $name {
            /// Inline semantic state and shared phase execution; no secondary widget owner.
            widget: super::widget::WidgetCore<$core>,
        }
        impl $name {
            /// Creates the unmounted final COM allocation after input validation.
            pub(super) fn create(host: ComPtr<IWidgetHostAbi>, resources: Rc<super::widget::Resources>, core: $core) -> ComPtr<$abi> {
                Self::from_value(Self { widget: super::widget::WidgetCore::new(host, resources, core) }).query().expect("class declares its editing interface")
            }
            /// Applies the complete existing semantic algorithm to copied input.
            fn update_widget(&self, state: WidgetUpdate, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
                self.widget.update(state, event, text)
            }
            /// Commits logical closure before final COM destruction.
            fn close_widget(&self) -> Result<(), NcStatus> { self.widget.state.close()?; Ok(()) }
        }
        widget_phase_impl!($name);
        leaf_layout_impl!($name);
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
    pub(super) fn set_font_role(&self, role: TextFontRole) -> Result<(), NcStatus> {
        self.state.writable()?;
        let role = match role {
            TextFontRole::BODY => crate::FontRole::Body,
            TextFontRole::SMALL => crate::FontRole::Small,
            TextFontRole::TITLE => crate::FontRole::Title,
            TextFontRole::HEADING => crate::FontRole::Heading,
            TextFontRole::MONO => crate::FontRole::Mono,
            _ => return Err(NC_E_INVALID_ARG),
        };
        self.commit_font(crate::FontRef::Role(role))
    }
    /// Resolves a font once against the exact bundle, never by name per glyph.
    pub(super) fn set_named_font(&self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.state.writable()?;
        let name = text(input)?;
        let font = self.resources.atlas.font_id(name).ok_or(nc_com::NC_E_NOT_FOUND)?;
        self.commit_font(crate::FontRef::Named(font))
    }
    /// Commits an already validated selection and preserves pending setter changes.
    fn commit_font(&self, font: crate::FontRef) -> Result<(), NcStatus> {
        let mut core = self.take()?;
        if *core.font() != font {
            core.set_font(font);
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
            fn set_font_role(&self, role: TextFontRole) -> Result<(), NcStatus> {
                self.widget.set_font_role(role)
            }
            /// Resolves and retains a stable named font identity at the setter boundary.
            fn set_named_font(&self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
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

impl<S: IconState> WidgetCore<S> {
    /// Resolves a semantic icon selection before borrowing the concrete state.
    pub(super) fn set_icon_role(&self, icon: ContentIcon) -> Result<(), NcStatus> {
        self.state.writable()?;
        let icon = match icon {
            ContentIcon::NONE => None,
            ContentIcon::CLOSE => Some(crate::IconRole::Close),
            ContentIcon::EXPAND => Some(crate::IconRole::Expand),
            ContentIcon::COLLAPSE => Some(crate::IconRole::Collapse),
            ContentIcon::CHECK => Some(crate::IconRole::Check),
            ContentIcon::EXPAND_DOWN => Some(crate::IconRole::ExpandDown),
            ContentIcon::OPEN_FOLDER => Some(crate::IconRole::OpenFolder),
            ContentIcon::CLOSED_FOLDER => Some(crate::IconRole::ClosedFolder),
            ContentIcon::FILE => Some(crate::IconRole::File),
            _ => return Err(NC_E_INVALID_ARG),
        };
        self.commit_icon(icon.map(crate::IconRef::Role))
    }
    /// Looks up a named icon once against this widget's exact immutable resource owner.
    pub(super) fn set_named_icon(&self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.state.writable()?;
        let icon = self.resources.atlas.icon_id(text(input)?).ok_or(nc_com::NC_E_NOT_FOUND)?;
        self.commit_icon(Some(crate::IconRef::Named(icon)))
    }
    /// Commits the selection and invalidates the inline visual's reserved size.
    fn commit_icon(&self, icon: Option<crate::IconRef>) -> Result<(), NcStatus> {
        let mut core = self.take()?;
        if core.set_icon(icon) {
            self.state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
}

/// Shares optional icon selection while preserving the concrete class and semantic data.
macro_rules! icon_content_impl {
    ($name:ident) => {
        impl IIconContent for $name {
            /// Validates a semantic icon selection before replacing the adjacent visual.
            fn set_icon_role(&self, icon: ContentIcon) -> Result<(), NcStatus> {
                self.widget.set_icon_role(icon)
            }
            /// Resolves an exact named icon once at the setter boundary.
            fn set_named_icon(&self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
                self.widget.set_named_icon(name)
            }
        }
    };
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
            fn measure(&self, scope: LayoutScope, bounds: LayoutLimits, layout: ComRef<'_, ILayoutServiceAbi>) -> Result<Size, NcStatus> {
                self.widget.measure_container(scope, bounds, layout)
            }
            /// Publishes only copied geometry after the complete concrete placement algorithm succeeds.
            fn place(&self, scope: LayoutScope, rect: Rect, layout: ComRef<'_, ILayoutServiceAbi>) -> Result<ContainerGeometry, NcStatus> {
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
            fn close_widget(&self) -> Result<(), NcStatus> {
                let _core = self.widget.take()?;
                if self.widget.state.close()? {
                    self.editor.close()?.finish()?;
                }
                Ok(())
            }
            /// Reuses the complete concrete update algorithm with copied input and owned state.
            fn update_widget(&self, state: WidgetUpdate, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
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
                Ok(nc_com::NcString::from_utf8_str(self.widget.take()?.$get()))
            }
            /// Applies the existing normalization/cursor policy, then compares committed text.
            fn set_text(&self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
                self.widget.state.writable()?;
                let input = text(input)?;
                let mut core = self.widget.take()?;
                // Compare normalized output, not raw CR/LF input. An editor may
                // still reset its cursor on a text-identical replacement.
                let previous = core.$get().to_owned();
                core.$set(input);
                if core.$get() != previous {
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
                Ok(self.widget.take()?.com_info())
            }
            /// Records a sticky shared request so hidden/skipped updates cannot lose the needed placement.
            fn set_offset(&self, offset: Point) -> Result<(), NcStatus> {
                self.widget.state.writable()?;
                self.widget.take()?.set_offset(crate::Vec2i::new(offset.x, offset.y));
                Ok(())
            }
            /// Disabling clears ranges and requests synchronously; the next layout gates structural children.
            fn set_enabled(&self, enabled: bool) -> Result<(), NcStatus> {
                self.widget.state.writable()?;
                self.widget.take()?.set_scrolling_enabled(enabled);
                Ok(())
            }
            /// Preserves an above-old-range end request until fresh content geometry is measured.
            fn scroll_to_end(&self) -> Result<(), NcStatus> {
                self.widget.state.writable()?;
                self.widget.take()?.scroll_to_end();
                Ok(())
            }
            /// Validates the complete reveal rectangle before changing either shared axis request.
            fn reveal(&self, rect: Rect) -> Result<(), NcStatus> {
                self.widget.state.writable()?;
                if rect.x < 0 || rect.y < 0 || rect.width < 0 || rect.height < 0 {
                    return Err(NC_E_INVALID_ARG);
                }
                self.widget.take()?.scroll_rect_into_view(local_rect(rect));
                Ok(())
            }
        }
    };
}
