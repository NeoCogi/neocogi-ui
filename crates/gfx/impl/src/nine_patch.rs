//! Typed nine-patch validation and expansion inside the GFX ownership boundary.
//!
//! The compositor receives one semantic operation and asks this module for its visible atlas
//! cells. This module knows patch structure and bundle-local image lookup; the compositor remains
//! responsible for clipping those cells into final vertices and publishing renderer batches.

use neocogi_com::{NC_E_INVALID_ARG, NC_E_OUT_OF_RANGE, NcStatus};
use neocogi_gfx_support::abi::{AssetIcon, AssetPatch, Insets, NinePatchDraw};
use neocogi_render_core::abi::Rect;

/// Number of rows and columns in the fixed patch grid.
const PATCH_AXIS_CELLS: usize = 3;
/// Number of cells in one complete three-by-three patch.
const PATCH_CELL_COUNT: usize = PATCH_AXIS_CELLS * PATCH_AXIS_CELLS;

/// Straight-alpha pink (#FFC0CB) diagnostic, with 0.5 rounded to the 8-bit alpha domain.
const SHRUNK_SOURCE_OVERLAY_RGBA: u32 = u32::from_le_bytes([255, 192, 203, 128]);

/// One visible atlas rectangle produced from a validated semantic patch.
#[derive(Clone, Copy)]
pub(super) struct AtlasCell {
    /// Unclipped destination in final drawable coordinates.
    pub(super) destination: Rect,
    /// Pixel-space source wholly derived from the compositor's current atlas tables.
    pub(super) source: Rect,
    /// Color modulation packed in the renderer contract's canonical channel order.
    pub(super) rgba: u32,
}

/// Fixed-capacity result of expanding one patch without allocation.
pub(super) struct ExpandedPatch {
    /// Geometric cell slots in row-major order; invisible cells remain explicitly absent.
    cells: [Option<AtlasCell>; PATCH_CELL_COUNT],
    /// Authenticated white tile for a pink overlay when source slices required shrinking.
    /// None means ordinary image/flat rendering; destination-only shrinking never sets this.
    diagnostic_white: Option<Rect>,
}

impl ExpandedPatch {
    /// Consumes the fixed grid and yields only cells that can contribute atlas pixels.
    pub(super) fn visible_cells(self) -> impl Iterator<Item = AtlasCell> {
        // Keep each diagnostic immediately after its image cell in painter order. Sampling the
        // white tile adds pink with source-over blending even on black artwork; tint modulation
        // alone could not do that. Empty/hidden cells remain absent and no allocation is needed.
        self.cells.into_iter().flatten().flat_map(move |cell| {
            [
                Some(cell),
                self.diagnostic_white.map(|source| AtlasCell {
                    destination: cell.destination,
                    source,
                    rgba: SHRUNK_SOURCE_OVERLAY_RGBA,
                }),
            ]
            .into_iter()
            .flatten()
        })
    }
}

