//! Independently built custom widget and container. No UI or renderer implementation is linked here.
//!
//! The two concrete objects share only lifecycle plumbing. The counter owns its scalar value and
//! event producer; the inset owns its padding and the host's sole authoritative child editor.
#![deny(missing_docs)]
use microui_contracts::{widget::WidgetState, *};
use microui_demo_contracts::extension::*;
use microui_redux::{
    Painter,
    events::{Activation, EventSource},
};
use nc_com::{com_class, ComPtr, ComRef, NcCopyFromBufferOffset, NcStatus};
use std::cell::Cell;

/// Common state is inline; the example adds no second widget allocation or type-erased data.
struct Common {
    /// The canonical lifecycle/options/invalidation implementation used by independent authors.
    state: WidgetState,
    /// Phase counters observe actual calls rather than estimates from a shadow tree.
    calls: Cell<WidgetCalls>,
    /// Outward calls can synchronously reenter; overlapping behavior reports BUSY.
    busy: Cell<bool>,
}
impl Common {
    /// Retains exactly the explicit owner capability supplied by the caller.
    fn new(host: ComRef<'_, IWidgetHostAbi>, options: WidgetOptions) -> Self {
        Self {
            state: WidgetState::new(host.add_ref(), options),
            calls: Cell::new(WidgetCalls::default()),
            busy: Cell::new(false),
        }
    }
    /// Reserves behavior without lending a RefCell guard across any foreign call.
    fn enter(&self) -> Result<Guard<'_>, NcStatus> {
        if self.busy.replace(true) { Err(UI_E_BUSY) } else { Ok(Guard(&self.busy)) }
    }
    /// Observational phases still reject a closed owner before entering behavior.
    fn read(&self) -> Result<Guard<'_>, NcStatus> {
        self.state.readable()?;
        self.enter()
    }
    /// Mutation requires host authority as well as exclusive behavior access.
    fn write(&self) -> Result<Guard<'_>, NcStatus> {
        self.state.writable()?;
        self.enter()
    }
}
/// Stack-only recursion reservation, restored after ordinary errors and unwinding alike.
struct Guard<'a>(
    /// The busy bit belongs to the same live COM allocation for this complete call.
    &'a Cell<bool>,
);
impl Drop for Guard<'_> {
    /// Failed outward calls cannot leave an otherwise live object permanently busy.
    fn drop(&mut self) {
        self.0.set(false);
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
            fn mount(&self) -> Result<(), NcStatus> {
                let _guard = self.common.write()?;
                self.common.state.mount()
            }
            /// Logical closure precedes descendant/event teardown and remains distinct from final release.
            fn close(&self) -> Result<(), NcStatus> {
                let _guard = self.common.enter()?;
                if self.common.state.close()? {
                    let mut calls = self.common.calls.get();
                    calls.closes += 1;
                    self.common.calls.set(calls);
                    self.close_content()?;
                }
                Ok(())
            }
            /// Policy observation is safe during outward behavior calls and has no mutable content borrow.
            fn properties(&self) -> Result<WidgetProperties, NcStatus> {
                Ok(WidgetProperties {
                    options: self.common.state.options()?,
                    keyboard: $keyboard,
                    frame: $frame,
                })
            }
            /// Common option edits retain a geometry marker without copying options into content state.
            fn set_options(&self, options: WidgetOptions) -> Result<(), NcStatus> {
                let _guard = self.common.write()?;
                self.common.state.set_options(options)
            }
            /// Validate complete copied input before the concrete behavior can mutate or emit.
            fn update(&self, state: WidgetUpdate, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
                let _guard = self.common.write()?;
                state.validate()?;
                decode_widget_event(event, text)?;
                let mut calls = self.common.calls.get();
                calls.updates += 1;
                self.common.calls.set(calls);
                let change = self.update_content(state);
                self.common.state.record_change(change);
                Ok(change)
            }
            /// Hidden-tree collection sees the same sticky marker as ordinary updates.
            fn take_change(&self) -> Result<GeometryChange, NcStatus> {
                self.common.state.take_change()
            }
            /// The borrowed painter is valid only for this invocation; content retains neither service nor scope.
            fn paint(&self, scope: PaintScope, state: WidgetPaint, painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
                let _guard = self.common.read()?;
                state.validate()?;
                let mut calls = self.common.calls.get();
                calls.paints += 1;
                self.common.calls.set(calls);
                self.paint_content(state, &Painter::from_call(painter, scope))
            }
        }
        impl IWidgetProbe for $name {
            /// Diagnostics deliberately remain readable after logical closure.
            fn calls(&self) -> Result<WidgetCalls, NcStatus> {
                Ok(self.common.calls.get())
            }
        }
    };
}

