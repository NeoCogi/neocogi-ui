//! Concrete COM ownership at the retained node boundary.
use super::*;
use super::paint::PaintService;
use crate::prelude::{AtlasHandle, Dimensioni, WidgetOption, KeyboardBehavior, WidgetUpdateCtx, WidgetPaintCtx, UiInputEvent, ControlRole, Constraints, Skin};
use crate::Change;

/// Node-owned COM capabilities; geometry caches remain in the surrounding node.
pub(crate) struct ComNode {
    /// Actual common widget capability, also retained by the application proxy.
    pub(super) widget: ComPtr<IWidgetAbi>,
    /// Runtime-returned changes persist until ancestor invalidation consumes them.
    pub(super) dirty: Cell<bool>,
    /// Cached preferred-size capability.
    pub(super) layout: ComPtr<ILeafLayoutAbi>,
    /// Validated policy refreshed after updates and during setter collection.
    pub(super) properties: Cell<WidgetProperties>,
    /// Owned immutable resource metrics.
    pub(super) metrics: ComPtr<ITextMetricsAbi>,
    /// Owned scope-checked recorder, never a wrapper around WidgetPaintCtx.
    pub(super) painter: PaintService,
    /// Error/phase state without a pointer back into the retained tree.
    pub(super) host: Rc<HostState>,
}

impl ComNode {
    /// Resolves validated COM policy into the private retained router's concrete values.
    pub(crate) fn policy(&self) -> (WidgetOption, KeyboardBehavior, crate::FrameRole) {
        let properties = self.properties.get();
        let frame = match properties.frame {
            WidgetFrame::BUTTON => crate::FrameRole::Control(ControlRole::Button),
            _ => crate::FrameRole::Surface(crate::SurfaceRole::GenericFrame),
        };
        (
            WidgetOption::from_bits_retain(properties.options.bits),
            KeyboardBehavior::from_bits_retain(properties.keyboard),
            frame,
        )
    }

    /// Refreshes copied routing policy without borrowing semantic widget storage.
    fn refresh_properties(&self) {
        match self.widget.properties().and_then(|properties| {
            properties.validate()?;
            Ok(properties)
        }) {
            Ok(properties) => {
                let previous = self.properties.replace(properties);
                if previous.options != properties.options || previous.frame != properties.frame {
                    self.dirty.set(true);
                }
            }
            Err(error) => {
                self.host.fail(error);
                self.dirty.set(true);
            }
        }
    }

    /// Delivers every update, including eventless calls and input beyond primary activation.
    pub(crate) fn update(&mut self, ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) -> Change {
        let state = super::input::snapshot(ctx, input, self.policy().1);
        let (event, text) = super::input::event(input);
        let change = match self.widget.update(state, event, NcCopyFromBufferOffset::new(0, text)) {
            Ok(GeometryChange::CHANGED) => Change::Changed,
            Ok(GeometryChange::NO_CHANGE) => Change::NoChange,
            Ok(_) => {
                self.host.fail(NC_E_INVALID_ARG);
                Change::Changed
            }
            Err(error) => {
                self.host.fail(error);
                Change::Changed
            }
        };
        // Editing modes can change keyboard policy without changing preferred size.
        // Refresh after the call so the next routed input sees the committed mode.
        let _phase = Observation::begin(&self.host);
        self.refresh_properties();
        change
    }
    /// Starts an observational scope and records/replays complete owned commands.
    pub(crate) fn paint(&mut self, ctx: &mut WidgetPaintCtx<'_>) {
        let _phase = Observation::begin(&self.host);
        if let Err(error) = self.painter.paint(&self.widget, ctx) {
            self.host.fail(error);
        }
    }
}

impl ComNode {
    /// Preserves finite/unbounded constraints across the temporary ABI conversion.
    pub(crate) fn measure(&self, _skin: &Skin, _atlas: &AtlasHandle, constraints: Constraints) -> Result<Dimensioni, NcStatus> {
        let _phase = Observation::begin(&self.host);
        let bounds = LayoutLimits {
            width: axis_limit(constraints.width),
            height: axis_limit(constraints.height),
        };
        match self.layout.measure(bounds, self.metrics.as_com_ref()) {
            Ok(size) if size.width >= 0 && size.height >= 0 => Ok(Dimensioni::new(size.width, size.height)),
            result => {
                let error = result.err().unwrap_or(NC_E_INVALID_ARG);
                self.host.fail(error);
                // Keep the ancestor path dirty until a successful retry. A
                // failed measurement must never become a cached zero preference.
                self.dirty.set(true);
                Err(error)
            }
        }
    }
}

/// Converts one legacy axis without losing the difference between zero and unbounded.
fn axis_limit(space: crate::AvailableSpace) -> AxisLimit {
    match space.bound() {
        Some(extent) => AxisLimit { kind: LimitKind::FINITE, extent },
        None => AxisLimit { kind: LimitKind::UNBOUNDED, extent: 0 },
    }
}

impl ComNode {
    /// Collects sticky setter changes without a second owner or weak mutation hook.
    pub(crate) fn is_dirty(&self) -> bool {
        let _phase = Observation::begin(&self.host);
        self.refresh_properties();
        match self.widget.take_change() {
            Ok(GeometryChange::CHANGED) => self.dirty.set(true),
            Ok(GeometryChange::NO_CHANGE) => {}
            Ok(_) => {
                self.host.fail(NC_E_INVALID_ARG);
                self.dirty.set(true);
            }
            Err(error) => {
                self.host.fail(error);
                self.dirty.set(true);
            }
        }
        self.dirty.get()
    }

    /// Consumes setter and update changes together at the ancestor propagation boundary.
    pub(crate) fn take_dirty(&self) -> bool {
        self.is_dirty();
        self.dirty.replace(false)
    }

    /// Records the geometry result of an update until traversal propagates it.
    pub(crate) fn mark_dirty(&self) {
        self.dirty.set(true);
    }
}

impl Drop for ComNode {
    /// Tree removal closes the widget while application COM references may remain.
    fn drop(&mut self) {
        if let Err(error) = self.widget.close() {
            self.host.fail(error);
        }
    }
}
