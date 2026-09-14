//! The runtime COM builder and generated direct exporter must publish equivalent portable data.
use super::*;
use crate::{AssetPublisherObject, GfxCompositorObject};
use neocogi_com::{ComPtr, INcStringAbi, INcStringComPtrExt, NC_E_INVALID_ARG, NC_E_OUT_OF_RANGE, NcCopyToBufferOffset, NcStatus, com_object};
use neocogi_ui::{NativePath, PathEncoding};
use neocogi_ui_runtime::assets::ImportedAssets;
use neocogi_platform_contracts::*;
use neocogi_renderer_contracts::{
    IRenderStreamAbi, IRenderer, IRendererAbi, IRendererInterfacesFor, IRgbaImageAbi, RenderBatch, RenderBatchAbi, RenderInfo, RenderVertex,
};

/// Renderer observations shared with the concrete COM mock after object publication.
#[derive(Default)]
struct RenderCapture {
    /// Number of complete synchronous submissions.
    calls: std::cell::Cell<u32>,
    /// Final bulk vertices copied while the stream token was active.
    vertices: std::cell::RefCell<Vec<RenderVertex>>,
    /// Final ordered batch records copied while the stream token was active.
    batches: std::cell::RefCell<Vec<RenderBatchAbi>>,
    /// Retained stream and expired invocation used to verify post-call revocation.
    retained: std::cell::RefCell<Option<(u64, ComPtr<IRenderStreamAbi>)>>,
}

#[com_object(interfaces(IRenderer))]
/// Test renderer that reads the complete bulk stream and deliberately retains it past submission.
struct CaptureRenderer {
    /// Local observation state contains no renderer-native object or frame borrow.
    capture: std::rc::Rc<RenderCapture>,
}

impl IRenderer for CaptureRenderer {
    /// Atlas preparation is outside the focused compositor stream test.
    fn prepare_atlas(&self, _atlas: neocogi_com::ComRef<'_, IRgbaImageAbi>) -> Result<(), NcStatus> {
        Ok(())
    }

    /// Copies every declared record while the invocation is active, then retains the stream.
    fn render(
        &self,
        info: RenderInfo,
        _atlas: neocogi_com::ComRef<'_, IRgbaImageAbi>,
        stream: neocogi_com::ComRef<'_, IRenderStreamAbi>,
    ) -> Result<(), NcStatus> {
        let mut vertices = vec![RenderVertex::default(); info.vertex_count as usize];
        let mut batches = vec![RenderBatch::Atlas { first: 0, count: 3 }.into(); info.batch_count as usize];
        assert_eq!(
            stream.vertices(info.invocation, NcCopyToBufferOffset::new(0, &mut vertices))?,
            info.vertex_count
        );
        assert_eq!(stream.batches(info.invocation, NcCopyToBufferOffset::new(0, &mut batches))?, info.batch_count);
        *self.capture.vertices.borrow_mut() = vertices;
        *self.capture.batches.borrow_mut() = batches;
        *self.capture.retained.borrow_mut() = Some((info.invocation, stream.add_ref()));
        self.capture.calls.set(self.capture.calls.get() + 1);
        Ok(())
    }
}

#[com_object(interfaces(INativePath))]
/// A caller-owned path deliberately tries to consume its builder during path decoding.
struct ReenterPath {
    /// Exact native path delegated after probing the mutation gate.
    path: NativePath,
    /// Same live draft whose outer operation owns the detached native state.
    builder: ComPtr<IAssetBuilderAbi>,
    /// Test observation independent of the COM allocation's lifetime.
    calls: std::rc::Rc<std::cell::Cell<u32>>,
}
impl INativePath for ReenterPath {
    /// A nested finish must report BUSY without consuming or exposing the outer draft.
    fn encoding(&self) -> Result<PathEncoding, NcStatus> {
        assert_eq!(self.builder.finish().unwrap_err(), GFX_E_BUSY);
        self.calls.set(self.calls.get() + 1);
        self.path.as_interface().encoding()
    }
    /// Forwards exact native units after the reentry probe succeeded.
    fn unit_count(&self) -> Result<u64, NcStatus> {
        self.path.as_interface().unit_count()
    }
    /// Forwards bytes without a lossy conversion through display text.
    fn unix_bytes(&self, output: neocogi_com::NcCopyToBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.path.as_interface().unix_bytes(output)
    }
    /// Keeps the same test valid for native Windows UTF-16 paths.
    fn windows_units(&self, output: neocogi_com::NcCopyToBufferOffset<'_, u16>) -> Result<(), NcStatus> {
        self.path.as_interface().windows_units(output)
    }
    /// Display remains an independent human-readable snapshot.
    fn display(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.path.as_interface().display()
    }
}

