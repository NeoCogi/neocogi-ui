//! Static application leaf implementations share lifecycle plumbing in one concrete COM allocation.
//!
//! Use this helper for local behavior whose editing state is already concrete application data.
//! Classes exposing their own editing interfaces can instead compose WidgetState and implement
//! IWidget directly. This helper creates no registered class, native node, erased value or Rust vtable.
use crate::*;
use crate::com::widget::WidgetState;
use neocogi_com::{com_object, ComPtr, ComRef, NcCopyFromBufferOffset, NcStatus};

/// Statically dispatched behavior stored inline in one application-owned COM widget.
pub trait LeafContent: 'static {
    /// Keyboard participation bits use the same copied contract as IWidget::properties.
    const KEYBOARD: u16 = 0;
    /// Theme frame role shared with ordinary controls; options choose whether to draw it.
    const FRAME: WidgetFrame = WidgetFrame::Generic;
    /// Computes preferred content dimensions under validated finite/unbounded constraints.
    fn measure(&self, bounds: LayoutLimits, metrics: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus>;
    /// Processes a validated copied event or an ordinary eventless update. Report CHANGED whenever
    /// external/local state changes measurement inputs; skipped hidden updates retain the documented
    /// author obligation to synchronize through a setter or a later truthful update result.
    fn update(&mut self, _state: WidgetUpdate, _event: WidgetEvent, _text: &str) -> Result<GeometryChange, NcStatus> {
        Ok(GeometryChange::NoChange)
    }
    /// Records drawing under the existing exclusive widget paint gate. Mutable access may
    /// accept completed paint-only cache values; geometry and routing mutation remain forbidden
    /// by host observation authority. Retained scope capabilities expire after this call.
    fn paint(&mut self, state: WidgetPaint, painter: &mut Painter<'_>) -> Result<(), NcStatus>;
}

#[com_object(interfaces(IWidget, ILeafLayout), ownership = exclusive)]
/// The generic parameter specializes ordinary Rust calls; only concrete IWidget/ILeafLayout cross COM.
struct LeafObject<Content: LeafContent> {
    /// One common options, mount, closure and geometry-invalidation state.
    state: WidgetState,
    /// Statically dispatched application behavior owned directly by this COM object.
    content: Content,
}
impl<Content: LeafContent> IWidget for LeafObject<Content> {
    /// Supplies the same runtime authority used by built-in COM objects.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        self.state.host().try_add_ref()
    }
    /// Mount-once semantics come from the single common state, not the content behavior.
    fn mount(&mut self) -> Result<(), NcStatus> {
        self.state.mount()
    }
    /// Common observations read only lifecycle state and remain safe during shared content calls.
    fn properties(&self) -> Result<WidgetProperties, NcStatus> {
        Ok(WidgetProperties {
            options: self.state.requested_flags()?,
            keyboard: Content::KEYBOARD,
            frame: Content::FRAME,
        })
    }
    /// Reads the requested configuration without including transient runtime policy.
    fn requested_flags(&self) -> Result<WidgetOptions, NcStatus> {
        self.state.requested_flags()
    }
    /// Options and their sticky geometry effect follow the standard widget contract.
    fn replace_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.state.replace_flags(flags)
    }
    /// Sets current requested bits through the same widget-owned mutation authority.
    fn set_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.state.set_flags(flags)
    }
    /// Clears requested bits without copying effective properties back into settings.
    fn clear_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.state.clear_flags(flags)
    }
    /// Validates all copied input before mutating content; changes remain sticky until collected.
    fn update(&mut self, state: WidgetUpdate, event: WidgetEvent, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
        self.state.writable()?;
        state.validate()?;
        let (event, text) = decode_widget_event(event, text)?;
        let change = self.content.update(state, event, text).inspect_err(|_| {
            // A failed behavior update may have committed part of its local state. Retain a
            // conservative geometry marker so recovery cannot reuse an earlier measurement.
            self.state.record_change(GeometryChange::Changed);
        })?;
        if change != GeometryChange::Changed && change != GeometryChange::NoChange {
            self.state.record_change(GeometryChange::Changed);
            return Err(neocogi_com::NC_E_INVALID_ARG);
        }
        self.state.record_change(change);
        Ok(change)
    }
    /// Setter and update invalidation use the same collection path.
    fn take_change(&mut self) -> Result<GeometryChange, NcStatus> {
        self.state.take_change()
    }
    /// Content receives a borrowing convenience; retaining raw paint authority still expires normally.
    fn paint(&mut self, scope: PaintScope, state: WidgetPaint, painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
        self.state.readable()?;
        state.validate()?;
        self.content.paint(state, &mut Painter::from_call(painter, scope))
    }
}
impl<Content: LeafContent> ILeafLayout for LeafObject<Content> {
    /// Observational preferred-size calls reject stale lifetime, malformed limits and content reentry.
    fn measure(&mut self, bounds: LayoutLimits, metrics: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        self.state.readable()?;
        bounds.width.validate()?;
        bounds.height.validate()?;
        self.content.measure(bounds, metrics)
    }
}

