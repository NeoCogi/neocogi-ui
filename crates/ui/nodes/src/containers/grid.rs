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

use crate::{AvailableSpace, ContainerWidget, Dimensioni, MeasureCtx, Recti, TrackSize, Widget, WidgetPaintCtx};

use crate::layout::{TrackResolver, resolve_tracks_in_place};
use super::{ContainerLayoutCtx};

/// Validated placement extent for one child owned by a [`Grid`].
///
/// A span describes the relationship between a grid and one child; it is deliberately not stored
/// on the child widget. Private fields make the non-zero invariant impossible to bypass through a struct
/// literal, so Grid layout does not need to repeatedly repair invalid input.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct GridSpan {
    /// Positive number of adjacent column tracks occupied by this one child.
    columns: usize,
    /// Positive number of adjacent row tracks occupied by this one child.
    rows: usize,
}

impl GridSpan {
    /// Default one-cell grid placement.
    pub const ONE: Self = Self { columns: 1, rows: 1 };

    /// Creates a span, normalizing a zero component to one track.
    pub const fn new(columns: usize, rows: usize) -> Self {
        Self {
            columns: if columns == 0 { 1 } else { columns },
            rows: if rows == 0 { 1 } else { rows },
        }
    }

    /// Returns the number of occupied columns.
    pub const fn columns(self) -> usize {
        self.columns
    }

    /// Returns the number of occupied rows.
    pub const fn rows(self) -> usize {
        self.rows
    }
}

impl Default for GridSpan {
    fn default() -> Self {
        Self::ONE
    }
}

/// Private owner of Grid edge metadata indexed to an adjacent [`Children`] collection.
///
/// Child ownership is deliberately not stored here. Every mutation receives the authoritative
/// collection explicitly and updates spans before returning. This keeps Grid metadata synchronized
/// while the concrete [`Container`] remains the sole strong owner of retained nodes.
#[derive(Default)]
struct GridItems {
    /// Grid-owned spans matched one-for-one with the adjacent child collection.
    spans: Vec<GridSpan>,
    /// Row-major placement derived from `spans` and `columns`.
    placements: Vec<GridPlacement>,
    /// Reused occupancy bitmap needed only while rebuilding `placements`.
    occupied: Vec<bool>,
    /// Effective column count used by the current derived placements.
    columns: usize,
}

impl GridItems {
    /// Returns the Grid-owned span associated with one child index.
    fn span(&self, index: usize) -> Option<GridSpan> {
        self.debug_assert_synchronized(self.spans.len());
        self.spans.get(index).copied()
    }

    /// Updates the placement column count and immediately restores the derived-data invariant.
    fn set_columns(&mut self, columns: usize) {
        self.columns = columns.max(1);
        self.rebuild_placements(self.spans.len());
    }

    /// Recomputes row-major placement into retained buffers.
    ///
    /// This runs only when Grid topology, spans, or column count changes. Immutable measurement
    /// reads the resulting placements and therefore never needs interior mutability or allocation.
    fn rebuild_placements(&mut self, child_count: usize) {
        grid_placements_into(child_count, self.columns, &self.spans, &mut self.placements, &mut self.occupied);
    }

    /// Checks the parallel-vector and derived-placement invariants at mutation boundaries.
    fn debug_assert_synchronized(&self, child_count: usize) {
        debug_assert_eq!(
            child_count,
            self.spans.len(),
            "Grid child and placement collections must remain index-synchronized"
        );
        debug_assert_eq!(
            child_count,
            self.placements.len(),
            "Grid child and derived placement collections must remain index-synchronized"
        );
    }
}

/// Application-facing mutable state for a retained two-dimensional container.
///
/// The COM container owns the authoritative ChildStore. This inline semantic state owns
/// child spans, both track axes, derived row-major placements, and reusable layout output.
/// Metadata remains index-matched after every committed topology edit. Grid-owned spans
/// and tracks can change without changing the child's actual COM identity.
///
/// # Spans and topology
///
/// Every private `GridSpan` is normalized to at least one row and one column. Push, insert, removal,
/// replacement, span changes, and column-track changes rebuild derived placements before the
/// updated state becomes observable. A retained occupancy bitmap is reused during those rebuilds,
/// and debug assertions keep children, spans, and placements index-synchronized.
///
/// A spanning child's intrinsic deficit is distributed across the non-Fixed tracks in its span.
/// Explicit Fixed tracks remain exact overflow boundaries. Integer deficit remainders go to earlier
/// eligible tracks, matching the track resolver's deterministic leading-pixel convention.
///
/// # Measurement and placement
///
/// Grid uses the shared scalar track resolver independently on both axes. Columns resolve before
/// rows because a child's resolved column span supplies the width constraint used to measure wrapped
/// height. The private immutable `ContainerWidget::measure` replays scalar column resolution whenever a row
/// needs a child's span width. That deliberate recomputation preserves the allocation-free `&self`
/// measurement contract.
///
/// Mutable placement instead fills retained column and row vectors, resolves every track once, and
/// reuses those exact extents for row measurement and final child rectangles. The vectors retain
/// capacity between commits and are derived scratch rather than semantic Grid state. Grid therefore
/// retains `O(columns + rows)` layout output while the shared scalar resolver itself uses constant
/// temporary space.
///
/// Grid always has an effective track on each axis, but an implicit empty Content track contributes
/// zero. An empty generic Grid remains zero-sized unless an explicit Fixed track contributes extent.
#[neocogi_com::com_object(neocogi_ui_nodes_contracts::Grid)]
pub struct Grid {
    /// Non-semantic COM lifecycle and host-backed phase resources.
    runtime: crate::widget_object::WidgetRuntime,
    /// Sole writer for children mounted through this container's COM contract.
    pub(crate) editor: neocogi_com::ComPtr<neocogi_ui_core_contracts::IChildrenEditorAbi>,
    /// Grid-owned spans and their derived row-major placements.
    items: GridItems,
    /// Horizontal track policies used by measurement and placement.
    column_tracks: Vec<TrackSize>,
    /// Vertical track policies used by measurement and placement.
    row_tracks: Vec<TrackSize>,
    /// Bound-dependent track extents reused only by the mutable layout phase.
    layout: GridLayout,
}

