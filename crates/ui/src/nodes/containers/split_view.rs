//! Two-pane layout and divider interaction live in one concrete mutable node.
//! Core retains child ownership, capture, focus, clipping, and phase ordering.

use crate::nodes::{
    AvailableSpace, Change, Constraints, ContainerLayoutCtx, ContainerWidget, ControlRole, ControlState, Dimensioni, KeyboardAction, KeyboardBehavior,
    MeasureCtx, MouseButton, Recti, UiInputEvent, Vec2i, Widget, WidgetOption, WidgetPaintCtx, WidgetUpdateCtx,
};
use crate::nodes::math::RectExt;
use crate::nodes::widget_object::{HostedWidget, WidgetRuntime, direct_container_layout_impl, direct_widget_phase_impl};
use neocogi_com::{ComPtr, NcStatus, NC_E_INVALID_ARG, NC_E_OUT_OF_RANGE};
use neocogi_ui::abi::core::{GeometryChange, IChildrenAbi, IChildrenEditorAbi, IWidgetHostAbi};
use neocogi_ui::abi::nodes::{SplitAxis, SplitOptions, SplitViewAbi};

/// Pointer offset within the divider prevents an initial jump when its edge is grabbed.
#[derive(Clone, Copy, Default)]
enum Drag {
    /// No divider gesture is owned by this node.
    #[default]
    Idle,
    /// Capture began inside the divider and remains authoritative in core.
    Active {
        /// Pointer distance from the divider's leading edge at the original press.
        offset: i32,
    },
}

/// One committed partition is shared by layout, painting, hit filtering, and dragging.
#[derive(Clone, Copy, Default)]
struct Partition {
    /// First pane's exact allocation.
    first: Recti,
    /// Divider's exact painted and interactive allocation.
    handle: Recti,
    /// Second pane's exact allocation.
    second: Recti,
    /// Lowest position permitted by the current allocation and minimum policy.
    low: i32,
    /// Highest position permitted by the current allocation and minimum policy.
    high: i32,
}

/// Reads movement-axis coordinates without depending on enum discriminant layout.
fn coordinate(axis: SplitAxis, point: Vec2i) -> i32 {
    match axis {
        SplitAxis::Horizontal => point.x,
        SplitAxis::Vertical => point.y,
        _ => unreachable!("split axis is validated before construction or mutation"),
    }
}

/// Selects the movement-axis extent of an allocation or measured preference.
fn extent(axis: SplitAxis, size: Dimensioni) -> i32 {
    coordinate(axis, Vec2i::new(size.width, size.height))
}

/// Returns a size from main/cross components for either supported axis.
fn size(axis: SplitAxis, main: i32, cross: i32) -> Dimensioni {
    match axis {
        SplitAxis::Horizontal => Dimensioni::new(main, cross),
        SplitAxis::Vertical => Dimensioni::new(cross, main),
        _ => unreachable!("split axis is validated"),
    }
}

/// Replaces the movement-axis bound while retaining the caller's cross-axis constraint.
fn limits(axis: SplitAxis, original: Constraints, main: AvailableSpace) -> Constraints {
    match axis {
        SplitAxis::Horizontal => Constraints::new(main, original.height),
        SplitAxis::Vertical => Constraints::new(original.width, main),
        _ => unreachable!("split axis is validated"),
    }
}

/// Rejects the complete invalid policy before allocating children or changing state.
fn validate(settings: SplitOptions) -> Result<(), NcStatus> {
    if !matches!(settings.axis, SplitAxis::Horizontal | SplitAxis::Vertical)
        || settings.position < 0
        || settings.minimum_first < 0
        || settings.minimum_second < 0
        || settings.handle_width <= 0
    {
        return Err(NC_E_INVALID_ARG);
    }
    Ok(())
}

