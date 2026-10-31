//! Production compositor tests use a small immutable atlas without optional builders.
use neocogi_gfx_support::testing::UiBatch;
use super::*;
use crate::test_support::{test_bundle, data};
use neocogi_com::{com_object};
use neocogi_render_core::abi::{IRenderer, IRendererVtblFor};

/// Native preparation observations remain separate from immutable asset ownership.
#[derive(Default)]
struct UploadProbe {
    /// Every attempted native preparation, including injected failures.
    calls: std::cell::Cell<u32>,
    /// Simulates a recoverable native allocation failure before candidate publication.
    fail: std::cell::Cell<bool>,
}

#[com_object(interfaces(neocogi_render_core::abi::IRenderHost))]
/// Count and reject native preparation while retaining the real CPU graphics contracts.
struct UploadHost {
    /// Shared fixture device supplies canonical resources and blocks.
    inner: ComPtr<neocogi_render_core::abi::IRenderHostAbi>,
    /// No callback or UI owner participates in this observation state.
    probe: std::rc::Rc<UploadProbe>,
}
impl neocogi_render_core::abi::IRenderDriverBound for UploadHost {
    fn driver_identity(&self) -> Result<ComPtr<neocogi_render_core::abi::IRenderDriverIdentityAbi>, NcStatus> {
        self.inner.driver_identity()
    }
}
impl neocogi_render_core::abi::IRenderHost for UploadHost {
    fn maintain(&self) -> Result<(), NcStatus> {
        self.inner.maintain()
    }
    fn import_ui_texture(
        &self,
        resources: ComRef<'_, neocogi_render_core::abi::IRenderResourcesAbi>,
        image: ComRef<'_, ITextureAbi>,
    ) -> Result<ComBox<neocogi_render_core::abi::IGpuTextureAbi>, NcStatus> {
        self.inner.import_ui_texture(resources, image)
    }
    fn prepare_ui_texture(
        &self,
        resources: ComRef<'_, neocogi_render_core::abi::IRenderResourcesAbi>,
        texture: ComRef<'_, neocogi_render_core::abi::IGpuTextureAbi>,
    ) -> Result<(), NcStatus> {
        self.probe.calls.set(self.probe.calls.get() + 1);
        if self.probe.fail.get() {
            return Err(NC_E_OUT_OF_MEMORY);
        }
        self.inner.prepare_ui_texture(resources, texture)
    }
    fn create_ui_client(
        &self,
        limits: neocogi_render_core::abi::RenderClientLimits,
    ) -> Result<ComBox<neocogi_render_core::abi::IRenderClientDomainAbi>, NcStatus> {
        self.inner.create_ui_client(limits)
    }
    fn recording_broker(&self) -> Result<ComPtr<neocogi_render_core::abi::IRenderViewBrokerAbi>, NcStatus> {
        self.inner.recording_broker()
    }
}

/// Metadata edits reuse the atlas; failed image replacement preserves the matching old metadata.
#[test]
fn compositor_prepares_only_changed_atlases_and_publishes_revisions_atomically() {
    let bundle = test_bundle();
    let mut compositor = GfxCompositorObject::create(bundle.as_com_ref()).unwrap();
    let probe = std::rc::Rc::new(UploadProbe::default());
    let host: ComPtr<neocogi_render_core::abi::IRenderHostAbi> = UploadHost::from_value(UploadHost {
        inner: neocogi_gfx_support::testing::host(),
        probe: probe.clone(),
    })
    .query()
    .unwrap();
    compositor.bind_render_host(host.as_com_ref()).unwrap();
    assert_eq!(probe.calls.get(), 1);
    let mut tables = neocogi_gfx_support::AssetTables::read(bundle.as_com_ref()).unwrap();
    tables.metrics.spacing += 1;
    let same_image = tables.into_revision(bundle.as_com_ref()).unwrap();
    assert_eq!(same_image.image().unwrap().as_raw().object, bundle.image().unwrap().as_raw().object);
    compositor.install_assets(same_image.as_com_ref()).unwrap();
    assert_eq!(probe.calls.get(), 1, "metadata edits retain the immutable prepared atlas");

    let mut candidate = data(&same_image);
    let line_size = candidate.tables.fonts[0].line_size;
    candidate.tables.fonts.to_mut()[0].line_size += 5;
    let changed_image = candidate.into_revision(same_image.as_com_ref()).unwrap();
    let capture = std::rc::Rc::new(RenderCapture::default());
    let renderer: ComPtr<IRendererAbi> = CaptureRenderer::from_value(CaptureRenderer { capture: capture.clone() }).query().unwrap();
    probe.fail.set(true);
    assert_eq!(compositor.install_assets(changed_image.as_com_ref()), Err(NC_E_OUT_OF_MEMORY));
    let clip = Rect { x: 0, y: 0, width: 80, height: 60 };
    for expected in [line_size, line_size + 5] {
        compositor.begin(Size { width: 80, height: 60 }, 0xff000000).unwrap();
        compositor
            .text(clip, 0, DrawPoint { x: 0, y: 0 }, 0xffffffff, NcCopyFromBufferOffset::new(0, b"A\nA"))
            .unwrap();
        compositor.submit(renderer.as_com_ref()).unwrap();
        let vertices = capture.vertices.borrow();
        assert_eq!(vertices.len(), 12);
        assert_eq!(vertices[6].y - vertices[0].y, expected as f32);
        drop(vertices);
        if expected == line_size {
            probe.fail.set(false);
            compositor.install_assets(changed_image.as_com_ref()).unwrap();
        }
    }
    assert_eq!(probe.calls.get(), 3, "initial upload, failed candidate, successful candidate");
}

