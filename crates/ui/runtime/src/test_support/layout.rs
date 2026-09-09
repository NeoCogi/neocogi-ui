//! Native geometry service for algorithm fixtures; COM fixtures use the real ScopedLayout.
use crate::{AtlasHandle, Children, Constraints, Dimensioni, Recti, Skin, ChildParticipation};
use crate::ui_node::layout_service::ScopedLayout;
use crate::ui_node::LayoutTraversal;
use neocogi_com::NcStatus;

/// The test harness alone can substitute borrowed native children for the COM service.
pub(crate) enum ChildLayout<'a> {
    /// Real implementation under test, including its expiring authority and error handling.
    Com(ScopedLayout<'a>),
    /// Direct algorithm fixture with resources valid for the enclosing invocation.
    Native {
        /// Skin paired with the fixture's atlas.
        skin: &'a Skin,
        /// Immutable fixture resources.
        atlas: &'a AtlasHandle,
        /// Sole borrowed child collection; never retained by a widget.
        children: &'a mut Children,
        /// Measurement-only fixtures need no recursive placement driver.
        runtime: Option<&'a mut LayoutTraversal>,
    },
}
impl<'a> ChildLayout<'a> {
    /// Binds the native fixture's borrowed children for one algorithm call.
    pub(crate) fn native(skin: &'a Skin, atlas: &'a AtlasHandle, children: &'a mut Children, runtime: Option<&'a mut LayoutTraversal>) -> Self {
        Self::Native { skin, atlas, children, runtime }
    }
    /// Native fixtures have no foreign-call failures; COM errors remain observable.
    pub(crate) fn result(&self) -> Result<(), NcStatus> {
        match self {
            Self::Com(layout) => layout.result(),
            Self::Native { .. } => Ok(()),
        }
    }
    /// Reads the authoritative fixture child count.
    pub(crate) fn child_count(&self) -> usize {
        match self {
            Self::Com(layout) => layout.child_count(),
            Self::Native { children, .. } => children.len(),
        }
    }
    /// Uses real node caches in both representations, retaining native traversal metrics.
    pub(crate) fn measure(&mut self, index: usize, constraints: Constraints) -> Option<Dimensioni> {
        match self {
            Self::Com(layout) => layout.measure(index, constraints),
            Self::Native { skin, atlas, children, runtime } => {
                let child = children.get_mut(index)?;
                Some(match runtime {
                    Some(runtime) => runtime.measure_node(child, skin, atlas, constraints),
                    None => child.measure(skin, atlas, constraints),
                })
            }
        }
    }
    /// Native placement requires the fixture's recursive layout driver.
    pub(crate) fn place(&mut self, index: usize, rect: Recti) -> Option<Dimensioni> {
        match self {
            Self::Com(layout) => layout.place(index, rect),
            Self::Native { skin, atlas, children, runtime } => Some(runtime.as_mut()?.layout_node_ref(children.get_mut(index)?, skin, atlas, rect)),
        }
    }
    /// Applies participation to the exact child later visited by the shared router.
    pub(crate) fn set_participation(&mut self, index: usize, participation: ChildParticipation) -> bool {
        match self {
            Self::Com(layout) => layout.set_participation(index, participation),
            Self::Native { children, .. } => {
                let Some(child) = children.get_mut(index) else {
                    return false;
                };
                child.state.participation = participation;
                true
            }
        }
    }
    /// Invalidates the same retained placement cache used by production traversal.
    pub(crate) fn invalidate_placement(&mut self, index: usize) -> bool {
        match self {
            Self::Com(layout) => layout.invalidate_placement(index),
            Self::Native { children, .. } => {
                let Some(child) = children.get_mut(index) else {
                    return false;
                };
                child.state.invalidate_layout();
                true
            }
        }
    }
}
