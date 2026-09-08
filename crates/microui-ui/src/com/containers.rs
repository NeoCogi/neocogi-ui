//! Concrete container editing reuses existing layout algorithms and one host-owned child sequence.
use super::*;
use super::widget::{HostedWidget, WidgetAlgorithm, WidgetRuntime};
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

/// Expands direct class methods for topology edits shared by Linear and Grid.
macro_rules! container_methods {
    () => {
        /// Exposes read-only topology without allowing callers to bypass concrete edge metadata.
        fn children(&self) -> Result<ComPtr<IChildrenAbi>, NcStatus> {
            <Self as HostedWidget>::runtime(self).state.readable()?;
            self.editor.view()
        }
        /// Publishes matching metadata even when the removed child's close reports an error.
        fn remove(&mut self, index: u32) -> Result<ChildEditResult, NcStatus> {
            <Self as HostedWidget>::runtime(self).state.writable()?;
            let outcome = self.editor.remove(index)?;
            <Self as ContainerAlgorithm>::edit_edges(self, EdgeEdit::Remove { index: index as usize });
            Ok(outcome)
        }
        /// Clears topology and metadata in the same guarded concrete operation.
        fn clear(&mut self) -> Result<ChildEditResult, NcStatus> {
            <Self as HostedWidget>::runtime(self).state.writable()?;
            let outcome = self.editor.clear()?;
            <Self as ContainerAlgorithm>::edit_edges(self, EdgeEdit::Clear);
            Ok(outcome)
        }
    };
}

/// Reuses the identical validated edge transaction for each concrete edge ABI type.
macro_rules! container_edges {
    ($edge:ty, $decode:ident, $encode:ident) => {
        /// Returns one copied relationship after checking the container's logical lifetime.
        fn edge(&self, index: u32) -> Result<$edge, NcStatus> {
            <Self as HostedWidget>::runtime(self).state.readable()?;
            Ok($encode(<Self as ContainerAlgorithm>::edge(self, index as usize).ok_or(nc_com::NC_E_OUT_OF_RANGE)?))
        }
        /// Validates metadata before replacing it, retaining invalidation only for an actual change.
        fn set_edge(&mut self, index: u32, edge: $edge) -> Result<(), NcStatus> {
            <Self as HostedWidget>::runtime(self).state.writable()?;
            let edge = $decode(edge)?;
            let previous = <Self as ContainerAlgorithm>::edge(self, index as usize).ok_or(nc_com::NC_E_OUT_OF_RANGE)?;
            if previous != edge {
                <Self as ContainerAlgorithm>::edit_edges(self, EdgeEdit::Replace { index: index as usize, edge });
                <Self as HostedWidget>::runtime_mut(self).state.record_change(GeometryChange::CHANGED);
            }
            Ok(())
        }
        /// Validates the complete relationship before the child can consume its sole attachment claim.
        fn insert(&mut self, index: u32, child: ComRef<'_, IWidgetAbi>, edge: $edge) -> Result<ChildEditResult, NcStatus> {
            <Self as HostedWidget>::runtime(self).state.writable()?;
            let edge = $decode(edge)?;
            let outcome = self.editor.insert(index, child)?;
            <Self as ContainerAlgorithm>::edit_edges(self, EdgeEdit::Insert { index: index as usize, edge });
            Ok(outcome)
        }
        /// Commits replacement metadata before returning any old-child teardown failure in the outcome.
        fn replace(&mut self, index: u32, child: ComRef<'_, IWidgetAbi>, edge: $edge) -> Result<ChildEditResult, NcStatus> {
            <Self as HostedWidget>::runtime(self).state.writable()?;
            let edge = $decode(edge)?;
            let outcome = self.editor.replace(index, child)?;
            <Self as ContainerAlgorithm>::edit_edges(self, EdgeEdit::Replace { index: index as usize, edge });
            Ok(outcome)
        }
    };
}

direct_widget_phase_impl!(crate::Linear);
direct_container_layout_impl!(crate::Linear);