/// Validates and expands one complete patch from the compositor's narrow installed sources.
pub(super) fn expand(destination: Rect, draw: NinePatchDraw, white: Rect, icons: &[AssetIcon]) -> Result<ExpandedPatch, NcStatus> {
    // Negative extents describe malformed drawing input. Empty destinations remain valid no-ops,
    // matching the other GFX rectangle operations and avoiding special cases in UI replay.
    if destination.width < 0 || destination.height < 0 {
        return Err(NC_E_INVALID_ARG);
    }
    validate_insets(draw.insets)?;
    if draw.center_visible > 1 {
        return Err(NC_E_INVALID_ARG);
    }

    let destinations = partition_rectangle(destination, draw.insets);
    let (sources, colors, diagnostic_white) = match draw.patch {
        AssetPatch::Flat {
            top_left,
            top,
            top_right,
            left,
            center,
            right,
            bottom_left,
            bottom,
            bottom_right,
        } => {
            // Flat colors sample the authenticated opaque-white tile. Keeping the same source for
            // all cells lets the ordinary atlas batch path provide color-only rendering.
            (
                [[white; PATCH_AXIS_CELLS]; PATCH_AXIS_CELLS],
                [top_left, top, top_right, left, center, right, bottom_left, bottom, bottom_right],
                None,
            )
        }
        AssetPatch::Image { icon, source: source_insets, tint } => {
            // Resolve the typed bundle-local index before producing any cell. An invalid foreign
            // operation therefore cannot append a prefix of an otherwise malformed patch.
            validate_insets(source_insets)?;
            let source_rectangle = icons.get(icon as usize).ok_or(NC_E_OUT_OF_RANGE)?.rect;
            // Retain authored slices until expansion, so stored and direct patches share
            // proportional normalization and diagnostics. Widen before adding opposing sides.
            let source_shrinks = i64::from(source_insets.left) + i64::from(source_insets.right) > i64::from(source_rectangle.width)
                || i64::from(source_insets.top) + i64::from(source_insets.bottom) > i64::from(source_rectangle.height);
            (
                partition_rectangle(source_rectangle, source_insets),
                [tint; PATCH_CELL_COUNT],
                source_shrinks.then_some(white),
            )
        }
    };

    let mut cells = [None; PATCH_CELL_COUNT];
    for index in 0..PATCH_CELL_COUNT {
        // Row and column are derived from the single canonical row-major index. This prevents flat
        // colors, destination cells, and image-source cells from acquiring independent orderings.
        let row = index / PATCH_AXIS_CELLS;
        let column = index % PATCH_AXIS_CELLS;
        if index == PATCH_CELL_COUNT / 2 && draw.center_visible == 0 {
            continue;
        }
        let destination = destinations[row][column];
        let source = sources[row][column];
        let rgba = colors[index];
        if destination.width == 0 || destination.height == 0 || source.width == 0 || source.height == 0 || rgba.to_le_bytes()[3] == 0 {
            continue;
        }
        cells[index] = Some(AtlasCell { destination, source, rgba });
    }
    Ok(ExpandedPatch { cells, diagnostic_white })
}

/// Rejects negative slice thickness before any geometry or resource lookup occurs.
fn validate_insets(insets: Insets) -> Result<(), NcStatus> {
    // UI normalizes programmatic values before crossing the ABI, while direct foreign callers must
    // satisfy the contract explicitly. One check therefore governs both source and destination data.
    if [insets.left, insets.top, insets.right, insets.bottom].into_iter().any(|value| value < 0) {
        Err(NC_E_INVALID_ARG)
    } else {
        Ok(())
    }
}

/// Divides one rectangle into an exact three-by-three grid using proportional tiny-axis behavior.
fn partition_rectangle(outer: Rect, insets: Insets) -> [[Rect; PATCH_AXIS_CELLS]; PATCH_AXIS_CELLS] {
    // Each axis is resolved independently. Source and destination rectangles consequently share
    // the same non-overlapping policy without duplicating nine-patch arithmetic in UI.
    let widths = partition_axis(outer.width, insets.left, insets.right);
    let heights = partition_axis(outer.height, insets.top, insets.bottom);
    let xs = partition_origins(outer.x, widths);
    let ys = partition_origins(outer.y, heights);

    [
        [
            Rect {
                x: xs[0],
                y: ys[0],
                width: widths[0],
                height: heights[0],
            },
            Rect {
                x: xs[1],
                y: ys[0],
                width: widths[1],
                height: heights[0],
            },
            Rect {
                x: xs[2],
                y: ys[0],
                width: widths[2],
                height: heights[0],
            },
        ],
        [
            Rect {
                x: xs[0],
                y: ys[1],
                width: widths[0],
                height: heights[1],
            },
            Rect {
                x: xs[1],
                y: ys[1],
                width: widths[1],
                height: heights[1],
            },
            Rect {
                x: xs[2],
                y: ys[1],
                width: widths[2],
                height: heights[1],
            },
        ],
        [
            Rect {
                x: xs[0],
                y: ys[2],
                width: widths[0],
                height: heights[2],
            },
            Rect {
                x: xs[1],
                y: ys[2],
                width: widths[1],
                height: heights[2],
            },
            Rect {
                x: xs[2],
                y: ys[2],
                width: widths[2],
                height: heights[2],
            },
        ],
    ]
}