/// Copies complete portable values out of an imported revision for deterministic comparisons.
fn data(source: &ComPtr<IAssetBundleAbi>) -> neocogi_gfx_support::AssetData {
    AssetData::read(source.as_com_ref()).unwrap()
}

/// Iterates every named appearance field for producer-output comparison only.
fn visuals(tables: &neocogi_gfx_support::AssetTables) -> impl Iterator<Item = &AssetVisual> {
    let surfaces = tables.surfaces.iter().flat_map(|values| [&values.normal, &values.disabled]);
    let controls = tables.controls.iter().flat_map(|values| {
        [
            &values.disabled,
            &values.enabled.normal,
            &values.enabled.hovered,
            &values.enabled.pressed,
            &values.focused.normal,
            &values.focused.hovered,
            &values.focused.pressed,
        ]
    });
    let menus = tables.menus.iter().flat_map(|values| {
        [
            &values.normal,
            &values.hovered,
            &values.pressed,
            &values.focused,
            &values.open,
            &values.disabled,
        ]
    });
    let chrome = tables.window_chrome.iter().flat_map(|values| [&values.base, &values.active, &values.disabled]);
    // Flattening is an assertion view; production storage and transport remain structured.
    surfaces.chain(controls).chain(menus).chain(chrome)
}

/// Both producer modes share the exact recipe and immutable object representation.
#[test]
fn com_builder_matches_direct_pixels_metrics_and_resolved_visuals() {
    let loader: ComPtr<AssetLoaderAbi> = AssetLoaderObject::from_value(AssetLoaderObject::default()).query().unwrap();
    let config = crate::test_recipe::atlas_config();
    let white = NativePath::new(&config.white_icon);
    let builder = loader
        .create_builder(
            Size {
                width: config.texture_width as i32,
                height: config.texture_height as i32,
            },
            white.as_interface(),
        )
        .unwrap();
    for icon in config.icons {
        builder
            .add_icon(NcCopyFromBufferOffset::new(0, icon.name.as_bytes()), NativePath::new(&icon.path).as_interface())
            .unwrap();
    }
    for font in config.fonts {
        builder
            .add_font(
                NcCopyFromBufferOffset::new(0, font.name.as_bytes()),
                NativePath::new(&font.path).as_interface(),
                font.size as u32,
            )
            .unwrap();
    }
    let runtime = builder.finish().unwrap();
    assert_eq!(builder.finish().unwrap_err(), GFX_E_CLOSED);
    let runtime = data(&runtime);
    let direct = data(&test_bundle());
    assert_eq!(runtime.tables.size, direct.tables.size);
    assert_eq!(runtime.pixels, direct.pixels);
    assert_eq!(runtime.tables.fonts, direct.tables.fonts);
    assert_eq!(runtime.tables.glyphs, direct.tables.glyphs);
    assert_eq!(runtime.tables.icons, direct.tables.icons);
    assert_eq!(runtime.tables.names, direct.tables.names);
    assert_eq!(runtime.tables.metrics, direct.tables.metrics);
    assert_eq!(runtime.tables.chrome, direct.tables.chrome);
    let runtime_visuals = visuals(&runtime.tables).collect::<Vec<_>>();
    let direct_visuals = visuals(&direct.tables).collect::<Vec<_>>();
    assert_eq!(runtime_visuals.len(), direct_visuals.len());
    for (a, b) in runtime_visuals.into_iter().zip(direct_visuals) {
        assert_eq!(a.insets, b.insets);
        assert_eq!(a.center_visible, b.center_visible);
        assert_eq!(a.content_color, b.content_color);
        assert_eq!(AssetPatch::try_from(a.patch).unwrap(), AssetPatch::try_from(b.patch).unwrap());
    }
}

