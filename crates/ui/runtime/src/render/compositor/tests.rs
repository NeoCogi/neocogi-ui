//! Production replay regressions inspect the actual GFX stream, without a second executor.

use super::*;
use crate::{color, SliceInsets, Vec2i};
use neocogi_com::{com_object, NcCopyToBufferOffset};
use neocogi_renderer_contracts::*;
use std::{cell::RefCell, rc::Rc};

/// Copied observations outlive a render call without keeping its stream authority active.
#[derive(Default)]
struct Capture {
    /// Successfully entered renderer calls, including deliberately rejected submissions.
    calls: usize,
    /// Exact vertices supplied by the production compositor in the most recent call.
    vertices: Vec<RenderVertex>,
    /// Decoded commands retain painter order and their native contract distinctions.
    batches: Vec<RenderBatch>,
    /// Stream and token are retained only to assert expiration after the foreign call returns.
    stream: Option<(ComPtr<IRenderStreamAbi>, u64)>,
    /// A test can reject the next submission without replacing composition behavior.
    fail: bool,
}

#[com_object(interfaces(IRenderer))]
/// Headless renderer observes completed geometry; GFX performs all expansion and clipping.
struct Recorder {
    /// IRenderer methods take shared access, so the callback and assertion owner need one cell.
    capture: Rc<RefCell<Capture>>,
}
impl IRenderer for Recorder {
    /// Logical image preparation requires no GPU upload in a stream-consumer fixture.
    fn prepare_atlas(&self, _: ComRef<'_, IRgbaImageAbi>) -> Result<(), NcStatus> {
        Ok(())
    }

    /// Copies the live stream before borrowing assertion storage, keeping foreign calls reentrant.
    fn render(&self, info: RenderInfo, _: ComRef<'_, IRgbaImageAbi>, stream: ComRef<'_, IRenderStreamAbi>) -> Result<(), NcStatus> {
        let mut vertices = vec![RenderVertex::default(); info.vertex_count as usize];
        let mut records = vec![RenderBatch::Atlas { first: 0, count: 0 }.into(); info.batch_count as usize];
        assert_eq!(
            stream.vertices(info.invocation, NcCopyToBufferOffset::new(0, &mut vertices))?,
            info.vertex_count
        );
        assert_eq!(stream.batches(info.invocation, NcCopyToBufferOffset::new(0, &mut records))?, info.batch_count);
        let batches = records.into_iter().map(RenderBatch::try_from).collect::<Result<Vec<_>, _>>()?;
        // Resolve every resource during the active scope. No resource-type registry or erased
        // application payload participates in this renderer fixture.
        for (index, batch) in batches.iter().enumerate() {
            match batch {
                RenderBatch::Texture { .. } => {
                    stream.texture(info.invocation, index as u32)?;
                }
                RenderBatch::Item { .. } => {
                    stream.item(info.invocation, index as u32)?;
                }
                RenderBatch::Atlas { .. } => {}
            }
        }
        let mut capture = self.capture.borrow_mut();
        capture.calls += 1;
        capture.vertices = vertices;
        capture.batches = batches;
        capture.stream = Some((stream.add_ref(), info.invocation));
        if capture.fail { Err(NC_E_INVALID_ARG) } else { Ok(()) }
    }
}

#[com_object(interfaces(ITexture))]
/// Immutable texture capability used to verify resource ordering independently of a GPU device.
struct Texture;
impl ITexture for Texture {
    /// A single texel makes fractional destination clipping easy to check exactly.
    fn size(&self) -> Result<Size, NcStatus> {
        Ok(Size { width: 1, height: 1 })
    }
}

#[com_object(interfaces(IRenderItem))]
/// Opaque application work remains an owned capability in the real command stream.
struct Item;
impl IRenderItem for Item {}

