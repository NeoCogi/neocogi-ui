//! Renderer-neutral frame composition owned by the deployable GFX implementation.
//!
//! UI supplies coarse painter-order operations. This module resolves atlas resources, expands
//! text, clips final geometry, retains external capabilities, and exposes one bulk stream to the
//! selected renderer. Backend-native objects remain entirely behind renderer interfaces.

use super::Diagnostic;
use neocogi_com::{
    ComPtr, ComRef, INcStringAbi, NC_E_BUSY, NC_E_INVALID_ARG, NC_E_OUT_OF_MEMORY, NC_E_OUT_OF_RANGE, NcCopyFromBufferOffset, NcOutputWriter, NcStatus,
    com_factory, com_object,
};
use neocogi_gfx_contracts::*;
use neocogi_gfx_support::{AssetTables, AsciiGlyphIndex};
use neocogi_renderer_contracts::{
    IRenderItemAbi, IRenderStream, IRenderStreamAbi, IRenderStreamVtblFor, IRgbaImageAbi, IRendererAbi, ITextureAbi, MAX_RENDER_BATCHES,
    MAX_RENDER_VERTICES, RenderBatch, RenderInfo, RenderVertex,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// One immutable resource projection installed atomically after complete bundle validation.
struct Resources {
    /// Namespace identity rejects unrelated revisions even when their local table slots coincide.
    catalog: ComPtr<IAssetCatalogAbi>,
    /// Validated metadata used for font, icon, and UV lookup.
    tables: AssetTables,
    /// Bounded exact ASCII slots, replaced atomically with their immutable font revision.
    glyph_ascii: Vec<AsciiGlyphIndex>,
    /// Cached authenticated white tile used by solid triangles and flat patch cells.
    white: Rect,
    /// Narrow image projection supplied to the renderer for its already prepared atlas.
    image: ComPtr<IRgbaImageAbi>,
}

impl Resources {
    /// Imports metadata and its image projection without copying the complete atlas pixel buffer.
    fn read(source: ComRef<'_, IAssetBundleAbi>) -> Result<Self, NcStatus> {
        let mut tables = AssetTables::read(source)?;
        // Glyph order is not part of the resource ABI. Sorting each private font partition once
        // makes per-character lookup logarithmic without constructing a second dynamic map.
        for font in tables.fonts.iter() {
            let end = font.glyphs.first.checked_add(font.glyphs.count).ok_or(NC_E_INVALID_ARG)?;
            tables
                .glyphs
                .to_mut()
                .get_mut(font.glyphs.first as usize..end as usize)
                .ok_or(NC_E_INVALID_ARG)?
                .sort_unstable_by_key(|glyph| glyph.scalar);
        }
        // Validation already proved this named tile exists and is opaque. Cache its rectangle once
        // per installed immutable revision instead of scanning icon names for every solid shape.
        let white = tables.white_rect();
        let glyph_ascii = AsciiGlyphIndex::build(&tables)?;
        Ok(Self {
            catalog: source.catalog()?,
            tables,
            glyph_ascii,
            white,
            image: source.image()?,
        })
    }
}

/// One active frame's copied scalar header.
#[derive(Clone, Copy)]
struct FrameState {
    /// Positive drawable dimensions establishing the final viewport.
    size: Size,
    /// Packed clear color forwarded unchanged to the renderer.
    clear: u32,
    /// Submitted work is bounded before scanning, converting, or clipping caller buffers.
    budget: neocogi_renderer_contracts::FrameBudget,
}

/// Nine image/flat cells plus at most nine source-shrink diagnostic overlays, six vertices each.
/// The complete operation stages on the stack and commits through one fallible append.
const MAX_NINE_PATCH_VERTICES: usize = 9 * 2 * 6;

/// Owned painter-order record retained until the synchronous renderer call finishes.
enum Batch {
    /// One contiguous sequence of complete atlas triangles.
    Atlas {
        /// First vertex in the frame's shared geometry storage.
        first: u32,
        /// Positive vertex count divisible by three.
        count: u32,
    },
    /// One clipped external-texture quad and its retained renderer capability.
    Texture {
        /// First of exactly four clockwise vertices.
        first: u32,
        /// Local Rc cloning avoids invoking foreign AddRef while batch storage is borrowed.
        texture: Rc<ComPtr<ITextureAbi>>,
    },
    /// One backend-specific item at an exact painter-order boundary.
    Item {
        /// Complete application content bounds in drawable coordinates.
        rect: Rect,
        /// Positive intersection of content, operation clip, and viewport.
        clip: Rect,
        /// Local Rc cloning avoids invoking foreign AddRef while batch storage is borrowed.
        item: Rc<ComPtr<IRenderItemAbi>>,
    },
}

impl Batch {
    /// Copies pointer-free metadata for the renderer's bulk stream traversal.
    fn record(&self) -> RenderBatch {
        match self {
            Self::Atlas { first, count } => RenderBatch::Atlas { first: *first, count: *count },
            Self::Texture { first, .. } => RenderBatch::Texture { first: *first },
            Self::Item { rect, clip, .. } => RenderBatch::Item { rect: *rect, clip: *clip },
        }
    }
}

/// Reusable frame storage shared only with the independently retainable stream object.
#[derive(Default)]
struct Storage {
    /// Nonzero only while one renderer invocation is authorized to read this storage.
    ///
    /// This is the sole scalar `Cell` in composition: the independently retained `IRenderStream`
    /// receives shared `&self` calls while the compositor must revoke that token immediately after
    /// a foreign renderer returns. All compositor construction and mutation otherwise uses `&mut`.
    active: Cell<u64>,
    /// Final geometry copied in bulk by the selected renderer.
    vertices: RefCell<Vec<RenderVertex>>,
    /// Ordered batches and their retained resource capabilities.
    batches: RefCell<Vec<Batch>>,
}

impl Storage {
    /// Rejects retained or guessed invocation tokens outside the one synchronous renderer call.
    fn check(&self, invocation: u64) -> Result<(), NcStatus> {
        if invocation != 0 && self.active.get() == invocation {
            Ok(())
        } else {
            Err(NC_E_INVALID_ARG)
        }
    }

    /// Releases foreign capabilities without holding a `RefCell` guard during their destructors.
    fn clear(&self) {
        self.vertices.borrow_mut().clear();
        let mut batches = std::mem::take(&mut *self.batches.borrow_mut());
        batches.clear();
        *self.batches.borrow_mut() = batches;
    }
}

#[com_object(interfaces(IRenderStream))]
/// Stable bulk-copy object whose authority is limited by a per-submission invocation token.
struct Stream {
    /// Storage remains valid if a hostile renderer retains this object beyond the call.
    storage: Rc<Storage>,
}

impl IRenderStream for Stream {
    /// Copies a bounded vertex suffix only while the supplied invocation remains active.
    fn vertices(&self, invocation: u64, mut output: NcOutputWriter<'_, RenderVertex>) -> Result<u32, NcStatus> {
        self.storage.check(invocation)?;
        let first = usize::try_from(output.offset()).map_err(|_| NC_E_INVALID_ARG)?;
        let vertices = self.storage.vertices.try_borrow().map_err(|_| NC_E_BUSY)?;
        let source = vertices.get(first..).ok_or(NC_E_INVALID_ARG)?;
        let count = source.len().min(output.capacity());
        output.extend_from_slice(&source[..count])?;
        u32::try_from(count).map_err(|_| NC_E_OUT_OF_RANGE)
    }

    /// Copies pointer-free batch records without invoking retained resources under a borrow.
    fn batches(&self, invocation: u64, mut output: NcOutputWriter<'_, RenderBatch>) -> Result<u32, NcStatus> {
        self.storage.check(invocation)?;
        let first = usize::try_from(output.offset()).map_err(|_| NC_E_INVALID_ARG)?;
        let batches = self.storage.batches.try_borrow().map_err(|_| NC_E_BUSY)?;
        let source = batches.get(first..).ok_or(NC_E_INVALID_ARG)?;
        let count = source.len().min(output.capacity());
        for batch in &source[..count] { output.push(batch.record())?; }
        u32::try_from(count).map_err(|_| NC_E_OUT_OF_RANGE)
    }

    /// Retains the requested texture after releasing the frame-storage borrow.
    fn texture(&self, invocation: u64, batch: u32) -> Result<ComPtr<ITextureAbi>, NcStatus> {
        self.storage.check(invocation)?;
        let texture = match self.storage.batches.try_borrow().map_err(|_| NC_E_BUSY)?.get(batch as usize) {
            Some(Batch::Texture { texture, .. }) => texture.clone(),
            _ => return Err(NC_E_INVALID_ARG),
        };
        Ok(texture.as_ref().clone())
    }

    /// Retains the requested custom item after releasing the frame-storage borrow.
    fn item(&self, invocation: u64, batch: u32) -> Result<ComPtr<IRenderItemAbi>, NcStatus> {
        self.storage.check(invocation)?;
        let item = match self.storage.batches.try_borrow().map_err(|_| NC_E_BUSY)?.get(batch as usize) {
            Some(Batch::Item { item, .. }) => item.clone(),
            _ => return Err(NC_E_INVALID_ARG),
        };
        Ok(item.as_ref().clone())
    }
}

/// Invalidates stream access even when the foreign renderer returns an error or unwinds locally.
struct ReadScope<'a> {
    /// Shared storage whose current invocation ends with this guard.
    storage: &'a Storage,
}