/// Renderer observations shared with the concrete COM mock after object publication.
#[derive(Default)]
struct RenderCapture {
    /// Number of complete synchronous submissions.
    calls: std::cell::Cell<u32>,
    /// Final bulk vertices copied while the stream token was active.
    vertices: std::cell::RefCell<Vec<RenderVertex>>,
    /// Final ordered batch records copied while the stream token was active.
    batches: std::cell::RefCell<Vec<UiBatch>>,
    /// Retained stream and expired invocation used to verify post-call revocation.
    retained: std::cell::RefCell<Option<(u64, ComPtr<IRenderStreamAbi>)>>,
}

#[com_object(interfaces(IRenderer))]
/// Test renderer that reads the complete bulk stream and deliberately retains it past submission.
struct CaptureRenderer {
    /// Local observation state contains no renderer-native object or frame borrow.
    capture: std::rc::Rc<RenderCapture>,
}

impl neocogi_render_core::abi::IRenderDriverBound for CaptureRenderer {
    fn driver_identity(&self) -> Result<neocogi_com::ComPtr<neocogi_render_core::abi::IRenderDriverIdentityAbi>, neocogi_com::NcStatus> {
        Err(neocogi_com::NC_E_NOINTERFACE)
    }
}
impl IRenderer for CaptureRenderer {
    /// Copies every declared record while the invocation is active, then retains the stream.
    fn render(&self, info: RenderInfo, stream: neocogi_com::ComRef<'_, IRenderStreamAbi>) -> Result<(), NcStatus> {
        let (vertices, batches) = neocogi_gfx_support::testing::read(info, stream)?;
        *self.capture.vertices.borrow_mut() = vertices;
        *self.capture.batches.borrow_mut() = batches;
        *self.capture.retained.borrow_mut() = Some((info.invocation, stream.try_add_ref()?));
        self.capture.calls.set(self.capture.calls.get() + 1);
        Ok(())
    }
}

/// Reads actual screen extents of one complete emitted quad, independently of vertex ordering.
fn quad_bounds(vertices: &[RenderVertex]) -> (f32, f32, f32, f32) {
    assert_eq!(vertices.len(), 6);
    (
        vertices.iter().map(|v| v.x).fold(f32::INFINITY, f32::min),
        vertices.iter().map(|v| v.y).fold(f32::INFINITY, f32::min),
        vertices.iter().map(|v| v.x).fold(f32::NEG_INFINITY, f32::max),
        vertices.iter().map(|v| v.y).fold(f32::NEG_INFINITY, f32::max),
    )
}