/// Mutable fixture owns the production replay inputs and one independent GFX compositor.
struct Composition {
    /// Local font/icon IDs match the exact source imported by GFX.
    atlas: AtlasHandle,
    /// Actual GFX implementation, used through the same contract as deployed UI.
    compositor: ComPtr<GfxCompositorAbi>,
    /// Headless consumer receives only final renderer-neutral batches.
    renderer: ComPtr<IRendererAbi>,
    /// Shared callback observations; other fixture state uses ordinary mutable borrows.
    capture: Rc<RefCell<Capture>>,
    /// Semantic operations are recorded by the production Painter.
    list: DisplayList,
}
impl Composition {
    /// Publishes one validated fixture bundle and binds both sides to the same resource tables.
    fn new(atlas: AtlasHandle) -> Self {
        let source = crate::test_support::assets::export_bundle(&crate::SkinBundle::from_atlas(atlas.clone())).unwrap();
        let compositor = neocogi_gfx_impl::GfxCompositorObject::create(source.as_com_ref()).unwrap();
        let capture = Rc::new(RefCell::new(Capture::default()));
        let renderer = Recorder::from_value(Recorder { capture: capture.clone() }).query().unwrap();
        Self {
            atlas,
            compositor,
            renderer,
            capture,
            list: DisplayList::new(),
        }
    }

    /// Calls the same replay function as RuntimeContext; no geometry is computed in the fixture.
    fn submit(&mut self, size: Size) -> Result<(), NcStatus> {
        super::submit(&mut self.list, &self.atlas, &mut self.compositor, self.renderer.as_com_ref(), size, 0xff000000)
    }
}

/// Asserts position and UV coordinates independently, allowing only floating-point rounding noise.
fn vertex(actual: RenderVertex, position: [f32; 2], uv: [f32; 2]) {
    for (actual, expected) in [actual.x, actual.y, actual.u, actual.v]
        .into_iter()
        .zip([position[0], position[1], uv[0], uv[1]])
    {
        assert!((actual - expected).abs() < 1.0e-6, "{actual} != {expected}");
    }
}

/// Builds deterministic glyph geometry retained from the old executor's numerical regressions.
fn atlas() -> AtlasHandle {
    let mut data = crate::test_support::test_asset_data_with_font_sizes(&[("body", 4)]);
    data.tables.size = Size { width: 8, height: 8 };
    data.tables.fonts.to_mut()[0].baseline = 4;
    data.pixels = vec![255; 8 * 8 * 4].into();
    let white_slots = data
        .tables
        .icons
        .iter()
        .map(|icon| data.tables.name(icon.name).unwrap() == "white")
        .collect::<Vec<_>>();
    for (icon, is_white) in data.tables.icons.to_mut().iter_mut().zip(white_slots) {
        icon.rect = if is_white {
            neocogi_ui_core_contracts::Rect { x: 0, y: 0, width: 1, height: 1 }
        } else {
            neocogi_ui_core_contracts::Rect { x: 4, y: 0, width: 4, height: 4 }
        };
    }
    for glyph in data.tables.glyphs.to_mut() {
        glyph.advance_x = 4;
        glyph.rect = match char::from_u32(glyph.scalar) {
            Some('_') => neocogi_ui_core_contracts::Rect { x: 4, y: 4, width: 4, height: 4 },
            Some(_) => neocogi_ui_core_contracts::Rect { x: 0, y: 4, width: 4, height: 4 },
            None => unreachable!("the canonical fixture contains only valid scalars"),
        };
    }
    AtlasHandle::from_validated_data(data, None).unwrap()
}