/// Recoverable edits restore the draft; final failure remains closed and diagnostically inspectable.
#[test]
fn failed_edit_can_retry_and_failed_finish_is_terminal() {
    let loader: ComPtr<AssetLoaderAbi> = AssetLoaderObject::from_value(AssetLoaderObject::default()).query().unwrap();
    let config = crate::test_recipe::atlas_config();
    let white = NativePath::new(&config.white_icon);
    assert_eq!(
        loader.create_builder(Size { width: i32::MAX, height: 5 }, white.as_interface()).unwrap_err(),
        NC_E_INVALID_ARG
    );
    let builder = loader.create_builder(Size { width: 96, height: 96 }, white.as_interface()).unwrap();
    assert_eq!(
        builder.add_icon(NcCopyFromBufferOffset::new(0, &[0xff]), white.as_interface()),
        Err(NC_E_INVALID_ARG)
    );
    // Buffer forms accept only one complete, nonempty input beginning at offset zero. Rejecting
    // either range leaves the same transactional draft available for the following valid edit.
    assert_eq!(
        builder.add_icon_bytes(NcCopyFromBufferOffset::new(0, b"empty-image"), NcCopyFromBufferOffset::new(0, &[]),),
        Err(NC_E_INVALID_ARG)
    );
    assert_eq!(
        builder.add_font_bytes(NcCopyFromBufferOffset::new(0, b"offset-font"), NcCopyFromBufferOffset::new(1, &[0]), 12,),
        Err(NC_E_INVALID_ARG)
    );
    builder.add_icon(NcCopyFromBufferOffset::new(0, b"extra"), white.as_interface()).unwrap();
    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    let reenter: ComPtr<INativePathAbi> = ReenterPath::from_value(ReenterPath {
        path: NativePath::new(&config.white_icon),
        builder: builder.clone(),
        calls: calls.clone(),
    })
    .query()
    .unwrap();
    builder
        .add_icon(NcCopyFromBufferOffset::new(0, b"reentrant-path"), reenter.as_com_ref())
        .unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(
        builder.add_icon(NcCopyFromBufferOffset::new(0, b"extra"), white.as_interface()),
        Err(NC_E_INVALID_ARG)
    );
    let diagnostic: ComPtr<IGfxDiagnosticsAbi> = builder.query().unwrap();
    let snapshot = diagnostic.diagnostic().unwrap();
    assert!(snapshot.to_string().unwrap().contains("extra"));
    // This draft intentionally omits the required body and semantic icons.
    assert_eq!(builder.finish().unwrap_err(), NC_E_INVALID_ARG);
    assert_eq!(builder.finish().unwrap_err(), GFX_E_CLOSED);
    assert!(snapshot.to_string().unwrap().contains("extra"));
}