impl Drop for ReadScope<'_> {
    /// Zero is never issued, so it unambiguously denotes an inactive stream.
    fn drop(&mut self) {
        self.storage.active.set(0);
    }
}

/// Axis inspected by one rectangular half-plane clipping edge.
#[derive(Clone, Copy)]
enum Axis {
    /// Compare horizontal vertex coordinates.
    X,
    /// Compare vertical vertex coordinates.
    Y,
}

/// Retained half of one rectangular clipping edge.
#[derive(Clone, Copy)]
enum Side {
    /// Keep coordinates greater than or equal to the boundary.
    Minimum,
    /// Keep coordinates less than or equal to the boundary.
    Maximum,
}

/// One complete clipping edge used by the Sutherland-Hodgman polygon pass.
#[derive(Clone, Copy)]
struct Edge {
    /// Coordinate selected from each vertex.
    axis: Axis,
    /// Which side of the scalar boundary is visible.
    side: Side,
    /// Finite drawable coordinate of this rectangle edge.
    boundary: f32,
}

impl Edge {
    /// Returns the coordinate selected by this edge's axis.
    fn coordinate(self, vertex: RenderVertex) -> f32 {
        match self.axis {
            Axis::X => vertex.x,
            Axis::Y => vertex.y,
        }
    }