/// Semantic primitives share one GFX frame and a coalesced atlas batch.
#[test]
fn semantic_atlas_operations_use_the_cached_atlas_and_one_executor() {
    let mut fixture = Composition::new(atlas());
    let font = fixture.atlas.font_id("body").unwrap();
    let icon = fixture.atlas.white_icon();
    let mut painter = crate::render::Painter::screen_space(&mut fixture.list, Recti::new(0, 0, 32, 32));
    painter.fill_rect(Recti::new(0, 0, 1, 1), color(255, 255, 255, 255)).unwrap();
    painter.text(font, "aa", Vec2i::default(), color(255, 255, 255, 255)).unwrap();
    painter.icon(icon, Recti::new(0, 0, 3, 3), color(255, 255, 255, 255)).unwrap();
    fixture.submit(Size { width: 32, height: 32 }).unwrap();
    let captured = fixture.capture.borrow();
    assert_eq!(captured.calls, 1);
    assert_eq!(captured.vertices.len(), 24);
    assert!(matches!(captured.batches.as_slice(), [RenderBatch::Atlas { first: 0, count: 24 }]));
    assert!(fixture.list.is_empty());
}

/// Missing glyphs and both newline encodings use the same production placement rules.
#[test]
fn streamed_glyphs_preserve_order_fallback_and_newline_positioning() {
    for text in ["a?\na", "a?\r\na"] {
        let mut fixture = Composition::new(atlas());
        let font = fixture.atlas.font_id("body").unwrap();
        crate::render::Painter::screen_space(&mut fixture.list, Recti::new(0, 0, 32, 32)).text(font, text, Vec2i::new(10, 10), color(255, 255, 255, 255)).unwrap();
        fixture.submit(Size { width: 32, height: 32 }).unwrap();
        let capture = fixture.capture.borrow();
        assert_eq!(capture.vertices.len(), 18);
        vertex(capture.vertices[0], [10., 10.], [0., 0.5]);
        vertex(capture.vertices[2], [14., 14.], [0.5, 1.]);
        vertex(capture.vertices[6], [14., 10.], [0.5, 0.5]);
        vertex(capture.vertices[12], [10., 14.], [0., 0.5]);
    }
}

/// Atlas rectangles retain fractional source coordinates when their destinations are clipped.
#[test]
fn atlas_rectangle_clipping_preserves_fractional_uvs() {
    let mut fixture = Composition::new(atlas());
    crate::render::Painter::screen_space(&mut fixture.list, Recti::new(20, 20, 40, 40)).fill_rect(Recti::new(0, 0, 100, 100), color(255, 255, 255, 255)).unwrap();
    fixture.submit(Size { width: 100, height: 100 }).unwrap();
    let capture = fixture.capture.borrow();
    assert_eq!(capture.vertices.len(), 6);
    vertex(capture.vertices[0], [20., 20.], [0.025, 0.025]);
    vertex(capture.vertices[2], [60., 60.], [0.075, 0.075]);
}

/// External texture interpolation and custom work preserve their place between ordinary batches.
#[test]
fn external_texture_clipping_preserves_uv_mapping_and_stream_order() {
    let mut fixture = Composition::new(atlas());
    let texture: ComPtr<ITextureAbi> = Texture::from_value(Texture).query().unwrap();
    let item: ComPtr<IRenderItemAbi> = Item::from_value(Item).query().unwrap();
    let rect = Recti::new(0, 0, 100, 100);
    fixture
        .list
        .push_icon(rect, fixture.atlas.white_icon(), Recti::new(0, 0, 1, 1), color(255, 255, 255, 255)).unwrap();
    fixture.list.push_com_texture(
        Recti::new(20, 20, 40, 40),
        rect,
        TextureRegion { u0: 0., v0: 0., u1: 1., v1: 1. },
        color(255, 255, 255, 255),
        texture,
    ).unwrap();
    fixture.list.push_com_item(Recti::new(10, 10, 20, 20), rect, item).unwrap();
    fixture
        .list
        .push_icon(rect, fixture.atlas.white_icon(), Recti::new(20, 0, 1, 1), color(255, 255, 255, 255)).unwrap();
    fixture.submit(Size { width: 100, height: 100 }).unwrap();
    let capture = fixture.capture.borrow();
    assert!(matches!(
        capture.batches.as_slice(),
        [
            RenderBatch::Atlas { .. },
            RenderBatch::Texture { first: 6 },
            RenderBatch::Item { .. },
            RenderBatch::Atlas { .. }
        ]
    ));
    vertex(capture.vertices[6], [20., 20.], [0.2, 0.2]);
    vertex(capture.vertices[8], [60., 60.], [0.6, 0.6]);
    match capture.batches[2] {
        RenderBatch::Item { rect, clip } => {
            assert_eq!(rect.width, 100);
            assert_eq!((clip.x, clip.y, clip.width, clip.height), (10, 10, 20, 20));
        }
        _ => unreachable!("batch kind was checked"),
    }
}

