//! Rust conveniences belonging to concrete node-owned contract values.

use crate::{ChildCrossSize, LayoutTrack, LinearEdge};

impl LinearEdge {
    /// Creates an ordinary relationship that follows the Linear line's resolved cross extent.
    pub fn new(main: LayoutTrack) -> Self {
        Self {
            main,
            cross: ChildCrossSize::Stretch,
        }
    }

    /// Requests an exact cross extent; the receiving container validates it before mutation.
    pub fn with_fixed_cross(mut self, extent: i32) -> Self {
        self.cross = ChildCrossSize::Fixed { extent };
        self
    }
}