/// Accepted extreme insets preserve both colored sides through real COM expansion and submission.
#[test]
fn compositor_extreme_patch_sides_cover_an_ordinary_destination() {
    let bundle = test_bundle();
    let mut compositor = GfxCompositorObject::create(bundle.as_com_ref()).unwrap();
    compositor.bind_render_host(neocogi_gfx_support::testing::host().as_com_ref()).unwrap();
    let capture = std::rc::Rc::new(RenderCapture::default());
    let renderer: ComPtr<IRendererAbi> = CaptureRenderer::from_value(CaptureRenderer { capture: capture.clone() }).query().unwrap();
    let destination = Rect { x: 5, y: 7, width: 100, height: 10 };
    let red = u32::from_le_bytes([255, 0, 0, 255]);
    let blue = u32::from_le_bytes([0, 0, 255, 255]);
    for (leading, trailing, split) in [(i32::MAX, i32::MAX, 55.0), (1_500_000_000, 1_000_000_000, 65.0)] {
        compositor.begin(Size { width: 120, height: 40 }, 0).unwrap();
        compositor
            .nine_patch(
                destination,
                destination,
                NinePatchDraw {
                    insets: Insets {
                        left: leading,
                        right: trailing,
                        top: 0,
                        bottom: 0,
                    },
                    center_visible: 1,
                    patch: AssetPatch::Flat {
                        top_left: 0,
                        top: 0,
                        top_right: 0,
                        left: red,
                        center: 0,
                        right: blue,
                        bottom_left: 0,
                        bottom: 0,
                        bottom_right: 0,
                    },
                },
            )
            .unwrap();
        compositor.submit(renderer.as_com_ref()).unwrap();
        let vertices = capture.vertices.borrow();
        assert_eq!(vertices.len(), 12);
        assert!(vertices[..6].iter().all(|v| v.rgba == red));
        assert!(vertices[6..].iter().all(|v| v.rgba == blue));
        assert_eq!(quad_bounds(&vertices[..6]), (5.0, 7.0, split, 17.0));
        assert_eq!(quad_bounds(&vertices[6..]), (split, 7.0, 105.0, 17.0));
        assert_eq!(capture.batches.borrow().len(), 1);
    }
}

/// Stored and direct image patches share shrinking, unchanged tint and ordered pink overlays.
#[test]
fn compositor_source_shrink_overlays_pink_and_preserves_valid_frame_work() {
    let base = test_bundle();
    let info = base.info().unwrap();
    assert_eq!(info.schema, 3);
    assert!(info.rgba_bytes().is_ok());
    assert_eq!(AssetInfo { schema: 2, ..info }.rgba_bytes(), Err(NC_E_INVALID_ARG));
    let mut stored = data(&base);
    let icon = stored.tables.icons.iter().position(|i| stored.tables.name(i.name) == Ok("close")).unwrap() as u32;
    let destination = Rect { x: 0, y: 0, width: 100, height: 20 };
    let visual = AssetVisual {
        insets: Insets { left: 50, right: 50, top: 0, bottom: 0 },
        center_visible: 1,
        content_color: 0,
        patch: AssetPatch::Image {
            icon,
            source: Insets {
                left: i32::MAX,
                right: i32::MAX,
                top: 0,
                bottom: 0,
            },
            tint: 0xff000000,
        },
    };
    stored.tables.surfaces.to_mut()[SurfaceRole::Panel.raw() as usize].normal = visual;
    stored.validate().unwrap();
    let source = stored.into_revision(base.as_com_ref()).unwrap();
    // Import through the actual bulk COM reader, preserving the original oversized source request.
    let imported = data(&source);
    let stored_visual = imported.tables.surfaces[SurfaceRole::Panel.raw() as usize].normal;
    assert_eq!(stored_visual.patch, visual.patch);
    let mut compositor = GfxCompositorObject::create(source.as_com_ref()).unwrap();
    compositor.bind_render_host(neocogi_gfx_support::testing::host().as_com_ref()).unwrap();
    let capture = std::rc::Rc::new(RenderCapture::default());
    let renderer: ComPtr<IRendererAbi> = CaptureRenderer::from_value(CaptureRenderer { capture: capture.clone() }).query().unwrap();
    for visual in [visual, stored_visual] {
        compositor.begin(Size { width: 100, height: 20 }, 0).unwrap();
        let draw = NinePatchDraw {
            insets: visual.insets,
            center_visible: visual.center_visible,
            patch: visual.patch,
        };
        compositor.nine_patch(destination, destination, draw).unwrap();
        // An invalid later patch must leave the complete earlier image/overlay operation intact.
        let bad = AssetPatch::Image {
            icon,
            source: Insets { left: -1, ..Insets::default() },
            tint: 0xffffffff,
        };
        assert_eq!(
            compositor.nine_patch(destination, destination, NinePatchDraw { patch: bad, ..draw }),
            Err(NC_E_INVALID_ARG)
        );
        compositor.submit(renderer.as_com_ref()).unwrap();
        let vertices = capture.vertices.borrow();
        assert_eq!(vertices.len(), 24);
        for (index, quad) in vertices.chunks_exact(6).enumerate() {
            let expected_color = if index % 2 == 0 {
                0xff000000
            } else {
                u32::from_le_bytes([255, 192, 203, 128])
            };
            assert!(quad.iter().all(|v| v.rgba == expected_color));
            let expected = if index < 2 { (0.0, 0.0, 50.0, 20.0) } else { (50.0, 0.0, 100.0, 20.0) };
            assert_eq!(quad_bounds(quad), expected);
            if index % 2 == 1 {
                assert!(quad.iter().all(|v| v.u == 0.0 || v.u == 1.0));
                assert!(quad.iter().all(|v| v.v == 0.0 || v.v == 1.0));
            }
        }
        assert_eq!(capture.batches.borrow().len(), 1);
    }
    // Diagnostics use the same effective clip as the image, with no unbounded overlay quad.
    compositor.begin(Size { width: 100, height: 20 }, 0).unwrap();
    let clip = Rect { x: 20, y: 0, width: 60, height: 20 };
    compositor
        .nine_patch(
            clip,
            destination,
            NinePatchDraw {
                insets: visual.insets,
                center_visible: visual.center_visible,
                patch: visual.patch,
            },
        )
        .unwrap();
    compositor.submit(renderer.as_com_ref()).unwrap();
    let clipped = capture.vertices.borrow();
    assert_eq!(clipped.len(), 24);
    for (index, quad) in clipped.chunks_exact(6).enumerate() {
        let expected = if index < 2 { (20.0, 0.0, 50.0, 20.0) } else { (50.0, 0.0, 80.0, 20.0) };
        assert_eq!(quad_bounds(quad), expected);
    }
    drop(clipped);
    // Destination-only shrinking, including an exact source fit, must remain untinted.
    compositor.begin(Size { width: 100, height: 20 }, 0).unwrap();
    compositor
        .nine_patch(
            destination,
            destination,
            NinePatchDraw {
                insets: Insets {
                    left: i32::MAX,
                    right: i32::MAX,
                    top: 0,
                    bottom: 0,
                },
                center_visible: 1,
                patch: AssetPatch::Image {
                    icon,
                    source: Insets { left: 2, right: 2, top: 0, bottom: 0 },
                    tint: 0xff000000,
                },
            },
        )
        .unwrap();
    compositor.submit(renderer.as_com_ref()).unwrap();
    assert_eq!(capture.vertices.borrow().len(), 12);
    assert!(capture.vertices.borrow().iter().all(|v| v.rgba == 0xff000000));
}

