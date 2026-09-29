//! End-to-end coverage for the deployable UI-to-GFX-to-renderer frame path.

use neocogi_com::{ComPtr, ComRef, NcCopyFromBufferOffset, NcCopyToBufferOffset, NcStatus, NC_E_OUT_OF_MEMORY, com_object};
use neocogi_gfx_contracts::{AssetAppearanceRevision, AssetPublisherAbi, GfxPluginFactoryAbi, IAssetBundleAbi};
use neocogi_renderer_contracts::{IRenderStreamAbi, IRenderer, IRendererAbi, IRendererVtblFor, IRgbaImageAbi, RenderBatch, RenderInfo, RenderVertex};
use neocogi_ui::{Rect, Size, TextWrapping, Ui, UI_E_OWNER};
use std::{cell::Cell, rc::Rc};

/// Identical drawable dimensions keep alternating frames comparable across independent UIs.
const SIZE: Size = Size { width: 200, height: 120 };

/// Counts complete renderer calls and records the bulk sizes observed during each active scope.
#[derive(Default)]
struct Observation {
    /// `IRenderer` deliberately exposes shared access, so callback mutation requires this cell.
    calls: Cell<u32>,
    /// Last vertex count copied from the compositor-owned stream.
    vertices: Cell<u32>,
    /// Last batch count copied from the compositor-owned stream.
    batches: Cell<u32>,
    /// Borrowed image identity observed only while the test retains its owning bundle.
    atlas: Cell<usize>,
}

#[com_object(interfaces(IRenderer))]
/// Minimal renderer that consumes the complete stream without owning a native graphics backend.
struct CaptureRenderer {
    /// Shared assertions remain available after COM object publication and final release.
    observation: Rc<Observation>,
}

impl IRenderer for CaptureRenderer {
    /// Accepts a logical upload without allocating a native graphics backend.
    fn prepare_atlas(&self, _atlas: ComRef<'_, IRgbaImageAbi>) -> Result<(), NcStatus> {
        Ok(())
    }

    /// Copies both bounded arrays while the invocation token is valid.
    fn render(&self, info: RenderInfo, atlas: ComRef<'_, IRgbaImageAbi>, stream: ComRef<'_, IRenderStreamAbi>) -> Result<(), NcStatus> {
        let mut vertices = vec![RenderVertex::default(); info.vertex_count as usize];
        let mut batches = vec![RenderBatch::Atlas { first: 0, count: 3 }; info.batch_count as usize];
        let vertex_count = stream.vertices(info.invocation, NcCopyToBufferOffset::new(0, &mut vertices))?;
        let batch_count = stream.batches(info.invocation, NcCopyToBufferOffset::new(0, &mut batches))?;
        self.observation.vertices.set(vertex_count);
        self.observation.batches.set(batch_count);
        self.observation.calls.set(self.observation.calls.get() + 1);
        self.observation.atlas.set(atlas.as_raw().object as usize);
        Ok(())
    }
}

#[com_object(interfaces(IRenderer))]
/// Rejects an atlas upload without retaining resources or adding mutable fixture state.
struct RejectingRenderer;

impl IRenderer for RejectingRenderer {
    /// Models a backend allocation failure before the runtime publishes a candidate revision.
    fn prepare_atlas(&self, _atlas: ComRef<'_, IRgbaImageAbi>) -> Result<(), NcStatus> {
        Err(NC_E_OUT_OF_MEMORY)
    }

    /// No frame should reach this upload-only failure probe.
    fn render(&self, _info: RenderInfo, _atlas: ComRef<'_, IRgbaImageAbi>, _stream: ComRef<'_, IRenderStreamAbi>) -> Result<(), NcStatus> {
        panic!("failed upload must not submit a frame")
    }
}

/// Constructs and mounts an ordinary UI through the public production entrypoint.
fn create_ui(assets: ComRef<'_, IAssetBundleAbi>, gfx: ComRef<'_, GfxPluginFactoryAbi>) -> Ui {
    let runtime = nc_ui::create_runtime_from_assets(assets, gfx).unwrap();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime).unwrap();
    let text = ui.widgets().text_block("Independent UI", TextWrapping::None).unwrap();
    ui.window("Independent UI", Rect { x: 4, y: 4, width: 160, height: 80 }, &text).unwrap();
    ui.update(SIZE).unwrap();
    ui
}