impl crate::Linear {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create_com(host: ComPtr<IWidgetHostAbi>, direction: FlowDirection, cross: CrossSizeAbi) -> Result<ComPtr<LinearAbi>, NcStatus> {
        let value = <Self as microui_contracts::Linear>::new(host.as_com_ref(), direction, cross)?;
        Ok(Self::from_value(value).query().expect("Linear declares its callable class contract"))
    }
}
impl microui_contracts::Linear for crate::Linear {
    /// Validates both layout policies before allocating child ownership or widget state.
    fn new(host: ComRef<'_, IWidgetHostAbi>, direction: FlowDirection, cross: CrossSizeAbi) -> Result<Self, NcStatus> {
        let direction = local_direction(direction)?;
        let cross = local_cross(cross)?;
        let host = host.add_ref();
        let editor = host.create_children()?;
        let runtime = WidgetRuntime::new(host, crate::WidgetOption::NO_INTERACT)?;
        Ok(Self::from_runtime(runtime, editor, direction, cross))
    }
    container_methods!();
    /// Returns the exact retained axis and leading-edge policy.
    fn direction(&self) -> Result<FlowDirection, NcStatus> {
        self.runtime().state.readable()?;
        Ok(flow_direction(crate::Linear::direction(self)))
    }
    /// Validates before changing direction and conservatively invalidating derived placement.
    fn set_direction(&mut self, direction: FlowDirection) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let direction = local_direction(direction)?;
        if crate::Linear::direction(self) != direction {
            crate::Linear::set_direction(self, direction);
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Returns the line's cross size independently of per-child cross relationships.
    fn cross_size(&self) -> Result<CrossSizeAbi, NcStatus> {
        self.runtime().state.readable()?;
        Ok(cross_size(crate::Linear::cross_size(self)))
    }
    /// Changes cross size only after validating its typed payload.
    fn set_cross_size(&mut self, cross: CrossSizeAbi) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let cross = local_cross(cross)?;
        if crate::Linear::cross_size(self) != cross {
            crate::Linear::set_cross_size(self, cross);
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    container_edges!(LinearEdge, local_linear_edge, linear_edge);
}

direct_widget_phase_impl!(crate::Grid);
direct_container_layout_impl!(crate::Grid);

impl crate::Grid {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        columns: NcCopyFromBufferOffset<'_, LayoutTrackAbi>,
        rows: NcCopyFromBufferOffset<'_, LayoutTrackAbi>,
    ) -> Result<ComPtr<GridAbi>, NcStatus> {
        let value = <Self as microui_contracts::Grid>::new(host.as_com_ref(), columns, rows)?;
        Ok(Self::from_value(value).query().expect("Grid declares its callable class contract"))
    }
}
impl microui_contracts::Grid for crate::Grid {
    /// Validates both complete track sequences before allocating child ownership or widget state.
    fn new(
        host: ComRef<'_, IWidgetHostAbi>,
        columns: NcCopyFromBufferOffset<'_, LayoutTrackAbi>,
        rows: NcCopyFromBufferOffset<'_, LayoutTrackAbi>,
    ) -> Result<Self, NcStatus> {
        let columns = local_tracks(columns)?;
        let rows = local_tracks(rows)?;
        let host = host.add_ref();
        let editor = host.create_children()?;
        let runtime = WidgetRuntime::new(host, crate::WidgetOption::NO_INTERACT)?;
        Ok(Self::from_runtime(runtime, editor, columns, rows))
    }
    container_methods!();
    /// Bulk-copies column policy from the one concrete Grid state.
    fn columns(&self, output: NcCopyToBufferOffset<'_, LayoutTrackAbi>) -> Result<u32, NcStatus> {
        self.runtime().state.readable()?;
        copy_tracks(self.column_tracks(), output)
    }
    /// Validates the complete input before rebuilding column-dependent span placement.
    fn set_columns(&mut self, tracks: NcCopyFromBufferOffset<'_, LayoutTrackAbi>) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let tracks = local_tracks(tracks)?;
        if self.column_tracks() != tracks {
            self.set_column_tracks(tracks);
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
    /// Bulk-copies row policy without exposing Grid's scratch or span vectors.
    fn rows(&self, output: NcCopyToBufferOffset<'_, LayoutTrackAbi>) -> Result<u32, NcStatus> {
        self.runtime().state.readable()?;
        copy_tracks(self.row_tracks(), output)
    }
    /// Replaces row policies only after the entire typed input passes validation.
    fn set_rows(&mut self, tracks: NcCopyFromBufferOffset<'_, LayoutTrackAbi>) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let tracks = local_tracks(tracks)?;
        if self.row_tracks() != tracks {
            self.set_row_tracks(tracks);
            self.runtime_mut().state.record_change(GeometryChange::CHANGED);
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
