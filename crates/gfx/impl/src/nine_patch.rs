//! Typed nine-patch validation and expansion inside the GFX ownership boundary.
//!
//! The compositor receives one semantic operation and asks this module for its visible atlas
//! cells. This module knows patch structure and bundle-local image lookup; the compositor remains
//! responsible for clipping those cells into final vertices and publishing renderer batches.

use neocogi_com::{NC_E_INVALID_ARG, NC_E_OUT_OF_RANGE, NcStatus};
use neocogi_gfx_contracts::{AssetIcon, AssetPatch, Insets, NinePatchDraw};
use neocogi_renderer_contracts::Rect;

/// Number of rows and columns in the fixed patch grid.
const PATCH_AXIS_CELLS: usize = 3;
/// Number of cells in one complete three-by-three patch.
const PATCH_CELL_COUNT: usize = PATCH_AXIS_CELLS * PATCH_AXIS_CELLS;

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
}

impl ExpandedPatch {
    /// Consumes the fixed grid and yields only cells that can contribute atlas pixels.
    pub(super) fn visible_cells(self) -> impl Iterator<Item = AtlasCell> {
        // Array iteration preserves row-major painter order while `flatten` removes semantic
        // omissions without allocating a secondary command list.
        self.cells.into_iter().flatten()
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
    let (sources, colors) = match draw.patch {
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
            )
        }
        AssetPatch::Image { icon, source: source_insets, tint } => {
            // Resolve the typed bundle-local index before producing any cell. An invalid foreign
            // operation therefore cannot append a prefix of an otherwise malformed patch.
            validate_insets(source_insets)?;
            let source_rectangle = icons.get(icon as usize).ok_or(NC_E_OUT_OF_RANGE)?.rect;
            (partition_rectangle(source_rectangle, source_insets), [tint; PATCH_CELL_COUNT])
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
    Ok(ExpandedPatch { cells })
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
    // Callers have already validated insets. A zero destination or source axis has no drawable
    // cells, while an ordinary axis assigns every unclaimed pixel to its center.
    if extent <= 0 {
        return [0, 0, 0];
    }
    let requested = leading.saturating_add(trailing);
    if requested <= extent {
        return [leading, extent - requested, trailing];
    }
    if requested == 0 {
        return [0, extent, 0];
    }

    // When fixed sides do not fit, scale them proportionally using wide arithmetic. Rounding the
    // leading side and assigning the exact remainder to the trailing side preserves full coverage.
    let scaled_leading = ((i64::from(extent) * i64::from(leading) + i64::from(requested) / 2) / i64::from(requested)) as i32;
    let scaled_leading = scaled_leading.clamp(0, extent);
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