/// Repacking into a larger atlas retains exact application resource identities at the receiving UI.
#[test]
fn derived_builder_preserves_catalog_and_can_add_private_resources() {
    let base = test_bundle();
    let imported = ImportedAssets::read(base.as_com_ref(), None).unwrap();
    let loader: ComPtr<AssetLoaderAbi> = AssetLoaderObject::from_value(AssetLoaderObject::default()).query().unwrap();
    let builder = loader
        .derive_builder(
            Size { width: 1024, height: 512 },
            base.as_com_ref(),
            AssetDeriveOptions {
                replace_semantic_fonts: false,
                replace_semantic_icons: false,
            },
        )
        .unwrap();
    let config = crate::test_recipe::atlas_config();
    builder
        .add_icon(
            NcCopyFromBufferOffset::new(0, b"theme-private"),
            NativePath::new(&config.white_icon).as_interface(),
        )
        .unwrap();
    let source = builder.finish().unwrap();
    let changed = ImportedAssets::read(source.as_com_ref(), Some(&imported)).unwrap();
    assert_eq!(imported.original().font_id("body"), changed.bundle().atlas().font_id("body"));
    assert!(changed.bundle().atlas().icon_id("theme-private").is_some());
    assert!(changed.original().icon_id("theme-private").is_none());
}

/// Appearance publication preserves resources and rejects malformed bulk records inside GFX.
#[test]
fn appearance_revision_is_a_validated_resource_preserving_transaction() {
    let base = test_bundle();
    let original = data(&base);
    let publisher: ComPtr<AssetPublisherAbi> = AssetPublisherObject::from_value(AssetPublisherObject::default()).query().unwrap();
    let mut metrics = original.tables.metrics;
    metrics.padding += 3;
    let revision = AssetAppearanceRevision { metrics, chrome: original.tables.chrome };
    let candidate = publisher
        .revise_appearance(
            base.as_com_ref(),
            revision,
            NcCopyFromBufferOffset::new(0, &original.tables.surfaces),
            NcCopyFromBufferOffset::new(0, &original.tables.controls),
            NcCopyFromBufferOffset::new(0, &original.tables.menus),
            NcCopyFromBufferOffset::new(0, &original.tables.window_chrome),
        )
        .unwrap();
    let changed = data(&candidate);
    assert_eq!(candidate.catalog().unwrap().as_ptr(), base.catalog().unwrap().as_ptr());
    assert_eq!(changed.pixels, original.pixels);
    assert_eq!(changed.tables.fonts, original.tables.fonts);
    assert_eq!(changed.tables.icons, original.tables.icons);
    assert_eq!(changed.tables.metrics, metrics);

    // A role table is all-or-nothing. The service validates range shape before copying any
    // caller-owned record or publishing a candidate with a partially replaced appearance family.
    assert_eq!(
        publisher
            .revise_appearance(
                base.as_com_ref(),
                revision,
                NcCopyFromBufferOffset::new(1, &original.tables.surfaces),
                NcCopyFromBufferOffset::new(0, &original.tables.controls),
                NcCopyFromBufferOffset::new(0, &original.tables.menus),
                NcCopyFromBufferOffset::new(0, &original.tables.window_chrome),
            )
            .unwrap_err(),
        NC_E_INVALID_ARG
    );
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
                }
                .into(),
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
                }
                .into(),
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
                }
                .into(),
            },
        ),
        Err(NC_E_OUT_OF_RANGE)
    );
    compositor.cancel().unwrap();
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
    let mut vertex = [RenderVertex::default()];
    assert_eq!(
        stream.vertices(invocation, NcCopyToBufferOffset::new(0, &mut vertex)),
        Err(NC_E_INVALID_ARG),
        "the renderer cannot reuse a stream after its synchronous call"
    );

    // Equal table positions from a separately constructed bundle do not establish revision
    // compatibility. The failed replacement leaves the compositor ready for a later frame.
    let unrelated = test_bundle();
    assert_eq!(compositor.install_assets(unrelated.as_com_ref()), Err(NC_E_INVALID_ARG));
    compositor.begin(Size { width: 1, height: 1 }, 0).unwrap();
    compositor.cancel().unwrap();
}

/// Constructs one standalone catalog without importing application deployment machinery.
fn test_bundle() -> ComPtr<IAssetBundleAbi> {
    // Each invocation owns a distinct catalog so revision authentication tests remain meaningful.
    Builder::from_config(&crate::test_recipe::atlas_config())
        .unwrap()
        .build()
        .unwrap()
        .into_bundle()
        .unwrap()
}
