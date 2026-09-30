//
// Copyright 2023-Present (c) Raja Lehtihet & Wael El Oraiby
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
// this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
// this list of conditions and the following disclaimer in the documentation
// and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
// may be used to endorse or promote products derived from this software without
// specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
// LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
// CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
// SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
// CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGE.
//

//! One concrete retained widget for horizontal and vertical linear layout.

use crate::{
    AtlasHandle, AvailableSpace, Constraints, ContainerLayoutCtx, ContainerWidget, Dimensioni, MeasureCtx, Recti, Skin, TrackSize, UiInputEvent, Widget,
    WidgetPaintCtx, WidgetUpdateCtx,
};

use crate::layout::TrackResolver;

/// Leading-edge direction of a one-dimensional retained layout.
///
/// Direction deliberately combines axis and reversal. This prevents the old design from storing a
/// vertical-only `reversed` flag in otherwise axis-neutral retained state, and it makes trailing-edge
/// horizontal layout available without introducing another concrete widget type.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum LinearDirection {
    /// Places child zero at the left edge and advances toward the right.
    #[default]
    LeftToRight,
    /// Places child zero at the right edge and advances toward the left.
    RightToLeft,
    /// Places child zero at the top edge and advances downward.
    TopToBottom,
    /// Places child zero at the bottom edge and advances upward.
    BottomToTop,
}

impl LinearDirection {
    /// Returns whether main-axis geometry uses horizontal coordinates.
    pub const fn is_horizontal(self) -> bool {
        // Match the named directions rather than relying on enum layout, which keeps this semantic
        // query correct if representation or variant order changes later.
        matches!(self, Self::LeftToRight | Self::RightToLeft)
    }

    /// Returns whether placement starts at the allocation's trailing edge.
    pub const fn is_reversed(self) -> bool {
        // Reversal changes only origins and cursor advancement; retained ownership order is stable.
        matches!(self, Self::RightToLeft | Self::BottomToTop)
    }

    fn axis(self) -> LinearAxis {
        // The private geometry adapter needs only the axis; leading-edge behavior remains on the
        // public direction and is consulted separately during placement.
        if self.is_horizontal() { LinearAxis::Horizontal } else { LinearAxis::Vertical }
    }
}

/// Shared cross-axis sizing behavior for a [`Linear`] container.
///
/// Cross sizing describes one shared line. Individual children may still request an exact cross
/// extent through [`LinearItem::with_fixed_cross`], but they do not change the line's own desired or
/// assigned size.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum LinearCrossSize {
    /// Uses the largest desired child cross extent and places the line at that extent.
    #[default]
    Content,
    /// Reports desired content during measurement and stretches the line across exact allocation.
    Stretch,
    /// Uses an exact non-negative cross extent and reports larger content as overflow.
    Fixed(i32),
}

impl LinearCrossSize {}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct LinearItemSpec {
    /// Parent-owned extent policy along the main axis.
    pub(crate) main: TrackSize,
    /// Optional exact cross extent, otherwise the line's resolved cross size.
    pub(crate) fixed_cross: Option<i32>,
}