impl Grid {
    /// Combines validated tracks with the runtime and authoritative COM child writer.
    pub(crate) fn from_runtime(
        runtime: crate::widget_object::WidgetRuntime,
        editor: neocogi_com::ComPtr<neocogi_ui_core_contracts::IChildrenEditorAbi>,
        column_tracks: Vec<TrackSize>,
        row_tracks: Vec<TrackSize>,
    ) -> Self {
        let items = GridItems {
            spans: Vec::new(),
            placements: Vec::new(),
            occupied: Vec::new(),
            columns: column_tracks.len().max(1),
        };
        Self {
            runtime,
            editor,
            items,
            column_tracks,
            row_tracks,
            layout: GridLayout::default(),
        }
    }

    /// Returns the column track policies.
    pub fn column_tracks(&self) -> &[TrackSize] {
        &self.column_tracks
    }

    /// Replaces the column track policies without changing child ownership.
    pub fn set_column_tracks(&mut self, tracks: impl IntoIterator<Item = TrackSize>) {
        self.column_tracks = tracks.into_iter().collect();
        self.items.set_columns(self.column_tracks.len());
    }

    /// Returns the row track policies.
    pub fn row_tracks(&self) -> &[TrackSize] {
        &self.row_tracks
    }

    /// Replaces the row track policies without changing child ownership.
    pub fn set_row_tracks(&mut self, tracks: impl IntoIterator<Item = TrackSize>) {
        self.row_tracks = tracks.into_iter().collect();
    }
}

impl crate::widget_object::WidgetAlgorithm for Grid {}

impl crate::widget_object::HostedWidget for Grid {
    /// Borrows common phase state stored directly beside Grid's layout fields.
    fn runtime(&self) -> &crate::widget_object::WidgetRuntime {
        &self.runtime
    }

    /// Exclusively borrows common phase state for mutation or resource refresh.
    fn runtime_mut(&mut self) -> &mut crate::widget_object::WidgetRuntime {
        &mut self.runtime
    }

    /// Closes the authoritative COM child sequence when this Grid is retired.
    fn close_contents(&mut self) -> Result<(), neocogi_com::NcStatus> {
        self.editor.close()?.finish()
    }
}

impl super::ContainerAlgorithm for Grid {
    type Edge = GridSpan;
    /// Returns the same span consumed by the existing row-major placement algorithm.
    fn edge(&self, index: usize) -> Option<Self::Edge> {
        self.items.span(index)
    }
    /// Commits validated metadata, rebuilding placements once after the matching topology commit.
    fn edit_edges(&mut self, edit: super::EdgeEdit<Self::Edge>) {
        use super::EdgeEdit;
        match edit {
            EdgeEdit::Insert { index, edge } => self.items.spans.insert(index, edge),
            EdgeEdit::Replace { index, edge } => self.items.spans[index] = edge,
            EdgeEdit::Remove { index } => {
                self.items.spans.remove(index);
            }
            EdgeEdit::Clear => self.items.spans.clear(),
        }
        self.items.rebuild_placements(self.items.spans.len());
    }
}

use crate::widget_object::{HostedWidget, WidgetRuntime, direct_container_layout_impl, direct_widget_phase_impl};
use neocogi_ui_core_contracts::{IWidgetHostAbi};
use neocogi_ui_nodes_contracts::{GridAbi, GridExtent, LayoutTrackAbi};
use neocogi_com::{ComPtr, NcCopyFromBufferOffset, NcCopyToBufferOffset, NcStatus};

direct_widget_phase_impl!(Grid);
direct_container_layout_impl!(Grid);

