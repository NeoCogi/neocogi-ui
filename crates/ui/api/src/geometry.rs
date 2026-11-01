//! Numeric admission for UI geometry, independent of renderer-owned scene coordinates.
//!
//! Public integer values are pixels. Explicit fractional drawing inputs round once to 1/64 pixel,
//! ties to even. This grid converts exactly to GPU f32 positions throughout the supported domain.
//! Content lengths and scroll offsets are separate wide quantities, not visible coordinates.

use neocogi_com::{NC_E_INVALID_ARG, NC_E_OUT_OF_RANGE, NcStatus};
use rs_math3d::{Fixed, MathError, Rect, Vec2f, Vec2i, Vector2};

/// Inclusive magnitude limit for a visible UI coordinate, in pixels.
pub const COORDINATE_LIMIT: i32 = 128 * 1024;
/// Number of stored subpixel units in a pixel.
pub const SUBPIXELS: i32 = 64;
/// Checked fractional coordinate storage; admission additionally enforces [`COORDINATE_LIMIT`].
pub type Scalar = Fixed<i32, 6>;
/// UI point using the same fixed scalar on both axes.
pub type Point = Vector2<Scalar>;
/// UI rectangle; origins and maximum edges are checked independently of extents.
pub type Rectangle = Rect<Scalar>;
/// Working scalar with room for complete-domain products and bounded polygon accumulation.
/// Twelve fractional bits preserve products of two admitted 1/64-pixel inputs exactly.
pub type WideScalar = Fixed<i64, 12>;
/// Working point for checked translations, normalization and geometric predicates.
pub type WidePoint = Vector2<WideScalar>;
/// Wide geometry also represents virtual content until scroll translation and clipping.
pub type WideRectangle = Rect<WideScalar>;

/// Converts a math error into the public distinction between invalid values and excess range.
pub fn status(error: MathError) -> NcStatus {
    match error {
        MathError::Overflow => NC_E_OUT_OF_RANGE,
        _ => NC_E_INVALID_ARG,
    }
}

/// Converts one integer pixel coordinate without a floating-point intermediate.
pub fn integer(value: i32) -> Result<Scalar, NcStatus> {
    if !(-COORDINATE_LIMIT..=COORDINATE_LIMIT).contains(&value) {
        return Err(NC_E_OUT_OF_RANGE);
    }
    Scalar::try_from_int(i64::from(value)).map_err(status)
}

/// Converts a fractional input after checking its original value, before quantization.
pub fn fractional(value: f32) -> Result<Scalar, NcStatus> {
    if !value.is_finite() {
        return Err(NC_E_INVALID_ARG);
    }
    if !(-(COORDINATE_LIMIT as f32)..=COORDINATE_LIMIT as f32).contains(&value) {
        return Err(NC_E_OUT_OF_RANGE);
    }
    Scalar::try_from_f32(value).map_err(status)
}

/// Admits a nonnegative pixel extent, which may span the complete signed coordinate domain.
pub fn integer_extent(value: i32) -> Result<Scalar, NcStatus> {
    if value < 0 {
        return Err(NC_E_INVALID_ARG);
    }
    if value > 2 * COORDINATE_LIMIT {
        return Err(NC_E_OUT_OF_RANGE);
    }
    Scalar::try_from_int(i64::from(value)).map_err(status)
}

/// Admits an explicit fractional extent before rounding; negative/nonfinite extents are invalid.
pub fn fractional_extent(value: f32) -> Result<Scalar, NcStatus> {
    if !value.is_finite() || value < 0.0 {
        return Err(NC_E_INVALID_ARG);
    }
    if value > (2 * COORDINATE_LIMIT) as f32 {
        return Err(NC_E_OUT_OF_RANGE);
    }
    Scalar::try_from_f32(value).map_err(status)
}

/// Admits an ordinary integer UI position.
pub fn integer_point(value: Vec2i) -> Result<Point, NcStatus> {
    Ok(Point::new(integer(value.x)?, integer(value.y)?))
}

/// Admits an explicitly fractional UI position, rounding once at this boundary.
pub fn fractional_point(value: Vec2f) -> Result<Point, NcStatus> {
    Ok(Point::new(fractional(value.x)?, fractional(value.y)?))
}

/// Widens any i32 pixel value exactly, including virtual layout values outside visible bounds.
/// The entire i32 domain times 4096 fits i64; this conversion performs no arithmetic narrowing.
pub const fn wide_integer(value: i32) -> WideScalar {
    WideScalar::from_bits((value as i64) * 4096)
}

/// Admits and widens an explicitly fractional position for checked geometry calculations.
pub fn wide_fractional_point(value: Vec2f) -> Result<WidePoint, NcStatus> {
    fractional_point(value)?.try_cast().ok_or(NC_E_OUT_OF_RANGE)
}