/// Complete retained widget for one-dimensional child measurement and placement.
///
/// `Linear` is the one concrete widget for every horizontal or vertical sequence. Its
/// generated `UiFactory::create_linear` constructor accepts a copied flow direction and cross-axis
/// rule, while the private `LinearDirection` retains both the coordinate axis and the leading edge.
/// Every direction uses the same semantic implementation and scoped child layout service.
/// The owning COM container keeps one ChildStore and this inline algorithm keeps only
/// index-matched LinearItemSpec entries, direction, cross-axis policy, and resolved_main
/// scratch. Edge edits commit through IChildrenEditor before publishing matching metadata;
/// failure before commitment preserves both sequences. No weak child handle exists in the
/// production semantic value. Native handles below are algorithm fixtures only.
///
/// The private `LinearAxis` adapter maps width/height and x/y into main/cross operations after the
/// retained direction selects an axis. A trailing-edge direction changes only cursor origins and
/// advancement; it does not reverse ownership, measurement, traversal, or track resolution.
///
/// # Immutable measurement
///
/// The private `ContainerWidget::measure` receives `&self`, so preferred-size measurement cannot mutate
/// retained scratch. For a non-empty sequence it:
///
/// 1. Reads non-negative style spacing and separates the incoming main and cross constraints.
/// 2. Measures intrinsic main-axis requirements and constructs a scalar `TrackResolver`.
/// 3. Replays each track to obtain its resolved main extent.
/// 4. Measures the child at that exact bounded main extent so responsive content, especially text,
///    reports the correct cross extent.
/// 5. Takes the maximum cross requirement, applies the horizontal non-empty control-line minimum
///    when relevant, and resolves `LinearCrossSize` as desired content or an exact fixed extent.
/// 6. Returns the track-sequence extent and resolved cross extent as desired size.
///
/// An empty sequence returns zero desired size. Scalar replay requires no temporary vector.
/// Repeated intrinsic queries normally hit each child's bounded measurement cache instead of
/// repeatedly executing leaf measurement.
///
/// # Mutable placement
///
/// Placement receives an exact parent-owned rectangle and mutable retained state. It:
///
/// 1. Clears `resolved_main` without releasing its capacity.
/// 2. Measures an intrinsic main extent only for Content tracks. Fixed ignores intrinsic main size,
///    while bounded Flex depends on the parent's remainder.
/// 3. Constructs one `TrackResolver` and overwrites the scratch entries with exact extents.
/// 4. Measures every child once at its exact main extent to determine responsive cross content. If
///    the cross constraint is also the final allocation, the query warms the exact cache entry used
///    when runtime commits the rectangle.
/// 5. Resolves the shared line from content, the assigned cross extent, or a fixed extent.
/// 6. Walks the resolved vector once, commits exact child rectangles, and publishes logical content
///    size including cross-axis overflow.
///
/// Forward placement advances from the leading edge by `extent + gap`. Reverse placement begins at
/// the trailing edge, subtracts an extent before placing, and then subtracts the gap. Both modes
/// consume the same resolved vector, so direction changes origins without changing sizing.
///
/// Placement is linear in the child count and reuses `O(n)` output scratch. Cross-axis Stretch
/// consumes only an exact placement allocation and does not manufacture desired size during
/// measurement. Content and Fixed overflow remain visible, and every geometry accumulation uses
/// saturating arithmetic.
#[neocogi_com::com_object(neocogi_ui_nodes_contracts::Linear, ownership = exclusive)]
pub struct Linear {
    /// Non-semantic COM lifecycle and host-backed phase resources.
    runtime: crate::widget_object::WidgetRuntime,
    /// Sole writer for children mounted through this container's COM contract.
    pub(crate) editor: neocogi_com::ComBox<neocogi_ui_core_contracts::IChildrenEditorAbi>,
    /// One relationship specification per owned child, in the same order.
    specs: Vec<LinearItemSpec>,
    /// Main axis and placement order.
    direction: LinearDirection,
    /// Shared cross-axis sizing policy.
    cross_size: LinearCrossSize,
    /// Reusable placement extents invalidated by topology or track edits.
    resolved_main: Vec<i32>,
}

impl Linear {
    /// Combines validated layout policy with its runtime and authoritative COM child writer.
    pub(crate) fn from_runtime(
        runtime: crate::widget_object::WidgetRuntime,
        editor: neocogi_com::ComBox<neocogi_ui_core_contracts::IChildrenEditorAbi>,
        direction: LinearDirection,
        cross_size: LinearCrossSize,
    ) -> Self {
        Self {
            runtime,
            editor,
            specs: Vec::new(),
            direction,
            cross_size,
            resolved_main: Vec::new(),
        }
    }

    /// Returns the complete main-axis direction.
    pub const fn direction(&self) -> LinearDirection {
        // Return the value by copy; callers never receive access to retained placement scratch.
        self.direction
    }

    /// Replaces direction without changing item order or parent-owned tracks.
    pub fn set_direction(&mut self, direction: LinearDirection) {
        // Resolved main extents are scalar sizes and could be reused across a simple reversal, but an
        // axis change reinterprets them. Clearing consistently keeps both changes unambiguous.
        self.direction = direction;
        self.resolved_main.clear();
    }

    /// Returns the shared cross-axis sizing behavior.
    pub const fn cross_size(&self) -> LinearCrossSize {
        // Cross policy is semantic state and contains no reference to the child collection.
        self.cross_size
    }