    /// Reports whether the vertex lies inside this edge's closed half-plane.
    fn contains(self, vertex: RenderVertex) -> bool {
        match self.side {
            Side::Minimum => self.coordinate(vertex) >= self.boundary,
            Side::Maximum => self.coordinate(vertex) <= self.boundary,
        }
    }

    /// Interpolates the crossing point of one segment and this axis-aligned edge.
    fn intersection(self, from: RenderVertex, to: RenderVertex) -> RenderVertex {
        // Widen before subtracting; accepted finite input may span the complete f32 range.
        let start = f64::from(self.coordinate(from));
        let distance = f64::from(self.coordinate(to)) - start;
        let amount = if distance == 0.0 {
            0.0
        } else {
            (f64::from(self.boundary) - start) / distance
        };
        let mut vertex = RenderVertex::lerp(from, to, amount);
        match self.axis {
            Axis::X => vertex.x = self.boundary,
            Axis::Y => vertex.y = self.boundary,
        }
        vertex
    }
}

#[com_factory(neocogi_gfx_contracts::GfxCompositor)]
/// Concrete GFX-owned compositor with reusable geometry and no backend-native state.
pub struct GfxCompositorObject {
    /// Current validated resource namespace and revision.
    resources: Resources,
    /// Stable stream-visible storage reused between frames.
    storage: Rc<Storage>,
    /// Independently retainable reader passed to each renderer submission.
    stream: ComPtr<IRenderStreamAbi>,
    /// Present only between successful `begin` and `submit`/`cancel` calls.
    frame: Option<FrameState>,
    /// Last issued nonzero invocation, incremented only for actual submissions.
    next_invocation: u64,
    /// First clipping workspace, reused for each input triangle.
    clip_input: Vec<RenderVertex>,
    /// Second clipping workspace, avoiding allocation during edge passes.
    clip_output: Vec<RenderVertex>,
    /// Last human-readable validation or renderer failure.
    diagnostic: Diagnostic,
}

impl GfxCompositorObject {
    /// Imports the initial bundle before publishing a compositor capability.
    pub fn create(source: ComRef<'_, IAssetBundleAbi>) -> Result<ComPtr<GfxCompositorAbi>, NcStatus> {
        let resources = Resources::read(source)?;
        let storage = Rc::new(Storage::default());
        let stream = Stream::from_value(Stream { storage: storage.clone() })
            .query()
            .expect("Stream declares IRenderStream");
        Self::from_value(Self {
            resources,
            storage,
            stream,
            frame: None,
            next_invocation: 0,
            clip_input: Vec::with_capacity(10),
            clip_output: Vec::with_capacity(10),
            diagnostic: Diagnostic::default(),
        })
        .query()
    }

    /// Reserves work before reading caller memory or retaining resources, even for clipped draws.
    fn charge(&mut self, bytes: usize, work: usize) -> Result<(), NcStatus> {
        self.frame.as_mut().ok_or(NC_E_INVALID_ARG)?.budget.charge(bytes, work)
    }

    /// Returns the active frame or records a concrete sequencing failure.
    fn active_frame(&self) -> Result<FrameState, NcStatus> {
        self.frame.ok_or(NC_E_INVALID_ARG)
    }

    /// Computes the positive operation-clip/viewport intersection using wide integer arithmetic.
    fn effective_clip(&self, clip: Rect) -> Result<Option<Rect>, NcStatus> {
        if clip.width < 0 || clip.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        if clip.width == 0 || clip.height == 0 {
            return Ok(None);
        }
        let frame = self.active_frame()?;
        Ok(intersection(
            clip,
            Rect {
                x: 0,
                y: 0,
                width: frame.size.width,
                height: frame.size.height,
            },
        ))
    }

    /// Appends output vertices and either extends or creates their atlas batch.
    fn append_atlas(&mut self, vertices: &[RenderVertex]) -> Result<(), NcStatus> {
        if vertices.is_empty() {
            return Ok(());
        }
        let count = u32::try_from(vertices.len()).map_err(|_| NC_E_OUT_OF_RANGE)?;
        let mut output = self.storage.vertices.borrow_mut();
        let first = u32::try_from(output.len()).map_err(|_| NC_E_OUT_OF_RANGE)?;
        if first.checked_add(count).filter(|end| *end <= MAX_RENDER_VERTICES).is_none() {
            return Err(NC_E_OUT_OF_RANGE);
        }
        let mut batches = self.storage.batches.borrow_mut();
        let can_merge = matches!(batches.last(), Some(Batch::Atlas { .. }));
        if !can_merge && batches.len() >= MAX_RENDER_BATCHES as usize {
            return Err(NC_E_OUT_OF_RANGE);
        }
        // Reserve both independently fallible destinations before changing either one. A failed
        // patch, text run, or primitive therefore cannot leave vertices without a matching batch.
        if !can_merge {
            batches.try_reserve(1).map_err(|_| NC_E_OUT_OF_MEMORY)?;
        }
        output.try_reserve(vertices.len()).map_err(|_| NC_E_OUT_OF_MEMORY)?;
        output.extend_from_slice(vertices);
        if let Some(Batch::Atlas { count: existing, .. }) = batches.last_mut() {
            *existing = existing.checked_add(count).ok_or(NC_E_OUT_OF_RANGE)?;
        } else {
            batches.push(Batch::Atlas { first, count });
        }
        Ok(())
    }