/// GFX expands one typed image patch atomically and rejects malformed semantic payloads.
#[test]
fn compositor_expands_typed_image_patch_and_rejects_invalid_payloads() {
    let bundle = test_bundle();
    let tables = data(&bundle).tables;
    let close = tables
        .icons
        .iter()
        .position(|icon| tables.name(icon.name) == Ok("close"))
        .expect("test bundle contains close") as u32;
    let source = tables.icons[close as usize].rect;
    assert!(source.width >= 2 && source.height >= 2, "the fixture must provide four nonempty source corners");
    let mut compositor = GfxCompositorObject::create(bundle.as_com_ref()).unwrap();
    compositor.bind_render_host(neocogi_gfx_support::testing::host().as_com_ref()).unwrap();
    let clip = Rect { x: 0, y: 0, width: 32, height: 32 };
    let destination = Rect { x: 4, y: 4, width: 16, height: 16 };
    compositor.begin(Size { width: 32, height: 32 }, 0xff000000).unwrap();
    compositor
        .nine_patch(
            clip,
            destination,
            NinePatchDraw {
                insets: Insets { left: 4, top: 4, right: 4, bottom: 4 },
                center_visible: 1,
                patch: AssetPatch::Image {
                    icon: close,
                    source: Insets {
                        left: source.width / 2,
                        top: source.height / 2,
                        right: source.width - source.width / 2,
                        bottom: source.height - source.height / 2,
                    },
                    tint: 0xffffffff,
                },
            },
        )
        .unwrap();
    let capture = std::rc::Rc::new(RenderCapture::default());
    let renderer: ComPtr<IRendererAbi> = CaptureRenderer::from_value(CaptureRenderer { capture: capture.clone() }).query().unwrap();
    compositor.submit(renderer.as_com_ref()).unwrap();
    assert_eq!(capture.calls.get(), 1);
    assert_eq!(capture.vertices.borrow().len(), 24, "four source corners must produce four atlas quads");
    assert_eq!(capture.batches.borrow().len(), 1, "one patch must remain one adjacent atlas batch");

    // Invalid flags and resource slots are rejected before any prefix can reach frame storage.
    compositor.begin(Size { width: 32, height: 32 }, 0xff000000).unwrap();
    assert_eq!(
        compositor.nine_patch(
            clip,
            destination,
            NinePatchDraw {
                insets: Insets::default(),
                center_visible: 2,
                patch: AssetPatch::Image {
                    icon: close,
                    source: Insets::default(),
                    tint: 0xffffffff,
                },
            },
        ),
        Err(NC_E_INVALID_ARG)
    );
    assert_eq!(
        compositor.nine_patch(
            clip,
            destination,
            NinePatchDraw {
                insets: Insets::default(),
                center_visible: 1,
                patch: AssetPatch::Image {
                    icon: u32::MAX,
                    source: Insets::default(),
                    tint: 0xffffffff,
                },
            },
        ),
        Err(NC_E_OUT_OF_RANGE)
    );
    compositor.cancel().unwrap();
}