    /// Replaces the shared cross-axis sizing behavior.
    pub fn set_cross_size(&mut self, cross_size: LinearCrossSize) {
        // Cross sizing does not alter main extents, but clear scratch so every geometry-affecting
        // public mutation has the same conservative invalidation boundary.
        self.cross_size = cross_size;
        self.resolved_main.clear();
    }

    /// Returns one index-matched specification, defaulting for a release-build mismatch.
    fn spec(&self, index: usize) -> LinearItemSpec {
        self.specs.get(index).copied().unwrap_or(LinearItemSpec {
            main: TrackSize::Content,
            fixed_cross: None,
        })
    }

    /// Checks the only retained parallel-collection invariant at mutation and traversal boundaries.
    fn debug_assert_synchronized(&self, child_count: usize) {
        debug_assert_eq!(
            child_count,
            self.specs.len(),
            "linear child and specification collections must remain index-synchronized"
        );
    }
}

impl crate::widget_object::WidgetAlgorithm for Linear {}

impl crate::widget_object::HostedWidget for Linear {
    /// Borrows common phase state stored directly beside Linear's layout fields.
    fn runtime(&self) -> &crate::widget_object::WidgetRuntime {
        &self.runtime
    }

    /// Exclusively borrows common phase state for mutation or resource refresh.
    fn runtime_mut(&mut self) -> &mut crate::widget_object::WidgetRuntime {
        &mut self.runtime
    }

}

impl super::ContainerAlgorithm for Linear {
    type Edge = LinearItemSpec;
    /// Copies the same relationship used by the existing track resolver.
    fn edge(&self, index: usize) -> Option<Self::Edge> {
        self.specs.get(index).copied()
    }
    /// Commits only validated metadata after the host's topology transaction, then discards derived scratch.
    fn edit_edges(&mut self, edit: super::EdgeEdit<Self::Edge>) {
        use super::EdgeEdit;
        match edit {
            EdgeEdit::Insert { index, edge } => self.specs.insert(index, edge),
            EdgeEdit::Replace { index, edge } => self.specs[index] = edge,
            EdgeEdit::Remove { index } => {
                self.specs.remove(index);
            }
            EdgeEdit::Clear => self.specs.clear(),
        }
        self.resolved_main.clear();
    }
}

use crate::widget_object::{HostedWidget, WidgetRuntime, direct_container_layout_impl, direct_widget_phase_impl};
use neocogi_ui_core_contracts::{IWidgetHostAbi};
use neocogi_ui_nodes_contracts::{ChildCrossSize, CrossSize, FlowDirection, LinearAbi, LinearEdge};
use neocogi_com::{ComPtr, NcStatus};

direct_widget_phase_impl!(Linear);
direct_container_layout_impl!(Linear);

impl Linear {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(crate) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: crate::widget_object::SharedAssets,
        direction: FlowDirection,
        cross: CrossSize,
    ) -> Result<neocogi_com::ComBox<LinearAbi>, NcStatus> {
        let value = Self::build(host, direction, cross, Some(&resources))?;
        Ok(Self::into_box(value).query().expect("Linear declares its callable class contract"))
    }

    /// Builds one linear container with either directly imported or runtime-shared resources.
    pub(crate) fn build(
        host: ComPtr<IWidgetHostAbi>,
        direction: FlowDirection,
        cross: CrossSize,
        resources: Option<&crate::widget_object::SharedAssets>,
    ) -> Result<Self, NcStatus> {
        let direction = local_direction(direction)?;
        let cross = local_cross(cross)?;
        let editor = host.create_children()?;
        let runtime = WidgetRuntime::for_resources(host, crate::WidgetOption::NO_INTERACT, resources)?;
        Ok(Self::from_runtime(runtime, editor, direction, cross))
    }
}

impl neocogi_ui_nodes_contracts::Linear for Linear {
    container_methods!();

    /// Returns the exact retained axis and leading-edge policy.
    fn direction(&self) -> Result<FlowDirection, NcStatus> {
        self.runtime().state.readable()?;
        Ok(flow_direction(Linear::direction(self)))
    }