/// Checks both the public resource snapshot and the exact image delivered in a nonempty frame.
fn assert_frame(ui: &Ui, assets: ComRef<'_, IAssetBundleAbi>, renderer: ComRef<'_, IRendererAbi>, observation: &Observation) {
    let image = assets.image().unwrap();
    let calls = observation.calls.get();
    // The original bug kept Ui::assets correct while silently substituting another image.
    assert_eq!(ui.assets().unwrap().as_raw().object, assets.as_raw().object);
    ui.render(SIZE, renderer).unwrap();
    assert_eq!(observation.calls.get(), calls + 1);
    assert_eq!(observation.atlas.get(), image.as_raw().object as usize);
    assert!(observation.vertices.get() > 0);
    assert!(observation.batches.get() > 0);
}

/// Publishes a distinct, catalog-compatible revision through the production GFX publisher.
fn appearance_revision(base: ComRef<'_, IAssetBundleAbi>) -> ComPtr<IAssetBundleAbi> {
    let data = neocogi_ui::assets::AssetData::read(base).expect("valid base resources");
    let publisher: ComPtr<AssetPublisherAbi> = neocogi_gfx_impl::AssetPublisherObject::from_value(Default::default()).query().unwrap();
    let mut metrics = data.tables.metrics;
    // Change an actual layout value so the revision is meaningful even if publication later
    // learns to preserve identity for an unchanged appearance.
    metrics.padding += 1;
    publisher
        .revise_appearance(
            base,
            AssetAppearanceRevision { metrics, chrome: data.tables.chrome },
            NcCopyFromBufferOffset::new(0, &data.tables.surfaces),
            NcCopyFromBufferOffset::new(0, &data.tables.controls),
            NcCopyFromBufferOffset::new(0, &data.tables.menus),
            NcCopyFromBufferOffset::new(0, &data.tables.window_chrome),
        )
        .expect("a padding edit must preserve catalog compatibility")
}

/// Construction, successful installation, failed upload, and closure affect only their own UI.
#[test]
fn shared_graphics_service_keeps_runtime_resources_independent() {
    let base = neocogi_demo_assets::load_bundle().unwrap();
    // Rebuild one image deliberately: this isolation regression must still distinguish
    // two atlas identities now that ordinary appearance edits preserve the same image.
    let styled = appearance_revision(base.as_com_ref());
    let revision = neocogi_ui::assets::AssetData::read(styled.as_com_ref())
        .unwrap()
        .into_revision(base.as_com_ref())
        .unwrap();
    let base_image = base.image().unwrap();
    let revision_image = revision.image().unwrap();
    assert_ne!(base_image.as_raw().object, revision_image.as_raw().object);
    let gfx = neocogi_ui_support::test_support::gfx_factory();
    let first = create_ui(base.as_com_ref(), gfx.as_com_ref());
    let mut second = create_ui(revision.as_com_ref(), gfx.as_com_ref());
    let observation = Rc::new(Observation::default());
    let renderer: ComPtr<IRendererAbi> = CaptureRenderer::from_value(CaptureRenderer { observation: observation.clone() })
        .query()
        .unwrap();

    // Alternate one renderer between both runtimes to expose stale shared resource state.
    assert_frame(&first, base.as_com_ref(), renderer.as_com_ref(), &observation);
    assert_frame(&second, revision.as_com_ref(), renderer.as_com_ref(), &observation);
    assert_frame(&first, base.as_com_ref(), renderer.as_com_ref(), &observation);

    let next = appearance_revision(revision.as_com_ref());
    let rejecting: ComPtr<IRendererAbi> = RejectingRenderer::from_value(RejectingRenderer).query().unwrap();
    assert_eq!(second.install_assets(next.as_com_ref(), rejecting.as_com_ref()), Err(NC_E_OUT_OF_MEMORY));
    assert_frame(&second, revision.as_com_ref(), renderer.as_com_ref(), &observation);
    assert_frame(&first, base.as_com_ref(), renderer.as_com_ref(), &observation);

    second.install_assets(next.as_com_ref(), renderer.as_com_ref()).unwrap();
    second.update(SIZE).unwrap();
    assert_frame(&second, next.as_com_ref(), renderer.as_com_ref(), &observation);
    assert_frame(&first, base.as_com_ref(), renderer.as_com_ref(), &observation);

    // Neither the factory nor a sibling UI needs to outlive the remaining runtime's session.
    second.close().unwrap();
    drop(second);
    drop(gfx);
    assert_frame(&first, base.as_com_ref(), renderer.as_com_ref(), &observation);
}

