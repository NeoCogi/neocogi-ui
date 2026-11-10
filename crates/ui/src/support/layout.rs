//! Finite/unbounded constraints and allocation-free resolution of parent-owned tracks.
//!
//! These value algorithms retain neither COM children nor runtime traversal state. Concrete
//! containers in the nodes provider author relationships and replay the resolver during their
//! own scoped layout calls; core uses only copied constraints at its COM service boundary.

use crate::support::Dimensioni;
use neocogi_ui::geometry::{flex_weight, wide_integer, WideScalar};

/// Available space on one measurement axis.
///
/// This is deliberately not encoded in a pixel count: a bounded zero-sized surface and an
/// unbounded measurement request are different inputs.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AvailableSpace {
    /// The parent imposes no maximum on this axis.
    Unbounded,
    /// The parent supplies a non-negative maximum extent.
    Bounded(i32),
}

impl AvailableSpace {
    /// Creates a bounded axis, normalizing an invalid negative extent at the public boundary.
    pub const fn bounded(extent: i32) -> Self {
        Self::Bounded(if extent < 0 { 0 } else { extent })
    }

    /// Returns the finite extent when this axis is bounded.
    pub const fn bound(self) -> Option<i32> {
        match self {
            Self::Unbounded => None,
            Self::Bounded(extent) => Some(extent),
        }
    }

    /// Removes a parent-owned inset, clamping exhausted finite space to zero.
    /// Negative insets remove nothing; an unbounded axis remains unbounded.
    pub const fn shrink(self, inset: i32) -> Self {
        let inset = if inset < 0 { 0 } else { inset };
        match self {
            Self::Unbounded => Self::Unbounded,
            Self::Bounded(extent) => Self::bounded(extent.saturating_sub(inset)),
        }
    }
}

/// Independent width and height constraints supplied during measurement.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Constraints {
    /// Horizontal measurement space.
    pub width: AvailableSpace,
    /// Vertical measurement space.
    pub height: AvailableSpace,
}

impl Constraints {
    /// Creates explicit axis constraints.
    pub const fn new(width: AvailableSpace, height: AvailableSpace) -> Self {
        Self { width, height }
    }

    /// Creates an unconstrained preferred-size query.
    pub const fn unbounded() -> Self {
        Self::new(AvailableSpace::Unbounded, AvailableSpace::Unbounded)
    }

    /// Creates a measurement constrained to a non-negative maximum size.
    pub const fn bounded(size: Dimensioni) -> Self {
        Self::new(AvailableSpace::bounded(size.width), AvailableSpace::bounded(size.height))
    }
}

impl Default for Constraints {
    fn default() -> Self {
        Self::unbounded()
    }
}

/// Sizing rule for one parent-owned, one-dimensional Linear or Grid track.
///
/// A horizontal Linear track resolves to one child width, and a vertical Linear track resolves to
/// one child height. Grid resolves column and row tracks independently; children may share or span
/// them. A track is therefore an axis extent, not a child, grid cell, or final rectangle.
///
/// The parent stores this rule, combines it with measured child content and available space, and
/// resolves it to an exact non-negative pixel extent during measurement or placement.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum TrackSize {
    /// Uses the measured content extent.
    #[default]
    Content,
    /// Uses an exact non-negative pixel extent.
    Fixed(i32),
    /// Receives a weighted share of bounded space left after content, fixed tracks, and gaps.
    /// Under an unbounded constraint it contributes its measured content extent.
    Flex(f32),
}

/// Replayable resolution of one ordered set of parent-owned tracks.
///
/// Construction summarizes reservations and flex weights without retaining the input sequence.
/// Callers then replay the same `(track, content)` pairs through [`Self::next`]. This keeps
/// immutable measurement allocation-free while giving placement exact per-child extents.
pub struct TrackResolver {
    /// Original axis bound determines whether flex tracks receive remaining pixels.
    available: AvailableSpace,
    /// Bounded pixels left after fixed/content reservations and inter-track gaps.
    flexible_space: i32,
    /// Sum of validated positive weights across the replayable track sequence.
    total_flex: WideScalar,
    /// Weight consumed by tracks already replayed through `next`.
    cumulative_flex: WideScalar,
    /// Integer pixels assigned so far; cumulative rounding keeps the final sum exact.
    distributed_flex: i32,
    /// Complete resolved axis extent including reservations, flex pixels, and gaps.
    extent: i32,
}