/// Triangle visibility shortcuts preserve attributes, boundary coverage and foreign-input checks.
#[test]
fn compositor_clips_triangles_without_changing_visible_attributes() {
    let bundle = test_bundle();
    let mut compositor = GfxCompositorObject::create(bundle.as_com_ref()).unwrap();
    compositor.bind_render_host(neocogi_gfx_support::testing::host().as_com_ref()).unwrap();
    let capture = std::rc::Rc::new(RenderCapture::default());
    let renderer: ComPtr<IRendererAbi> = CaptureRenderer::from_value(CaptureRenderer { capture: capture.clone() }).query().unwrap();
    let clip = Rect { x: 0, y: 0, width: 10, height: 10 };
    let inside = [
        RenderVertex {
            x: 0.,
            y: 0.,
            u: 0.,
            v: 0.,
            rgba: 0xff000000,
        },
        RenderVertex {
            x: 10.,
            y: 0.,
            u: 1.,
            v: 0.,
            rgba: 0xff000040,
        },
        RenderVertex {
            x: 0.,
            y: 10.,
            u: 0.,
            v: 1.,
            rgba: 0xff000080,
        },
    ];
    compositor.begin(Size { width: 10, height: 10 }, 0xff000000).unwrap();
    compositor.atlas_triangles(clip, NcCopyFromBufferOffset::new(0, &inside)).unwrap();
    // Each displaced triangle is wholly outside one different edge and emits no vertices.
    for (x, y) in [(-20., 0.), (20., 0.), (0., -20.), (0., 20.)] {
        let outside = inside.map(|vertex| RenderVertex {
            x: vertex.x + x,
            y: vertex.y + y,
            ..vertex
        });
        compositor.atlas_triangles(clip, NcCopyFromBufferOffset::new(0, &outside)).unwrap();
    }
    // Clipping the left vertex produces a quadrilateral with two half-way UV/color intersections.
    let crossing = [
        RenderVertex { x: -4., y: 2., ..inside[0] },
        RenderVertex { x: 4., y: 2., ..inside[1] },
        RenderVertex { x: 4., y: 6., u: 1., ..inside[2] },
    ];
    compositor.atlas_triangles(clip, NcCopyFromBufferOffset::new(0, &crossing)).unwrap();
    // Visibility never bypasses ABI validation, even for otherwise invisible input.
    for invalid in [RenderVertex { x: f32::NAN, ..inside[0] }, RenderVertex { x: -20., u: 2., ..inside[0] }] {
        assert_eq!(
            compositor.atlas_triangles(clip, NcCopyFromBufferOffset::new(0, &[invalid; 3])),
            Err(NC_E_INVALID_ARG)
        );
    }
    compositor.submit(renderer.as_com_ref()).unwrap();
    let actual: Vec<_> = capture.vertices.borrow().iter().map(|v| (v.x, v.y, v.u, v.v, v.rgba)).collect();
    assert_eq!(&actual[..3], &inside.map(|v| (v.x, v.y, v.u, v.v, v.rgba)));
    assert_eq!(
        &actual[3..],
        &[
            (0., 4., 0.5, 0.5, 0xff000040),
            (0., 2., 0.5, 0., 0xff000020),
            (4., 2., 1., 0., 0xff000040),
            (0., 4., 0.5, 0.5, 0xff000040),
            (4., 2., 1., 0., 0xff000040),
            (4., 6., 1., 1., 0xff000080),
        ]
    );
    assert_eq!(capture.batches.borrow().len(), 1, "visible operations retain one adjacent atlas batch");

    // All vertices may lie outside while the triangle still covers the entire clip rectangle.
    // Area verifies full coverage independently of the clipper's triangulation starting corner.
    compositor.begin(Size { width: 10, height: 10 }, 0xff000000).unwrap();
    let enclosing = [(-10., -10.), (40., -10.), (-10., 40.)].map(|(x, y)| RenderVertex { x, y, ..inside[0] });
    compositor.atlas_triangles(clip, NcCopyFromBufferOffset::new(0, &enclosing)).unwrap();
    compositor.submit(renderer.as_com_ref()).unwrap();
    let vertices = capture.vertices.borrow();
    assert!(vertices.iter().all(|v| (0. ..=10.).contains(&v.x) && (0. ..=10.).contains(&v.y)));
    let area: f32 = vertices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|[a, b, c]| ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)) * 0.5)
        .sum();
    assert_eq!(area, 100.);
}