/// Partitions exact bounds without negative rectangles or losing the requested position.
fn partition(settings: SplitOptions, rect: Recti) -> Partition {
    let total = extent(settings.axis, Dimensioni::new(rect.width, rect.height)).max(0);
    let handle = settings.handle_width.min(total);
    let available = total - handle;
    let minimum = i64::from(settings.minimum_first) + i64::from(settings.minimum_second);
    let (low, high) = if minimum <= i64::from(available) {
        (settings.minimum_first, available - settings.minimum_second)
    } else {
        // Both minima cannot fit. Compress them proportionally, with the remainder
        // assigned to the second pane. Growing again restores the requested split.
        let first = (i64::from(available) * i64::from(settings.minimum_first) / minimum) as i32;
        (first, first)
    };
    let first = settings.position.clamp(low, high);
    let mut result = Partition { low, high, ..Partition::default() };
    match settings.axis {
        SplitAxis::Horizontal => {
            result.first = Recti::new(rect.x, rect.y, first, rect.height.max(0));
            result.handle = Recti::new(rect.x.saturating_add(first), rect.y, handle, rect.height.max(0));
            result.second = Recti::new(result.handle.x.saturating_add(handle), rect.y, available - first, rect.height.max(0));
        }
        SplitAxis::Vertical => {
            result.first = Recti::new(rect.x, rect.y, rect.width.max(0), first);
            result.handle = Recti::new(rect.x, rect.y.saturating_add(first), rect.width.max(0), handle);
            result.second = Recti::new(rect.x, result.handle.y.saturating_add(handle), rect.width.max(0), available - first);
        }
        _ => unreachable!("split axis is validated"),
    }
    result
}

#[neocogi_com::com_object(neocogi_ui::abi::nodes::SplitView, ownership = exclusive)]
/// Concrete container owns pane sizing and its divider; no sibling handles share mutable state.
pub struct SplitView {
    /// Exclusive topology authority; callers receive only its read-only view.
    editor: neocogi_com::ComBox<IChildrenEditorAbi>,
    /// Committed child count, updated only after successful topology edits.
    count: usize,
    /// Requested policy stays separate from allocation-dependent clamping.
    settings: SplitOptions,
    /// Last successful layout partition supplies the divider's hit bounds.
    partition: Partition,
    /// Exclusive gesture state reconciled against core's capture on every update.
    drag: Drag,
    /// Last routed local pointer position distinguishes the handle from inert pane backgrounds.
    pointer: Option<Vec2i>,
    /// Drops after private children/inboxes so their receivers are released first.
    runtime: WidgetRuntime,
}

impl SplitView {
    /// Creates an empty, independently editable split view bound to one core runtime.
    pub(crate) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: crate::nodes::widget_object::SharedAssets,
        settings: SplitOptions,
        queue: Option<ComPtr<neocogi_ui::events::IEventReceiverAbi>>,
    ) -> Result<neocogi_com::ComBox<SplitViewAbi>, NcStatus> {
        validate(settings)?;
        let editor = host.create_children()?;
        let runtime = WidgetRuntime::for_resources(host, WidgetOption::NONE, Some(&resources))?;
        let mut value = Self {
            runtime,
            editor,
            count: 0,
            settings,
            partition: Partition::default(),
            drag: Drag::Idle,
            pointer: None,
        };
        value.runtime.state = value.runtime.state.with_events(queue);
        Self::into_box(value).query().map_err(|(status, _unpublished)| status)
    }

    /// Retires geometry and gesture state after a committed topology or policy edit.
    fn invalidate(&mut self) {
        self.drag = Drag::Idle;
        self.partition = Partition::default();
        self.runtime.state.record_change(GeometryChange::Changed);
    }

    /// Commits a user request against the latest bounds, notifying only on an actual change.
    fn move_divider(&mut self, requested: i32) -> Result<Change, neocogi_com::NcStatus> {
        let position = requested.clamp(self.partition.low, self.partition.high);
        let current = extent(self.settings.axis, Dimensioni::new(self.partition.first.width, self.partition.first.height));
        if position == current {
            // An immobile handle in a narrow allocation must not overwrite the
            // user's wider preferred position merely because it was dragged.
            return Ok(Change::NoChange);
        }
        self.settings.position = position;
        self.runtime.state.record_change(GeometryChange::Changed);
        let event = neocogi_ui::abi::nodes::events::split_changed(position)?;
        self.runtime.state.post(event)?;
        Ok(Change::Changed)
    }
}

impl HostedWidget for SplitView {
    /// Borrows the common lifetime and phase state beside the concrete sizing policy.
    fn runtime(&self) -> &WidgetRuntime {
        &self.runtime
    }
    /// Exclusively borrows common state without interior-mutability containers.
    fn runtime_mut(&mut self) -> &mut WidgetRuntime {
        &mut self.runtime
    }
}

