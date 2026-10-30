//! Thin production display-list replay into the independently deployed GFX compositor.
//!
//! UI keeps semantic painting and painter order. GFX owns resource expansion, final clipping,
//! batching, stream lifetime, and renderer submission, so those implementations are not linked
//! into the UI runtime shared object.

use super::{Color, DisplayList};
use crate::{AtlasHandle, Recti};
use neocogi_com::{ComPtr, ComRef, NC_E_INVALID_ARG, NcCopyFromBufferOffset, NcStatus};
use neocogi_gfx_support::abi::{AssetPatch, DrawPoint, GfxCompositorAbi, Insets, NinePatchDraw, SolidVertex};
use neocogi_render_core::abi::{IRendererAbi, Rect, Size};

/// Replays one complete display list and clears it on success or failure.
pub(crate) fn submit(
    list: &mut DisplayList,
    atlas: &AtlasHandle,
    compositor: &mut ComPtr<GfxCompositorAbi>,
    renderer: ComRef<'_, IRendererAbi>,
    size: Size,
    clear: u32,
) -> Result<(), NcStatus> {
    let result = submit_once(list, atlas, compositor, renderer, size, clear);
    if result.is_err() {
        // Cancellation is idempotent and releases resources retained by operations preceding the
        // failure. Preserve the operation's original status for the UI caller.
        let _ = compositor.cancel();
    }
    list.clear();
    result
}

/// Begins, replays, and submits one list without owning its outer cleanup policy.
fn submit_once(
    list: &mut DisplayList,
    atlas: &AtlasHandle,
    compositor: &mut ComPtr<GfxCompositorAbi>,
    renderer: ComRef<'_, IRendererAbi>,
    size: Size,
    clear: u32,
) -> Result<(), NcStatus> {
    compositor.begin(size, clear)?;
    let super::display_list::DisplayList {
        ops,
        solid_geometry,
        atlas_vertices,
        text,
        ..
    } = list;
    let triangles = solid_geometry.triangles();
    for operation in ops.drain(..) {
        let clip = copied_rect(operation.clip);
        match operation.kind {
            super::display_list::DrawKind::GpuView { block } => {
                if let Some(block) = block {
                    compositor.render_view(block).map_err(|(status, _block)| status)?;
                }
            }
            super::display_list::DrawKind::AtlasTriangles { vertices } => {
                let vertices = atlas_vertices.get(vertices).ok_or(NC_E_INVALID_ARG)?;
                compositor.atlas_triangles(clip, NcCopyFromBufferOffset::new(0, vertices))?;
            }
            super::display_list::DrawKind::ComTexture { texture, rect, source, color } => {
                compositor.texture(clip, copied_rect(rect), source, packed(color), texture.as_com_ref())?
            }
            super::display_list::DrawKind::NinePatch { rect, patch } => {
                // UI crosses the boundary once with the semantic value it recorded. GFX resolves
                // all nine destination/source cells and owns their final clipping and batching.
                compositor.nine_patch(clip, copied_rect(rect), copied_nine_patch(patch, atlas))?;
            }
            super::display_list::DrawKind::Text { font, pos, color, text: range } => {
                let text = text.get(range).ok_or(NC_E_INVALID_ARG)?;
                compositor.text(
                    clip,
                    atlas.font_index(font),
                    DrawPoint { x: pos.x, y: pos.y },
                    packed(color),
                    NcCopyFromBufferOffset::new(0, text.as_bytes()),
                )?;
            }
            super::display_list::DrawKind::Icon { id, rect, color } => {
                compositor.icon(clip, atlas.icon_index(id), copied_rect(rect), packed(color))?;
            }
            super::display_list::DrawKind::SolidTriangles { triangles: range } => {
                let triangles = triangles.get(range.as_range()).ok_or(NC_E_INVALID_ARG)?;
                // A fixed stack chunk bounds staging independently of the recorded range size.
                // GFX merges contiguous material batches, preserving painter order across chunks.
                let mut converted = [SolidVertex { x: 0.0, y: 0.0, rgba: 0 }; 768];
                for chunk in triangles.chunks(256) {
                    let mut count = 0;
                    for triangle in chunk {
                        for vertex in triangle.vertices() {
                            converted[count] = SolidVertex {
                                x: vertex.position.x,
                                y: vertex.position.y,
                                rgba: u32::from_le_bytes([vertex.color.x, vertex.color.y, vertex.color.z, vertex.color.w]),
                            };
                            count += 1;
                        }
                    }
                    compositor.solid_triangles(clip, NcCopyFromBufferOffset::new(0, &converted[..count]))?;
                }
            }
        }
    }
    compositor.submit(renderer)
}

/// Copies the UI math rectangle into the canonical renderer contract value.
fn copied_rect(rect: Recti) -> Rect {
    Rect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

/// Converts one UI patch into its pointer-free GFX operation without expanding any geometry.
fn copied_nine_patch(patch: super::NinePatch, atlas: &AtlasHandle) -> NinePatchDraw {
    // Programmatic UI values historically normalize negative insets at render time. Preserve that
    // public behavior at the type boundary while leaving partitioning and visibility entirely to GFX.
    let insets = copied_insets(patch.insets.normalized());
    let content = match patch.content {
        super::NinePatchContent::Flat { cells } => AssetPatch::Flat {
            top_left: packed_cell(cells.top_left),
            top: packed_cell(cells.top),
            top_right: packed_cell(cells.top_right),
            left: packed_cell(cells.left),
            center: packed_cell(cells.center),
            right: packed_cell(cells.right),
            bottom_left: packed_cell(cells.bottom_left),
            bottom: packed_cell(cells.bottom),
            bottom_right: packed_cell(cells.bottom_right),
        },
        super::NinePatchContent::Image { image } => AssetPatch::Image {
            // Stable UI identities become bundle-local slots only at this GFX boundary. No atlas
            // rectangle or normalized texture coordinate leaks back into semantic display storage.
            icon: atlas.icon_index(image.icon),
            source: copied_insets(image.source_insets.normalized()),
            tint: packed(image.tint),
        },
    };
    NinePatchDraw {
        insets,
        center_visible: u8::from(patch.center_visible),
        patch: content,
    }
}

/// Copies normalized UI insets into the equivalent GFX contract record.
fn copied_insets(insets: crate::SliceInsets) -> Insets {
    // Both modules deliberately use named edges, so this conversion cannot depend on field order.
    Insets {
        left: insets.left,
        top: insets.top,
        right: insets.right,
        bottom: insets.bottom,
    }
}

/// Encodes one closed flat-cell variant in the asset patch's transparent-zero convention.
fn packed_cell(cell: super::NinePatchCell) -> u32 {
    // `NinePatchCell::color` canonicalizes transparent colors to Empty. Encoding Empty as zero also
    // matches authored `AssetPatch::Flat` records and gives GFX one visibility rule for both paths.
    match cell {
        super::NinePatchCell::Empty => 0,
        super::NinePatchCell::Color { color } => packed(color),
    }
}

/// Packs independently addressable channels into the contract's canonical little-endian value.
fn packed(color: Color) -> u32 {
    u32::from_le_bytes([color.r, color.g, color.b, color.a])
}

#[cfg(test)]
mod tests;