/// Nine-patch source slicing is observed through GFX's actual final triangles.
#[test]
fn image_nine_patch_submits_nine_sliced_atlas_quads_without_a_flush() {
    let mut fixture = Composition::new(atlas());
    let image = crate::NinePatchImage::new(fixture.atlas.icon_id("close").unwrap(), SliceInsets::uniform(1), color(255, 255, 255, 255));
    let patch = crate::NinePatch::image(SliceInsets::new(4, 3, 5, 4), image);
    crate::render::Painter::screen_space(&mut fixture.list, Recti::new(0, 0, 64, 64)).nine_patch(Recti::new(0, 0, 30, 24), patch).unwrap();
    fixture.submit(Size { width: 64, height: 64 }).unwrap();
    let capture = fixture.capture.borrow();
    assert_eq!(capture.vertices.len(), 54);
    assert_eq!(capture.batches.len(), 1);
    vertex(capture.vertices[0], [0., 0.], [0.5, 0.]);
    vertex(capture.vertices[2], [4., 3.], [0.625, 0.125]);
    vertex(capture.vertices[24], [4., 3.], [0.625, 0.125]);
    vertex(capture.vertices[26], [25., 20.], [0.875, 0.375]);
    vertex(capture.vertices[48], [25., 20.], [0.875, 0.375]);
    vertex(capture.vertices[50], [30., 24.], [1., 0.5]);
}

/// Border-only policy is enforced by GFX after UI retains an otherwise complete flat patch.
#[test]
fn flat_nine_patch_filters_its_hidden_center_during_gfx_expansion() {
    let mut fixture = Composition::new(atlas());
    let patch = crate::NinePatch::framed(SliceInsets::uniform(2), color(10, 20, 30, 255), Some(color(40, 50, 60, 255))).without_center();
    crate::render::Painter::screen_space(&mut fixture.list, Recti::new(0, 0, 32, 32)).nine_patch(Recti::new(4, 5, 20, 18), patch).unwrap();

    fixture.submit(Size { width: 32, height: 32 }).unwrap();

    let capture = fixture.capture.borrow();
    assert_eq!(capture.vertices.len(), 48, "eight visible border cells must produce eight quads");
    assert!(matches!(capture.batches.as_slice(), [RenderBatch::Atlas { first: 0, count: 48 }]));
}

/// Rejected renderer calls expire their stream and clear commands before the next frame.
#[test]
fn frame_finish_failure_is_observable_and_the_next_frame_can_complete() {
    let mut fixture = Composition::new(atlas());
    fixture.capture.borrow_mut().fail = true;
    assert_eq!(fixture.submit(Size { width: 32, height: 32 }), Err(NC_E_INVALID_ARG));
    assert!(fixture.list.is_empty());
    let (stream, invocation) = fixture.capture.borrow().stream.clone().unwrap();
    assert!(stream.vertices(invocation, NcCopyToBufferOffset::new(0, &mut [])).is_err());
    fixture.capture.borrow_mut().fail = false;
    fixture.submit(Size { width: 32, height: 32 }).unwrap();
    assert_eq!(fixture.capture.borrow().calls, 2);
}