impl crate::nodes::widget_object::WidgetAlgorithm for SplitView {
    /// A partial split has no interactive divider; its existing children remain eligible.
    fn effective_options(&self, options: WidgetOption) -> WidgetOption {
        if self.count == 2 { options } else { options | WidgetOption::NO_INTERACT }
    }
}

direct_widget_phase_impl!(SplitView);
direct_container_layout_impl!(SplitView);

impl neocogi_ui::abi::nodes::SplitView for SplitView {
    /// Exposes observation without exposing attachment or removal authority.
    fn children(&self) -> Result<ComPtr<IChildrenAbi>, NcStatus> {
        self.runtime.state.readable()?;
        self.editor.view()
    }
    /// Checks the two-pane capacity before the new child consumes its attachment claim.
    fn insert(
        &mut self,
        index: u32,
        child: neocogi_com::ComBox<neocogi_ui::abi::core::IWidgetAbi>,
    ) -> Result<(), (NcStatus, neocogi_com::ComBox<neocogi_ui::abi::core::IWidgetAbi>)> {
        let prepared = (|| {
            self.runtime.state.writable()?;
            if self.count >= 2 {
                return Err(NC_E_OUT_OF_RANGE);
            }
            Ok(())
        })();
        if let Err(status) = prepared {
            return Err((status, child));
        }
        self.editor.insert(index, child)?;
        self.count += 1;
        self.invalidate();
        Ok(())
    }
    /// Publishes invalidation after the editor commits replacement and drops the old owner.
    fn replace(
        &mut self,
        index: u32,
        child: neocogi_com::ComBox<neocogi_ui::abi::core::IWidgetAbi>,
    ) -> Result<(), (NcStatus, neocogi_com::ComBox<neocogi_ui::abi::core::IWidgetAbi>)> {
        let prepared = (|| {
            self.runtime.state.writable()?;
            Ok(())
        })();
        if let Err(status) = prepared {
            return Err((status, child));
        }
        self.editor.replace(index, child)?;
        self.invalidate();
        Ok(())
    }
    /// Publishes the smaller topology after dropping the removed owner.
    fn remove(&mut self, index: u32) -> Result<(), NcStatus> {
        self.runtime.state.writable()?;
        self.editor.remove(index)?;
        self.count -= 1;
        self.invalidate();
        Ok(())
    }
    /// Clears the complete split while retaining the container's identity and settings.
    fn clear(&mut self) -> Result<(), NcStatus> {
        self.runtime.state.writable()?;
        self.editor.clear()?;
        self.count = 0;
        self.invalidate();
        Ok(())
    }
    /// Returns requested policy rather than an allocation-dependent shadow value.
    fn settings(&self) -> Result<SplitOptions, NcStatus> {
        self.runtime.state.readable()?;
        Ok(self.settings)
    }
    /// Validates all fields atomically; programmatic changes do not publish user events.
    fn set_settings(&mut self, settings: SplitOptions) -> Result<(), NcStatus> {
        self.runtime.state.writable()?;
        validate(settings)?;
        if self.settings != settings {
            self.settings = settings;
            self.invalidate();
        }
        Ok(())
    }
    /// Edits one field under the same exclusive COM borrow as validation and invalidation.
    fn set_position(&mut self, position: i32) -> Result<(), NcStatus> {
        // Reuse the authoritative complete-policy setter with the current local fields.
        // No caller snapshot can accidentally restore an older axis, minimum or width.
        self.set_settings(SplitOptions { position, ..self.settings })
    }
}

