//! Independently built custom widget and container. No UI or renderer implementation is linked here.
//!
//! The two concrete objects share only lifecycle plumbing. The counter owns its scalar value and
//! event producer; the inset owns its padding and the host's sole authoritative child editor.
#![deny(missing_docs)]
use neocogi_ui::com::widget::WidgetState;
use neocogi_ui_core_contracts::*;
use neocogi_demo_contracts::*;
use neocogi_ui::{
    Painter,
    events::{Activation, EventSource},
};
use neocogi_com::{ComPtr, ComRef, NcCopyFromBufferOffset, NcStatus, com_factory, com_object};

/// Common state is inline; the example adds no second widget allocation or type-erased data.
struct Common {
    /// The canonical lifecycle/options/invalidation implementation used by independent authors.
    state: WidgetState,
    /// Phase counters observe actual calls rather than estimates from a shadow tree.
    calls: WidgetCalls,
}
impl Common {
    /// Retains exactly the explicit owner capability supplied by the caller.
    fn new(host: ComRef<'_, IWidgetHostAbi>, options: WidgetOptions) -> Self {
        Self {
            state: WidgetState::new(host.add_ref(), options),
            calls: WidgetCalls::default(),
        }
    }
}

/// Common inherited methods are identical; concrete content methods below carry all policy/data.
macro_rules! widget {
    ($name:ident, $keyboard:expr, $frame:expr) => {
        impl IWidget for $name {
            /// Borrowing the host is explicit; returning it owns one ordinary COM reference.
            fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
                Ok(self.common.state.host().add_ref())
            }
            /// The shared state accepts exactly one attachment over the object's lifetime.
            fn mount(&mut self) -> Result<(), NcStatus> {
                self.common.state.mount()
            }
            /// Logical closure precedes descendant/event teardown and remains distinct from final release.
            fn close(&mut self) -> Result<(), NcStatus> {
                if self.common.state.close()? {
                    self.common.calls.closes += 1;
                    self.close_content()?;
                }
                Ok(())
            }
            /// Policy observation reads only common state and remains available during shared calls.
            fn properties(&self) -> Result<WidgetProperties, NcStatus> {
                Ok(WidgetProperties {
                    options: self.common.state.options()?,
                    keyboard: $keyboard,
                    frame: $frame,
                })
            }
            /// Common option edits retain a geometry marker without copying options into content state.
            fn set_options(&mut self, options: WidgetOptions) -> Result<(), NcStatus> {
                self.common.state.set_options(options)
            }
            /// Validate complete copied input before the concrete behavior can mutate or emit.
            fn update(&mut self, state: WidgetUpdate, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
                self.common.state.writable()?;
                state.validate()?;
                decode_widget_event(event, text)?;
                self.common.calls.updates += 1;
                let change = self.update_content(state);
                self.common.state.record_change(change);
                Ok(change)
            }
            /// Hidden-tree collection sees the same sticky marker as ordinary updates.
            fn take_change(&mut self) -> Result<GeometryChange, NcStatus> {
                self.common.state.take_change()
            }
            /// The borrowed painter is valid only for this invocation; content retains neither service nor scope.
            fn paint(&mut self, scope: PaintScope, state: WidgetPaint, painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
                self.common.state.readable()?;
                state.validate()?;
                self.common.calls.paints += 1;
                self.paint_content(state, &mut Painter::from_call(painter, scope))
            }
        }
        impl IWidgetProbe for $name {
            /// Diagnostics deliberately remain readable after logical closure.
            fn calls(&self) -> Result<WidgetCalls, NcStatus> {
                Ok(self.common.calls)
            }
        }
    };
}

#[com_object(neocogi_demo_contracts::Counter)]
/// Private implementation of the externally declared counter class.
struct CounterObject {
    /// Common options are authoritative here, without a duplicate presentation copy.
    common: Common,
    /// The one semantic counter value; labels are short-lived snapshots of it.
    count: u32,
    /// The shared static producer retains queued snapshots, never the application's callback.
    submitted: EventSource<Activation>,
}
impl CounterObject {
    /// Validates runtime authority and publishes one fully initialized counter identity.
    fn create(host: ComRef<'_, IWidgetHostAbi>) -> Result<ComPtr<CounterAbi>, NcStatus> {
        host.check_mutation()?;
        Self::from_value(Self {
            common: Common::new(host, WidgetOptions::FRAME),
            count: 0,
            submitted: EventSource::new(),
        })
        .query()
    }