/// Failed frame creation clears owned operations and allows a later valid replay.
#[test]
fn invalid_frame_dimensions_discard_commands_before_renderer_submission() {
    let mut fixture = Composition::new(atlas());
    for size in [Size { width: 0, height: 32 }, Size { width: 32, height: -1 }] {
        fixture.list.push_icon(
            Recti::new(0, 0, 32, 32),
            fixture.atlas.white_icon(),
            Recti::new(0, 0, 1, 1),
            color(255, 255, 255, 255),
        ).unwrap();
        assert!(fixture.submit(size).is_err());
        assert!(fixture.list.is_empty());
    }
    assert_eq!(fixture.capture.borrow().calls, 0);
    fixture.submit(Size { width: 32, height: 32 }).unwrap();
}

/// Intersections interpolate both source coordinates and color in the deployed triangle clipper.
#[test]
fn clipped_atlas_triangles_interpolate_edge_attributes() {
    let mut fixture = Composition::new(atlas());
    fixture.list.push_atlas_triangles(
        Recti::new(0, 0, 10, 10),
        Vec2i::default(),
        &[
            RenderVertex {
                x: -10.,
                y: 0.,
                u: 0.,
                v: 0.,
                rgba: 0xff0000ff,
            },
            RenderVertex {
                x: 10.,
                y: 0.,
                u: 1.,
                v: 0.,
                rgba: 0xffff0000,
            },
            RenderVertex {
                x: 0.,
                y: 10.,
                u: 0.5,
                v: 1.,
                rgba: 0xff00ff00,
            },
        ],
    ).unwrap();
    fixture.submit(Size { width: 20, height: 20 }).unwrap();
    let capture = fixture.capture.borrow();
    let edge = capture
        .vertices
        .iter()
        .find(|vertex| vertex.x.abs() < 1.0e-5 && vertex.y.abs() < 1.0e-5)
        .unwrap();
    vertex(*edge, [0., 0.], [0.5, 0.]);
    let [red, green, blue, alpha] = edge.rgba.to_le_bytes();
    assert!((i16::from(red) - 128).abs() <= 1);
    assert!((i16::from(blue) - 128).abs() <= 1);
    assert_eq!((green, alpha), (0, 255));
}

/// Fully contained triangles preserve exact attributes and winding through UI replay and GFX.
#[test]
fn contained_atlas_triangles_preserve_both_windings() {
    let vertices = [
        RenderVertex {
            x: 10.,
            y: 10.,
            u: 0.1,
            v: 0.2,
            rgba: 0x281e140a,
        },
        RenderVertex {
            x: 90.,
            y: 10.,
            u: 0.8,
            v: 0.2,
            rgba: 0x50463c32,
        },
        RenderVertex {
            x: 10.,
            y: 90.,
            u: 0.1,
            v: 0.9,
            rgba: 0x786e645a,
        },
    ];
    for triangle in [vertices, [vertices[0], vertices[2], vertices[1]]] {
        let mut fixture = Composition::new(atlas());
        fixture.list.push_atlas_triangles(Recti::new(0, 0, 100, 100), Vec2i::default(), &triangle).unwrap();
        fixture.submit(Size { width: 100, height: 100 }).unwrap();
        assert_eq!(fixture.capture.borrow().vertices, triangle);
    }
}

/// Empty clips and disjoint operations contribute no vertices, batches or resource references.
#[test]
fn fully_clipped_operations_leave_existing_output_unchanged() {
    for clip in [Recti::new(0, 0, 0, 10), Recti::new(0, 0, 10, 0), Recti::new(0, 0, 10, 10)] {
        let mut fixture = Composition::new(atlas());
        let texture: ComPtr<ITextureAbi> = Texture::from_value(Texture).query().unwrap();
        let item: ComPtr<IRenderItemAbi> = Item::from_value(Item).query().unwrap();
        fixture.list.push_icon(
            Recti::new(0, 0, 40, 40),
            fixture.atlas.white_icon(),
            Recti::new(0, 0, 1, 1),
            color(255, 255, 255, 255),
        ).unwrap();
        let destination = Recti::new(20, 20, 10, 10);
        fixture.list.push_com_texture(
            clip,
            destination,
            TextureRegion { u0: 0., v0: 0., u1: 1., v1: 1. },
            color(255, 255, 255, 255),
            texture,
        ).unwrap();
        fixture.list.push_com_item(clip, destination, item).unwrap();
        fixture.list.push_icon(clip, fixture.atlas.white_icon(), destination, color(255, 255, 255, 255)).unwrap();
        fixture.submit(Size { width: 40, height: 40 }).unwrap();
        let capture = fixture.capture.borrow();
        assert_eq!(capture.vertices.len(), 6);
        assert!(matches!(capture.batches.as_slice(), [RenderBatch::Atlas { first: 0, count: 6 }]));
    }
}