impl TrackResolver {
    /// Summarizes an ordered track axis and computes its complete extent including gaps.
    pub fn new(available: AvailableSpace, gap: i32, count: usize, items: impl IntoIterator<Item = (TrackSize, i32)>) -> Result<Self, NcStatus> {
        Self::try_new(available, gap, count, items.into_iter().map(Ok))
    }

    /// Measurement and checked arithmetic errors propagate before placement can use a partial summary.
    pub fn try_new(
        available: AvailableSpace,
        gap: i32,
        count: usize,
        items: impl IntoIterator<Item = Result<(TrackSize, i32), NcStatus>>,
    ) -> Result<Self, NcStatus> {
        let mut total_flex = WideScalar::ZERO;
        let mut reserved = WideScalar::ZERO;
        let gaps = i64::try_from(count.saturating_sub(1)).map_err(|_| neocogi_com::NC_E_OUT_OF_RANGE)?;
        let total_gap = wide_integer(gap.max(0))
            .checked_mul(WideScalar::try_from_int(gaps).map_err(neocogi_ui::geometry::status)?)
            .map_err(neocogi_ui::geometry::status)?;
        for item in items {
            let (track, content) = item?;
            match track {
                TrackSize::Content => reserved = reserved.checked_add(wide_integer(content.max(0))).map_err(neocogi_ui::geometry::status)?,
                TrackSize::Fixed(value) => reserved = reserved.checked_add(wide_integer(value.max(0))).map_err(neocogi_ui::geometry::status)?,
                TrackSize::Flex(weight) => {
                    let weight = flex_weight(weight)?;
                    if matches!(available, AvailableSpace::Bounded(_)) {
                        total_flex = total_flex.checked_add(weight).map_err(neocogi_ui::geometry::status)?;
                    } else {
                        reserved = reserved.checked_add(wide_integer(content.max(0))).map_err(neocogi_ui::geometry::status)?;
                    }
                }
            }
        }
        let flexible = if total_flex > WideScalar::ZERO {
            wide_integer(available.bound().unwrap_or(0))
                .checked_sub(total_gap)
                .and_then(|v| v.checked_sub(reserved))
                .map_err(neocogi_ui::geometry::status)?
                .max(WideScalar::ZERO)
        } else {
            WideScalar::ZERO
        };
        let extent = reserved
            .checked_add(total_gap)
            .and_then(|v| v.checked_add(flexible))
            .map_err(neocogi_ui::geometry::status)?;
        let extent = i32::try_from(extent.to_bits() / 4096).map_err(|_| neocogi_com::NC_E_OUT_OF_RANGE)?;
        Ok(Self {
            available,
            flexible_space: (flexible.to_bits() / 4096) as i32,
            total_flex,
            cumulative_flex: WideScalar::ZERO,
            distributed_flex: 0,
            extent,
        })
    }

    /// Resolves the next replayed track to one exact non-negative extent.
    pub fn next(&mut self, track: TrackSize, content: i32) -> Result<i32, NcStatus> {
        match (self.available, track) {
            (_, TrackSize::Content) => Ok(content.max(0)),
            (_, TrackSize::Fixed(value)) => Ok(value.max(0)),
            (AvailableSpace::Unbounded, TrackSize::Flex(_)) => Ok(content.max(0)),
            (AvailableSpace::Bounded(_), TrackSize::Flex(weight)) => {
                let weight = flex_weight(weight)?;
                if self.total_flex == WideScalar::ZERO {
                    return Ok(0);
                }
                let cumulative = self.cumulative_flex.checked_add(weight).map_err(neocogi_ui::geometry::status)?;
                if cumulative > self.total_flex {
                    return Err(neocogi_com::NC_E_INVALID_ARG);
                }
                // Exact cumulative ceiling is a pixel-allocation policy. Multiplying raw fixed
                // weights by i32 pixels in i128 avoids quantizing the ratio before its final ceil.
                // Both operands are bounded by i64/i32, so the product fits i128.
                let numerator = i128::from(self.flexible_space) * i128::from(cumulative.to_bits());
                let denominator = i128::from(self.total_flex.to_bits());
                let cumulative_pixels = ((numerator + denominator - 1) / denominator) as i32;
                let extent = cumulative_pixels - self.distributed_flex;
                self.cumulative_flex = cumulative;
                self.distributed_flex = cumulative_pixels;
                Ok(extent)
            }
        }
    }