impl Grid {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(crate) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: crate::widget_object::SharedAssets,
        columns: NcCopyFromBufferOffset<'_, LayoutTrackAbi>,
        rows: NcCopyFromBufferOffset<'_, LayoutTrackAbi>,
    ) -> Result<ComPtr<GridAbi>, NcStatus> {
        let value = Self::build(host, columns, rows, Some(&resources))?;
        Ok(Self::from_value(value).query().expect("Grid declares its callable class contract"))
    }

    /// Builds one Grid with either directly imported or runtime-shared resources.
    fn build(
        host: ComPtr<IWidgetHostAbi>,
        columns: NcCopyFromBufferOffset<'_, LayoutTrackAbi>,
        rows: NcCopyFromBufferOffset<'_, LayoutTrackAbi>,
        resources: Option<&crate::widget_object::SharedAssets>,
    ) -> Result<Self, NcStatus> {
        let columns = local_tracks(columns)?;
        let rows = local_tracks(rows)?;
        let editor = host.create_children()?;
        let runtime = WidgetRuntime::for_resources(host, crate::WidgetOption::NO_INTERACT, resources)?;
        Ok(Self::from_runtime(runtime, editor, columns, rows))
    }
}

impl neocogi_ui_nodes_contracts::Grid for Grid {
    container_methods!();

    /// Copies column policy from the one concrete Grid state.
    fn columns(&self, output: NcCopyToBufferOffset<'_, LayoutTrackAbi>) -> Result<u32, NcStatus> {
        self.runtime().state.readable()?;
        copy_tracks(self.column_tracks(), output)
    }

    /// Validates all input before rebuilding column-dependent span placement.
    fn set_columns(&mut self, tracks: NcCopyFromBufferOffset<'_, LayoutTrackAbi>) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let tracks = local_tracks(tracks)?;
        if self.column_tracks() != tracks {
            self.set_column_tracks(tracks);
            self.runtime_mut().state.record_change(neocogi_ui_core_contracts::GeometryChange::Changed);
        }
        Ok(())
    }

    /// Copies row policy without exposing Grid's placement scratch.
    fn rows(&self, output: NcCopyToBufferOffset<'_, LayoutTrackAbi>) -> Result<u32, NcStatus> {
        self.runtime().state.readable()?;
        copy_tracks(self.row_tracks(), output)
    }

    /// Replaces row policy only after validating the complete typed input.
    fn set_rows(&mut self, tracks: NcCopyFromBufferOffset<'_, LayoutTrackAbi>) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let tracks = local_tracks(tracks)?;
        if self.row_tracks() != tracks {
            self.set_row_tracks(tracks);
            self.runtime_mut().state.record_change(neocogi_ui_core_contracts::GeometryChange::Changed);
        }
        Ok(())
    }

    container_edges!(GridExtent, local_grid_edge, grid_edge);
}

/// Validates a complete track sequence before constructing or mutating Grid state.
fn local_tracks(tracks: NcCopyFromBufferOffset<'_, LayoutTrackAbi>) -> Result<Vec<TrackSize>, NcStatus> {
    if tracks.offset() != 0 || tracks.len() > u32::MAX as usize {
        return Err(neocogi_com::NC_E_INVALID_ARG);
    }
    tracks.as_slice().iter().copied().map(super::local_track).collect()
}

/// Copies a requested track range and returns the complete retained cardinality.
fn copy_tracks(tracks: &[TrackSize], mut output: NcCopyToBufferOffset<'_, LayoutTrackAbi>) -> Result<u32, NcStatus> {
    let offset = usize::try_from(output.offset()).map_err(|_| neocogi_com::NC_E_INVALID_ARG)?;
    let source = tracks.get(offset..).ok_or(neocogi_com::NC_E_INVALID_ARG)?;
    for (target, track) in output.as_mut_slice().iter_mut().zip(source) {
        *target = super::layout_track(*track);
    }
    u32::try_from(tracks.len()).map_err(|_| neocogi_com::NC_E_OUT_OF_RANGE)
}

/// Rejects empty spans before the existing Grid placement algorithm can clamp their edges.
fn local_grid_edge(edge: GridExtent) -> Result<GridSpan, NcStatus> {
    if edge.columns == 0 || edge.rows == 0 {
        return Err(neocogi_com::NC_E_INVALID_ARG);
    }
    Ok(GridSpan::new(edge.columns as usize, edge.rows as usize))
}

/// Copies one positive retained span into its public contract value.
fn grid_edge(edge: GridSpan) -> GridExtent {
    GridExtent {
        columns: edge.columns() as u32,
        rows: edge.rows() as u32,
    }
}

/// Reusable output buffers for one committed Grid layout.
///
/// Measurement does not read or mutate these buffers. It follows the scalar path below; layout
/// fills them so each resolved track is computed once before placing all children.
#[derive(Default)]
struct GridLayout {
    /// Resolved widths indexed by column.
    columns: Vec<i32>,
    /// Resolved heights indexed by row.
    rows: Vec<i32>,
}