#[com_class(interfaces(ICounter, ILeafLayout, IActivationEvents, IWidgetProbe))]
/// One real derived COM widget, including ICounter's inherited IWidget projection.
struct Counter {
    /// Common options are authoritative here, without a duplicate presentation copy.
    common: Common,
    /// The one semantic counter value; labels are short-lived snapshots of it.
    count: Cell<u32>,
    /// The shared static producer retains queued snapshots, never the application's callback.
    submitted: EventSource<Activation>,
}
impl Counter {
    /// User activation commits before emission, capturing the matching label for deferred delivery.
    fn update_content(&self, state: WidgetUpdate) -> GeometryChange {
        if state.clicked || state.action == WidgetAction::ACTIVATE {
            self.count.set(self.count.get().saturating_add(1));
            self.submitted.emit(Activation { label: self.count.get().to_string() });
            GeometryChange::CHANGED
        } else {
            GeometryChange::NO_CHANGE
        }
    }
    /// Standard body text requires no knowledge of native fonts, atlas packing or GPU types.
    fn paint_content(&self, state: WidgetPaint, painter: &Painter<'_>) -> Result<(), NcStatus> {
        painter.text(state.rect, &self.count.get().to_string())
    }
    /// Ordinary interaction work expires even if callers retain the closed control.
    fn close_content(&self) -> Result<(), NcStatus> {
        self.submitted.close_ordinary();
        Ok(())
    }
}
// Focus=1, Tab=2, Enter=4 and Space=8 are the canonical WidgetProperties keyboard bits.
widget!(Counter, 1 | 2 | 4 | 8, WidgetFrame::BUTTON);
impl ICounter for Counter {
    /// Reads the committed scalar directly from the real object.
    fn count(&self) -> Result<u32, NcStatus> {
        self.common.state.readable()?;
        Ok(self.count.get())
    }
    /// Silent programmatic edits invalidate only when the scalar actually changes.
    fn set_count(&self, value: u32) -> Result<(), NcStatus> {
        let _guard = self.common.write()?;
        if self.count.replace(value) != value {
            self.common.state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
}
impl IActivationEvents for Counter {
    /// Subscription uses the exact same producer/lease implementation as built-in controls.
    fn subscribe(&self, sink: ComRef<'_, IActivationSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        let _guard = self.common.write()?;
        self.submitted.subscribe(sink)
    }
}
impl ILeafLayout for Counter {
    /// The host's current metrics supply button text geometry; constraints are checked before call-out.
    fn measure(&self, bounds: LayoutLimits, metrics: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        let _guard = self.common.read()?;
        bounds.width.validate()?;
        bounds.height.validate()?;
        let mut calls = self.common.calls.get();
        calls.measures += 1;
        self.common.calls.set(calls);
        metrics.button_size(NcCopyFromBufferOffset::new(0, self.count.get().to_string().as_bytes()))
    }
}

#[com_class(interfaces(IInset, IContainerLayout, IWidgetProbe))]
/// Concrete custom container; the host's editor owns its sole child and enforces attachment/lifetime rules.
struct Inset {
    /// Common lifecycle has no parent backpointer or mirrored geometry.
    common: Common,
    /// Authored inset; child measurement and placement use this same scalar.
    padding: Cell<i32>,
    /// Private writer; public children() returns its distinct read-only view.
    editor: ComPtr<IChildrenEditorAbi>,
}
impl Inset {
    /// This layout-only container has no input-dependent semantic state.
    fn update_content(&self, _state: WidgetUpdate) -> GeometryChange {
        GeometryChange::NO_CHANGE
    }
    /// Children paint through ordinary retained traversal, without a second recursive paint path.
    fn paint_content(&self, _state: WidgetPaint, _painter: &Painter<'_>) -> Result<(), NcStatus> {
        Ok(())
    }
    /// Child teardown can fail after topology commits; preserve the ordinary committed result.
    fn close_content(&self) -> Result<(), NcStatus> {
        self.editor.close()?.finish()
    }
}
widget!(Inset, 0, WidgetFrame::PANEL);
impl IContainer for Inset {
    /// Exposes actual topology without granting the writer needed to bypass the one-child policy.
    fn children(&self) -> Result<ComPtr<IChildrenAbi>, NcStatus> {
        self.common.state.readable()?;
        self.editor.view()
    }
    /// The underlying editor rejects invalid indices and closes only the removed subtree.
    fn remove(&self, index: u32) -> Result<ChildEditResult, NcStatus> {
        let _guard = self.common.write()?;
        self.editor.remove(index)
    }
    /// Emptying preserves the container allocation and its authored padding for future attachment.
    fn clear(&self) -> Result<ChildEditResult, NcStatus> {
        let _guard = self.common.write()?;
        self.editor.clear()
    }
}
impl IInset for Inset {
    /// Read cardinality from its authoritative sequence; no second has-child flag can become stale.
    fn set_child(&self, child: ComRef<'_, IWidgetAbi>) -> Result<ChildEditResult, NcStatus> {
        let _guard = self.common.write()?;
        if self.editor.view()?.child_count()? == 0 {
            self.editor.insert(0, child)
        } else {
            self.editor.replace(0, child)
        }
    }
    /// Validate before mutation so rejected layout settings preserve the old committed geometry.
    fn set_padding(&self, padding: i32) -> Result<(), NcStatus> {
        let _guard = self.common.write()?;
        if !(0..=i32::MAX / 2).contains(&padding) {
            return Err(nc_com::NC_E_INVALID_ARG);
        }
        if self.padding.replace(padding) != padding {
            self.common.state.record_change(GeometryChange::CHANGED);
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
    fn measure(&self, scope: LayoutScope, bounds: LayoutLimits, layout: ComRef<'_, ILayoutServiceAbi>) -> Result<Size, NcStatus> {
        let _guard = self.common.read()?;
        bounds.width.validate()?;
        bounds.height.validate()?;
        let mut calls = self.common.calls.get();
        calls.measures += 1;
        self.common.calls.set(calls);
        let inset = self.padding.get() * 2;
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
    fn place(&self, scope: LayoutScope, rect: Rect, layout: ComRef<'_, ILayoutServiceAbi>) -> Result<ContainerGeometry, NcStatus> {
        let _guard = self.common.read()?;
        if rect.width < 0 || rect.height < 0 {
            return Err(nc_com::NC_E_INVALID_ARG);
        }
        let mut calls = self.common.calls.get();
        calls.placements += 1;
        self.common.calls.set(calls);
        let padding = self.padding.get();
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
    if limit.kind == LimitKind::FINITE {
        limit.extent = limit.extent.saturating_sub(inset).max(0);
    }
    limit
}

#[com_class(class_id(EXTENSION_PROVIDER_CLASS), name = "microui.example.ExtensionProvider", version = "0.9.0-alpha.0",
    constructor(args = none), interfaces(IExtensionProvider))]
/// Stateless activation root co-packages both custom classes in this one optional assembly.
pub struct ExtensionProvider;
impl IExtensionProvider for ExtensionProvider {
    /// Authentication precedes publication; the object retains the supplied host explicitly.
    fn counter(&self, host: ComRef<'_, IWidgetHostAbi>) -> Result<ComPtr<ICounterAbi>, NcStatus> {
        host.check_mutation()?;
        Counter::from_value(Counter {
            common: Common::new(host, WidgetOptions::FRAME),
            count: Cell::new(0),
            submitted: EventSource::new(),
        })
        .query()
    }
    /// One empty editor is the only topology allocation; no native child list is created in this plugin.
    fn inset(&self, host: ComRef<'_, IWidgetHostAbi>) -> Result<ComPtr<IInsetAbi>, NcStatus> {
        host.check_mutation()?;
        let editor = host.create_children()?;
        Inset::from_value(Inset {
            common: Common::new(host, WidgetOptions::NONE),
            padding: Cell::new(4),
            editor,
        })
        .query()
    }
}
nc_com::plugin! {
    plugin_id(auto_guid = "microui.plugin.extension-example"),
    name = "microui.plugin.extension-example",
    version = "0.9.0-alpha.0",
    classes = [ExtensionProvider],
}
#[cfg(feature = "dynamic-plugin")]
nc_com::export_plugin!();