/// Divides one non-negative extent into leading, stretchable, and trailing lengths.
fn partition_axis(extent: i32, leading: i32, trailing: i32) -> [i32; PATCH_AXIS_CELLS] {
    // Callers have validated nonnegative extent and sides. Empty axes have no drawable cells;
    // ordinary axes assign every unclaimed pixel to the center. Widen before adding the sides
    // so the true ratio is preserved over the entire accepted destination/source domain.
    if extent == 0 {
        return [0, 0, 0];
    }
    let requested = i64::from(leading) + i64::from(trailing);
    if requested <= i64::from(extent) {
        return [leading, (i64::from(extent) - requested) as i32, trailing];
    }

    // When fixed sides do not fit, scale them proportionally using wide arithmetic. Rounding the
    // leading side and assigning the exact remainder to the trailing side preserves full coverage.
    // requested > extent > 0 and leading <= requested bound the rounded quotient to 0..=extent.
    // Even MAX*MAX + requested/2 fits i64. Narrow only this bounded final cell.
    let scaled_leading = ((i64::from(extent) * i64::from(leading) + requested / 2) / requested) as i32;
    [scaled_leading, 0, extent - scaled_leading]
}

/// Computes row or column origins without overflowing the public signed coordinate domain.
fn partition_origins(origin: i32, lengths: [i32; PATCH_AXIS_CELLS]) -> [i32; PATCH_AXIS_CELLS] {
    // Saturation matches the renderer's rectangle policy for extreme but representable application
    // coordinates. Final viewport clipping removes cells that lie outside drawable space.
    let center = origin.saturating_add(lengths[0]);
    let trailing = center.saturating_add(lengths[1]);
    [origin, center, trailing]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Boundary partitions retain exact coverage and round the leading proportional side once.
    #[test]
    fn extreme_and_boundary_partitions_preserve_proportions_and_coverage() {
        let cases = [
            (0, i32::MAX, i32::MAX, [0, 0, 0]),
            (100, 0, 0, [0, 100, 0]),
            (100, 4, 6, [4, 90, 6]),
            (5, 4, 6, [2, 0, 3]),
            (100, i32::MAX, i32::MAX, [50, 0, 50]),
            (100, 1_500_000_000, 1_000_000_000, [60, 0, 40]),
            (100, i32::MAX, 0, [100, 0, 0]),
            (100, 0, i32::MAX, [0, 0, 100]),
            (i32::MAX, i32::MAX / 2, i32::MAX / 2, [i32::MAX / 2, 1, i32::MAX / 2]),
            (i32::MAX, i32::MAX / 2, i32::MAX / 2 + 1, [i32::MAX / 2, 0, i32::MAX / 2 + 1]),
            (i32::MAX, i32::MAX / 2 + 1, i32::MAX / 2 + 1, [1_073_741_824, 0, 1_073_741_823]),
            (i32::MAX, i32::MAX, i32::MAX, [1_073_741_824, 0, 1_073_741_823]),
            (1, i32::MAX, i32::MAX, [1, 0, 0]),
        ];
        for (extent, leading, trailing, expected) in cases {
            let actual = partition_axis(extent, leading, trailing);
            assert_eq!(actual, expected, "extent={extent}, sides={leading}/{trailing}");
            assert!(actual.into_iter().all(|cell| (0..=extent).contains(&cell)));
            assert_eq!(actual.into_iter().map(i64::from).sum::<i64>(), i64::from(extent));
        }
        // Adjacent combinations exercise the rounding invariant without duplicating the
        // production quotient. The leading cell's ratio error cannot exceed half a pixel.
        for extent in [0, 1, 5, 100, i32::MAX] {
            for leading in [0, 1, 4, i32::MAX / 2, i32::MAX] {
                for trailing in [0, 1, 6, i32::MAX / 2 + 1, i32::MAX] {
                    let cells = partition_axis(extent, leading, trailing);
                    assert!(cells.into_iter().all(|cell| cell >= 0 && cell <= extent));
                    assert_eq!(cells.into_iter().map(i64::from).sum::<i64>(), i64::from(extent));
                    let requested = i128::from(leading) + i128::from(trailing);
                    if requested > i128::from(extent) {
                        assert_eq!(cells[1], 0);
                        let error = (i128::from(cells[0]) * requested - i128::from(extent) * i128::from(leading)).abs();
                        assert!(error * 2 <= requested);
                    }
                }
            }
        }
    }

    /// Either source axis triggers the overlay; hidden/empty/transparent geometry stays absent.
    #[test]
    fn source_shrink_diagnostics_follow_both_axes_and_visibility() {
        let white = Rect { x: 0, y: 0, width: 4, height: 4 };
        let icons = [AssetIcon { name: Default::default(), rect: white }];
        for (source, sides, original_cells) in [
            (
                Insets {
                    left: i32::MAX,
                    right: i32::MAX,
                    top: 0,
                    bottom: 0,
                },
                Insets { left: 50, right: 50, top: 0, bottom: 0 },
                2,
            ),
            (
                Insets {
                    left: 0,
                    right: 0,
                    top: i32::MAX,
                    bottom: i32::MAX,
                },
                Insets { left: 0, right: 0, top: 50, bottom: 50 },
                2,
            ),
            (Insets::uniform(i32::MAX), Insets::uniform(50), 4),
        ] {
            let destination = Rect { x: 0, y: 0, width: 100, height: 100 };
            let draw = NinePatchDraw {
                insets: sides,
                center_visible: 0,
                patch: AssetPatch::Image { icon: 0, source, tint: 0xffffffff },
            };
            let cells: Vec<_> = expand(destination, draw, white, &icons).unwrap().visible_cells().collect();
            assert_eq!(cells.len(), original_cells * 2);
            for pair in cells.as_chunks::<2>().0 {
                assert_eq!(components(pair[0].destination), components(pair[1].destination));
                assert_eq!(pair[0].rgba, 0xffffffff);
                assert_eq!(pair[1].rgba, u32::from_le_bytes([255, 192, 203, 128]));
                assert_eq!(components(pair[1].source), components(white));
                let source = pair[0].source;
                assert!(source.x >= 0 && source.y >= 0 && source.x + source.width <= 4 && source.y + source.height <= 4);
            }
            let invisible = NinePatchDraw {
                patch: AssetPatch::Image { icon: 0, source, tint: 0 },
                ..draw
            };
            assert_eq!(expand(destination, invisible, white, &icons).unwrap().visible_cells().count(), 0);
            assert_eq!(
                expand(Rect { width: 0, ..destination }, draw, white, &icons).unwrap().visible_cells().count(),
                0
            );
        }
    }

    /// Converts one rectangle into an equality-friendly tuple for focused geometry assertions.
    fn components(rectangle: Rect) -> (i32, i32, i32, i32) {
        // The renderer math type deliberately does not require structural equality.
        (rectangle.x, rectangle.y, rectangle.width, rectangle.height)
    }

    /// Ordinary insets retain fixed edges and assign the remainder to the stretchable center.
    #[test]
    fn regular_partition_preserves_requested_outer_cells() {
        let cells = partition_rectangle(Rect { x: 10, y: 20, width: 20, height: 18 }, Insets { left: 2, top: 3, right: 4, bottom: 5 });

        assert_eq!(components(cells[0][0]), (10, 20, 2, 3));
        assert_eq!(components(cells[0][1]), (12, 20, 14, 3));
        assert_eq!(components(cells[1][1]), (12, 23, 14, 10));
        assert_eq!(components(cells[2][2]), (26, 33, 4, 5));
    }

    /// Oversized opposing insets collapse the center without overlap or lost destination pixels.
    #[test]
    fn tiny_partition_scales_fixed_cells_proportionally() {
        let cells = partition_rectangle(Rect { x: 5, y: 6, width: 3, height: 2 }, Insets { left: 4, top: 3, right: 6, bottom: 7 });

        assert_eq!(cells[0].iter().map(|cell| cell.width).sum::<i32>(), 3);
        assert_eq!(cells.iter().map(|row| row[0].height).sum::<i32>(), 2);
        assert_eq!(components(cells[1][1]), (6, 7, 0, 0));
    }
}
