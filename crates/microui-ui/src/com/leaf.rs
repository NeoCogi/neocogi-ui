//! Shared phase execution for concrete built-in COM classes.
//!
//! Each class stores its semantic state inline in Leaf<S>. The private generic
//! helper has no vtable, type erasure, secondary widget identity or tree ownership.
use super::*;
use crate::math::RectExt;
use microui_contracts::widget::WidgetState;
use crate::{AtlasHandle, Skin, LeafWidget, WidgetOption, WidgetUpdateCtx, WidgetPaintCtx, Constraints, AvailableSpace};
use crate::render::{DisplayList, Renderer};

/// Private semantic algorithms reused by each statically known built-in class.
pub(crate) trait LeafState: LeafWidget {
    /// Copies common options into the algorithm's existing presentation state.
    fn set_options(&mut self, options: WidgetOption);
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

/// Inline common state plus the one concrete semantic state of a COM leaf.
pub(super) struct Leaf<S: LeafState> {
    /// Shared lifecycle, options and pending setter invalidation.
    pub(super) state: WidgetState,
    /// Concrete algorithm state; no Any or dynamically selected Rust widget.
    pub(super) core: RefCell<S>,
    /// Resource ownership contains no reference back to a runtime or parent.
    resources: Rc<Resources>,
}

impl<S: LeafState> Leaf<S> {
    /// Composes an unmounted semantic value into its final COM allocation.
    pub(super) fn new(host: ComPtr<IWidgetHostAbi>, resources: Rc<Resources>, core: S) -> Self {
        let options = WidgetOptions { bits: core.widget_opt().bits() };
        Self {
            state: WidgetState::new(host, options),
            core: RefCell::new(core),
            resources,
        }
    }

    /// Refreshes routing policy from the same state that handles update and paint.
    pub(super) fn properties(&self) -> Result<WidgetProperties, NcStatus> {
        let options = self.state.options()?;
        let core = self.core.try_borrow().map_err(|_| UI_E_BUSY)?;
        Ok(WidgetProperties {
            options,
            keyboard: core.keyboard_behavior().bits(),
            frame: super::node::frame_contract(core.frame_appearance_role()),
        })
    }

    /// Commits shared options and concrete presentation together after validation.
    pub(super) fn set_options(&self, options: WidgetOptions) -> Result<(), NcStatus> {
        self.state.writable()?;
        options.validate()?;
        let mut core = self.core.try_borrow_mut().map_err(|_| UI_E_BUSY)?;
        // The second state check is local host authority, not an app callback.
        // Commit it before the infallible algorithm assignment.
        drop(core);
        self.state.set_options(options)?;
        core = self.core.try_borrow_mut().map_err(|_| UI_E_BUSY)?;
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
        let change = self.core.try_borrow_mut().map_err(|_| UI_E_BUSY)?.update(&mut ctx, input.as_ref());
        Ok(match change {
            crate::Change::Changed => GeometryChange::CHANGED,
            crate::Change::NoChange => GeometryChange::NO_CHANGE,
        })
    }

    /// Reuses the existing preferred-size algorithm with exact finite/unbounded bounds.
    pub(super) fn measure(&self, bounds: LayoutLimits) -> Result<Size, NcStatus> {
        self.state.readable()?;
        let constraints = Constraints::new(available(bounds.width)?, available(bounds.height)?);
        let size = self
            .core
            .try_borrow()
            .map_err(|_| UI_E_BUSY)?
            .measure(&self.resources.skin, &self.resources.atlas, constraints);
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
            self.core.try_borrow_mut().map_err(|_| UI_E_BUSY)?.paint(&mut ctx);
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

/// Converts one checked contract axis without treating finite zero as unbounded.
fn available(axis: AxisLimit) -> Result<AvailableSpace, NcStatus> {
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
macro_rules! leaf_widget_impl {
    ($name:ident) => {
        impl IWidget for $name {
            /// Returns the runtime authority without retaining a tree or parent.
            fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
                Ok(self.leaf.state.host().add_ref())
            }
            /// Claims the only attachment after runtime validation succeeds.
            fn mount(&self) -> Result<(), NcStatus> {
                self.leaf.state.mount()
            }
            /// Closes semantic capabilities and the class's concrete event sources.
            fn close(&self) -> Result<(), NcStatus> {
                self.close_widget()
            }
            /// Copies policy from the same state used by update and painting.
            fn properties(&self) -> Result<WidgetProperties, NcStatus> {
                self.leaf.properties()
            }
            /// Commits validated common and semantic presentation options together.
            fn set_options(&self, options: WidgetOptions) -> Result<(), NcStatus> {
                self.leaf.set_options(options)
            }
            /// Reuses the concrete control's full pointer/key/text update algorithm.
            fn update(&self, state: WidgetUpdate, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
                self.update_widget(state, event, text)
            }
            /// Drains the sticky setter marker without erasing unconsumed changes.
            fn take_change(&self) -> Result<GeometryChange, NcStatus> {
                self.leaf.state.take_change()
            }
            /// Records complete owned geometry before calling a foreign painter.
            fn paint(&self, scope: PaintScope, state: WidgetPaint, painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
                self.leaf.paint(scope, state, painter)
            }
        }
        impl ILeafLayout for $name {
            /// Measures with the resource revision owned by this runtime-bound class.
            fn measure(&self, bounds: LayoutLimits, _metrics: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
                self.leaf.measure(bounds)
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
            leaf: super::leaf::Leaf<$core>,
        }
        impl $name {
            /// Creates the unmounted final COM allocation after input validation.
            pub(super) fn create(host: ComPtr<IWidgetHostAbi>, resources: Rc<super::leaf::Resources>, core: $core) -> ComPtr<$abi> {
                Self::from_value(Self { leaf: super::leaf::Leaf::new(host, resources, core) }).query().expect("class declares its editing interface")
            }
            /// Applies the complete existing semantic algorithm to copied input.
            fn update_widget(&self, state: WidgetUpdate, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
                self.leaf.update(state, event, text)
            }
            /// Commits logical closure before final COM destruction.
            fn close_widget(&self) -> Result<(), NcStatus> { self.leaf.state.close()?; Ok(()) }
        }
        leaf_widget_impl!($name);
    };
}

/// Static typography access implemented only by semantic states which own a font.
pub(crate) trait FontState: LeafState {
    /// Borrows the actual stable font selection used by measurement and paint.
    fn font(&self) -> &crate::FontRef;
    /// Replaces that same selection after the resource owner validates it.
    fn set_font(&mut self, font: crate::FontRef);
}

impl<S: FontState> Leaf<S> {
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
        let mut core = self.core.try_borrow_mut().map_err(|_| UI_E_BUSY)?;
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
                self.leaf.set_font_role(role)
            }
            /// Resolves and retains a stable named font identity at the setter boundary.
            fn set_named_font(&self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
                self.leaf.set_named_font(name)
            }
        }
    };
}

/// Static access to the adjacent visual stored by a button or list row.
pub(crate) trait IconState: LeafState {
    /// Replaces the visual while retaining label/font state; reports an actual visual change.
    fn set_icon(&mut self, icon: Option<crate::IconRef>) -> bool;
}

impl<S: IconState> Leaf<S> {
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
        let mut core = self.core.try_borrow_mut().map_err(|_| UI_E_BUSY)?;
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
                self.leaf.set_icon_role(icon)
            }
            /// Resolves an exact named icon once at the setter boundary.
            fn set_named_icon(&self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
                self.leaf.set_named_icon(name)
            }
        }
    };
}