/// Validates the ordinary rectangle contract, including its exclusive maximum edges.
/// A width can span twice the coordinate limit; it is not itself a signed position.
pub fn rectangle(value: crate::Rect) -> Result<Rectangle, NcStatus> {
    if value.width < 0 || value.height < 0 {
        return Err(NC_E_INVALID_ARG);
    }
    let min = integer_point(Vec2i::new(value.x, value.y))?;
    let result = Rectangle::new(min.x, min.y, integer_extent(value.width)?, integer_extent(value.height)?);
    let max = result.checked_max().map_err(status)?;
    for edge in [max.x, max.y] {
        if edge > Scalar::from_bits(COORDINATE_LIMIT * SUBPIXELS) {
            return Err(NC_E_OUT_OF_RANGE);
        }
    }
    Ok(result)
}

/// Admits a dimensionless positive flex input into the fixed working domain.
/// Coefficients are not pixel coordinates, but must be finite and remain positive after quantization.
pub fn flex_weight(weight: f32) -> Result<WideScalar, NcStatus> {
    if !weight.is_finite() || weight <= 0.0 {
        return Err(neocogi_com::NC_E_INVALID_ARG);
    }
    let result = WideScalar::try_from_f32(weight).map_err(neocogi_ui::geometry::status)?;
    if result <= WideScalar::ZERO {
        return Err(neocogi_com::NC_E_OUT_OF_RANGE);
    }
    Ok(result)
}

/// Converts virtual content metadata exactly before projection into a bounded visible viewport.
/// Semantic text/frame extents and pre-scroll clip requests are model-space integer data, not
/// visible UI positions. Their complete i32 edges fit the wide working domain without saturation.
pub fn virtual_rectangle(value: crate::Rect) -> Result<WideRectangle, NcStatus> {
    if value.width < 0 || value.height < 0 {
        return Err(NC_E_INVALID_ARG);
    }
    let result = WideRectangle::new(
        wide_integer(value.x),
        wide_integer(value.y),
        wide_integer(value.width),
        wide_integer(value.height),
    );
    result.checked_max().map_err(status)?;
    Ok(result)
}

/// Checks generated fixed positions before their final conversion to rendering floats.
pub fn render_point(value: WidePoint) -> Result<Vec2f, NcStatus> {
    let limit = wide_integer(COORDINATE_LIMIT);
    if value.x < -limit || value.x > limit || value.y < -limit || value.y > limit {
        return Err(NC_E_OUT_OF_RANGE);
    }
    value.try_cast().ok_or(NC_E_OUT_OF_RANGE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_and_fractional_boundaries_share_one_domain() {
        for value in [-COORDINATE_LIMIT, 0, COORDINATE_LIMIT] {
            assert_eq!(integer(value).unwrap(), fractional(value as f32).unwrap());
        }
        assert_eq!(integer(COORDINATE_LIMIT + 1), Err(NC_E_OUT_OF_RANGE));
        assert_eq!(fractional(f32::NAN), Err(NC_E_INVALID_ARG));
        assert_eq!(fractional(f32::INFINITY), Err(NC_E_INVALID_ARG));
        assert_eq!(fractional(1e20), Err(NC_E_OUT_OF_RANGE));
        assert_eq!(fractional(10.5).unwrap().to_bits(), 672);
        assert_eq!(
            integer_extent(2 * COORDINATE_LIMIT).unwrap(),
            fractional_extent((2 * COORDINATE_LIMIT) as f32).unwrap()
        );
        assert_eq!(integer_extent(-1), Err(NC_E_INVALID_ARG));
        assert_eq!(fractional_extent(262_145.0), Err(NC_E_OUT_OF_RANGE));
        assert_eq!(fractional(0.5 / 64.0).unwrap().to_bits(), 0);
        assert_eq!(fractional(1.5 / 64.0).unwrap().to_bits(), 2);
        assert_eq!(fractional(-1.5 / 64.0).unwrap().to_bits(), -2);
    }

    #[test]
    fn rectangle_checks_edges_and_allows_the_complete_span() {
        let r = crate::Rect {
            x: -COORDINATE_LIMIT,
            y: 0,
            width: 2 * COORDINATE_LIMIT,
            height: 1,
        };
        assert_eq!(rectangle(r).unwrap().checked_max().unwrap().x, integer(COORDINATE_LIMIT).unwrap());
        let r = crate::Rect {
            x: COORDINATE_LIMIT,
            y: 0,
            width: 1,
            height: 1,
        };
        assert!(matches!(rectangle(r), Err(NC_E_OUT_OF_RANGE)));
        let r = crate::Rect { x: 0, y: 0, width: -1, height: 1 };
        assert!(matches!(rectangle(r), Err(NC_E_INVALID_ARG)));
    }

    #[test]
    fn wide_products_keep_small_areas_at_large_origins() {
        use rs_math3d::Vector3;
        let a = wide_fractional_point(Vec2f::new(131_000.0, 131_000.0)).unwrap();
        let b = wide_fractional_point(Vec2f::new(131_000.5, 131_000.0)).unwrap();
        let cross = Vector3::checked_cross(&Vector3::new(a.x, a.y, WideScalar::ZERO), &Vector3::new(b.x, b.y, WideScalar::ZERO)).unwrap();
        assert_eq!(cross.z.to_f64(), -65_500.0);
        assert_eq!(render_point(a).unwrap().x, 131_000.0);
    }
}