/// Shared child-measurement surface used by Grid's immutable and placement-time solvers.
trait GridMeasureCtx {
    /// Returns nonnegative spacing shared by both axis solvers.
    fn spacing(&self) -> i32;
    /// Returns a child preference or the unchanged service failure.
    fn measure_child(&mut self, index: usize, constraints: crate::Constraints) -> Result<Dimensioni, NcStatus>;
}

impl GridMeasureCtx for MeasureCtx<'_> {
    /// Reads the appearance revision installed for this measurement pass.
    fn spacing(&self) -> i32 {
        self.skin().metrics().spacing.max(0)
    }

    /// Forwards failures without replacing missing geometry with zero.
    fn measure_child(&mut self, index: usize, constraints: crate::Constraints) -> Result<Dimensioni, NcStatus> {
        MeasureCtx::measure_child(self, index, constraints)
    }
}

/// Placement supplies the same indexed measurement capability without an adapter struct.
impl GridMeasureCtx for ContainerLayoutCtx<'_> {
    /// Reads the common spacing policy used by both measurement phases.
    fn spacing(&self) -> i32 {
        self.skin().metrics().spacing.max(0)
    }
    /// Measures a direct child through the bounded placement context.
    fn measure_child(&mut self, index: usize, constraints: crate::Constraints) -> Result<Dimensioni, NcStatus> {
        ContainerLayoutCtx::measure_child(self, index, constraints)
    }
}

impl ContainerWidget for Grid {
    /// Measures both axes and returns the first child service failure.
    fn measure(&self, ctx: &mut MeasureCtx<'_>, constraints: crate::Constraints) -> Result<Dimensioni, NcStatus> {
        grid_size(ctx, self, constraints)
    }

    /// Commits placements only after both track axes have resolved successfully.
    fn place(&mut self, ctx: &mut ContainerLayoutCtx<'_>, rect: Recti) -> Result<(), NcStatus> {
        // Move the reusable vectors out temporarily so layout can read the remaining Grid fields
        // without adding another RefCell or allocating per frame.
        let mut layout = std::mem::take(&mut self.layout);
        let result = layout_grid(self, ctx, rect, &mut layout);
        // Preserve reusable storage on failure; the next attempt recomputes every extent.
        self.layout = layout;
        result
    }
}

