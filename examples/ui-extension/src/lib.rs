//! Independently built custom widget and container. No UI or renderer implementation is linked here.
//!
//! The two concrete objects share only lifecycle plumbing. The counter owns its scalar value and
//! event producer; the inset owns its padding and the host's sole authoritative child editor.
#![deny(missing_docs)]
use neocogi_ui::UiEventSource;
mod settings;
use neocogi_ui::com::widget::WidgetState;
use neocogi_ui_core_contracts::*;
use neocogi_demo_contracts::*;
use neocogi_ui::Painter;
use neocogi_com::{ComBox, ComPtr, ComRef, NcCopyFromBufferOffset, NcOutputWriter, NcStatus, NC_E_INVALID_ARG, com_object};
use neocogi_ui_design::DesignWidgetManifest;
use neocogi_ui_document_contracts::IDesignWidgetCatalog;

/// Common state is inline; the example adds no second widget allocation or type-erased data.
struct Common {
    /// The canonical lifecycle/options/invalidation implementation used by independent authors.
    state: WidgetState,
    /// Phase counters observe actual calls rather than estimates from a shadow tree.
    calls: WidgetCalls,
}
impl Common {
    /// Retains exactly the explicit owner capability supplied by the caller.
    fn new(host: ComRef<'_, IWidgetHostAbi>, options: WidgetOptions) -> Result<Self, NcStatus> {
        Ok(Self {
            state: WidgetState::new(host.try_add_ref()?, options),
            calls: WidgetCalls::default(),
        })
    }
}