/// A shared text clip preserves line placement and validation even when a run emits no geometry.
#[test]
fn compositor_text_preserves_empty_run_validation_and_line_placement() {
    let bundle = test_bundle();
    let tables = data(&bundle).tables;
    let font = tables.fonts.iter().position(|font| tables.name(font.name) == Ok("body")).unwrap() as u32;
    let metrics = tables.fonts[font as usize];
    let space = tables.glyphs[metrics.glyphs.first as usize..(metrics.glyphs.first + metrics.glyphs.count) as usize]
        .iter()
        .find(|glyph| glyph.scalar == u32::from(' '))
        .unwrap();
    assert!(space.rect.width == 0 || space.rect.height == 0, "fixture space contributes no drawable area");
    let mut compositor = GfxCompositorObject::create(bundle.as_com_ref()).unwrap();
    compositor.bind_render_host(neocogi_gfx_support::testing::host().as_com_ref()).unwrap();
    let origin = DrawPoint { x: 40, y: 40 };
    let full_clip = Rect {
        x: -100,
        y: -100,
        width: 500,
        height: 500,
    };
    let empty_clip = Rect { x: 0, y: 0, width: 0, height: 0 };
    let invalid_clip = Rect { x: 0, y: 0, width: -1, height: 10 };
    compositor.begin(Size { width: 256, height: 256 }, 0xff000000).unwrap();
    // Offscreen text still authenticates its protocol; visibility must not hide malformed input.
    assert_eq!(
        compositor.text(empty_clip, font, origin, 0xffffffff, NcCopyFromBufferOffset::new(0, &[0xff])),
        Err(NC_E_INVALID_ARG)
    );
    assert_eq!(
        compositor.text(empty_clip, u32::MAX, origin, 0xffffffff, NcCopyFromBufferOffset::new(0, b"")),
        Err(NC_E_OUT_OF_RANGE)
    );
    assert_eq!(
        compositor.text(empty_clip, font, origin, 0xffffffff, NcCopyFromBufferOffset::new(1, b"A")),
        Err(NC_E_INVALID_ARG)
    );
    // Clip errors are observed only for drawable glyphs, matching the previous per-glyph path.
    for text in [b"".as_slice(), b" \r\n".as_slice()] {
        compositor
            .text(invalid_clip, font, origin, 0xffffffff, NcCopyFromBufferOffset::new(0, text))
            .unwrap();
    }
    assert_eq!(
        compositor.text(invalid_clip, font, origin, 0xffffffff, NcCopyFromBufferOffset::new(0, b"A")),
        Err(NC_E_INVALID_ARG)
    );
    compositor
        .text(empty_clip, font, origin, 0xffffffff, NcCopyFromBufferOffset::new(0, b"A"))
        .unwrap();
    compositor
        .text(full_clip, font, origin, 0xff123456, NcCopyFromBufferOffset::new(0, b"A\r\nA"))
        .unwrap();
    let capture = std::rc::Rc::new(RenderCapture::default());
    let renderer: ComPtr<IRendererAbi> = CaptureRenderer::from_value(CaptureRenderer { capture: capture.clone() }).query().unwrap();
    compositor.submit(renderer.as_com_ref()).unwrap();
    let vertices = capture.vertices.borrow();
    assert_eq!(vertices.len(), 12, "only the two visible glyphs emit geometry");
    // CRLF advances exactly one line; source coordinates and tint are shared by both glyphs.
    for (first, second) in vertices[..6].iter().zip(&vertices[6..]) {
        assert_eq!((first.x, first.u, first.v, first.rgba), (second.x, second.u, second.v, second.rgba));
        assert_eq!(first.rgba, 0xff123456);
        assert_eq!(second.y - first.y, metrics.line_size as f32);
    }
}