    /// Clips complete triangles and appends the triangulated result to the shared atlas batch.
    fn append_clipped_triangles(&mut self, clip: Rect, vertices: &[RenderVertex]) -> Result<(), NcStatus> {
        let Some(clip) = self.effective_clip(clip)? else {
            return Ok(());
        };
        let edges = [
            Edge {
                axis: Axis::X,
                side: Side::Minimum,
                boundary: clip.x as f32,
            },
            Edge {
                axis: Axis::X,
                side: Side::Maximum,
                boundary: (i64::from(clip.x) + i64::from(clip.width)) as f32,
            },
            Edge {
                axis: Axis::Y,
                side: Side::Minimum,
                boundary: clip.y as f32,
            },
            Edge {
                axis: Axis::Y,
                side: Side::Maximum,
                boundary: (i64::from(clip.y) + i64::from(clip.height)) as f32,
            },
        ];
        for triangle in vertices.as_chunks::<3>().0 {
            // Public entry points have already rejected nonfinite coordinates. Four outside bits
            // use the same closed edges as the polygon clipper, including vertices on a boundary.
            let [a, b, c] = triangle.map(|vertex| {
                u8::from(vertex.x < edges[0].boundary)
                    | (u8::from(vertex.x > edges[1].boundary) << 1)
                    | (u8::from(vertex.y < edges[2].boundary) << 2)
                    | (u8::from(vertex.y > edges[3].boundary) << 3)
            });
            match (a | b | c, a & b & c) {
                // All attributes and winding survive unchanged without visiting scratch buffers.
                (0, _) => {
                    self.append_atlas(triangle)?;
                    continue;
                }
                // No common outside edge: the triangle can intersect or even enclose the clip.
                (_, 0) => {}
                // A common outside half-plane proves the entire triangle is invisible.
                _ => continue,
            }
            self.clip_input.clear();
            self.clip_input.extend_from_slice(triangle);
            for edge in edges {
                self.clip_output.clear();
                clip_polygon(edge, &self.clip_input, &mut self.clip_output);
                std::mem::swap(&mut self.clip_input, &mut self.clip_output);
                if self.clip_input.is_empty() {
                    break;
                }
            }
            if self.clip_input.len() >= 3 {
                // A clipped convex triangle remains convex. A fan preserves winding and emits
                // complete triples without a separate temporary geometry allocation.
                for index in 1..self.clip_input.len() - 1 {
                    let output = [self.clip_input[0], self.clip_input[index], self.clip_input[index + 1]];
                    self.append_atlas(&output)?;
                }
            }
        }
        Ok(())
    }

    /// Produces two clipped triangles after validating one private atlas-cell primitive.
    fn atlas_rect_triangles(&self, destination: Rect, source: Rect, rgba: u32, clip: Rect) -> Result<Option<[RenderVertex; 6]>, NcStatus> {
        if destination.width < 0 || destination.height < 0 || !contains_nonnegative(atlas_bounds(&self.resources.tables), source) {
            return Err(NC_E_INVALID_ARG);
        }
        // Empty structural cells and transparent colors contribute no pixels. Nine-patch expansion
        // normally removes them first, while text and defensive direct helpers share this check.
        if destination.width == 0 || destination.height == 0 || source.width == 0 || source.height == 0 || rgba.to_le_bytes()[3] == 0 {
            return Ok(None);
        }
        let Some(vertices) = clipped_quad(
            destination,
            source.x as f32 / self.resources.tables.size.width as f32,
            source.y as f32 / self.resources.tables.size.height as f32,
            (i64::from(source.x) + i64::from(source.width)) as f32 / self.resources.tables.size.width as f32,
            (i64::from(source.y) + i64::from(source.height)) as f32 / self.resources.tables.size.height as f32,
            rgba,
            clip,
        ) else {
            return Ok(None);
        };
        Ok(Some([vertices[0], vertices[1], vertices[2], vertices[0], vertices[2], vertices[3]]))
    }

    /// Appends one text/icon rectangle within the caller's already resolved viewport intersection.
    fn append_atlas_rect(&mut self, clip: Rect, destination: Rect, source: Rect, rgba: u32) -> Result<(), NcStatus> {
        let Some(vertices) = self.atlas_rect_triangles(destination, source, rgba, clip)? else {
            return Ok(());
        };
        self.append_atlas(&vertices)
    }