impl Widget for Grid {
    fn paint(&self, _ctx: &mut WidgetPaintCtx<'_>) {}
}

/// Resolves both Grid axes and commits every child at its derived placement.
///
/// Columns are resolved before rows because a child's allocated column span is its text-wrapping
/// bound and therefore affects the preferred height contributed to row tracks.
fn layout_grid(state: &Grid, ctx: &mut ContainerLayoutCtx<'_>, rect: Recti, layout: &mut GridLayout) -> Result<(), NcStatus> {
    let spacing = ctx.skin().metrics().spacing.max(0);
    // Resolve and retain each column width once for both row measurement and final placement.
    let available_width = AvailableSpace::bounded(rect.width);
    let available_height = AvailableSpace::bounded(rect.height);
    {
        resolve_columns_into(ctx, state, available_width, &mut layout.columns)?;

        // Row preferences are measured using the resolved column spans, then allocated vertically.
        resolve_rows_into(ctx, state, available_height, &layout.columns, &mut layout.rows)?;
    }

    // Placement and both track vectors are now authoritative for this layout commit.
    for placement in state.items.placements.iter().copied() {
        // child_origin = grid_origin + preceding_track_offset.
        let x = rect.x.saturating_add(track_offset(&layout.columns, placement.column, spacing));
        let y = rect.y.saturating_add(track_offset(&layout.rows, placement.row, spacing));
        let width = track_span(&layout.columns, placement.column, placement.column_span, spacing);
        let height = track_span(&layout.rows, placement.row, placement.row_span, spacing);
        ctx.layout_child(placement.child_index, Recti::new(x, y, width, height))?;
    }
    Ok(())
}

/// Measures Grid preferred size without mutating Grid state or layout buffers.
///
/// The pure measurement path resolves scalar column spans on demand when measuring row content.
/// This costs recomputation but preserves `Widget::measure(&self)` and performs no allocation.
fn grid_size(ctx: &mut MeasureCtx<'_>, state: &Grid, constraints: crate::Constraints) -> Result<Dimensioni, NcStatus> {
    let spacing = ctx.skin().metrics().spacing.max(0);
    let (columns, rows) = grid_dimensions(state);
    // Column preferences depend only on intrinsic child widths and Grid column spans.
    let width = measured_tracks(&state.column_tracks, columns, constraints.width, spacing, |index| {
        preferred_column(ctx, state, index, spacing)
    })?;
    // Row preferences additionally depend on each child's resolved column-span width.
    let height = measured_tracks(&state.row_tracks, rows, constraints.height, spacing, |index| {
        preferred_row(ctx, state, index, spacing, |ctx, placement| {
            resolved_column_span(ctx, state, spacing, constraints.width, placement)
        })
    })?;
    Ok(Dimensioni::new(width, height))
}

/// Returns the effective column and row counts represented by Grid configuration and placement.
fn grid_dimensions(state: &Grid) -> (usize, usize) {
    // A Grid always exposes at least one track on each axis, even when it has no children or
    // explicit tracks. Row spans may extend beyond the explicit row-track list.
    let columns = state.column_tracks.len().max(1);
    let rows = state
        .items
        .placements
        .iter()
        // occupied_row_end = placement_row + row_span.
        .map(|placement| placement.row.saturating_add(placement.row_span))
        .max()
        .unwrap_or(0)
        .max(state.row_tracks.len())
        .max(1);
    (columns, rows)
}

/// Computes one column's intrinsic minimum from its explicit track and spanning children.
fn preferred_column(ctx: &mut impl GridMeasureCtx, state: &Grid, index: usize, spacing: i32) -> Result<i32, NcStatus> {
    // A fixed track is an overflow boundary. Measuring children cannot change its extent.
    if let TrackSize::Fixed(extent) = track(&state.column_tracks, index) {
        return Ok(extent.max(0));
    }
    let mut preferred = 0;

    // Only children whose spans cover this column can increase its minimum.
    for placement in state.items.placements.iter().copied() {
        // column_end = first_column + column_span.
        let column_end = placement.column.saturating_add(placement.column_span);
        if index < placement.column || index >= column_end {
            continue;
        }
        let minimum = ctx.measure_child(placement.child_index, crate::Constraints::unbounded())?.width;
        preferred = preferred.max(contribution_for_track(
            &state.column_tracks,
            index,
            placement.column,
            placement.column_span,
            spacing,
            minimum,
        ));
    }
    Ok(preferred)
}

/// Computes one row's intrinsic minimum using caller-provided resolved child-span widths.
///
/// Layout supplies widths from its resolved column vector. Immutable measurement supplies a pure
/// scalar resolver. Keeping that choice at the call site prevents this helper from knowing about
/// caches, phases, or mutable container state.
fn preferred_row<C: GridMeasureCtx>(
    ctx: &mut C,
    state: &Grid,
    index: usize,
    spacing: i32,
    mut child_width: impl FnMut(&mut C, GridPlacement) -> Result<i32, NcStatus>,
) -> Result<i32, NcStatus> {
    // Skip both child measurement and column-span resolution when neither can affect this row.
    if let TrackSize::Fixed(extent) = track(&state.row_tracks, index) {
        return Ok(extent.max(0));
    }
    let mut preferred = 0;
    // Measure only children crossing this row, at the width of their complete column span.
    for placement in state.items.placements.iter().copied() {
        // row_end = first_row + row_span.
        let row_end = placement.row.saturating_add(placement.row_span);
        if index < placement.row || index >= row_end {
            continue;
        }
        let width = child_width(ctx, placement)?;
        let minimum = ctx
            .measure_child(
                placement.child_index,
                crate::Constraints::new(AvailableSpace::bounded(width), AvailableSpace::Unbounded),
            )?
            .height;
        preferred = preferred.max(contribution_for_track(
            &state.row_tracks,
            index,
            placement.row,
            placement.row_span,
            spacing,
            minimum,
        ));
    }
    Ok(preferred)
}

/// Returns the minimum required from `index` for one spanning child contribution.
///
/// The span begins at each track's explicit fixed contribution. Any remaining deficit is divided among
/// non-fixed tracks; fixed tracks never grow to hide overflow. Integer remainders are assigned from
/// left to right so the result is deterministic and, when a flexible track exists, the complete
/// span covers the child minimum.
fn contribution_for_track(tracks: &[TrackSize], index: usize, start: usize, span: usize, spacing: i32, minimum: i32) -> i32 {
    let current_track = track(tracks, index);
    let base = fixed_track_extent(current_track);
    if matches!(current_track, TrackSize::Fixed(_)) {
        // Fixed is an explicit overflow boundary, not a minimum that content may enlarge.
        return base;
    }

    // Establish how much of the child's minimum is already covered by base tracks and the spacing
    // internal to its span. `rank` identifies this track among only the flexible tracks.
    // span_end = span_start + max(span, 1).
    let end = start.saturating_add(span.max(1));
    // internal_gap_count = span - 1; initial_coverage starts with spacing * internal_gap_count.
    let mut initial = spacing.max(0).saturating_mul(span.saturating_sub(1) as i32);
    let mut flexible = 0_i32;
    let mut rank = 0_i32;
    for track_index in start..end {
        let track_size = track(tracks, track_index);
        // initial_coverage = previous_coverage + track_intrinsic_extent.
        initial = initial.saturating_add(fixed_track_extent(track_size));
        if !matches!(track_size, TrackSize::Fixed(_)) {
            if track_index < index {
                rank += 1;
            }
            flexible += 1;
        }
    }
    if flexible == 0 {
        return base;
    }

    // Divide only the uncovered pixels. Earlier flexible tracks receive the indivisible remainder.
    // deficit = max(child_minimum - initial_coverage, 0).
    let deficit = minimum.max(0).saturating_sub(initial);
    let increment = deficit / flexible + i32::from(rank < deficit % flexible);
    // contributed_extent = fixed_base_extent + allocated_deficit_increment.
    base.saturating_add(increment)
}

/// Resolves the width of one child's column span during Grid preferred-size measurement.
///
/// No resolved-width vector is available in `measure(&self)`, so this function replays the scalar
/// column allocator and accumulates only the requested span.
fn resolved_column_span(
    ctx: &mut impl GridMeasureCtx,
    state: &Grid,
    spacing: i32,
    available_width: AvailableSpace,
    placement: GridPlacement,
) -> Result<i32, NcStatus> {
    let columns = state.column_tracks.len().max(1);
    let mut resolver = TrackResolver::try_new(
        available_width,
        spacing,
        columns,
        (0..columns).map(|index| Ok((track(&state.column_tracks, index), preferred_column(ctx, state, index, spacing)?))),
    )?;
    let mut width = 0_i32;
    // placement_end = first_column + column_span.
    let placement_end = placement.column.saturating_add(placement.column_span);
    for index in 0..columns {
        let size = resolver.next(track(&state.column_tracks, index), preferred_column(ctx, state, index, spacing)?);
        if index >= placement.column && index < placement_end {
            // span_width = previous_span_width + resolved_track_width.
            width = width.saturating_add(size);
        }
    }
    // internal_gap_count = column_span - 1; spacing_total = spacing * internal_gap_count.
    let gap_count = placement.column_span.saturating_sub(1) as i32;
    let spacing_total = spacing.max(0).saturating_mul(gap_count);
    // complete_span_width = resolved_track_widths + internal_spacing.
    Ok(width.saturating_add(spacing_total))
}

/// Fills the layout-phase column buffer with resolved widths.
fn resolve_columns_into(ctx: &mut impl GridMeasureCtx, state: &Grid, available_width: AvailableSpace, columns: &mut Vec<i32>) -> Result<(), NcStatus> {
    let count = state.column_tracks.len().max(1);
    let spacing = ctx.spacing();
    columns.clear();
    for index in 0..count {
        columns.push(preferred_column(ctx, state, index, spacing)?);
    }
    resolve_tracks_in_place(available_width, spacing, &state.column_tracks, columns);
    Ok(())
}

/// Fills the layout-phase row buffer using already resolved column widths.
fn resolve_rows_into(
    ctx: &mut impl GridMeasureCtx,
    state: &Grid,
    available_height: AvailableSpace,
    columns: &[i32],
    rows: &mut Vec<i32>,
) -> Result<(), NcStatus> {
    let (_, count) = grid_dimensions(state);
    let spacing = ctx.spacing();
    rows.clear();
    for index in 0..count {
        rows.push(preferred_row(ctx, state, index, spacing, |_ctx, placement| {
            Ok(track_span(columns, placement.column, placement.column_span, spacing))
        })?);
    }
    resolve_tracks_in_place(available_height, spacing, &state.row_tracks, rows);
    Ok(())
}

/// Measures one complete track axis through the allocation rules shared with layout.
///
/// Unbounded axes can return the intrinsic summary directly. Bounded axes replay preferences in
/// order because `Remainder` depends on the pixels consumed by earlier tracks.
fn measured_tracks(
    tracks: &[TrackSize],
    count: usize,
    available: AvailableSpace,
    spacing: i32,
    mut preferred: impl FnMut(usize) -> Result<i32, NcStatus>,
) -> Result<i32, NcStatus> {
    let mut resolver = TrackResolver::try_new(available, spacing, count, (0..count).map(|index| Ok((track(tracks, index), preferred(index)?))))?;
    for index in 0..count {
        resolver.next(track(tracks, index), preferred(index)?);
    }
    Ok(resolver.extent())
}

/// Returns an explicit track or `Content` for an implicit track.
fn track(tracks: &[TrackSize], index: usize) -> TrackSize {
    tracks.get(index).copied().unwrap_or(TrackSize::Content)
}

fn fixed_track_extent(track: TrackSize) -> i32 {
    match track {
        TrackSize::Fixed(extent) => extent.max(0),
        TrackSize::Content | TrackSize::Flex(_) => 0,
    }
}

/// Sums a resolved track span including only the gaps internal to that span.
fn track_span(tracks: &[i32], start: usize, span: usize, spacing: i32) -> i32 {
    // track_total = sum(resolved_track_extents in the requested span).
    let track_total = tracks.iter().skip(start).take(span.max(1)).fold(0_i32, |sum, size| sum.saturating_add(*size));
    // internal_gap_count = span - 1; spacing_total = spacing * internal_gap_count.
    let gap_count = span.saturating_sub(1) as i32;
    let spacing_total = spacing.max(0).saturating_mul(gap_count);
    // span_extent = track_total + spacing_total.
    track_total.saturating_add(spacing_total)
}

/// Returns the offset preceding a track after earlier extents and their following gaps.
fn track_offset(tracks: &[i32], count: usize, spacing: i32) -> i32 {
    // preceding_track_total = sum(resolved extents before the requested track).
    let track_total = tracks.iter().take(count).fold(0_i32, |sum, size| sum.saturating_add(*size));
    // preceding_spacing_total = spacing * preceding_track_count.
    let spacing_total = spacing.max(0).saturating_mul(count as i32);
    // track_offset = preceding_track_total + preceding_spacing_total.
    track_total.saturating_add(spacing_total)
}

/// Concrete row-major placement of one child inside a Grid.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct GridPlacement {
    /// Index into the authoritative child and span collections.
    child_index: usize,
    /// First occupied column.
    column: usize,
    /// First occupied row.
    row: usize,
    /// Effective column span after clamping at the right Grid edge.
    column_span: usize,
    /// Validated row span, which may extend implicit rows.
    row_span: usize,
}

