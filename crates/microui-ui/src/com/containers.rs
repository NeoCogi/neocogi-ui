//! Concrete container editing reuses existing layout algorithms and one host-owned child sequence.
use super::*;
use super::widget::{WidgetAlgorithm, WidgetCore};
use nc_com::NcCopyToBufferOffset;

/// Metadata-only edit applied after the host editor commits the corresponding topology operation.
pub(crate) enum EdgeEdit<E> {
    /// Inserts one validated relationship at an already validated position.
    Insert { index: usize, edge: E },
    /// Replaces one relationship without changing metadata cardinality.
    Replace { index: usize, edge: E },
    /// Removes one relationship after the corresponding subtree has been removed.
    Remove { index: usize },
    /// Clears all relationships while retaining reusable storage.
    Clear,
}

/// Static metadata access; concrete types and algorithms stay inside their one COM allocation.
pub(crate) trait ContainerAlgorithm: WidgetAlgorithm + crate::ContainerWidget {
    /// Concrete relationship data for this container, without a generic property bag.
    type Edge: Copy + PartialEq;
    /// Reads validated indexed metadata independently of child ownership.
    fn edge(&self, index: usize) -> Option<Self::Edge>;
    /// Publishes an infallible metadata edit after topology/index validation has succeeded.
    fn edit_edges(&mut self, edit: EdgeEdit<Self::Edge>);
}

/// Shares concrete storage, lifecycle, and scoped layout plumbing across callable containers.
macro_rules! container_object {
    ($name:ident, $core:ty, $contract:path) => {
        #[com_object($contract)]
        /// One concrete container COM allocation, including inherited common widget state.
        pub struct $name {
            /// Concrete semantic state and shared phase execution, with no Rust dynamic widget vtable.
            widget: WidgetCore<$core>,
            /// Private writer owns the one authoritative tree; public callers receive its distinct read-only view.
            editor: ComPtr<IChildrenEditorAbi>,
        }
        impl $name {
            /// Allocates common widget and child storage from already validated concrete policy.
            fn from_core(host: ComPtr<IWidgetHostAbi>, core: $core) -> Result<Self, NcStatus> {
                let editor = host.create_children()?;
                Ok(Self {
                    widget: WidgetCore::new(host, crate::WidgetOption::NO_INTERACT, core)?,
                    editor,
                })
            }
            /// Closes logical widget state and all descendant ownership before final COM release.
            fn close_widget(&self) -> Result<(), NcStatus> {
                // A child attachment may call out while this container owns its
                // semantic state. Reject reentrant closure before changing lifetime.
                let _core = self.widget.take()?;
                if self.widget.state.close()? {
                    self.editor.close()?.finish()?;
                }
                Ok(())
            }
            /// Reuses the concrete container's full input and eventless update algorithm.
            fn update_widget(&self, state: WidgetUpdate, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
                self.widget.update(state, event, text)
            }
        }
        widget_phase_impl!($name);
        container_layout_impl!($name);
    };
}

/// Expands direct class methods for topology edits shared by Linear and Grid.
macro_rules! container_methods {
    () => {
        /// Exposes read-only topology without allowing callers to bypass concrete edge metadata.
        fn children(&self) -> Result<ComPtr<IChildrenAbi>, NcStatus> {
            self.widget.state.readable()?;
            self.editor.view()
        }
        /// Publishes matching metadata even when the removed child's close reports an error.
        fn remove(&self, index: u32) -> Result<ChildEditResult, NcStatus> {
            self.widget.state.writable()?;
            let mut core = self.widget.take()?;
            let outcome = self.editor.remove(index)?;
            core.edit_edges(EdgeEdit::Remove { index: index as usize });
            Ok(outcome)
        }
        /// Clears topology and metadata in the same guarded concrete operation.
        fn clear(&self) -> Result<ChildEditResult, NcStatus> {
            self.widget.state.writable()?;
            let mut core = self.widget.take()?;
            let outcome = self.editor.clear()?;
            core.edit_edges(EdgeEdit::Clear);
            Ok(outcome)
        }
    };
}

