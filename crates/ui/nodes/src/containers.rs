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

//! Built-in retained containers.

use super::container::ContainerLayoutCtx;

/// Requested relationship edit; validate and reserve its metadata before changing ownership.
pub(super) enum EdgeEdit<E> {
    /// Inserts one relationship at an index at most the current child count.
    Insert {
        /// Position of the new child, permitting append at the current length.
        index: usize,
        /// Validated concrete relationship for the incoming child.
        edge: E,
    },
    /// Replaces one relationship without changing metadata cardinality.
    Replace {
        /// Existing child's index; applies to topology replacement or metadata-only edits.
        index: usize,
        /// Complete replacement relationship, not a partial update.
        edge: E,
    },
    /// Removes the relationship of an existing child.
    Remove {
        /// Existing child whose owner is dropped only after preparation succeeds.
        index: usize,
    },
    /// Clears all relationships; each algorithm chooses whether to retain capacity.
    Clear,
}

impl<E> EdgeEdit<E> {
    /// Checks indices and computes prospective cardinality before allocation or attachment.
    fn resulting_len(&self, len: usize) -> Result<usize, neocogi_com::NcStatus> {
        use neocogi_com::NC_E_OUT_OF_RANGE;
        match self {
            Self::Insert { index, .. } if *index <= len && len < u32::MAX as usize => Ok(len + 1),
            Self::Replace { index, .. } if *index < len => Ok(len),
            Self::Remove { index } if *index < len => Ok(len - 1),
            Self::Clear => Ok(0),
            _ => Err(NC_E_OUT_OF_RANGE),
        }
    }

    /// Applies an index-validated edit to storage already reserved for the resulting length.
    fn apply(self, values: &mut Vec<E>) {
        match self {
            Self::Insert { index, edge } => values.insert(index, edge),
            Self::Replace { index, edge } => values[index] = edge,
            Self::Remove { index } => {
                values.remove(index);
            }
            Self::Clear => values.clear(),
        }
    }
}

/// Typed relationship access shared by the editable built-in container algorithms.
pub(super) trait ContainerAlgorithm: crate::widget_object::WidgetAlgorithm + crate::ContainerWidget {
    /// Concrete relationship data retained by this container.
    type Edge: Copy + PartialEq;

    /// Fully validated metadata that can be published without allocation or failure.
    type Prepared;

    /// Reads one relationship without exposing the owning metadata collection.
    fn edge(&self, index: usize) -> Option<Self::Edge>;

    /// Prepares a prospective edit without changing observable metadata or ownership.
    /// Reserving scratch capacity is allowed; failure leaves the old semantic state intact.
    fn prepare_edges(&mut self, edit: EdgeEdit<Self::Edge>) -> Result<Self::Prepared, neocogi_com::NcStatus>;

    /// Publishes prepared metadata after the matching host operation succeeds.
    fn commit_edges(&mut self, prepared: Self::Prepared);
}

/// Implements topology methods whose preparation/commit order is shared by Linear, Grid and Form.
macro_rules! container_methods {
    () => {
        /// Exposes read-only topology without allowing callers to bypass relationship metadata.
        fn children(&self) -> Result<neocogi_com::ComPtr<neocogi_ui::abi::core::IChildrenAbi>, neocogi_com::NcStatus> {
            <Self as crate::widget_object::HostedWidget>::runtime(self).state.readable()?;
            self.editor.view()
        }

        /// Publishes matching metadata after the editor drops the removed owner.
        fn remove(&mut self, index: u32) -> Result<(), neocogi_com::NcStatus> {
            <Self as crate::widget_object::HostedWidget>::runtime(self).state.writable()?;
            let prepared = <Self as crate::containers::ContainerAlgorithm>::prepare_edges(self, crate::containers::EdgeEdit::Remove { index: index as usize })?;
            let outcome = self.editor.remove(index)?;
            <Self as crate::containers::ContainerAlgorithm>::commit_edges(self, prepared);
            Ok(outcome)
        }

        /// Clears topology and relationship metadata in the same guarded operation.
        fn clear(&mut self) -> Result<(), neocogi_com::NcStatus> {
            <Self as crate::widget_object::HostedWidget>::runtime(self).state.writable()?;
            let prepared = <Self as crate::containers::ContainerAlgorithm>::prepare_edges(self, crate::containers::EdgeEdit::Clear)?;
            let outcome = self.editor.clear()?;
            <Self as crate::containers::ContainerAlgorithm>::commit_edges(self, prepared);
            Ok(outcome)
        }
    };
}