/// Rebuilds row-major placements and occupancy into reusable Grid-owned buffers.
fn grid_placements_into(child_count: usize, columns: usize, spans: &[GridSpan], placements: &mut Vec<GridPlacement>, occupied: &mut Vec<bool>) {
    let columns = columns.max(1);
    occupied.clear();
    placements.clear();
    placements.reserve(child_count);
    let mut search_row = 0;
    let mut search_column = 0;

    // Continue after each placed span rather than restarting at the origin for every child.
    for index in 0..child_count {
        let (row, column) = first_free_grid_cell(occupied, columns, search_row, search_column);
        let span = spans.get(index).copied().unwrap_or(GridSpan::ONE);
        // A span is intrinsically non-zero. Only the parent-dependent right-edge clamp remains.
        // column_span = min(requested_span, max(column_count - start_column, 1)).
        let column_span = span.columns().min(columns.saturating_sub(column).max(1));
        let row_span = span.rows();
        mark_grid_occupied(occupied, columns, row, column, row_span, column_span);
        placements.push(GridPlacement {
            child_index: index,
            column,
            row,
            column_span,
            row_span,
        });

        search_row = row;
        // next_search_column = placed_column + occupied_column_span.
        search_column = column.saturating_add(column_span);
        while search_column >= columns {
            search_column -= columns;
            search_row += 1;
        }
    }
}