    /// User activation commits before emission, capturing the matching label for deferred delivery.
    fn update_content(&mut self, state: WidgetUpdate) -> GeometryChange {
        if state.clicked || state.action == WidgetAction::Activate {
            self.count = self.count.saturating_add(1);
            self.submitted.emit(Activation { label: self.count.to_string() });
            GeometryChange::Changed
        } else {
            GeometryChange::NoChange
        }
    }
    /// Standard body text requires no knowledge of native fonts, atlas packing or GPU types.
    fn paint_content(&self, state: WidgetPaint, painter: &mut Painter<'_>) -> Result<(), NcStatus> {
        painter.text(state.rect, &self.count.to_string())
    }
    /// Ordinary interaction work expires even if callers retain the closed control.
    fn close_content(&mut self) -> Result<(), NcStatus> {
        self.submitted.close_ordinary();
        Ok(())
    }
}
// Focus=1, Tab=2, Enter=4 and Space=8 are the canonical WidgetProperties keyboard bits.
widget!(CounterObject, 1 | 2 | 4 | 8, WidgetFrame::Button);
impl Counter for CounterObject {
    /// Reads the committed scalar directly from the real object.
    fn count(&self) -> Result<u32, NcStatus> {
        self.common.state.readable()?;
        Ok(self.count)
    }
    /// Silent programmatic edits invalidate only when the scalar actually changes.
    fn set_count(&mut self, value: u32) -> Result<(), NcStatus> {
        self.common.state.writable()?;
        if self.count != value {
            self.count = value;
            self.common.state.record_change(GeometryChange::Changed);
        }
        Ok(())
    }
}
impl IActivationEvents for CounterObject {
    /// Subscription uses the exact same producer/lease implementation as built-in controls.
    fn subscribe(&self, sink: ComRef<'_, IActivationSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.common.state.writable()?;
        self.submitted.subscribe(sink)
    }
}
impl ILeafLayout for CounterObject {
    /// The host's current metrics supply button text geometry; constraints are checked before call-out.
    fn measure(&mut self, bounds: LayoutLimits, metrics: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        self.common.state.readable()?;
        bounds.width.validate()?;
        bounds.height.validate()?;
        self.common.calls.measures += 1;
        metrics.button_size(NcCopyFromBufferOffset::new(0, self.count.to_string().as_bytes()))
    }
}