/// Implements the identical validated relationship transaction for each concrete edge type.
macro_rules! container_edges {
    ($edge:ty, $decode:ident, $encode:ident) => {
        /// Returns one copied relationship after checking the container's logical lifetime.
        fn edge(&self, index: u32) -> Result<$edge, neocogi_com::NcStatus> {
            <Self as crate::widget_object::HostedWidget>::runtime(self).state.readable()?;
            Ok($encode(
                <Self as crate::containers::ContainerAlgorithm>::edge(self, index as usize).ok_or(neocogi_com::NC_E_OUT_OF_RANGE)?,
            ))
        }

        /// Replaces valid metadata and invalidates layout only when the value changes.
        fn set_edge(&mut self, index: u32, edge: $edge) -> Result<(), neocogi_com::NcStatus> {
            <Self as crate::widget_object::HostedWidget>::runtime(self).state.writable()?;
            let edge = $decode(edge)?;
            let previous = <Self as crate::containers::ContainerAlgorithm>::edge(self, index as usize).ok_or(neocogi_com::NC_E_OUT_OF_RANGE)?;
            if previous != edge {
                let prepared =
                    <Self as crate::containers::ContainerAlgorithm>::prepare_edges(self, crate::containers::EdgeEdit::Replace { index: index as usize, edge })?;
                <Self as crate::containers::ContainerAlgorithm>::commit_edges(self, prepared);
                <Self as crate::widget_object::HostedWidget>::runtime_mut(self)
                    .state
                    .record_change(neocogi_ui::abi::core::GeometryChange::Changed);
            }
            Ok(())
        }

        /// Prepares all relationship storage before the child consumes its sole attachment claim.
        fn insert(
            &mut self,
            index: u32,
            child: neocogi_com::ComBox<neocogi_ui::abi::core::IWidgetAbi>,
            edge: $edge,
        ) -> Result<(), (neocogi_com::NcStatus, neocogi_com::ComBox<neocogi_ui::abi::core::IWidgetAbi>)> {
            let prepared = (|| {
                <Self as crate::widget_object::HostedWidget>::runtime(self).state.writable()?;
                let edge = $decode(edge)?;
                <Self as crate::containers::ContainerAlgorithm>::prepare_edges(self, crate::containers::EdgeEdit::Insert { index: index as usize, edge })
            })();
            let prepared = match prepared {
                Ok(prepared) => prepared,
                Err(status) => return Err((status, child)),
            };
            let outcome = self.editor.insert(index, child)?;
            <Self as crate::containers::ContainerAlgorithm>::commit_edges(self, prepared);
            Ok(outcome)
        }

        /// Prepares replacement metadata before the editor replaces and destroys the old owner.
        fn replace(
            &mut self,
            index: u32,
            child: neocogi_com::ComBox<neocogi_ui::abi::core::IWidgetAbi>,
            edge: $edge,
        ) -> Result<(), (neocogi_com::NcStatus, neocogi_com::ComBox<neocogi_ui::abi::core::IWidgetAbi>)> {
            let prepared = (|| {
                <Self as crate::widget_object::HostedWidget>::runtime(self).state.writable()?;
                let edge = $decode(edge)?;
                <Self as crate::containers::ContainerAlgorithm>::prepare_edges(self, crate::containers::EdgeEdit::Replace { index: index as usize, edge })
            })();
            let prepared = match prepared {
                Ok(prepared) => prepared,
                Err(status) => return Err((status, child)),
            };
            let outcome = self.editor.replace(index, child)?;
            <Self as crate::containers::ContainerAlgorithm>::commit_edges(self, prepared);
            Ok(outcome)
        }
    };
}

/// Validates one typed track before topology or semantic mutation begins.
pub(super) fn local_track(track: neocogi_ui::abi::nodes::LayoutTrack) -> Result<crate::TrackSize, neocogi_com::NcStatus> {
    match track {
        neocogi_ui::abi::nodes::LayoutTrack::Content => Ok(crate::TrackSize::Content),
        neocogi_ui::abi::nodes::LayoutTrack::Fixed { extent } if extent >= 0 => Ok(crate::TrackSize::Fixed(extent)),
        neocogi_ui::abi::nodes::LayoutTrack::Flex { weight } => {
            neocogi_ui::geometry::flex_weight(weight)?;
            Ok(crate::TrackSize::Flex(weight))
        }
        _ => Err(neocogi_com::NC_E_INVALID_ARG),
    }
}

/// Copies one retained track policy into the native public contract value.
pub(super) fn layout_track(track: crate::TrackSize) -> neocogi_ui::abi::nodes::LayoutTrack {
    match track {
        crate::TrackSize::Content => neocogi_ui::abi::nodes::LayoutTrack::Content,
        crate::TrackSize::Fixed(extent) => neocogi_ui::abi::nodes::LayoutTrack::Fixed { extent },
        crate::TrackSize::Flex(weight) => neocogi_ui::abi::nodes::LayoutTrack::Flex { weight },
    }
}

mod form;
pub use form::Form;
mod disclosure;
mod grid;
mod linear;
mod scroll_area;
mod split_view;
pub(crate) use scroll_area::{ScrollComposition, ScrollModel, ScrollParts};

pub use disclosure::{Disclosure};
pub use grid::Grid;
pub use linear::Linear;
pub use scroll_area::ScrollArea;
pub use split_view::SplitView;