    /// Expands, clips, and commits one semantic patch as a single atlas-batch mutation.
    fn append_nine_patch(&mut self, clip: Rect, destination: Rect, draw: NinePatchDraw) -> Result<(), NcStatus> {
        // Validate the complete operation and resolve its current bundle resources before touching
        // frame output. Invalid tags, booleans, insets, or image slots cannot leave partial cells.
        let expanded = crate::nine_patch::expand(destination, draw, self.resources.white, self.resources.tables.icons.as_ref())?;
        let Some(clip) = self.effective_clip(clip)? else {
            return Ok(());
        };
        // A fixed stack buffer stages the whole semantic operation without another allocation or
        // helper abstraction. Only its complete six-vertex cell prefix becomes frame-visible.
        let mut vertices = [RenderVertex::default(); MAX_NINE_PATCH_VERTICES];
        let mut vertex_count = 0;
        for cell in expanded.visible_cells() {
            if let Some(cell_vertices) = self.atlas_rect_triangles(cell.destination, cell.source, cell.rgba, clip)? {
                let next_count = vertex_count + cell_vertices.len();
                vertices[vertex_count..next_count].copy_from_slice(&cell_vertices);
                vertex_count = next_count;
            }
        }
        // One append enforces vertex and batch limits for the complete patch and lets adjacent
        // atlas operations coalesce without exposing individual cells through the ABI.
        self.append_atlas(&vertices[..vertex_count])
    }

    /// Finds one glyph in the sorted private partition, falling back to underscore when absent.
    fn glyph(&self, font_slot: usize, font: AssetFont, scalar: u32) -> Result<AssetGlyph, NcStatus> {
        if scalar < 128 {
            let index = &self.resources.glyph_ascii[font_slot];
            let slot = index.get(scalar).or_else(|| index.get(u32::from('_'))).ok_or(NC_E_INVALID_ARG)?;
            return Ok(self.resources.tables.glyphs[slot]);
        }
        let end = font.glyphs.first.checked_add(font.glyphs.count).ok_or(NC_E_INVALID_ARG)?;
        let glyphs = self
            .resources
            .tables
            .glyphs
            .get(font.glyphs.first as usize..end as usize)
            .ok_or(NC_E_INVALID_ARG)?;
        let index = glyphs
            .binary_search_by_key(&scalar, |glyph| glyph.scalar)
            .or_else(|_| glyphs.binary_search_by_key(&u32::from('_'), |glyph| glyph.scalar))
            .map_err(|_| NC_E_INVALID_ARG)?;
        Ok(glyphs[index])
    }
}

impl GfxCompositor for GfxCompositorObject {
    /// Replaces resource metadata only with a validated revision from the original namespace.
    fn install_assets(&mut self, assets: ComRef<'_, IAssetBundleAbi>) -> Result<(), NcStatus> {
        if self.frame.is_some() {
            return Err(NC_E_BUSY);
        }
        let resources = Resources::read(assets).inspect_err(|status| {
            self.diagnostic.failure(*status);
        })?;
        if resources.catalog.as_raw().object != self.resources.catalog.as_raw().object {
            return Err(self.diagnostic.failure("asset revision belongs to another catalog"));
        }
        self.resources = resources;
        Ok(())
    }

    /// Starts an empty frame after checking dimensions and sequencing.
    fn begin(&mut self, size: Size, clear: u32) -> Result<(), NcStatus> {
        if self.frame.is_some() {
            return Err(NC_E_BUSY);
        }
        if size.width <= 0 || size.height <= 0 {
            return Err(self.diagnostic.failure("drawable dimensions must be positive"));
        }
        self.storage.clear();
        self.frame = Some(FrameState {
            size,
            clear,
            budget: neocogi_renderer_contracts::FrameBudget::new(),
        });
        Ok(())
    }

    /// Maps solid vertices to the authenticated white tile and clips complete triangles.
    fn solid_triangles(&mut self, clip: Rect, vertices: NcCopyFromBufferOffset<'_, SolidVertex>) -> Result<(), NcStatus> {
        if vertices.offset() != 0 || !vertices.len().is_multiple_of(3) {
            return Err(NC_E_INVALID_ARG);
        }
        self.charge(core::mem::size_of_val(vertices.as_slice()), vertices.len())?;
        if vertices.as_slice().iter().any(|vertex| !vertex.x.is_finite() || !vertex.y.is_finite()) {
            return Err(NC_E_INVALID_ARG);
        }
        let white = self.resources.white;
        let u = (white.x as f32 + white.width as f32 * 0.5) / self.resources.tables.size.width as f32;
        let v = (white.y as f32 + white.height as f32 * 0.5) / self.resources.tables.size.height as f32;
        // Preserve clip validation for empty buffers before entering the per-triangle loop.
        self.effective_clip(clip)?;
        // Convert one triangle on the stack: no full-input scratch allocation survives a frame.
        for triangle in vertices.as_slice().as_chunks::<3>().0 {
            let converted = triangle.map(|vertex| RenderVertex {
                x: vertex.x,
                y: vertex.y,
                u,
                v,
                rgba: vertex.rgba,
            });
            self.append_clipped_triangles(clip, &converted)?;
        }
        Ok(())
    }