/// Coarse operations expand inside GFX, use one renderer call, and expire the retained stream.
#[test]
fn compositor_owns_final_geometry_batching_and_stream_scope() {
    let bundle = test_bundle();
    let tables = data(&bundle).tables;
    let body = tables
        .fonts
        .iter()
        .position(|font| tables.name(font.name) == Ok("body"))
        .expect("test bundle contains body") as u32;
    let close = tables
        .icons
        .iter()
        .position(|icon| tables.name(icon.name) == Ok("close"))
        .expect("test bundle contains close") as u32;
    let mut compositor = GfxCompositorObject::create(bundle.as_com_ref()).unwrap();
    compositor.bind_render_host(neocogi_gfx_support::testing::host().as_com_ref()).unwrap();
    compositor.begin(Size { width: 80, height: 60 }, 0xff000000).unwrap();
    compositor
        .solid_triangles(
            Rect { x: 0, y: 0, width: 80, height: 60 },
            NcCopyFromBufferOffset::new(
                0,
                &[
                    SolidVertex { x: -5.0, y: 2.0, rgba: 0xff0000ff },
                    SolidVertex { x: 12.0, y: 2.0, rgba: 0xff00ff00 },
                    SolidVertex { x: 2.0, y: 14.0, rgba: 0xffff0000 },
                ],
            ),
        )
        .unwrap();
    compositor
        .text(
            Rect { x: 0, y: 0, width: 80, height: 60 },
            body,
            DrawPoint { x: 4, y: 6 },
            0xffffffff,
            NcCopyFromBufferOffset::new(0, b"A _"),
        )
        .unwrap();
    compositor
        .icon(
            Rect { x: 0, y: 0, width: 80, height: 60 },
            close,
            Rect { x: 30, y: 10, width: 24, height: 24 },
            0xffffffff,
        )
        .unwrap();

    let capture = std::rc::Rc::new(RenderCapture::default());
    let renderer: ComPtr<IRendererAbi> = CaptureRenderer::from_value(CaptureRenderer { capture: capture.clone() }).query().unwrap();
    compositor.submit(renderer.as_com_ref()).unwrap();

    assert_eq!(capture.calls.get(), 1);
    assert!(!capture.vertices.borrow().is_empty());
    assert_eq!(capture.vertices.borrow().len() % 3, 0);
    assert_eq!(capture.batches.borrow().len(), 1, "adjacent primary-atlas output must merge");
    let (invocation, stream) = capture.retained.borrow().as_ref().cloned().unwrap();
    assert_eq!(
        stream.view(invocation, 0).unwrap_err(),
        NC_E_INVALID_ARG,
        "expired streams cannot expose blocks"
    );

    // Equal table positions from a separately constructed bundle do not establish revision
    // compatibility. The failed replacement leaves the compositor ready for a later frame.
    let unrelated = test_bundle();
    assert_eq!(compositor.install_assets(unrelated.as_com_ref()), Err(NC_E_INVALID_ARG));
    compositor.begin(Size { width: 1, height: 1 }, 0).unwrap();
    compositor.cancel().unwrap();
}

/// The crossing fraction must remain accurate when subtracting endpoints would overflow f32.
#[test]
fn extreme_finite_crossings_have_finite_interpolated_attributes() {
    let from = RenderVertex {
        x: -f32::MAX,
        y: -f32::MAX,
        u: 0.0,
        v: 0.0,
        rgba: 0,
    };
    let to = RenderVertex {
        x: f32::MAX,
        y: f32::MAX,
        u: 1.0,
        v: 1.0,
        rgba: u32::MAX,
    };
    for axis in [Axis::X, Axis::Y] {
        let vertex = Edge { axis, side: Side::Minimum, boundary: 0.0 }.intersection(from, to);
        assert_eq!([vertex.x, vertex.y], [0.0, 0.0]);
        assert_eq!([vertex.u, vertex.v], [0.5, 0.5]);
        assert_eq!(vertex.rgba, 0x80808080);
    }
}