/// Reuses the identical validated edge transaction for each concrete edge ABI type.
macro_rules! container_edges {
    ($edge:ty, $decode:ident, $encode:ident) => {
        /// Returns one copied relationship after checking the container's logical lifetime.
        fn edge(&self, index: u32) -> Result<$edge, NcStatus> {
            self.widget.state.readable()?;
            Ok($encode(self.widget.take()?.edge(index as usize).ok_or(nc_com::NC_E_OUT_OF_RANGE)?))
        }
        /// Validates metadata before replacing it, retaining invalidation only for an actual change.
        fn set_edge(&self, index: u32, edge: $edge) -> Result<(), NcStatus> {
            self.widget.state.writable()?;
            let edge = $decode(edge)?;
            let mut core = self.widget.take()?;
            let previous = core.edge(index as usize).ok_or(nc_com::NC_E_OUT_OF_RANGE)?;
            if previous != edge {
                core.edit_edges(EdgeEdit::Replace { index: index as usize, edge });
                self.widget.state.record_change(GeometryChange::CHANGED);
            }
            Ok(())
        }
        /// Validates the complete relationship before the child can consume its sole attachment claim.
        fn insert(&self, index: u32, child: ComRef<'_, IWidgetAbi>, edge: $edge) -> Result<ChildEditResult, NcStatus> {
            self.widget.state.writable()?;
            let edge = $decode(edge)?;
            let mut core = self.widget.take()?;
            let outcome = self.editor.insert(index, child)?;
            core.edit_edges(EdgeEdit::Insert { index: index as usize, edge });
            Ok(outcome)
        }
        /// Commits replacement metadata before returning any old-child teardown failure in the outcome.
        fn replace(&self, index: u32, child: ComRef<'_, IWidgetAbi>, edge: $edge) -> Result<ChildEditResult, NcStatus> {
            self.widget.state.writable()?;
            let edge = $decode(edge)?;
            let mut core = self.widget.take()?;
            let outcome = self.editor.replace(index, child)?;
            core.edit_edges(EdgeEdit::Replace { index: index as usize, edge });
            Ok(outcome)
        }
    };
}