    /// Clips caller-normalized atlas triangles and preserves every interpolated attribute.
    fn atlas_triangles(&mut self, clip: Rect, vertices: NcCopyFromBufferOffset<'_, RenderVertex>) -> Result<(), NcStatus> {
        self.charge(core::mem::size_of_val(vertices.as_slice()), vertices.len())?;
        if vertices.offset() != 0
            || !vertices.len().is_multiple_of(3)
            || vertices.as_slice().iter().any(|vertex| {
                ![vertex.x, vertex.y, vertex.u, vertex.v].iter().all(|value| value.is_finite())
                    || !(0.0..=1.0).contains(&vertex.u)
                    || !(0.0..=1.0).contains(&vertex.v)
            })
        {
            return Err(NC_E_INVALID_ARG);
        }
        self.append_clipped_triangles(clip, vertices.as_slice())
    }

    /// Resolves and appends one complete typed patch through current bundle resources.
    fn nine_patch(&mut self, clip: Rect, destination: Rect, draw: NinePatchDraw) -> Result<(), NcStatus> {
        // Reserve the bounded worst case, including source diagnostics, in the same single
        // operation admission. Even fully clipped patches cannot bypass work accounting.
        self.charge(0, MAX_NINE_PATCH_VERTICES)?;
        self.append_nine_patch(clip, destination, draw)
    }

    /// Expands UTF-8 with bundle-local metrics, CRLF normalization, and underscore fallback.
    fn text(&mut self, clip: Rect, font: u32, origin: DrawPoint, rgba: u32, text: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.charge(text.len(), text.len())?;
        if text.offset() != 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let text = std::str::from_utf8(text.as_slice()).map_err(|_| NC_E_INVALID_ARG)?;
        let font_slot = font as usize;
        let font = *self.resources.tables.fonts.get(font_slot).ok_or(NC_E_OUT_OF_RANGE)?;
        // Every glyph shares one frame/operation intersection. Retain its Result until the first
        // drawable glyph so empty and zero-area text keep their existing clip-validation behavior.
        let clip = self.effective_clip(clip);
        let mut pen_x = 0_i128;
        let mut baseline_y = i128::from(font.baseline);
        let mut characters = text.chars().peekable();
        while let Some(character) = characters.next() {
            if character == '\n' || character == '\r' {
                if character == '\r' && characters.peek() == Some(&'\n') {
                    characters.next();
                }
                pen_x = 0;
                baseline_y += i128::from(font.line_size);
                continue;
            }
            let glyph = self.glyph(font_slot, font, u32::from(character))?;
            let destination = Rect {
                x: clamp_i128(i128::from(origin.x) + pen_x + i128::from(glyph.offset_x)),
                y: clamp_i128(i128::from(origin.y) + baseline_y - i128::from(glyph.offset_y) - i128::from(glyph.rect.height)),
                width: glyph.rect.width,
                height: glyph.rect.height,
            };
            // Valid font tables may represent whitespace with a zero-area source. It advances the
            // pen but contributes no geometry and therefore never enters positive-rect validation.
            if glyph.rect.width > 0
                && glyph.rect.height > 0
                && let Some(clip) = clip?
            {
                self.append_atlas_rect(clip, destination, glyph.rect, rgba)?;
            }
            pen_x += i128::from(glyph.advance_x);
        }
        Ok(())
    }

    /// Centers one bundle-local atlas icon using wide coordinate arithmetic before clipping.
    fn icon(&mut self, clip: Rect, icon: u32, destination: Rect, rgba: u32) -> Result<(), NcStatus> {
        self.charge(0, 6)?;
        if destination.width < 0 || destination.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let source = self.resources.tables.icons.get(icon as usize).ok_or(NC_E_OUT_OF_RANGE)?.rect;
        let centered = Rect {
            x: clamp_i64(i64::from(destination.x) + (i64::from(destination.width) - i64::from(source.width)) / 2),
            y: clamp_i64(i64::from(destination.y) + (i64::from(destination.height) - i64::from(source.height)) / 2),
            width: source.width,
            height: source.height,
        };
        // Icons resolve the same viewport intersection once before entering the private helper.
        match self.effective_clip(clip)? {
            Some(clip) => self.append_atlas_rect(clip, centered, source, rgba),
            None => Ok(()),
        }
    }

    /// Appends one complete clipped texture quad after both CPU buffers admit capacity.
    /// Reservation failure leaves earlier geometry/batches intact and releases the temporary owner.
    fn texture(&mut self, clip: Rect, destination: Rect, source: TextureRegion, rgba: u32, texture: ComRef<'_, ITextureAbi>) -> Result<(), NcStatus> {
        self.charge(0, 4)?;
        source.validate()?;
        if destination.width < 0 || destination.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        if destination.width == 0 || destination.height == 0 || rgba.to_le_bytes()[3] == 0 {
            return Ok(());
        }
        let Some(clip) = self.effective_clip(clip)? else {
            return Ok(());
        };
        let Some(vertices) = clipped_quad(destination, source.u0, source.v0, source.u1, source.v1, rgba, clip) else {
            return Ok(());
        };
        let mut output = self.storage.vertices.borrow_mut();
        let first = u32::try_from(output.len()).map_err(|_| NC_E_OUT_OF_RANGE)?;
        if first.checked_add(4).filter(|end| *end <= MAX_RENDER_VERTICES).is_none() || self.storage.batches.borrow().len() >= MAX_RENDER_BATCHES as usize {
            return Err(NC_E_OUT_OF_RANGE);
        }
        let retained = Rc::new(texture.add_ref());
        output.try_reserve(4).map_err(|_| NC_E_OUT_OF_MEMORY)?;
        let mut batches = self.storage.batches.borrow_mut();
        batches.try_reserve(1).map_err(|_| NC_E_OUT_OF_MEMORY)?;
        // Capacity growth may survive a later reservation error, but recorded lengths may not.
        // Commit the complete quad and its owner together, after both fallible reservations.
        output.extend_from_slice(&vertices);
        batches.push(Batch::Texture { first, texture: retained });
        Ok(())
    }