impl ContainerWidget for SplitView {
    /// Measures both panes at their proposed allocations so wrapped content responds correctly.
    fn measure(&self, ctx: &mut MeasureCtx<'_>, constraints: Constraints) -> Result<Dimensioni, NcStatus> {
        Ok(match ctx.child_count() {
            0 => Dimensioni::default(),
            1 => ctx.measure_child(0, constraints)?,
            _ => {
                let axis = self.settings.axis;
                let bound = match axis {
                    SplitAxis::Horizontal => constraints.width.bound(),
                    SplitAxis::Vertical => constraints.height.bound(),
                    _ => unreachable!("split axis is validated"),
                };
                let (first_main, second_main) = match bound {
                    Some(bound) => {
                        let dimensions = size(axis, bound, 0);
                        let split = partition(self.settings, Recti::new(0, 0, dimensions.width, dimensions.height));
                        (
                            AvailableSpace::bounded(extent(axis, Dimensioni::new(split.first.width, split.first.height))),
                            AvailableSpace::bounded(extent(axis, Dimensioni::new(split.second.width, split.second.height))),
                        )
                    }
                    None => (
                        AvailableSpace::bounded(self.settings.position.max(self.settings.minimum_first)),
                        AvailableSpace::Unbounded,
                    ),
                };
                let first = ctx.measure_child(0, limits(axis, constraints, first_main))?;
                let second = ctx.measure_child(1, limits(axis, constraints, second_main))?;
                let main = extent(axis, first)
                    .max(self.settings.minimum_first)
                    .max(self.settings.position)
                    .saturating_add(self.settings.handle_width)
                    .saturating_add(extent(axis, second).max(self.settings.minimum_second));
                let cross = match axis {
                    SplitAxis::Horizontal => first.height.max(second.height),
                    SplitAxis::Vertical => first.width.max(second.width),
                    _ => unreachable!("split axis is validated"),
                };
                size(axis, main, cross)
            }
        })
    }
    /// Places the two panes around one reserved hit strip, keeping overflow inside the split.
    fn place(&mut self, ctx: &mut ContainerLayoutCtx<'_>, rect: Recti) -> Result<(), NcStatus> {
        self.partition = Partition::default();
        match ctx.child_count() {
            0 => {}
            1 => {
                ctx.layout_child(0, rect)?;
            }
            _ => {
                self.partition = partition(self.settings, rect);
                ctx.layout_child(0, self.partition.first)?;
                ctx.layout_child(1, self.partition.second)?;
            }
        }
        ctx.set_content_size(Dimensioni::new(rect.width.max(0), rect.height.max(0)));
        ctx.set_child_overflow_propagation(false);
        Ok(())
    }
    /// Only divider presses acquire capture; background motion clears hover and wheel input bubbles.
    fn accepts_event(&self, event: &UiInputEvent) -> bool {
        match event {
            UiInputEvent::MouseDown { pos, button } | UiInputEvent::MouseUp { pos, button } => {
                button.intersects(MouseButton::LEFT) && self.partition.handle.contains_point(*pos)
            }
            // Inert child backgrounds may target this container. Observe movement
            // there to clear handle hover, without starting a divider gesture.
            UiInputEvent::MouseMove { .. } | UiInputEvent::MouseDrag { .. } => true,
            UiInputEvent::Key { .. } | UiInputEvent::Cancel => true,
            _ => false,
        }
    }
}