/// Clip arithmetic evaluates far rectangle edges without overflowing the signed coordinate range.
#[test]
fn extreme_integer_destinations_keep_texture_vertices_finite_and_inside_the_clip() {
    let mut fixture = Composition::new(atlas());
    let texture: ComPtr<ITextureAbi> = Texture::from_value(Texture).query().unwrap();
    fixture.list.push_com_texture(
        Recti::new(10, 10, 20, 20),
        Recti::new(-10, -10, i32::MAX, i32::MAX),
        TextureRegion { u0: 0., v0: 0., u1: 1., v1: 1. },
        color(255, 255, 255, 255),
        texture,
    ).unwrap();
    fixture.submit(Size { width: 40, height: 40 }).unwrap();
    let capture = fixture.capture.borrow();
    assert_eq!(capture.vertices.len(), 4);
    for vertex in &capture.vertices {
        assert!([vertex.x, vertex.y, vertex.u, vertex.v].into_iter().all(f32::is_finite));
        assert!((10.0..=30.0).contains(&vertex.x) && (10.0..=30.0).contains(&vertex.y));
        assert!((0.0..=1.0).contains(&vertex.u) && (0.0..=1.0).contains(&vertex.v));
    }
}

/// Repeated recording reuses the production polygon and command allocations after warmup.
#[test]
fn warmed_painter_recording_reuses_its_geometry_workspaces() {
    let mut list = DisplayList::new();
    let polygon = [
        crate::Vec2f::new(0., 0.),
        crate::Vec2f::new(30., 0.),
        crate::Vec2f::new(30., 30.),
        crate::Vec2f::new(15., 15.),
        crate::Vec2f::new(0., 30.),
    ];
    let record = |list: &mut DisplayList| {
        let mut painter = crate::render::Painter::screen_space(list, Recti::new(0, 0, 100, 100));
        painter.fill_polygon(&polygon, color(255, 255, 255, 255)).unwrap();
    };
    record(&mut list);
    list.clear();
    let measurement = crate::test_support::AllocationMeasurement::begin();
    for _ in 0..32 {
        record(&mut list);
        list.clear();
    }
    let count = measurement.finish();
    assert_eq!(count.events, 0);
}

/// Stack-sized replay chunks retain every triangle's order and still coalesce into one atlas batch.
#[test]
fn solid_replay_preserves_order_across_staging_chunks() {
    let mut fixture = Composition::new(atlas());
    let mut vertices = Vec::new();
    for index in 0..300u32 {
        for (x, y) in [(1.0, 1.0), (2.0, 1.0), (1.0, 2.0)] {
            vertices.push(neocogi_ui_core_contracts::ColoredVertex { x, y, rgba: 0xff000000 | index });
        }
    }
    crate::render::Painter::screen_space(&mut fixture.list, Recti::new(0, 0, 32, 32)).colored_triangles(&vertices).unwrap();
    fixture.submit(Size { width: 32, height: 32 }).unwrap();
    let capture = fixture.capture.borrow();
    assert_eq!(capture.vertices.len(), vertices.len());
    for (actual, expected) in capture.vertices.iter().zip(vertices) {
        assert_eq!((actual.x, actual.y, actual.rgba), (expected.x, expected.y, expected.rgba));
    }
    assert!(matches!(capture.batches.as_slice(), [RenderBatch::Atlas { count: 900, .. }]));
}
