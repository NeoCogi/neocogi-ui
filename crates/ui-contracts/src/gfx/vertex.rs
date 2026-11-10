//! UI vertex helpers belong to GFX; native renderers consume only explicit generic buffer layouts.
use crate::gfx::abi::RenderVertex;
use rs_math3d::{Color4b, Vec2f};

/// Maximum expanded vertices retained by one compositor frame before generic lowering.
pub const MAX_UI_VERTICES: u32 = 16 * 1024 * 1024;
/// Maximum private UI/client operation boundaries in one compositor frame.
pub const MAX_UI_BATCHES: u32 = 1024 * 1024;

impl Default for RenderVertex {
    /// Returns a transparent vertex at the origin.
    fn default() -> Self {
        Self { x: 0.0, y: 0.0, u: 0.0, v: 0.0, rgba: 0 }
    }
}

impl RenderVertex {
    /// Creates one textured UI vertex for GFX clipping and lowering.
    pub fn new(position: Vec2f, tex_coord: Vec2f, color: Color4b) -> Self {
        // Packing once at the producer boundary gives all backends the same
        // channel order without a second renderer-private vertex structure.
        Self {
            x: position.x,
            y: position.y,
            u: tex_coord.x,
            v: tex_coord.y,
            rgba: u32::from_le_bytes(color.to_array()),
        }
    }

    /// Returns the drawable position used by private UI clipping.
    pub fn position(&self) -> Vec2f {
        Vec2f::new(self.x, self.y)
    }

    /// Returns the normalized image coordinates used by clipping and sampling.
    pub fn tex_coord(&self) -> Vec2f {
        Vec2f::new(self.u, self.v)
    }

    /// Decodes the packed UI color into channel storage.
    pub fn color(&self) -> Color4b {
        Color4b::from_array(self.rgba.to_le_bytes())
    }

    /// Linearly interpolates every attribute for geometry clipping.
    pub fn lerp(left: Self, right: Self, amount: f64) -> Self {
        // Keep the clipping fraction and differences wide: subtracting two finite f32
        // endpoints can overflow in f32 before multiplication restores the range.
        // A clamped convex combination stays within the finite endpoint interval.
        let amount = amount.clamp(0.0, 1.0);
        let left_color = left.rgba.to_le_bytes();
        let right_color = right.rgba.to_le_bytes();
        Self {
            x: (f64::from(left.x) + (f64::from(right.x) - f64::from(left.x)) * amount) as f32,
            y: (f64::from(left.y) + (f64::from(right.y) - f64::from(left.y)) * amount) as f32,
            u: (f64::from(left.u) + (f64::from(right.u) - f64::from(left.u)) * amount) as f32,
            v: (f64::from(left.v) + (f64::from(right.v) - f64::from(left.v)) * amount) as f32,
            rgba: u32::from_le_bytes([
                Self::lerp_channel(left_color[0], right_color[0], amount),
                Self::lerp_channel(left_color[1], right_color[1], amount),
                Self::lerp_channel(left_color[2], right_color[2], amount),
                Self::lerp_channel(left_color[3], right_color[3], amount),
            ]),
        }
    }

    /// Interpolates one packed channel and rounds it back into byte storage.
    fn lerp_channel(left: u8, right: u8, amount: f64) -> u8 {
        // Inputs and the clamped amount make adding one half sufficient for
        // nearest rounding without requiring a `std` floating-point helper.
        let interpolated = f64::from(left) + (f64::from(right) - f64::from(left)) * amount;
        (interpolated + 0.5).clamp(0.0, 255.0) as u8
    }
}

/// Numerical regressions exercise the contract independently of renderer selection.
#[cfg(test)]
mod interpolation_tests {
    use super::*;

    /// Finite opposite extremes must produce bounded attributes, including at either endpoint.
    #[test]
    fn finite_extremes_interpolate_without_overflow() {
        let left = RenderVertex {
            x: -f32::MAX,
            y: f32::MAX,
            u: -f32::MAX,
            v: f32::MAX,
            rgba: 0,
        };
        let right = RenderVertex {
            x: f32::MAX,
            y: -f32::MAX,
            u: f32::MAX,
            v: -f32::MAX,
            rgba: u32::MAX,
        };
        for amount in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let value = RenderVertex::lerp(left, right, amount);
            assert!([value.x, value.y, value.u, value.v].into_iter().all(f32::is_finite));
        }
        let midpoint = RenderVertex::lerp(left, right, 0.5);
        assert_eq!([midpoint.x, midpoint.y, midpoint.u, midpoint.v], [0.0; 4]);
        assert_eq!(midpoint.rgba, 0x80808080);
    }
}
