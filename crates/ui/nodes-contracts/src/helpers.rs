//! Rust conveniences belonging to concrete node-owned contract values.

use crate::{ChildCrossSize, LayoutTrack, LinearEdge};

impl LinearEdge {
    /// Creates an ordinary relationship that follows the Linear line's resolved cross extent.
    pub fn new(main: LayoutTrack) -> Self {
        Self { main, cross: ChildCrossSize::Stretch }
    }

    /// Requests an exact cross extent; the receiving container validates it before mutation.
    pub fn with_fixed_cross(mut self, extent: i32) -> Self {
        self.cross = ChildCrossSize::Fixed { extent };
        self
    }
}

impl crate::GridLimits {
    /// Rejects unusable ceilings before a host accepts this resource policy.
    /// The i32 axis bound matches track-count arithmetic in pixel layout; the
    /// checked product also makes every permitted flat occupancy index representable.
    /// No storage is allocated for the maximum: only actual placements allocate cells.
    pub fn validate(self) -> Result<(), neocogi_com::NcStatus> {
        if self.max_columns == 0 || self.max_rows == 0 {
            return Err(neocogi_com::NC_E_INVALID_ARG);
        }
        let cells = (self.max_columns as usize).checked_mul(self.max_rows as usize);
        match cells {
            Some(cells) if cells <= isize::MAX as usize && self.max_columns <= i32::MAX as u32 && self.max_rows <= i32::MAX as u32 => Ok(()),
            _ => Err(neocogi_com::NC_E_OUT_OF_RANGE),
        }
    }
}