    /// Returns the exact resolved track span including gaps.
    pub const fn extent(&self) -> i32 {
        self.extent
    }
}

/// Resolves measured content extents in place and returns their complete track span.
///
/// Grid uses this slice form with retained placement buffers; linear layout replays
/// [`TrackResolver`] directly into its own retained extent buffer.
pub fn resolve_tracks_in_place(available: AvailableSpace, gap: i32, tracks: &[TrackSize], content: &mut [i32]) -> Result<i32, NcStatus> {
    // Normalize caller-owned measurements before the resolver summarizes or overwrites them.
    content.iter_mut().for_each(|extent| *extent = (*extent).max(0));
    let mut resolver = TrackResolver::new(
        available,
        gap,
        content.len(),
        content.iter().enumerate().map(|(index, extent)| (track_at(tracks, index), *extent)),
    )?;
    for (index, extent) in content.iter_mut().enumerate() {
        *extent = resolver.next(track_at(tracks, index), *extent)?;
    }
    Ok(resolver.extent())
}

/// Returns explicit track metadata or the content-sized default for an omitted index.
fn track_at(tracks: &[TrackSize], index: usize) -> TrackSize {
    tracks.get(index).copied().unwrap_or(TrackSize::Content)
}

use neocogi_ui::abi::core::{AxisLimit, LimitKind, LayoutLimits};
use neocogi_com::NcStatus;

/// Converts one checked contract axis without treating finite zero as unbounded.
pub fn available(axis: AxisLimit) -> Result<AvailableSpace, NcStatus> {
    axis.validate()?;
    Ok(match axis.kind {
        LimitKind::Finite => AvailableSpace::Bounded(axis.extent),
        LimitKind::Unbounded => AvailableSpace::Unbounded,
        _ => return Err(neocogi_com::NC_E_INVALID_ARG),
    })
}

/// Validates both axes before producing private finite/unbounded constraints.
pub fn constraints(limits: LayoutLimits) -> Result<Constraints, NcStatus> {
    Ok(Constraints::new(available(limits.width)?, available(limits.height)?))
}

/// Preserves zero versus unbounded when entering the COM layout boundary.
pub fn limits(constraints: Constraints) -> LayoutLimits {
    LayoutLimits {
        width: axis_limit(constraints.width),
        height: axis_limit(constraints.height),
    }
}

/// Converts one private constraint without losing an explicitly bounded zero.
fn axis_limit(space: AvailableSpace) -> AxisLimit {
    match space.bound() {
        Some(extent) => AxisLimit { kind: LimitKind::Finite, extent },
        None => AxisLimit { kind: LimitKind::Unbounded, extent: 0 },
    }
}

#[cfg(all(test, feature = "test-support"))]
mod explicit_constraint_tests {
    use super::*;

    /// Finite insets consume space, including the last pixel, without creating a sentinel.
    #[test]
    fn shrinking_finite_space_never_produces_negative_or_unbounded_limits() {
        for extent in [0, 1, 2, 3, 4, i32::MAX] {
            for inset in [i32::MIN, -1, 0, 1, 3, i32::MAX] {
                let expected = (i64::from(extent) - i64::from(inset.max(0))).max(0) as i32;
                let shrunk = AvailableSpace::bounded(extent).shrink(inset);
                assert_eq!(shrunk, AvailableSpace::Bounded(expected), "{extent} - {inset}");
                assert_eq!(available(axis_limit(shrunk)), Ok(shrunk));
                assert_eq!(AvailableSpace::Unbounded.shrink(inset), AvailableSpace::Unbounded);
            }
        }
    }

    #[test]
    fn bounded_constructor_preserves_zero_and_normalizes_negative_extents() {
        assert_eq!(AvailableSpace::bounded(0), AvailableSpace::Bounded(0));
        assert_eq!(AvailableSpace::bounded(-7), AvailableSpace::Bounded(0));
    }

    #[test]
    fn constraints_keep_axis_bounds_independent() {
        let constraints = Constraints::new(AvailableSpace::Unbounded, AvailableSpace::Bounded(24));
        assert_eq!(constraints.width.bound(), None);
        assert_eq!(constraints.height.bound(), Some(24));
    }
}

#[cfg(all(test, feature = "test-support"))]
mod track_resolution_tests {
    use super::*;

