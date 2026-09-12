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
#[cfg(test)]
use super::{Children, Node};

/// Metadata-only edit applied after the host commits the matching child-topology operation.
pub(super) enum EdgeEdit<E> {
    /// Inserts one validated relationship at an already validated position.
    Insert { index: usize, edge: E },
    /// Replaces one relationship without changing metadata cardinality.
    Replace { index: usize, edge: E },
    /// Removes one relationship after the corresponding subtree has been removed.
    Remove { index: usize },
    /// Clears all relationships while retaining reusable storage.
    Clear,
}

/// Typed relationship access shared by the two editable built-in container algorithms.
pub(super) trait ContainerAlgorithm: crate::ui_node::widget_object::WidgetAlgorithm + crate::ContainerWidget {
    /// Concrete relationship data retained by this container.
    type Edge: Copy + PartialEq;

    /// Reads one relationship without exposing the owning metadata collection.
    fn edge(&self, index: usize) -> Option<Self::Edge>;

    /// Publishes an infallible metadata edit after topology validation succeeds.
    fn edit_edges(&mut self, edit: EdgeEdit<Self::Edge>);
}

/// Implements topology methods whose behavior is identical for Linear and Grid.
macro_rules! container_methods {
    () => {
        /// Exposes read-only topology without allowing callers to bypass relationship metadata.
        fn children(&self) -> Result<neocogi_com::ComPtr<neocogi_ui_contracts::IChildrenAbi>, neocogi_com::NcStatus> {
            <Self as crate::ui_node::widget_object::HostedWidget>::runtime(self).state.readable()?;
            self.editor.view()
        }

        /// Publishes matching metadata even when removed-child teardown reports an error.
        fn remove(&mut self, index: u32) -> Result<neocogi_ui_contracts::ChildEditResult, neocogi_com::NcStatus> {
            <Self as crate::ui_node::widget_object::HostedWidget>::runtime(self).state.writable()?;
            let outcome = self.editor.remove(index)?;
            <Self as crate::ui_node::containers::ContainerAlgorithm>::edit_edges(self, crate::ui_node::containers::EdgeEdit::Remove { index: index as usize });
            Ok(outcome)
        }

        /// Clears topology and relationship metadata in the same guarded operation.
        fn clear(&mut self) -> Result<neocogi_ui_contracts::ChildEditResult, neocogi_com::NcStatus> {
            <Self as crate::ui_node::widget_object::HostedWidget>::runtime(self).state.writable()?;
            let outcome = self.editor.clear()?;
            <Self as crate::ui_node::containers::ContainerAlgorithm>::edit_edges(self, crate::ui_node::containers::EdgeEdit::Clear);
            Ok(outcome)
        }
    };
}

