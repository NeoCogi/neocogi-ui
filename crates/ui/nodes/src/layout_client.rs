//! Nodes-owned adapter for borrowed, expiring core layout capabilities.
//!
//! The client keeps only its copied scope and cardinality in ordinary fields. It owns no
//! retained children or traversal state; all authoritative operations cross ILayoutService.
use crate::boundary::copied_rect;
use crate::{Constraints, Dimensioni};
use neocogi_com::{ComRef, NcStatus};
use neocogi_ui::abi::core::*;
pub(crate) use neocogi_ui_support::layout::constraints;
use neocogi_ui_support::layout::limits;

/// Rust-local adapter lets existing concrete algorithms share the external scoped contract.
/// It borrows a COM projection only during the synchronous implementation call; the service itself owns its frame.
pub(crate) struct ScopedLayout<'a> {
    /// Exact authority supplied to this container's current measurement or placement.
    pub(crate) scope: LayoutScope,
    /// Borrowed projection avoids per-child AddRef/Release operations.
    pub(crate) service: ComRef<'a, ILayoutServiceAbi>,
    /// Validated before running algorithms whose metadata must match this cardinality.
    pub(crate) count: usize,
}
impl<'a> ScopedLayout<'a> {
    /// Resolves cardinality before invoking any algorithm, never substituting a fabricated zero on failure.
    pub(crate) fn new(scope: LayoutScope, service: ComRef<'a, ILayoutServiceAbi>) -> Result<Self, NcStatus> {
        let count = service.child_count(scope)? as usize;
        Ok(Self { scope, service, count })
    }
    /// Prevents usize truncation at the indexed ABI boundary.
    pub(crate) fn index(&self, index: usize) -> Result<u32, NcStatus> {
        u32::try_from(index).map_err(|_| neocogi_com::NC_E_OUT_OF_RANGE)
    }
    /// Measures with the same constraints and node cache as an independent COM container.
    pub(crate) fn measure(&mut self, index: usize, constraints: Constraints) -> Result<Dimensioni, NcStatus> {
        let index = self.index(index)?;
        self.service
            .measure_child(self.scope, index, limits(constraints))
            .map(|size| Dimensioni::new(size.width, size.height))
    }
    /// Returns cardinality already authenticated for this scope.
    pub(crate) fn child_count(&self) -> usize {
        self.count
    }
    /// Places one child with copied geometry and returns failures directly.
    pub(crate) fn place(&mut self, index: usize, rect: crate::Recti) -> Result<Dimensioni, NcStatus> {
        let index = self.index(index)?;
        self.service
            .place_child(self.scope, index, copied_rect(rect))
            .map(|size| Dimensioni::new(size.width, size.height))
    }
    /// Applies subtree participation through the authoritative runtime service.
    pub(crate) fn set_participation(&mut self, index: usize, participation: crate::ChildParticipation) -> Result<(), NcStatus> {
        let index = self.index(index)?;
        self.service.set_participation(self.scope, index, participation)
    }
    /// Marks a child placement dependency dirty without changing the assigned rectangle.
    pub(crate) fn invalidate_placement(&mut self, index: usize) -> Result<(), NcStatus> {
        let index = self.index(index)?;
        self.service.child_placement_changed(self.scope, index)
    }
}