    /// Changes direction only after validating the complete typed value.
    fn set_direction(&mut self, direction: FlowDirection) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let direction = local_direction(direction)?;
        if Linear::direction(self) != direction {
            Linear::set_direction(self, direction);
            self.runtime_mut().state.record_change(neocogi_ui_core_contracts::GeometryChange::Changed);
        }
        Ok(())
    }

    /// Returns the line's cross size independently of per-child relationships.
    fn cross_size(&self) -> Result<CrossSize, NcStatus> {
        self.runtime().state.readable()?;
        Ok(cross_size(Linear::cross_size(self)))
    }

    /// Changes cross size only after validating its typed payload.
    fn set_cross_size(&mut self, cross: CrossSize) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let cross = local_cross(cross)?;
        if Linear::cross_size(self) != cross {
            Linear::set_cross_size(self, cross);
            self.runtime_mut().state.record_change(neocogi_ui_core_contracts::GeometryChange::Changed);
        }
        Ok(())
    }

    container_edges!(LinearEdge, local_linear_edge, linear_edge);
}

/// Decodes the complete axis and leading-edge choice without a reversal side channel.
fn local_direction(direction: FlowDirection) -> Result<LinearDirection, NcStatus> {
    match direction {
        FlowDirection::LeftToRight => Ok(LinearDirection::LeftToRight),
        FlowDirection::RightToLeft => Ok(LinearDirection::RightToLeft),
        FlowDirection::TopToBottom => Ok(LinearDirection::TopToBottom),
        FlowDirection::BottomToTop => Ok(LinearDirection::BottomToTop),
        _ => Err(neocogi_com::NC_E_INVALID_ARG),
    }
}

/// Encodes the exact retained direction without inferring it from geometry.
fn flow_direction(direction: LinearDirection) -> FlowDirection {
    match direction {
        LinearDirection::LeftToRight => FlowDirection::LeftToRight,
        LinearDirection::RightToLeft => FlowDirection::RightToLeft,
        LinearDirection::TopToBottom => FlowDirection::TopToBottom,
        LinearDirection::BottomToTop => FlowDirection::BottomToTop,
    }
}

/// Validates a line cross-axis policy, including exact nonnegative extents.
fn local_cross(cross: CrossSize) -> Result<LinearCrossSize, NcStatus> {
    match cross {
        CrossSize::Content => Ok(LinearCrossSize::Content),
        CrossSize::Stretch => Ok(LinearCrossSize::Stretch),
        CrossSize::Fixed { extent } if extent >= 0 => Ok(LinearCrossSize::Fixed(extent)),
        _ => Err(neocogi_com::NC_E_INVALID_ARG),
    }
}

/// Encodes the retained line cross policy without a duplicate cached representation.
fn cross_size(cross: LinearCrossSize) -> CrossSize {
    match cross {
        LinearCrossSize::Content => CrossSize::Content,
        LinearCrossSize::Stretch => CrossSize::Stretch,
        LinearCrossSize::Fixed(extent) => CrossSize::Fixed { extent },
    }
}

/// Validates both axes of one Linear relationship before consuming a child claim.
fn local_linear_edge(edge: LinearEdge) -> Result<LinearItemSpec, NcStatus> {
    let main = super::local_track(edge.main)?;
    let fixed_cross = match edge.cross {
        ChildCrossSize::Stretch => None,
        ChildCrossSize::Fixed { extent } if extent >= 0 => Some(extent),
        _ => return Err(neocogi_com::NC_E_INVALID_ARG),
    };
    Ok(LinearItemSpec { main, fixed_cross })
}

/// Copies one retained Linear relationship into its public contract value.
fn linear_edge(edge: LinearItemSpec) -> LinearEdge {
    LinearEdge {
        main: super::layout_track(edge.main),
        cross: match edge.fixed_cross {
            Some(extent) => ChildCrossSize::Fixed { extent },
            None => ChildCrossSize::Stretch,
        },
    }
}

#[derive(Copy, Clone)]
enum LinearAxis {
    Horizontal,
    Vertical,
}

impl LinearAxis {
    fn main_space(self, constraints: Constraints) -> AvailableSpace {
        match self {
            Self::Horizontal => constraints.width,
            Self::Vertical => constraints.height,
        }
    }

    fn cross_space(self, constraints: Constraints) -> AvailableSpace {
        match self {
            Self::Horizontal => constraints.height,
            Self::Vertical => constraints.width,
        }
    }