/// Implements the identical validated relationship transaction for each concrete edge type.
macro_rules! container_edges {
    ($edge:ty, $decode:ident, $encode:ident) => {
        /// Returns one copied relationship after checking the container's logical lifetime.
        fn edge(&self, index: u32) -> Result<$edge, neocogi_com::NcStatus> {
            <Self as crate::ui_node::widget_object::HostedWidget>::runtime(self).state.readable()?;
            Ok($encode(
                <Self as crate::ui_node::containers::ContainerAlgorithm>::edge(self, index as usize).ok_or(neocogi_com::NC_E_OUT_OF_RANGE)?,
            ))
        }

        /// Replaces valid metadata and invalidates layout only when the value changes.
        fn set_edge(&mut self, index: u32, edge: $edge) -> Result<(), neocogi_com::NcStatus> {
            <Self as crate::ui_node::widget_object::HostedWidget>::runtime(self).state.writable()?;
            let edge = $decode(edge)?;
            let previous = <Self as crate::ui_node::containers::ContainerAlgorithm>::edge(self, index as usize).ok_or(neocogi_com::NC_E_OUT_OF_RANGE)?;
            if previous != edge {
                <Self as crate::ui_node::containers::ContainerAlgorithm>::edit_edges(
                    self,
                    crate::ui_node::containers::EdgeEdit::Replace { index: index as usize, edge },
                );
                <Self as crate::ui_node::widget_object::HostedWidget>::runtime_mut(self)
                    .state
                    .record_change(neocogi_ui_contracts::GeometryChange::Changed);
            }
            Ok(())
        }

        /// Validates the relationship before the child consumes its sole attachment claim.
        fn insert(
            &mut self,
            index: u32,
            child: neocogi_com::ComRef<'_, neocogi_ui_contracts::IWidgetAbi>,
            edge: $edge,
        ) -> Result<neocogi_ui_contracts::ChildEditResult, neocogi_com::NcStatus> {
            <Self as crate::ui_node::widget_object::HostedWidget>::runtime(self).state.writable()?;
            let edge = $decode(edge)?;
            let outcome = self.editor.insert(index, child)?;
            <Self as crate::ui_node::containers::ContainerAlgorithm>::edit_edges(
                self,
                crate::ui_node::containers::EdgeEdit::Insert { index: index as usize, edge },
            );
            Ok(outcome)
        }

        /// Commits replacement metadata before returning old-child teardown status.
        fn replace(
            &mut self,
            index: u32,
            child: neocogi_com::ComRef<'_, neocogi_ui_contracts::IWidgetAbi>,
            edge: $edge,
        ) -> Result<neocogi_ui_contracts::ChildEditResult, neocogi_com::NcStatus> {
            <Self as crate::ui_node::widget_object::HostedWidget>::runtime(self).state.writable()?;
            let edge = $decode(edge)?;
            let outcome = self.editor.replace(index, child)?;
            <Self as crate::ui_node::containers::ContainerAlgorithm>::edit_edges(
                self,
                crate::ui_node::containers::EdgeEdit::Replace { index: index as usize, edge },
            );
            Ok(outcome)
        }
    };
}

/// Validates one typed track before topology or semantic mutation begins.
pub(super) fn local_track(track: neocogi_ui_contracts::LayoutTrackAbi) -> Result<crate::TrackSize, neocogi_com::NcStatus> {
    match neocogi_ui_contracts::LayoutTrack::try_from(track).map_err(|_| neocogi_com::NC_E_INVALID_ARG)? {
        neocogi_ui_contracts::LayoutTrack::Content => Ok(crate::TrackSize::Content),
        neocogi_ui_contracts::LayoutTrack::Fixed { extent } if extent >= 0 => Ok(crate::TrackSize::Fixed(extent)),
        neocogi_ui_contracts::LayoutTrack::Flex { weight } if weight.is_finite() && weight > 0.0 => Ok(crate::TrackSize::Flex(weight)),
        _ => Err(neocogi_com::NC_E_INVALID_ARG),
    }
}

/// Copies one retained track policy into its typed ABI representation.
pub(super) fn layout_track(track: crate::TrackSize) -> neocogi_ui_contracts::LayoutTrackAbi {
    match track {
        crate::TrackSize::Content => neocogi_ui_contracts::LayoutTrack::Content,
        crate::TrackSize::Fixed(extent) => neocogi_ui_contracts::LayoutTrack::Fixed { extent },
        crate::TrackSize::Flex(weight) => neocogi_ui_contracts::LayoutTrack::Flex { weight },
    }
    .into()
}

mod disclosure;
mod grid;
mod linear;
mod scroll_area;
pub(crate) use scroll_area::{ScrollComposition, ScrollModel, ScrollParts};

pub use disclosure::{Disclosure};
#[cfg(test)]
pub use disclosure::{DisclosureParameters};
pub use grid::Grid;
#[cfg(test)]
pub use grid::GridParameters;
pub use linear::Linear;
#[cfg(test)]
pub use linear::{LinearCrossSize, LinearItem, LinearParameters};
pub use scroll_area::ScrollArea;
#[cfg(test)]
pub use scroll_area::ScrollAreaOption;
#[cfg(test)]
pub use scroll_area::{ScrollAreaParameters};