impl WidgetFactory {
    /// Creates one local COM leaf with statically dispatched inline content and normal runtime authority.
    pub fn leaf<Content: LeafContent>(&self, content: Content, options: WidgetOptions) -> Result<OwnedWidget<IWidgetAbi>, NcStatus> {
        options.validate()?;
        let object = LeafObject::into_box(LeafObject {
            state: WidgetState::new(self.host()?, options),
            content,
        });
        Ok(OwnedWidget::from_interface(object.query().expect("LeafObject declares IWidget")))
    }
}

/// Borrowed geometry publication authority for one admitted private leaf handler.
/// It refers to the owning allocation's existing sticky state; it creates no
/// independent receiver, COM object or retained lifetime. Publish immediately after
/// committing a measurement input, including before a later fallible operation.
pub struct LeafDispatchContext<'a> {
    /// Existing inline widget state, borrowed only for this exclusive callback.
    state: &'a mut WidgetState,
}
impl LeafDispatchContext<'_> {
    /// Makes committed content geometry visible before the next queued UI input.
    /// Visual-only changes need no notification. Earlier notifications remain sticky.
    pub fn geometry_changed(&mut self) {
        self.state.record_change(GeometryChange::Changed);
    }
}

/// Private behavior for a leaf whose concrete allocation also owns its receiver/model.
/// Passive content implements only `LeafContent` and never advertises a dispatcher.
pub trait DispatchingLeafContent: LeafContent {
    /// Attempts at most one private message. A failed attempt is consumed; successors remain queued.
    /// Return false only when idle. Publish committed geometry through `context`
    /// before returning, even when the handler subsequently fails. A failed call
    /// also retains conservative invalidation for partially committed behavior.
    fn dispatch_next(&mut self, context: &mut LeafDispatchContext<'_>) -> Result<bool, NcStatus>;
}

#[com_object(interfaces(IWidget, ILeafLayout, IWidgetDispatcher), ownership = exclusive)]
/// Optional behavior shares the leaf's ownership and generated exclusive borrow gate.
struct DispatchingLeafObject<Content: DispatchingLeafContent> {
    /// Inline lifecycle/content value, never a separately callable COM allocation.
    leaf: LeafObject<Content>,
}
impl<Content: DispatchingLeafContent> IWidget for DispatchingLeafObject<Content> {
    /// Delegates directly to the inline leaf under this allocation's common admission gate.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        self.leaf.host()
    }
    /// Delegates directly to the inline leaf under this allocation's common admission gate.
    fn mount(&mut self) -> Result<(), NcStatus> {
        self.leaf.mount()
    }
    /// Delegates directly to the inline leaf under this allocation's common admission gate.
    fn properties(&self) -> Result<WidgetProperties, NcStatus> {
        self.leaf.properties()
    }
    /// Delegates directly to the inline leaf under this allocation's common admission gate.
    fn requested_flags(&self) -> Result<WidgetOptions, NcStatus> {
        self.leaf.requested_flags()
    }
    /// Delegates directly to the inline leaf under this allocation's common admission gate.
    fn replace_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.leaf.replace_flags(flags)
    }
    /// Delegates directly to the inline leaf under this allocation's common admission gate.
    fn set_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.leaf.set_flags(flags)
    }
    /// Delegates directly to the inline leaf under this allocation's common admission gate.
    fn clear_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.leaf.clear_flags(flags)
    }
    /// Delegates directly to the inline leaf under this allocation's common admission gate.
    fn update(&mut self, state: WidgetUpdate, event: WidgetEvent, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
        self.leaf.update(state, event, text)
    }
    /// Delegates directly to the inline leaf under this allocation's common admission gate.
    fn take_change(&mut self) -> Result<GeometryChange, NcStatus> {
        self.leaf.take_change()
    }
    /// Delegates directly to the inline leaf under this allocation's common admission gate.
    fn paint(&mut self, scope: PaintScope, state: WidgetPaint, painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
        self.leaf.paint(scope, state, painter)
    }
}
impl<Content: DispatchingLeafContent> ILeafLayout for DispatchingLeafObject<Content> {
    /// Uses the same validated observational content call as passive leaves.
    fn measure(&mut self, bounds: LayoutLimits, metrics: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        self.leaf.measure(bounds, metrics)
    }
}
impl<Content: DispatchingLeafContent> IWidgetDispatcher for DispatchingLeafObject<Content> {
    /// Admission uses common runtime authority before entering concrete private behavior.
    fn dispatch_next(&mut self) -> Result<bool, NcStatus> {
        self.leaf.state.writable()?;
        let result = self.leaf.content.dispatch_next(&mut LeafDispatchContext { state: &mut self.leaf.state });
        if result.is_err() {
            self.leaf.state.record_change(GeometryChange::Changed);
        }
        result
    }
}
impl WidgetFactory {
    /// Creates one dispatching leaf; its optional projection owns no second state or receiver.
    pub fn dispatching_leaf<Content: DispatchingLeafContent>(&self, content: Content, options: WidgetOptions) -> Result<OwnedWidget<IWidgetAbi>, NcStatus> {
        options.validate()?;
        let object = DispatchingLeafObject::into_box(DispatchingLeafObject {
            leaf: LeafObject {
                state: WidgetState::new(self.host()?, options),
                content,
            },
        });
        Ok(OwnedWidget::from_interface(object.query().expect("dispatching leaf declares IWidget")))
    }
}