impl Widget for SplitView {
    /// Reconciles capture first; only a press in the committed handle starts a gesture.
    fn update(&mut self, ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) -> Result<Change, neocogi_com::NcStatus> {
        if !ctx.active() {
            self.drag = Drag::Idle;
        }
        if let Some(position) = input.and_then(UiInputEvent::position) {
            self.pointer = Some(position);
        } else if !ctx.hovered() {
            self.pointer = None;
        }
        match (self.drag, input) {
            (_, Some(UiInputEvent::MouseDown { pos, button }))
                if ctx.active() && button.intersects(MouseButton::LEFT) && self.partition.handle.contains_point(*pos) =>
            {
                self.drag = Drag::Active {
                    offset: coordinate(self.settings.axis, *pos) - coordinate(self.settings.axis, Vec2i::new(self.partition.handle.x, self.partition.handle.y)),
                };
                Ok(Change::NoChange)
            }
            (Drag::Active { offset }, Some(UiInputEvent::MouseDrag { pos, .. })) => {
                self.move_divider(coordinate(self.settings.axis, *pos).saturating_sub(offset))
            }
            (_, Some(UiInputEvent::MouseUp { .. } | UiInputEvent::Cancel)) => {
                self.drag = Drag::Idle;
                Ok(Change::NoChange)
            }
            (_, Some(UiInputEvent::Key { .. })) => {
                let delta = match (self.settings.axis, ctx.action(input, self.keyboard_behavior())) {
                    (SplitAxis::Horizontal, Some(KeyboardAction::Decrease)) | (SplitAxis::Vertical, Some(KeyboardAction::Increase)) => -8,
                    (SplitAxis::Horizontal, Some(KeyboardAction::Increase)) | (SplitAxis::Vertical, Some(KeyboardAction::Decrease)) => 8,
                    _ => return Ok(Change::NoChange),
                };
                let current = extent(self.settings.axis, Dimensioni::new(self.partition.first.width, self.partition.first.height));
                self.move_divider(current.saturating_add(delta))
            }
            _ => Ok(Change::NoChange),
        }
    }
    /// Uses normal button appearance for the handle and a small centered grip for discoverability.
    fn paint(&self, ctx: &mut WidgetPaintCtx<'_>) {
        let handle = self.partition.handle;
        if handle.width <= 0 || handle.height <= 0 {
            return;
        }
        let hovered = ctx.hovered() && self.pointer.is_some_and(|position| handle.contains_point(position));
        let state = ControlState::from_interaction(ctx.enabled(), hovered, ctx.focused(), ctx.active() && hovered);
        ctx.draw_control_state(ControlRole::Button, state, handle);
        let color = ctx.skin().control(ControlRole::Button, state).content_color;
        ctx.with_clip(handle, |ctx| {
            for offset in [-4, 0, 4] {
                let (x, y) = match self.settings.axis {
                    SplitAxis::Horizontal => (handle.x + handle.width / 2, handle.y.saturating_add(handle.height / 2).saturating_add(offset)),
                    SplitAxis::Vertical => (handle.x.saturating_add(handle.width / 2).saturating_add(offset), handle.y + handle.height / 2),
                    _ => unreachable!("split axis is validated"),
                };
                ctx.draw_rect(Recti::new(x, y, 1, 1), color);
            }
        });
    }
    /// Includes a complete split in Tab traversal; arrows move in their spatial direction.
    fn keyboard_behavior(&self) -> KeyboardBehavior {
        match (self.count, self.settings.axis) {
            (2, SplitAxis::Horizontal) => KeyboardBehavior::TAB_STOP | KeyboardBehavior::ADJUST_HORIZONTAL,
            (2, SplitAxis::Vertical) => KeyboardBehavior::TAB_STOP | KeyboardBehavior::ADJUST_VERTICAL,
            _ => KeyboardBehavior::NONE,
        }
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    //! Pure partition checks cover both axes and arithmetic beyond normal desktop sizes.
    use super::*;

    /// Ordinary asymmetric limits distinguish pane-order mistakes in either orientation.
    fn settings(axis: SplitAxis) -> SplitOptions {
        SplitOptions {
            axis,
            position: 120,
            minimum_first: 80,
            minimum_second: 100,
            handle_width: 8,
        }
    }

    /// Normal bounds honor requests, clamp extremes, and never lose divider pixels.
    #[test]
    fn partitions_are_exact_on_both_axes() {
        for axis in [SplitAxis::Horizontal, SplitAxis::Vertical] {
            for (position, expected) in [(0, 80), (120, 120), (i32::MAX, 292)] {
                let dimensions = size(axis, 400, 70);
                let split = partition(
                    SplitOptions { position, ..settings(axis) },
                    Recti::new(0, 0, dimensions.width, dimensions.height),
                );
                assert_eq!(extent(axis, Dimensioni::new(split.first.width, split.first.height)), expected);
                assert_eq!(extent(axis, Dimensioni::new(split.handle.width, split.handle.height)), 8);
                assert_eq!(extent(axis, Dimensioni::new(split.second.width, split.second.height)), 392 - expected);
            }
        }
    }

    /// Undersized and extreme allocations compress minima without negative geometry or overflow.
    #[test]
    fn narrow_allocations_compress_minima_proportionally() {
        for (total, first, second, handle) in [(98, 40, 50, 8), (4, 0, 0, 4), (0, 0, 0, 0)] {
            let split = partition(settings(SplitAxis::Horizontal), Recti::new(0, 0, total, 50));
            assert_eq!((split.first.width, split.second.width, split.handle.width), (first, second, handle));
        }
        let limits = SplitOptions {
            minimum_first: i32::MAX,
            minimum_second: i32::MAX,
            ..settings(SplitAxis::Horizontal)
        };
        let split = partition(limits, Recti::new(0, 0, i32::MAX, 50));
        assert_eq!(
            i64::from(split.first.width) + i64::from(split.second.width) + i64::from(split.handle.width),
            i64::from(i32::MAX)
        );
        assert_eq!(split.low, split.high);
    }
}