/// Common inherited methods are identical; concrete content methods below carry all policy/data.
macro_rules! widget {
    ($name:ident, $keyboard:expr, $frame:expr) => {
        impl IWidget for $name {
            /// Borrowing the host is explicit; returning it owns one ordinary COM reference.
            fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
                self.common.state.host().try_add_ref()
            }
            /// The shared state accepts exactly one attachment over the object's lifetime.
            fn mount(&mut self) -> Result<(), NcStatus> {
                self.common.state.mount()
            }
            /// Policy observation reads only common state and remains available during shared calls.
            fn properties(&self) -> Result<WidgetProperties, NcStatus> {
                Ok(WidgetProperties {
                    options: self.common.state.requested_flags()?,
                    keyboard: $keyboard,
                    frame: $frame,
                })
            }
            /// Reads the requested configuration without including transient runtime policy.
            fn requested_flags(&self) -> Result<WidgetOptions, NcStatus> {
                self.common.state.requested_flags()
            }
            /// Common option edits retain a geometry marker without copying options into content state.
            fn replace_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
                self.common.state.replace_flags(flags)
            }
            /// Sets current requested bits through the same widget-owned mutation authority.
            fn set_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
                self.common.state.set_flags(flags)
            }
            /// Clears requested bits without copying effective properties back into settings.
            fn clear_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
                self.common.state.clear_flags(flags)
            }
            /// Validate complete copied input before the concrete behavior can mutate or emit.
            fn update(&mut self, state: WidgetUpdate, event: WidgetEvent, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
                self.common.state.writable()?;
                state.validate()?;
                decode_widget_event(event, text)?;
                self.common.calls.updates += 1;
                let change = self.update_content(state)?;
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

#[neocogi_com::com_object(neocogi_demo_contracts::Counter, ownership = exclusive)]
/// Private implementation of the externally declared counter class.
struct CounterObject {
    /// Common options are authoritative here, without a duplicate presentation copy.
    common: Common,
    /// The one semantic counter value; labels are short-lived snapshots of it.
    count: u32,
    /// Accepted provider-owned constructor policy, shared with the design editor.
    settings: settings::Settings,
}
impl CounterObject {
    /// Validates runtime authority and publishes one fully initialized counter identity.
    fn create(
        host: ComRef<'_, IWidgetHostAbi>,
        queue: Option<ComPtr<neocogi_ui::events::IEventReceiverAbi>>,
        settings: settings::Settings,
    ) -> Result<ComBox<CounterAbi>, NcStatus> {
        host.check_mutation()?;
        let mut common = Common::new(host, WidgetOptions::FRAME)?;
        common.state = common.state.with_events(queue);
        Self::into_box(Self {
            common,
            count: settings.initial(),
            settings,
        })
        .query()
        .map_err(|(status, _unpublished)| status)
    }

    /// User activation commits before posting its marker and captured counter value.
    fn update_content(&mut self, state: WidgetUpdate) -> Result<GeometryChange, NcStatus> {
        if state.clicked || state.action == WidgetAction::Activate {
            self.count = self.settings.next(self.count);
            // A rejected foreign post cannot erase this already committed change.
            self.common.state.record_change(GeometryChange::Changed);
            self.common.state.post(neocogi_ui_core_contracts::events::activated()?)?;
            self.common.state.post(neocogi_demo_contracts::events::counter_changed(self.count)?)?;
            Ok(GeometryChange::Changed)
        } else {
            Ok(GeometryChange::NoChange)
        }
    }
    /// Standard body text requires no knowledge of native fonts, atlas packing or GPU types.
    fn paint_content(&self, state: WidgetPaint, painter: &mut Painter<'_>) -> Result<(), NcStatus> {
        painter.text(state.rect, &self.count.to_string())
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
        if !self.settings.accepts(value) {
            return Err(neocogi_com::NC_E_OUT_OF_RANGE);
        }
        if self.count != value {
            self.count = value;
            self.common.state.record_change(GeometryChange::Changed);
        }
        Ok(())
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

#[com_object(interfaces(IInset, IContainerLayout, IWidgetProbe), ownership = exclusive)]
/// Concrete custom container; the host's editor owns its sole child and enforces attachment/lifetime rules.
struct Inset {
    /// Common lifecycle has no parent backpointer or mirrored geometry.
    common: Common,
    /// Authored inset; child measurement and placement use this same scalar.
    padding: i32,
    /// Private writer; public children() returns its distinct read-only view.
    editor: neocogi_com::ComBox<IChildrenEditorAbi>,
}
impl Inset {
    /// This layout-only container has no input-dependent semantic state.
    fn update_content(&mut self, _state: WidgetUpdate) -> Result<GeometryChange, NcStatus> {
        Ok(GeometryChange::NoChange)
    }
    /// Children paint through ordinary retained traversal, without a second recursive paint path.
    fn paint_content(&self, _state: WidgetPaint, _painter: &mut Painter<'_>) -> Result<(), NcStatus> {
        Ok(())
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
    fn remove(&mut self, index: u32) -> Result<(), NcStatus> {
        self.common.state.writable()?;
        self.editor.remove(index)
    }
    /// Emptying preserves the container allocation and its authored padding for future attachment.
    fn clear(&mut self) -> Result<(), NcStatus> {
        self.common.state.writable()?;
        self.editor.clear()
    }
}
impl IInset for Inset {
    /// Read cardinality from its authoritative sequence; no second has-child flag can become stale.
    fn set_child(&mut self, child: neocogi_com::ComBox<IWidgetAbi>) -> Result<(), (NcStatus, neocogi_com::ComBox<IWidgetAbi>)> {
        let prepared = (|| {
            self.common.state.writable()?;
            self.editor.view()?.child_count()
        })();
        let count = match prepared {
            Ok(count) => count,
            Err(status) => return Err((status, child)),
        };
        if count == 0 {
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
    fn accepts_event(&self, event: WidgetEvent, text: NcCopyFromBufferOffset<'_, u8>) -> Result<bool, NcStatus> {
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

#[com_object(neocogi_demo_contracts::ExtensionProvider)]
/// Stateless shorthand factory creates the interface-based inset container.
pub struct ExtensionProviderObject;
impl IDesignWidgetCatalog for ExtensionProviderObject {
    /// The example publishes its leaf counter; its inset is a container and
    /// cannot be represented by the document's custom leaf schema.
    fn widget_count(&self) -> Result<u32, NcStatus> {
        Ok(1)
    }

    /// Copies generated Counter IDL metadata through the common COM catalog.
    fn describe(&self, index: u32, mut output: NcOutputWriter<'_, u8>) -> Result<u32, NcStatus> {
        if index != 0 || output.offset() != 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let manifest = DesignWidgetManifest::from_source("example.counter", settings::Settings::default().encode()?, &CounterAbi::EVENT_SOURCE)
            .map_err(|_| NC_E_INVALID_ARG)?;
        let bytes = manifest.encode().map_err(|_| NC_E_INVALID_ARG)?;
        let required = u32::try_from(bytes.len()).map_err(|_| NC_E_INVALID_ARG)?;
        if output.capacity() >= bytes.len() {
            output.extend_from_slice(&bytes)?;
        }
        Ok(required)
    }

    /// Constructs the concrete counter through COM after checking the property payload.
    fn create_widget(
        &self,
        index: u32,
        host: ComRef<'_, IWidgetHostAbi>,
        settings: NcCopyFromBufferOffset<'_, u8>,
        events: neocogi_com::NcValueRef<'_, neocogi_ui::events::EventBinding>,
    ) -> Result<ComBox<IWidgetAbi>, NcStatus> {
        if index != 0 || settings.offset() != 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let settings = settings::Settings::decode(settings.as_slice())?;
        // Catalog transport carries a COM receiver, never an untyped event payload.
        // Validate the selected product's exact queue before allocating any widget.
        CounterObject::create(
            host,
            match events.to_owned()? {
                neocogi_ui::events::EventBinding::Unobserved => None,
                neocogi_ui::events::EventBinding::Observed(queue) => Some(queue),
            },
            settings,
        )?
        .query::<IWidgetAbi>()
        .map_err(|(status, _unpublished)| status)
    }
    /// The draft uses the designer's control factory while configuration stays inside this plugin.
    fn create_editor(
        &self,
        index: u32,
        widgets: ComRef<'_, neocogi_ui::UiFactoryAbi>,
        settings: NcCopyFromBufferOffset<'_, u8>,
    ) -> Result<ComBox<neocogi_ui_document_contracts::IDesignSettingsEditorAbi>, NcStatus> {
        if index != 0 || settings.offset() != 0 {
            return Err(NC_E_INVALID_ARG);
        }
        settings::create(widgets, settings.as_slice())
    }
}

impl ExtensionProvider for ExtensionProviderObject {
    /// Creates a counter only after its supplied host accepts mutation.
    fn counter(
        &self,
        host: ComRef<'_, IWidgetHostAbi>,
        events: neocogi_com::NcValueRef<'_, neocogi_ui::events::EventBinding>,
    ) -> Result<ComBox<CounterAbi>, NcStatus> {
        CounterObject::create(host, events.to_owned()?.into_queue(), settings::Settings::default())
    }

    /// One empty editor is the only topology allocation; no native child list is created in this plugin.
    fn inset(&self, host: ComRef<'_, IWidgetHostAbi>) -> Result<ComBox<IInsetAbi>, NcStatus> {
        host.check_mutation()?;
        let editor = host.create_children()?;
        Inset::into_box(Inset {
            common: Common::new(host, WidgetOptions::NONE)?,
            padding: 4,
            editor,
        })
        .query()
        .map_err(|(status, _unpublished)| status)
    }
}
/// Acquires one owned root; object construction remains on its typed methods.
#[neocogi_com::plugin(name = "neocogi.example.ui-extension")]
fn acquire_root(_runtime: ::neocogi_com::Runtime) -> Result<neocogi_com::ComPtr<neocogi_demo_contracts::ExtensionProviderAbi>, neocogi_com::NcStatus> {
    ExtensionProviderObject::from_value(ExtensionProviderObject).query()
}