    fn constraints(self, main: AvailableSpace, cross: AvailableSpace) -> Constraints {
        match self {
            Self::Horizontal => Constraints::new(main, cross),
            Self::Vertical => Constraints::new(cross, main),
        }
    }

    fn main(self, size: Dimensioni) -> i32 {
        match self {
            Self::Horizontal => size.width,
            Self::Vertical => size.height,
        }
    }

    fn cross(self, size: Dimensioni) -> i32 {
        match self {
            Self::Horizontal => size.height,
            Self::Vertical => size.width,
        }
    }

    fn size(self, main: i32, cross: i32) -> Dimensioni {
        match self {
            Self::Horizontal => Dimensioni::new(main, cross),
            Self::Vertical => Dimensioni::new(cross, main),
        }
    }

    fn main_origin(self, rect: Recti) -> i32 {
        match self {
            Self::Horizontal => rect.x,
            Self::Vertical => rect.y,
        }
    }

    fn cross_origin(self, rect: Recti) -> i32 {
        match self {
            Self::Horizontal => rect.y,
            Self::Vertical => rect.x,
        }
    }

    fn main_extent(self, rect: Recti) -> i32 {
        match self {
            Self::Horizontal => rect.width,
            Self::Vertical => rect.height,
        }
    }

    fn cross_extent(self, rect: Recti) -> i32 {
        match self {
            Self::Horizontal => rect.height,
            Self::Vertical => rect.width,
        }
    }

    fn rect(self, main: i32, cross: i32, main_extent: i32, cross_extent: i32) -> Recti {
        match self {
            Self::Horizontal => Recti::new(main, cross, main_extent, cross_extent),
            Self::Vertical => Recti::new(cross, main, cross_extent, main_extent),
        }
    }
}

/// Measures the retained direction and cross policy without mutating per-pass geometry.
fn measure(ctx: &mut MeasureCtx<'_>, state: &Linear, constraints: Constraints) -> Result<Dimensioni, NcStatus> {
    // Direction is retained semantic state, so private callers cannot accidentally pair horizontal
    // geometry with a vertical widget or pass an orientation-specific optional policy.
    let orientation = state.direction.axis();
    let count = ctx.child_count();
    if count == 0 {
        return Ok(Dimensioni::default());
    }
    state.debug_assert_synchronized(count);
    let gap = ctx.skin().metrics().spacing.max(0);
    let main_space = orientation.main_space(constraints);
    let cross_space = orientation.cross_space(constraints);
    let mut resolver = track_resolver_for_measure(ctx, state, orientation, main_space, cross_space, gap)?;
    let mut cross_content = 0;
    for index in 0..count {
        let placement = state.spec(index);
        let initial = measure_child(ctx, orientation, index, AvailableSpace::Unbounded, cross_space, placement)?;
        let main = resolver.next(placement.main, orientation.main(initial));
        let child = measure_child(ctx, orientation, index, AvailableSpace::Bounded(main), cross_space, placement)?;
        cross_content = cross_content.max(placement.fixed_cross.unwrap_or_else(|| orientation.cross(child)).max(0));
    }
    let cross_content = cross_content.max(minimum_content_cross(state.direction, ctx.skin(), ctx.atlas()));
    let cross = resolve_measured_cross(state.cross_size, cross_content);
    Ok(orientation.size(resolver.extent(), cross))
}

/// Summarizes child preferences without retaining temporary measurement vectors.
fn track_resolver_for_measure(
    ctx: &mut MeasureCtx<'_>,
    state: &Linear,
    orientation: LinearAxis,
    main_space: AvailableSpace,
    cross_space: AvailableSpace,
    gap: i32,
) -> Result<TrackResolver, NcStatus> {
    TrackResolver::try_new(
        main_space,
        gap,
        ctx.child_count(),
        (0..ctx.child_count()).map(|index| {
            let placement = state.spec(index);
            let child = measure_child(ctx, orientation, index, AvailableSpace::Unbounded, cross_space, placement)?;
            Ok((placement.main, orientation.main(child)))
        }),
    )
}