/// Finds the first unoccupied cell at or after a row-major search position.
fn first_free_grid_cell(occupied: &mut Vec<bool>, columns: usize, mut row: usize, mut column: usize) -> (usize, usize) {
    loop {
        // Grow by complete rows so `row * columns + column` remains a valid flat index.
        // required_cells = (row + 1) * column_count.
        let required = row.saturating_add(1).saturating_mul(columns);
        if occupied.len() < required {
            occupied.resize(required, false);
        }
        while column < columns {
            if !occupied[row * columns + column] {
                return (row, column);
            }
            column += 1;
        }
        row += 1;
        column = 0;
    }
}

/// Marks every cell covered by one validated placement span.
fn mark_grid_occupied(occupied: &mut Vec<bool>, columns: usize, row: usize, column: usize, row_span: usize, column_span: usize) {
    // Row spans may extend the current bitmap; column spans are clamped at the right Grid edge.
    // end_row = start_row + max(row_span, 1); required_cells = end_row * column_count.
    let end_row = row.saturating_add(row_span.max(1));
    let required = end_row.saturating_mul(columns);
    if occupied.len() < required {
        occupied.resize(required, false);
    }
    // end_column = min(start_column + max(column_span, 1), column_count).
    let end_column = column.saturating_add(column_span.max(1)).min(columns);
    for y in row..end_row {
        for x in column..end_column {
            occupied[y * columns + x] = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_span_normalizes_zero_components_once() {
        assert_eq!(GridSpan::new(0, 0), GridSpan::ONE);
        assert_eq!(GridSpan::new(0, 3), GridSpan::new(1, 3));
        assert_eq!(GridSpan::new(4, 0), GridSpan::new(4, 1));
        assert_eq!(GridSpan::new(4, 3).columns(), 4);
        assert_eq!(GridSpan::new(4, 3).rows(), 3);
    }

    #[test]
    fn placement_uses_validated_spans_and_clamps_only_at_the_right_edge() {
        let mut placements = Vec::new();
        let mut occupied = Vec::new();
        grid_placements_into(
            3,
            3,
            &[GridSpan::new(2, 1), GridSpan::new(8, 1), GridSpan::new(1, 2)],
            &mut placements,
            &mut occupied,
        );

        assert_eq!(
            placements,
            [
                GridPlacement {
                    child_index: 0,
                    column: 0,
                    row: 0,
                    column_span: 2,
                    row_span: 1,
                },
                GridPlacement {
                    child_index: 1,
                    column: 2,
                    row: 0,
                    column_span: 1,
                    row_span: 1,
                },
                GridPlacement {
                    child_index: 2,
                    column: 0,
                    row: 1,
                    column_span: 1,
                    row_span: 2,
                },
            ]
        );
    }

    #[test]
    fn grid_fixed_span_tracks_do_not_grow_to_hide_child_overflow() {
        let tracks = [TrackSize::Fixed(10), TrackSize::Fixed(12)];
        assert_eq!(contribution_for_track(&tracks, 0, 0, 2, 2, 50), 10);
        assert_eq!(contribution_for_track(&tracks, 1, 0, 2, 2, 50), 12);
    }

    /// Fixed grid measurement must not cross into any child's measurement service.
    struct FixedGridMetrics;
    impl GridMeasureCtx for FixedGridMetrics {
        /// Spacing contributes only between the explicitly sized tracks.
        fn spacing(&self) -> i32 {
            2
        }
        /// Even one request is redundant: child content cannot change either fixed axis.
        fn measure_child(&mut self, _: usize, _: crate::Constraints) -> Result<Dimensioni, NcStatus> {
            panic!("fixed track measured child content");
        }
    }

    /// Wide fixed grids resolve their configured extents without work proportional to child content.
    #[test]
    fn fixed_grid_resolves_both_axes_without_measuring_children() {
        let mut grid = crate::test_support::algorithm::Algorithm::new(crate::WidgetOption::NONE, |runtime| {
            let editor = runtime.state.host().create_children().unwrap();
            Grid::from_runtime(runtime, editor, vec![TrackSize::Fixed(24); 32], vec![TrackSize::Fixed(24); 32])
        });
        let items = &mut grid.items;
        grid_placements_into(1024, 32, &[], &mut items.placements, &mut items.occupied);
        let mut layout = GridLayout::default();
        resolve_columns_into(&mut FixedGridMetrics, &grid, AvailableSpace::bounded(1000), &mut layout.columns).unwrap();
        resolve_rows_into(&mut FixedGridMetrics, &grid, AvailableSpace::bounded(1000), &layout.columns, &mut layout.rows).unwrap();
        assert_eq!(layout.columns, vec![24; 32]);
        assert_eq!(layout.rows, vec![24; 32]);
    }
    /// Records synchronous measurement calls with ordinary exclusive mutation.
    struct FailingMetrics {
        /// Number of children reached before the selected failure.
        calls: usize,
        /// Exact invocation that rejects measurement, counting from one.
        fail_at: usize,
    }
    impl GridMeasureCtx for FailingMetrics {
        /// Omit gaps so the test isolates service failures from track arithmetic.
        fn spacing(&self) -> i32 {
            0
        }
        /// Reports a distinctive status at the requested point in scalar replay.
        fn measure_child(&mut self, _: usize, _: crate::Constraints) -> Result<Dimensioni, NcStatus> {
            self.calls += 1;
            match self.calls == self.fail_at {
                true => Err(neocogi_com::NC_E_OUT_OF_RANGE),
                false => Ok(Dimensioni::new(12, 8)),
            }
        }
    }

    /// Failure during either track axis stops the solver before subsequent child measurements.
    #[test]
    fn grid_measurement_stops_at_the_first_failed_child() {
        let mut grid = crate::test_support::algorithm::Algorithm::new(crate::WidgetOption::NONE, |runtime| {
            let editor = runtime.state.host().create_children().unwrap();
            Grid::from_runtime(runtime, editor, vec![TrackSize::Content; 2], vec![TrackSize::Content; 2])
        });
        let items = &mut grid.items;
        grid_placements_into(4, 2, &[], &mut items.placements, &mut items.occupied);
        let mut context = FailingMetrics { calls: 0, fail_at: 2 };
        let mut columns = Vec::new();
        assert_eq!(
            resolve_columns_into(&mut context, &grid, AvailableSpace::Unbounded, &mut columns),
            Err(neocogi_com::NC_E_OUT_OF_RANGE)
        );
        assert_eq!(context.calls, 2);
        context.calls = 0;
        let mut rows = Vec::new();
        assert_eq!(
            resolve_rows_into(&mut context, &grid, AvailableSpace::Unbounded, &[12, 12], &mut rows),
            Err(neocogi_com::NC_E_OUT_OF_RANGE)
        );
        assert_eq!(context.calls, 2);
    }

    /// A failure during replay must be returned even after the summary pass succeeded.
    #[test]
    fn scalar_track_replay_preserves_the_original_error() {
        for fail_at in [1, 3] {
            let mut calls = 0;
            let result = measured_tracks(&[TrackSize::Content; 2], 2, AvailableSpace::Unbounded, 0, |_| {
                calls += 1;
                match calls == fail_at {
                    true => Err(neocogi_com::NC_E_OUT_OF_RANGE),
                    false => Ok(10),
                }
            });
            assert_eq!(result, Err(neocogi_com::NC_E_OUT_OF_RANGE));
            assert_eq!(calls, fail_at);
        }
    }
}