    #[test]
    fn unbounded_tracks_use_content_except_for_fixed_extents() {
        let mut content = [10, 20, 30, 40];
        let extent = resolve_tracks_in_place(
            AvailableSpace::Unbounded,
            2,
            &[TrackSize::Content, TrackSize::Fixed(7), TrackSize::Flex(1.0)],
            &mut content,
        );
        assert_eq!(content, [10, 7, 30, 40]);
        assert_eq!(extent.unwrap(), 93);
    }

    #[test]
    fn bounded_tracks_reserve_content_fixed_and_gaps_before_flex() {
        let mut content = [10, 99, 99];
        let extent = resolve_tracks_in_place(
            AvailableSpace::bounded(40),
            2,
            &[TrackSize::Content, TrackSize::Fixed(8), TrackSize::Flex(1.0)],
            &mut content,
        );
        assert_eq!(content, [10, 8, 18]);
        assert_eq!(extent.unwrap(), 40);
    }

    #[test]
    fn bounded_content_and_fixed_tracks_do_not_claim_unused_space() {
        // A bound informs responsive measurement; without a valid flex recipient it is not itself
        // part of the track sequence's desired size.
        let mut content = [10, 99];
        let extent = resolve_tracks_in_place(AvailableSpace::bounded(100), 2, &[TrackSize::Content, TrackSize::Fixed(8)], &mut content);

        assert_eq!(content, [10, 8]);
        assert_eq!(extent.unwrap(), 20);
    }

    #[test]
    fn invalid_flex_weights_reject_before_distribution() {
        let mut content = [10, 20, 30];
        let result = resolve_tracks_in_place(
            AvailableSpace::bounded(100),
            2,
            &[TrackSize::Content, TrackSize::Flex(0.0), TrackSize::Flex(f32::NAN)],
            &mut content,
        );
        assert_eq!(result, Err(neocogi_com::NC_E_INVALID_ARG));
        assert_eq!(content, [10, 20, 30]);
    }

    #[test]
    fn rounding_pixels_are_deterministic_and_keep_the_exact_bound() {
        let mut content = [0; 3];
        let extent = resolve_tracks_in_place(
            AvailableSpace::bounded(10),
            0,
            &[TrackSize::Flex(1.0), TrackSize::Flex(1.0), TrackSize::Flex(1.0)],
            &mut content,
        );
        assert_eq!(content, [4, 3, 3]);
        assert_eq!(extent.unwrap(), 10);
    }

    #[test]
    fn content_overflow_does_not_shrink_or_overlap_tracks() {
        let mut content = [20, 30, 50];
        let extent = resolve_tracks_in_place(
            AvailableSpace::bounded(40),
            3,
            &[TrackSize::Content, TrackSize::Fixed(30), TrackSize::Flex(1.0)],
            &mut content,
        );
        assert_eq!(content, [20, 30, 0]);
        assert_eq!(extent.unwrap(), 56);
    }

    #[test]
    fn bounded_zero_is_not_unbounded() {
        let mut content = [12];
        let extent = resolve_tracks_in_place(AvailableSpace::bounded(0), 0, &[TrackSize::Flex(1.0)], &mut content);
        assert_eq!(content, [0]);
        assert_eq!(extent.unwrap(), 0);
    }

    /// Ratio rounding must occur only at the final pixel, including thirds and fractional weights.
    #[test]
    fn fixed_flex_keeps_exact_ceiling_and_reports_unrepresentable_metadata() {
        let mut thirds = [0; 3];
        assert_eq!(
            resolve_tracks_in_place(AvailableSpace::bounded(3), 0, &[TrackSize::Flex(1.0); 3], &mut thirds).unwrap(),
            3
        );
        assert_eq!(thirds, [1, 1, 1]);
        let mut halves = [0; 2];
        assert_eq!(
            resolve_tracks_in_place(AvailableSpace::bounded(4), 0, &[TrackSize::Flex(1.5), TrackSize::Flex(0.5)], &mut halves).unwrap(),
            4
        );
        assert_eq!(halves, [3, 1]);
        assert_eq!(flex_weight(1e30), Err(neocogi_com::NC_E_OUT_OF_RANGE));
        assert_eq!(flex_weight(f32::MIN_POSITIVE), Err(neocogi_com::NC_E_OUT_OF_RANGE));
        assert!(matches!(
            TrackResolver::new(AvailableSpace::Unbounded, 0, 2, [(TrackSize::Fixed(i32::MAX), 0); 2]),
            Err(neocogi_com::NC_E_OUT_OF_RANGE)
        ));
    }
}
