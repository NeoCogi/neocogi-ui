//! Thin production display-list replay into the independently deployed GFX compositor.
//!
//! UI keeps semantic painting and painter order. GFX owns resource expansion, final clipping,
//! batching, stream lifetime, and renderer submission, so those implementations are not linked
//! into the UI runtime shared object.

use super::{Color, DisplayList};
use crate::{AtlasHandle, Recti};
use neocogi_com::{ComPtr, ComRef, NC_E_INVALID_ARG, NcCopyFromBufferOffset, NcStatus};
use neocogi_gfx_contracts::{DrawPoint, GfxCompositorAbi, SolidVertex};
use neocogi_renderer_contracts::{IRendererAbi, Rect, Size};

/// Replays one complete display list and clears it on success or failure.
pub(crate) fn submit(
    list: &mut DisplayList,
    atlas: &AtlasHandle,
    compositor: &mut ComPtr<GfxCompositorAbi>,
    renderer: ComRef<'_, IRendererAbi>,
    size: Size,
    clear: u32,
    solid_scratch: &mut Vec<SolidVertex>,
) -> Result<(), NcStatus> {
    let result = submit_once(list, atlas, compositor, renderer, size, clear, solid_scratch);
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
    solid_scratch: &mut Vec<SolidVertex>,
) -> Result<(), NcStatus> {
    compositor.begin(size, clear)?;
    let super::display_list::DisplayList {
        ops,
        solid_geometry,
        atlas_vertices,
        text,
    } = list;
    let triangles = solid_geometry.triangles();
    for operation in ops.drain(..) {
        let clip = copied_rect(operation.clip);
        match operation.kind {
            super::display_list::DrawKind::AtlasTriangles { vertices } => {
                let vertices = atlas_vertices.get(vertices).ok_or(NC_E_INVALID_ARG)?;
                compositor.atlas_triangles(clip, NcCopyFromBufferOffset::new(0, vertices))?;
            }
            super::display_list::DrawKind::ComTexture { texture, rect, source, color } => {
                compositor.texture(clip, copied_rect(rect), source, packed(color), texture.as_com_ref())?
            }
            super::display_list::DrawKind::ComItem { item, rect } => {
                compositor.item(clip, copied_rect(rect), item.as_com_ref())?;
            }
            super::display_list::DrawKind::NinePatch { rect, patch } => {
                let destinations = patch.geometry(rect);
                match patch.content {
                    super::NinePatchContent::Flat { cells } => {
                        let cells = cells.rows();
                        let source = copied_rect(atlas.get_icon_rect(atlas.white_icon()));
                        for row in 0..3 {
                            for column in 0..3 {
                                if row == 1 && column == 1 && !patch.center_visible {
                                    continue;
                                }
                                let super::NinePatchCell::Color { color } = cells[row][column] else {
                                    continue;
                                };
                                compositor.atlas_rect(clip, copied_rect(destinations[row][column]), source, packed(color))?;
                            }
                        }
                    }
                    super::NinePatchContent::Image { image } => {
                        let source = atlas.get_icon_rect(image.icon);
                        let sources = super::nine_patch::geometry_with_insets(source, image.source_insets);
                        for row in 0..3 {
                            for column in 0..3 {
                                if row == 1 && column == 1 && !patch.center_visible {
                                    continue;
                                }
                                compositor.atlas_rect(
                                    clip,
                                    copied_rect(destinations[row][column]),
                                    copied_rect(sources[row][column]),
                                    packed(image.tint),
                                )?;
                            }
                        }
                    }
                }
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
                solid_scratch.clear();
                solid_scratch.reserve(triangles.len().saturating_mul(3));
                for triangle in triangles {
                    for vertex in triangle.vertices() {
                        solid_scratch.push(SolidVertex {
                            x: vertex.position.x,
                            y: vertex.position.y,
                            rgba: u32::from_le_bytes([vertex.color.x, vertex.color.y, vertex.color.z, vertex.color.w]),
                        });
                    }
                }
                compositor.solid_triangles(clip, NcCopyFromBufferOffset::new(0, solid_scratch))?;
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

/// Packs independently addressable channels into the contract's canonical little-endian value.
fn packed(color: Color) -> u32 {
    u32::from_le_bytes([color.r, color.g, color.b, color.a])
}