    /// Retains one renderer-specific item with its final positive visible rectangle.
    fn item(&mut self, clip: Rect, content: Rect, item: ComRef<'_, IRenderItemAbi>) -> Result<(), NcStatus> {
        self.charge(0, 0)?;
        if content.width <= 0 || content.height <= 0 {
            return if content.width == 0 || content.height == 0 {
                Ok(())
            } else {
                Err(NC_E_INVALID_ARG)
            };
        }
        let Some(clip) = self.effective_clip(clip)?.and_then(|clip| intersection(clip, content)) else {
            return Ok(());
        };
        if self.storage.batches.borrow().len() >= MAX_RENDER_BATCHES as usize {
            return Err(NC_E_OUT_OF_RANGE);
        }
        let retained = Rc::new(item.add_ref());
        let mut batches = self.storage.batches.borrow_mut();
        batches.try_reserve(1).map_err(|_| NC_E_OUT_OF_MEMORY)?;
        batches.push(Batch::Item { rect: content, clip, item: retained });
        Ok(())
    }

    /// Performs the frame's only renderer call and expires all retained resource capabilities.
    fn submit(&mut self, renderer: ComRef<'_, IRendererAbi>) -> Result<(), NcStatus> {
        let result = (|| {
            let frame = self.frame.take().ok_or(NC_E_INVALID_ARG)?;
            let invocation = self.next_invocation.checked_add(1).ok_or(NC_E_OUT_OF_RANGE)?;
            self.next_invocation = invocation;
            let vertex_count = u32::try_from(self.storage.vertices.borrow().len()).map_err(|_| NC_E_OUT_OF_RANGE)?;
            let batch_count = u32::try_from(self.storage.batches.borrow().len()).map_err(|_| NC_E_OUT_OF_RANGE)?;
            self.storage.active.set(invocation);
            let _scope = ReadScope { storage: &self.storage };
            renderer.render(
                RenderInfo {
                    size: frame.size,
                    clear: frame.clear,
                    invocation,
                    vertex_count,
                    batch_count,
                },
                self.resources.image.as_com_ref(),
                self.stream.as_com_ref(),
            )
        })();
        // Submit owns cleanup even when validation fails before entering the renderer. Callers do
        // not need a compensating cancellation merely to release retained textures or items.
        self.storage.clear();
        result.inspect_err(|status| {
            self.diagnostic.failure(*status);
        })
    }

    /// Abandons a frame and releases its retained capabilities while preserving allocations.
    fn cancel(&mut self) -> Result<(), NcStatus> {
        self.frame = None;
        self.storage.clear();
        Ok(())
    }
}

impl IGfxDiagnostics for GfxCompositorObject {
    /// Returns an owned snapshot unaffected by later composition attempts.
    fn diagnostic(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        Ok(self.diagnostic.snapshot())
    }
}

/// Applies one clipping edge to a convex polygon without allocating.
fn clip_polygon(edge: Edge, input: &[RenderVertex], output: &mut Vec<RenderVertex>) {
    let Some(mut previous) = input.last().copied() else {
        return;
    };
    let mut previous_inside = edge.contains(previous);
    for current in input.iter().copied() {
        let current_inside = edge.contains(current);
        if current_inside != previous_inside {
            output.push(edge.intersection(previous, current));
        }
        if current_inside {
            output.push(current);
        }
        previous = current;
        previous_inside = current_inside;
    }
}

/// Returns the atlas's positive full pixel bounds.
fn atlas_bounds(tables: &AssetTables) -> Rect {
    Rect {
        x: 0,
        y: 0,
        width: tables.size.width,
        height: tables.size.height,
    }
}

/// Reports whether one nonnegative rectangle lies wholly inside positive bounds.
fn contains_nonnegative(outer: Rect, inner: Rect) -> bool {
    // Empty source cells remain bounded data: their origin and nonempty axis must still lie inside
    // the atlas. Wide edge arithmetic rejects overflow without excluding a zero-area no-op.
    outer.width > 0
        && outer.height > 0
        && inner.width >= 0
        && inner.height >= 0
        && inner.x >= outer.x
        && inner.y >= outer.y
        && i64::from(inner.x) + i64::from(inner.width) <= i64::from(outer.x) + i64::from(outer.width)
        && i64::from(inner.y) + i64::from(inner.height) <= i64::from(outer.y) + i64::from(outer.height)
}