container_object!(LinearObject, crate::Linear, microui_contracts::Linear);
impl LinearObject {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create(
        host: ComPtr<IWidgetHostAbi>,
        direction: FlowDirection,
        cross: CrossSizeAbi,
    ) -> Result<ComPtr<LinearAbi>, NcStatus> {
        let value = <Self as Linear>::new(host.as_com_ref(), direction, cross)?;
        Ok(Self::from_value(value)
            .query()
            .expect("LinearObject declares the Linear class contract"))
    }
}
impl Linear for LinearObject {
    /// Validates both layout policies before allocating child ownership or widget state.
    fn new(host: ComRef<'_, IWidgetHostAbi>, direction: FlowDirection, cross: CrossSizeAbi) -> Result<Self, NcStatus> {
        let direction = local_direction(direction)?;
        let cross = local_cross(cross)?;
        Self::from_core(host.add_ref(), crate::Linear::com_new(direction, cross))
    }
    container_methods!();
    /// Returns the exact retained axis and leading-edge policy.
    fn direction(&self) -> Result<FlowDirection, NcStatus> {
        self.widget.state.readable()?;
        Ok(flow_direction(self.widget.take()?.direction()))
    }
    /// Validates before changing direction and conservatively invalidating derived placement.
    fn set_direction(&self, direction: FlowDirection) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        let direction = local_direction(direction)?;
        let mut core = self.widget.take()?;
        if core.direction() != direction {
            core.set_direction(direction);
            self.widget.state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Returns the line's cross size independently of per-child cross relationships.
    fn cross_size(&self) -> Result<CrossSizeAbi, NcStatus> {
        self.widget.state.readable()?;
        Ok(cross_size(self.widget.take()?.cross_size()))
    }
    /// Changes cross size only after validating its typed payload.
    fn set_cross_size(&self, cross: CrossSizeAbi) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        let cross = local_cross(cross)?;
        let mut core = self.widget.take()?;
        if core.cross_size() != cross {
            core.set_cross_size(cross);
            self.widget.state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    container_edges!(LinearEdge, local_linear_edge, linear_edge);
}

container_object!(GridObject, crate::Grid, microui_contracts::Grid);
impl GridObject {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create(
        host: ComPtr<IWidgetHostAbi>,
        columns: NcCopyFromBufferOffset<'_, LayoutTrackAbi>,
        rows: NcCopyFromBufferOffset<'_, LayoutTrackAbi>,
    ) -> Result<ComPtr<GridAbi>, NcStatus> {
        let value = <Self as Grid>::new(host.as_com_ref(), columns, rows)?;
        Ok(Self::from_value(value)
            .query()
            .expect("GridObject declares the Grid class contract"))
    }
}
impl Grid for GridObject {
    /// Validates both complete track sequences before allocating child ownership or widget state.
    fn new(
        host: ComRef<'_, IWidgetHostAbi>,
        columns: NcCopyFromBufferOffset<'_, LayoutTrackAbi>,
        rows: NcCopyFromBufferOffset<'_, LayoutTrackAbi>,
    ) -> Result<Self, NcStatus> {
        let columns = local_tracks(columns)?;
        let rows = local_tracks(rows)?;
        Self::from_core(host.add_ref(), crate::Grid::com_new(columns, rows))
    }
    container_methods!();
    /// Bulk-copies column policy from the one concrete Grid state.
    fn columns(&self, output: NcCopyToBufferOffset<'_, LayoutTrackAbi>) -> Result<u32, NcStatus> {
        self.widget.state.readable()?;
        copy_tracks(self.widget.take()?.column_tracks(), output)
    }
    /// Validates the complete input before rebuilding column-dependent span placement.
    fn set_columns(&self, tracks: NcCopyFromBufferOffset<'_, LayoutTrackAbi>) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        let tracks = local_tracks(tracks)?;
        let mut core = self.widget.take()?;
        if core.column_tracks() != tracks {
            core.set_column_tracks(tracks);
            self.widget.state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Bulk-copies row policy without exposing Grid's scratch or span vectors.
    fn rows(&self, output: NcCopyToBufferOffset<'_, LayoutTrackAbi>) -> Result<u32, NcStatus> {
        self.widget.state.readable()?;
        copy_tracks(self.widget.take()?.row_tracks(), output)
    }
    /// Replaces row policies only after the entire typed input passes validation.
    fn set_rows(&self, tracks: NcCopyFromBufferOffset<'_, LayoutTrackAbi>) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        let tracks = local_tracks(tracks)?;
        let mut core = self.widget.take()?;
        if core.row_tracks() != tracks {
            core.set_row_tracks(tracks);
            self.widget.state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    container_edges!(GridExtent, local_grid_edge, grid_edge);
}

/// Validates a typed track before any topology or semantic mutation.
fn local_track(track: LayoutTrackAbi) -> Result<crate::TrackSize, NcStatus> {
    match LayoutTrack::try_from(track).map_err(|_| NC_E_INVALID_ARG)? {
        LayoutTrack::Content => Ok(crate::TrackSize::Content),
        LayoutTrack::Fixed { extent } if extent >= 0 => Ok(crate::TrackSize::Fixed(extent)),
        LayoutTrack::Flex { weight } if weight.is_finite() && weight > 0.0 => Ok(crate::TrackSize::Flex(weight)),
        _ => Err(NC_E_INVALID_ARG),
    }
}
/// Copies the actual retained track policy into its typed ABI representation.
fn layout_track(track: crate::TrackSize) -> LayoutTrackAbi {
    match track {
        crate::TrackSize::Content => LayoutTrack::Content,
        crate::TrackSize::Fixed(x) => LayoutTrack::Fixed { extent: x },
        crate::TrackSize::Flex(x) => LayoutTrack::Flex { weight: x },
    }
    .into()
}
/// Validates the whole bulk track replacement before allocating a new container or borrowing live state.
pub(super) fn local_tracks(tracks: NcCopyFromBufferOffset<'_, LayoutTrackAbi>) -> Result<Vec<crate::TrackSize>, NcStatus> {
    if tracks.offset() != 0 || tracks.len() > u32::MAX as usize {
        return Err(NC_E_INVALID_ARG);
    }
    tracks.as_slice().iter().copied().map(local_track).collect()
}
/// Copies a requested policy range and returns total cardinality for bounded caller allocation.
fn copy_tracks(tracks: &[crate::TrackSize], mut output: NcCopyToBufferOffset<'_, LayoutTrackAbi>) -> Result<u32, NcStatus> {
    let offset = usize::try_from(output.offset()).map_err(|_| NC_E_INVALID_ARG)?;
    let source = tracks.get(offset..).ok_or(NC_E_INVALID_ARG)?;
    for (target, track) in output.as_mut_slice().iter_mut().zip(source) {
        *target = layout_track(*track);
    }
    u32::try_from(tracks.len()).map_err(|_| nc_com::NC_E_OUT_OF_RANGE)
}
/// Decodes the complete axis/leading-edge choice with no independent reversal flag.
pub(super) fn local_direction(direction: FlowDirection) -> Result<crate::LinearDirection, NcStatus> {
    match direction {
        FlowDirection::LEFT_TO_RIGHT => Ok(crate::LinearDirection::LeftToRight),
        FlowDirection::RIGHT_TO_LEFT => Ok(crate::LinearDirection::RightToLeft),
        FlowDirection::TOP_TO_BOTTOM => Ok(crate::LinearDirection::TopToBottom),
        FlowDirection::BOTTOM_TO_TOP => Ok(crate::LinearDirection::BottomToTop),
        _ => Err(NC_E_INVALID_ARG),
    }
}
/// Encodes the exact retained direction without inferring it from geometry.
fn flow_direction(direction: crate::LinearDirection) -> FlowDirection {
    match direction {
        crate::LinearDirection::LeftToRight => FlowDirection::LEFT_TO_RIGHT,
        crate::LinearDirection::RightToLeft => FlowDirection::RIGHT_TO_LEFT,
        crate::LinearDirection::TopToBottom => FlowDirection::TOP_TO_BOTTOM,
        crate::LinearDirection::BottomToTop => FlowDirection::BOTTOM_TO_TOP,
    }
}
/// Validates a line cross-axis policy, including exact nonnegative extents.
pub(super) fn local_cross(cross: CrossSizeAbi) -> Result<crate::LinearCrossSize, NcStatus> {
    match CrossSize::try_from(cross).map_err(|_| NC_E_INVALID_ARG)? {
        CrossSize::Content => Ok(crate::LinearCrossSize::Content),
        CrossSize::Stretch => Ok(crate::LinearCrossSize::Stretch),
        CrossSize::Fixed { extent: x } if x >= 0 => Ok(crate::LinearCrossSize::Fixed(x)),
        _ => Err(NC_E_INVALID_ARG),
    }
}
/// Encodes the existing line cross policy without a duplicate cached representation.
fn cross_size(cross: crate::LinearCrossSize) -> CrossSizeAbi {
    match cross {
        crate::LinearCrossSize::Content => CrossSize::Content,
        crate::LinearCrossSize::Stretch => CrossSize::Stretch,
        crate::LinearCrossSize::Fixed(x) => CrossSize::Fixed { extent: x },
    }
    .into()
}
/// Validates both axes of one Linear relationship before consuming any child claim.
fn local_linear_edge(edge: LinearEdge) -> Result<crate::ui_node::LinearItemSpec, NcStatus> {
    let main = local_track(edge.main)?;
    let fixed_cross = match ChildCrossSize::try_from(edge.cross).map_err(|_| NC_E_INVALID_ARG)? {
        ChildCrossSize::Stretch => None,
        ChildCrossSize::Fixed { extent: x } if x >= 0 => Some(x),
        _ => return Err(NC_E_INVALID_ARG),
    };
    Ok(crate::ui_node::LinearItemSpec { main, fixed_cross })
}
/// Copies the concrete relationship into the public contract.
fn linear_edge(edge: crate::ui_node::LinearItemSpec) -> LinearEdge {
    LinearEdge {
        main: layout_track(edge.main),
        cross: match edge.fixed_cross {
            Some(x) => ChildCrossSize::Fixed { extent: x },
            None => ChildCrossSize::Stretch,
        }
        .into(),
    }
}
/// Rejects empty spans; right-edge clamping remains the existing Grid placement algorithm's policy.
fn local_grid_edge(edge: GridExtent) -> Result<crate::GridSpan, NcStatus> {
    if edge.columns == 0 || edge.rows == 0 {
        return Err(NC_E_INVALID_ARG);
    }
    Ok(crate::GridSpan::new(edge.columns as usize, edge.rows as usize))
}
/// Copies positive spans that were validated at the ABI boundary.
fn grid_edge(edge: crate::GridSpan) -> GridExtent {
    GridExtent {
        columns: edge.columns() as u32,
        rows: edge.rows() as u32,
    }
}