/// Fully clipped and empty input still consumes the command budget; cancelling begins a fresh frame.
#[test]
fn compositor_intake_limits_apply_before_expansion_and_reset_with_frames() {
    use neocogi_render_core::abi::{MAX_DRAW_COMMANDS, MAX_DRAW_WORK};
    let bundle = test_bundle();
    let mut compositor = GfxCompositorObject::create(bundle.as_com_ref()).unwrap();
    compositor.bind_render_host(neocogi_gfx_support::testing::host().as_com_ref()).unwrap();
    let size = Size { width: 32, height: 32 };
    let clip = Rect { x: 0, y: 0, width: 0, height: 0 };
    compositor.begin(size, 0).unwrap();
    // No valid UTF-8 or glyph lookup is needed to reject a run beyond the work limit.
    let excessive = vec![255; MAX_DRAW_WORK + 1];
    assert_eq!(
        compositor.text(clip, 0, DrawPoint { x: 0, y: 0 }, 0, NcCopyFromBufferOffset::new(0, &excessive)),
        Err(NC_E_OUT_OF_RANGE)
    );
    for _ in 0..MAX_DRAW_COMMANDS {
        compositor.atlas_triangles(clip, NcCopyFromBufferOffset::new(0, &[])).unwrap();
    }
    assert_eq!(compositor.atlas_triangles(clip, NcCopyFromBufferOffset::new(0, &[])), Err(NC_E_OUT_OF_RANGE));
    compositor.cancel().unwrap();
    compositor.begin(size, 0).unwrap();
    compositor.atlas_triangles(clip, NcCopyFromBufferOffset::new(0, &[])).unwrap();
    // Empty solid buffers retain the same clip validation as nonempty buffers.
    let invalid = Rect { width: -1, ..clip };
    assert_eq!(compositor.solid_triangles(invalid, NcCopyFromBufferOffset::new(0, &[])), Err(NC_E_INVALID_ARG));
}

/// Actual COM text expansion retains missing-ASCII/Unicode fallback and replaces revision indices.
#[test]
fn glyph_acceleration_preserves_unicode_fallback_and_revision_publication() {
    let original = test_bundle();
    let mut fixture = data(&original);
    for glyph in fixture.tables.glyphs.to_mut() {
        match char::from_u32(glyph.scalar).unwrap() {
            'A' => glyph.advance_x = 3,
            '_' => glyph.advance_x = 5,
            'B' => {
                glyph.scalar = u32::from('Ω');
                glyph.advance_x = 7;
            }
            _ => {}
        }
    }
    // Foreign producers may publish unsorted font partitions; lookup semantics are identical.
    fixture.tables.glyphs.to_mut().reverse();
    let source = fixture.clone().into_bundle().unwrap();
    let mut compositor = GfxCompositorObject::create(source.as_com_ref()).unwrap();
    compositor.bind_render_host(neocogi_gfx_support::testing::host().as_com_ref()).unwrap();
    let capture = std::rc::Rc::new(RenderCapture::default());
    let renderer: ComPtr<IRendererAbi> = CaptureRenderer::from_value(CaptureRenderer { capture: capture.clone() }).query().unwrap();
    let size = Size { width: 128, height: 32 };
    let clip = Rect { x: 0, y: 0, width: 128, height: 32 };
    for (replacement, expected) in [(false, [0., 3., 8., 15.]), (true, [0., 11., 16., 33.])] {
        if replacement {
            for glyph in fixture.tables.glyphs.to_mut() {
                match char::from_u32(glyph.scalar).unwrap() {
                    'A' => glyph.advance_x = 11,
                    'Ω' => glyph.advance_x = 17,
                    _ => {}
                }
            }
            let revision = fixture.clone().into_revision(source.as_com_ref()).unwrap();
            compositor.install_assets(revision.as_com_ref()).unwrap();
        }
        compositor.begin(size, 0).unwrap();
        compositor
            .text(clip, 0, DrawPoint { x: 0, y: 0 }, 0xffffffff, NcCopyFromBufferOffset::new(0, "ABΩ€".as_bytes()))
            .unwrap();
        compositor.submit(renderer.as_com_ref()).unwrap();
        let vertices = capture.vertices.borrow();
        assert_eq!(vertices.len(), 24);
        let positions = vertices.chunks_exact(6).map(|quad| quad[0].x).collect::<Vec<_>>();
        assert_eq!(positions, expected, "both missing characters use the underscore advance");
    }
}