/// Computes a positive rectangle intersection without overflowing signed coordinates.
fn intersection(left: Rect, right: Rect) -> Option<Rect> {
    if left.width <= 0 || left.height <= 0 || right.width <= 0 || right.height <= 0 {
        return None;
    }
    let x0 = i64::from(left.x).max(i64::from(right.x));
    let y0 = i64::from(left.y).max(i64::from(right.y));
    let x1 = (i64::from(left.x) + i64::from(left.width)).min(i64::from(right.x) + i64::from(right.width));
    let y1 = (i64::from(left.y) + i64::from(left.height)).min(i64::from(right.y) + i64::from(right.height));
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some(Rect {
        x: clamp_i64(x0),
        y: clamp_i64(y0),
        width: clamp_i64(x1 - x0),
        height: clamp_i64(y1 - y0),
    })
}

/// Clips one destination rectangle and interpolates its normalized source edges.
fn clipped_quad(destination: Rect, u0: f32, v0: f32, u1: f32, v1: f32, rgba: u32, clip: Rect) -> Option<[RenderVertex; 4]> {
    let visible = intersection(destination, clip)?;
    let (clipped_u0, clipped_v0, clipped_u1, clipped_v1) =
        if visible.x == destination.x && visible.y == destination.y && visible.width == destination.width && visible.height == destination.height {
            // Fully visible cells keep their original UV endpoints without four redundant divisions.
            (u0, v0, u1, v1)
        } else {
            // Only a genuinely clipped cell needs interpolation. Wide differences preserve the
            // existing behavior for rectangles whose far edges exceed the signed coordinate range.
            let x_offset0 = (i64::from(visible.x) - i64::from(destination.x)) as f32;
            let y_offset0 = (i64::from(visible.y) - i64::from(destination.y)) as f32;
            let x_offset1 = (i64::from(visible.x) + i64::from(visible.width) - i64::from(destination.x)) as f32;
            let y_offset1 = (i64::from(visible.y) + i64::from(visible.height) - i64::from(destination.y)) as f32;
            let x_extent = destination.width as f32;
            let y_extent = destination.height as f32;
            (
                u0 + (x_offset0 / x_extent) * (u1 - u0),
                v0 + (y_offset0 / y_extent) * (v1 - v0),
                u0 + (x_offset1 / x_extent) * (u1 - u0),
                v0 + (y_offset1 / y_extent) * (v1 - v0),
            )
        };
    let x0 = visible.x as f32;
    let y0 = visible.y as f32;
    let x1 = (i64::from(visible.x) + i64::from(visible.width)) as f32;
    let y1 = (i64::from(visible.y) + i64::from(visible.height)) as f32;
    Some([
        RenderVertex {
            x: x0,
            y: y0,
            u: clipped_u0,
            v: clipped_v0,
            rgba,
        },
        RenderVertex {
            x: x1,
            y: y0,
            u: clipped_u1,
            v: clipped_v0,
            rgba,
        },
        RenderVertex {
            x: x1,
            y: y1,
            u: clipped_u1,
            v: clipped_v1,
            rgba,
        },
        RenderVertex {
            x: x0,
            y: y1,
            u: clipped_u0,
            v: clipped_v1,
            rgba,
        },
    ])
}

/// Saturates wide text arithmetic only at the renderer-facing coordinate boundary.
fn clamp_i128(value: i128) -> i32 {
    value.clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32
}

/// Saturates wide rectangle arithmetic only at the renderer-facing coordinate boundary.
fn clamp_i64(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Full, exact, partial and empty intersections retain consistent positions and texture edges.
    #[test]
    fn rectangle_clipping_preserves_uvs_and_interpolates_only_visible_edges() {
        let destination = Rect { x: -8, y: -4, width: 16, height: 8 };
        // Binary fractions make these independently specified UV expectations exact in f32.
        for (clip, bounds, uv) in [
            (Rect { x: -16, y: -8, width: 32, height: 16 }, [-8., -4., 8., 4.], [0.25, 0.375, 0.75, 0.625]),
            (destination, [-8., -4., 8., 4.], [0.25, 0.375, 0.75, 0.625]),
            (Rect { x: 0, y: -4, width: 8, height: 8 }, [0., -4., 8., 4.], [0.5, 0.375, 0.75, 0.625]),
            (Rect { x: -4, y: -2, width: 8, height: 4 }, [-4., -2., 4., 2.], [0.375, 0.4375, 0.625, 0.5625]),
        ] {
            let actual = clipped_quad(destination, 0.25, 0.375, 0.75, 0.625, 0xff123456, clip).unwrap();
            let [x0, y0, x1, y1] = bounds;
            let [u0, v0, u1, v1] = uv;
            assert_eq!(
                actual.map(|vertex| (vertex.x, vertex.y, vertex.u, vertex.v, vertex.rgba)),
                [
                    (x0, y0, u0, v0, 0xff123456),
                    (x1, y0, u1, v0, 0xff123456),
                    (x1, y1, u1, v1, 0xff123456),
                    (x0, y1, u0, v1, 0xff123456)
                ]
            );
        }
        // Touching an edge contributes no area; neither does a fully disjoint rectangle.
        for x in [8, 20] {
            assert!(clipped_quad(destination, 0.25, 0.375, 0.75, 0.625, 0xff123456, Rect { x, y: 0, width: 4, height: 4 }).is_none());
        }
    }
}

/// Regressions run whenever this production module is available.
#[cfg(test)]
#[path = "compositor/tests.rs"]
mod stream_tests;