/// Sharing a factory permits unrelated catalogs, while an individual runtime still rejects
/// replacing its namespace with another UI's catalog.
#[test]
fn shared_factory_supports_independent_catalogs() {
    let base = neocogi_demo_assets::load_bundle().unwrap();
    let unrelated = neocogi_demo_assets::load_bundle().unwrap();
    assert_ne!(base.catalog().unwrap().as_raw().object, unrelated.catalog().unwrap().as_raw().object);
    let gfx = neocogi_ui_support::test_support::gfx_factory();
    let mut first = create_ui(base.as_com_ref(), gfx.as_com_ref());
    let second = create_ui(unrelated.as_com_ref(), gfx.as_com_ref());
    let observation = Rc::new(Observation::default());
    let renderer: ComPtr<IRendererAbi> = CaptureRenderer::from_value(CaptureRenderer { observation: observation.clone() })
        .query()
        .unwrap();

    assert_frame(&first, base.as_com_ref(), renderer.as_com_ref(), &observation);
    assert_frame(&second, unrelated.as_com_ref(), renderer.as_com_ref(), &observation);
    assert_eq!(first.install_assets(unrelated.as_com_ref(), renderer.as_com_ref()), Err(UI_E_OWNER));
    assert_frame(&first, base.as_com_ref(), renderer.as_com_ref(), &observation);
    assert_frame(&second, unrelated.as_com_ref(), renderer.as_com_ref(), &observation);
}

/// Reusing the same bundle must still allocate independent frame state for every factory call.
#[test]
fn standalone_compositor_frame_does_not_block_runtime_construction_or_rendering() {
    let assets = neocogi_demo_assets::load_bundle().unwrap();
    let gfx = neocogi_ui_support::test_support::gfx_factory();
    let mut standalone = gfx.create_compositor(assets.as_com_ref()).unwrap();
    standalone.begin(SIZE, 0).unwrap();
    // An active standalone frame must remain active while the UI creates and submits its own.
    let ui = create_ui(assets.as_com_ref(), gfx.as_com_ref());
    let observation = Rc::new(Observation::default());
    let renderer: ComPtr<IRendererAbi> = CaptureRenderer::from_value(CaptureRenderer { observation: observation.clone() })
        .query()
        .unwrap();
    assert_frame(&ui, assets.as_com_ref(), renderer.as_com_ref(), &observation);
    standalone.submit(renderer.as_com_ref()).unwrap();
    assert_eq!(observation.calls.get(), 2);
    assert_frame(&ui, assets.as_com_ref(), renderer.as_com_ref(), &observation);
}

/// A production runtime must delegate final expansion and submit exactly one bulk renderer frame.
#[test]
fn production_runtime_uses_the_gfx_compositor() {
    let assets = neocogi_demo_assets::load_bundle().expect("embedded test assets must load");
    let gfx = neocogi_ui_support::test_support::gfx_factory();
    let runtime =
        nc_ui::create_runtime_from_assets(assets.as_com_ref(), gfx.as_com_ref()).expect("matching UI and GFX resources must create a runtime");
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime).expect("the object must expose every UI capability");
    let text = ui
        .widgets()
        .text_block("GFX boundary", TextWrapping::None)
        .expect("the built-in text block must construct");
    ui.window("Boundary", Rect { x: 4, y: 4, width: 160, height: 80 }, &text)
        .expect("the text block must mount in one window");
    let size = Size { width: 200, height: 120 };
    ui.update(size).expect("the retained tree must commit before rendering");

    let observation = Rc::new(Observation::default());
    let renderer: ComPtr<IRendererAbi> = CaptureRenderer::from_value(CaptureRenderer { observation: observation.clone() })
        .query()
        .expect("the mock declares IRenderer");
    let result = ui.render(size, renderer.as_com_ref());
    assert_eq!(result, Ok(()), "UI, GFX, and renderer must complete one frame");

    assert_eq!(observation.calls.get(), 1);
    assert!(observation.vertices.get() > 0);
    assert!(observation.batches.get() > 0);
}