/// Applies the item cross constraint and propagates the child service result.
fn measure_child(
    ctx: &mut MeasureCtx<'_>,
    orientation: LinearAxis,
    index: usize,
    main: AvailableSpace,
    cross: AvailableSpace,
    placement: LinearItemSpec,
) -> Result<Dimensioni, NcStatus> {
    let cross = placement.fixed_cross.map(AvailableSpace::Bounded).unwrap_or(cross);
    ctx.measure_child(index, orientation.constraints(main, cross))
}

/// Resolves and commits one exact linear allocation.
fn place(ctx: &mut ContainerLayoutCtx<'_>, state: &mut Linear, rect: Recti) -> Result<(), NcStatus> {
    // Snapshot axis and policy before mutably borrowing scratch fields below; these copied values
    // also make it explicit that one retained configuration drives the complete placement pass.
    let orientation = state.direction.axis();
    let count = ctx.child_count();
    if count == 0 {
        ctx.set_content_size(Dimensioni::default());
        return Ok(());
    }
    state.debug_assert_synchronized(count);
    let gap = ctx.skin().metrics().spacing.max(0);
    let main_space = AvailableSpace::Bounded(orientation.main_extent(rect).max(0));
    let cross_space = AvailableSpace::Bounded(orientation.cross_extent(rect).max(0));

    // Resolve every main-axis extent once into retained scratch. Only Content needs an intrinsic
    // main-axis measurement under a bounded allocation: Fixed ignores content and Flex divides the
    // remaining bound. The subsequent bounded measurement supplies responsive cross-axis content.
    let specs = &state.specs;
    let resolved_main = &mut state.resolved_main;
    resolved_main.clear();
    for (index, spec) in specs.iter().copied().enumerate() {
        let content = if matches!(spec.main, TrackSize::Content) {
            let desired = measure_layout_child(ctx, orientation, index, AvailableSpace::Unbounded, cross_space, spec)?;
            orientation.main(desired)
        } else {
            0
        };
        resolved_main.push(content.max(0));
    }

    // TrackResolver reads the complete immutable summary before this loop replaces desired content
    // with exact allocated extents. The retained vector then drives both cross measurement and
    // placement, avoiding a second resolver and another intrinsic child-measurement replay.
    let mut resolver = TrackResolver::new(
        main_space,
        gap,
        count,
        specs.iter().zip(resolved_main.iter()).map(|(spec, content)| (spec.main, *content)),
    );
    for (spec, extent) in specs.iter().zip(resolved_main.iter_mut()) {
        *extent = resolver.next(spec.main, *extent);
    }
    let extent = resolver.extent();

    // Responsive children are measured once at their exact main-axis extent. This pass determines
    // the shared cross extent or overflow and also warms the exact measurement consumed
    // by the runtime when each child rectangle is committed below.
    let mut cross_content = 0;
    for (index, (spec, main)) in specs.iter().copied().zip(resolved_main.iter().copied()).enumerate() {
        let child = measure_layout_child(ctx, orientation, index, AvailableSpace::Bounded(main), cross_space, spec)?;
        cross_content = cross_content.max(spec.fixed_cross.unwrap_or_else(|| orientation.cross(child)).max(0));
    }
    let cross_content = cross_content.max(minimum_content_cross(state.direction, ctx.skin(), ctx.atlas()));
    let line_cross = resolve_placed_cross(state.cross_size, orientation.cross_extent(rect), cross_content);

    let reversed = state.direction.is_reversed();
    let mut cursor = if reversed {
        orientation.main_origin(rect).saturating_add(orientation.main_extent(rect))
    } else {
        orientation.main_origin(rect)
    };
    for (index, (spec, main)) in specs.iter().copied().zip(resolved_main.iter().copied()).enumerate() {
        if reversed {
            cursor = cursor.saturating_sub(main);
        }
        let cross = spec.fixed_cross.unwrap_or(line_cross);
        let child_rect = orientation.rect(cursor, orientation.cross_origin(rect), main, cross);
        ctx.layout_child(index, child_rect)?;
        if reversed {
            cursor = cursor.saturating_sub(gap);
        } else {
            cursor = cursor.saturating_add(main).saturating_add(gap);
        }
    }
    ctx.set_content_size(orientation.size(extent, line_cross.max(cross_content)));
    Ok(())
}