#[com_object(interfaces(IInset, IContainerLayout, IWidgetProbe))]
/// Concrete custom container; the host's editor owns its sole child and enforces attachment/lifetime rules.
struct Inset {
    /// Common lifecycle has no parent backpointer or mirrored geometry.
    common: Common,
    /// Authored inset; child measurement and placement use this same scalar.
    padding: i32,
    /// Private writer; public children() returns its distinct read-only view.
    editor: ComPtr<IChildrenEditorAbi>,
}
impl Inset {
    /// This layout-only container has no input-dependent semantic state.
    fn update_content(&mut self, _state: WidgetUpdate) -> GeometryChange {
        GeometryChange::NoChange
    }
    /// Children paint through ordinary retained traversal, without a second recursive paint path.
    fn paint_content(&self, _state: WidgetPaint, _painter: &mut Painter<'_>) -> Result<(), NcStatus> {
        Ok(())
    }
    /// Child teardown can fail after topology commits; preserve the ordinary committed result.
    fn close_content(&mut self) -> Result<(), NcStatus> {
        self.editor.close()?.finish()
    }
}
widget!(Inset, 0, WidgetFrame::Panel);
impl IContainer for Inset {
    /// Exposes actual topology without granting the writer needed to bypass the one-child policy.
    fn children(&self) -> Result<ComPtr<IChildrenAbi>, NcStatus> {
        self.common.state.readable()?;
        self.editor.view()
    }
    /// The underlying editor rejects invalid indices and closes only the removed subtree.
    fn remove(&mut self, index: u32) -> Result<ChildEditResult, NcStatus> {
        self.common.state.writable()?;
        self.editor.remove(index)
    }
    /// Emptying preserves the container allocation and its authored padding for future attachment.
    fn clear(&mut self) -> Result<ChildEditResult, NcStatus> {
        self.common.state.writable()?;
        self.editor.clear()
    }
}
impl IInset for Inset {
    /// Read cardinality from its authoritative sequence; no second has-child flag can become stale.
    fn set_child(&mut self, child: ComRef<'_, IWidgetAbi>) -> Result<ChildEditResult, NcStatus> {
        self.common.state.writable()?;
        if self.editor.view()?.child_count()? == 0 {
            self.editor.insert(0, child)
        } else {
            self.editor.replace(0, child)
        }
    }
    /// Validate before mutation so rejected layout settings preserve the old committed geometry.
    fn set_padding(&mut self, padding: i32) -> Result<(), NcStatus> {
        self.common.state.writable()?;
        if !(0..=i32::MAX / 2).contains(&padding) {
            return Err(neocogi_com::NC_E_INVALID_ARG);
        }
        if self.padding != padding {
            self.padding = padding;
            self.common.state.record_change(GeometryChange::Changed);
        }
        Ok(())
    }
}
impl IContainerLayout for Inset {
    /// Geometry and editing identify the same exact read-only collection.
    fn children(&self) -> Result<ComPtr<IChildrenAbi>, NcStatus> {
        IContainer::children(self)
    }
    /// Bounded indexed services recurse into the child without lending it a native tree borrow.
    fn measure(&mut self, scope: LayoutScope, bounds: LayoutLimits, layout: ComRef<'_, ILayoutServiceAbi>) -> Result<Size, NcStatus> {
        self.common.state.readable()?;
        bounds.width.validate()?;
        bounds.height.validate()?;
        self.common.calls.measures += 1;
        let inset = self.padding * 2;
        let child = if layout.child_count(scope)? == 0 {
            Size::default()
        } else {
            layout.measure_child(
                scope,
                0,
                LayoutLimits {
                    width: shrink(bounds.width, inset),
                    height: shrink(bounds.height, inset),
                },
            )?
        };
        Ok(Size {
            width: child.width.saturating_add(inset),
            height: child.height.saturating_add(inset),
        })
    }
    /// All child coordinates are local; runtime supplies final clipping/translation and validates complete placement.
    fn place(&mut self, scope: LayoutScope, rect: Rect, layout: ComRef<'_, ILayoutServiceAbi>) -> Result<ContainerGeometry, NcStatus> {
        self.common.state.readable()?;
        if rect.width < 0 || rect.height < 0 {
            return Err(neocogi_com::NC_E_INVALID_ARG);
        }
        self.common.calls.placements += 1;
        let padding = self.padding;
        if layout.child_count(scope)? != 0 {
            layout.place_child(
                scope,
                0,
                Rect {
                    x: padding,
                    y: padding,
                    width: rect.width.saturating_sub(padding * 2).max(0),
                    height: rect.height.saturating_sub(padding * 2).max(0),
                },
            )?;
        }
        Ok(ContainerGeometry::new(rect))
    }
    /// No branch-level filtering: the ordinary runtime remains responsible for child input eligibility.
    fn accepts_event(&self, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<bool, NcStatus> {
        self.common.state.readable()?;
        decode_widget_event(event, text)?;
        Ok(true)
    }
}
/// Remove decoration from finite constraints while preserving an explicitly unbounded axis.
fn shrink(mut limit: AxisLimit, inset: i32) -> AxisLimit {
    if limit.kind == LimitKind::Finite {
        limit.extent = limit.extent.saturating_sub(inset).max(0);
    }
    limit
}

#[com_factory(neocogi_demo_contracts::ExtensionProvider)]
/// Stateless shorthand factory creates the interface-based inset container.
pub struct ExtensionProviderObject;
impl ExtensionProvider for ExtensionProviderObject {
    /// Creates a counter only after its supplied host accepts mutation.
    fn counter(&self, host: ComRef<'_, IWidgetHostAbi>) -> Result<ComPtr<CounterAbi>, NcStatus> {
        CounterObject::create(host)
    }

    /// One empty editor is the only topology allocation; no native child list is created in this plugin.
    fn inset(&self, host: ComRef<'_, IWidgetHostAbi>) -> Result<ComPtr<IInsetAbi>, NcStatus> {
        host.check_mutation()?;
        let editor = host.create_children()?;
        Inset::from_value(Inset {
            common: Common::new(host, WidgetOptions::NONE),
            padding: 4,
            editor,
        })
        .query()
    }
}
neocogi_com::factory_plugin! {
    name = "neocogi.example.ui-extension",
    version = "0.9.0-alpha.0",
    contracts = [neocogi_ui_core_contracts::MODULE, neocogi_demo_contracts::MODULE],
    factory = ExtensionProviderObject,
}
#[cfg(feature = "dynamic-plugin")]
neocogi_com::export_factory_plugin!();