/// Measures placement content without substituting geometry on failure.
fn measure_layout_child(
    ctx: &mut ContainerLayoutCtx<'_>,
    orientation: LinearAxis,
    index: usize,
    main: AvailableSpace,
    cross: AvailableSpace,
    placement: LinearItemSpec,
) -> Result<Dimensioni, NcStatus> {
    let cross = placement.fixed_cross.map(AvailableSpace::Bounded).unwrap_or(cross);
    ctx.measure_child(index, orientation.constraints(main, cross))
}

impl ContainerWidget for Linear {
    /// Measures child preferences, stopping immediately on a failed service call.
    fn measure(&self, ctx: &mut MeasureCtx<'_>, constraints: Constraints) -> Result<Dimensioni, NcStatus> {
        // Route the public widget contract through the scalar immutable pass; placement scratch is
        // intentionally unavailable through this shared reference.
        measure(ctx, self, constraints)
    }

    /// Places children in policy order and returns the first failed operation.
    fn place(&mut self, ctx: &mut ContainerLayoutCtx<'_>, rect: Recti) -> Result<(), NcStatus> {
        // Indexed context access keeps collection ownership private while Linear supplies
        // matching relationship metadata and reusable exact extents.
        place(ctx, self, rect)
    }
}

impl Widget for Linear {
    fn update(&mut self, _ctx: &mut WidgetUpdateCtx<'_>, _input: Option<&UiInputEvent>) -> crate::Change {
        // Linear owns no event-driven semantic state, so runtime update traversal only visits its
        // descendants after this intentionally empty surface callback.

        // Child updates report their own changes independently of this static surface.
        crate::Change::NoChange
    }

    fn paint(&self, _ctx: &mut WidgetPaintCtx<'_>) {
        // The container emits no drawing commands of its own; child widgets paint their allocations.
    }
}

/// Returns the style-derived cross-axis minimum for an ordinary horizontal control line.
fn minimum_content_cross(direction: LinearDirection, style: &Skin, atlas: &AtlasHandle) -> i32 {
    // Vertical sequences have no manufactured minimum width. Horizontal sequences retain the
    // established control-row convention of one font line plus symmetric style padding.
    if !direction.is_horizontal() {
        return 0;
    }
    let padding = style.metrics().padding.max(0);
    (atlas.get_font_height(style.resolve_font_role(atlas, crate::FontRole::Body)) as i32)
        .saturating_add(padding.saturating_mul(2))
        .max(padding.saturating_mul(2))
}

/// Resolves desired cross size without treating a finite constraint as an allocation.
fn resolve_measured_cross(cross_size: LinearCrossSize, content: i32) -> i32 {
    let content = content.max(0);
    match cross_size {
        // Stretch affects only exact placement. During measurement it reports the same desired
        // content as Content, preserving the desired-size/allocation boundary.
        LinearCrossSize::Content | LinearCrossSize::Stretch => content,
        LinearCrossSize::Fixed(extent) => extent.max(0),
    }
}

/// Resolves the shared line extent for one exact placement allocation.
fn resolve_placed_cross(cross_size: LinearCrossSize, allocated: i32, content: i32) -> i32 {
    let content = content.max(0);
    match cross_size {
        LinearCrossSize::Content => content,
        LinearCrossSize::Stretch => allocated.max(0),
        LinearCrossSize::Fixed(extent) => extent.max(0),
    }
}

#[cfg(test)]
mod cross_size_tests {
    use super::*;

    #[test]
    fn cross_size_separates_desired_measurement_from_exact_placement() {
        // Stretch remains content-sized during measurement and consumes only an exact allocation;
        // Fixed is exact in both phases and its named constructor normalizes hostile input.
        assert_eq!(resolve_measured_cross(LinearCrossSize::Content, 20), 20);
        assert_eq!(resolve_measured_cross(LinearCrossSize::Stretch, 20), 20);
        assert_eq!(resolve_placed_cross(LinearCrossSize::Content, 80, 20), 20);
        assert_eq!(resolve_placed_cross(LinearCrossSize::Stretch, 80, 20), 80);
        assert_eq!(resolve_measured_cross(LinearCrossSize::Fixed(12), 20), 12);
        assert_eq!(resolve_placed_cross(LinearCrossSize::Fixed(12), 80, 20), 12);
        assert_eq!(resolve_measured_cross(LinearCrossSize::Fixed(-7), 20), 0);
    }
}
