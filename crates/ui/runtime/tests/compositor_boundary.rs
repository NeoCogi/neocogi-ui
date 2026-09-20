//! End-to-end coverage for the deployable UI-to-GFX-to-renderer frame path.

use neocogi_com::{ComPtr, ComRef, INcStringComPtrExt, NcCopyFromBufferOffset, NcCopyToBufferOffset, NcStatus, com_object};
use neocogi_gfx_contracts::{AssetAppearanceRevision, AssetPublisherAbi, IAssetBundleAbi};
use neocogi_gfx_impl::GfxCompositorObject;
use neocogi_renderer_contracts::{IRenderStreamAbi, IRenderer, IRendererAbi, IRendererInterfacesFor, IRgbaImageAbi, RenderBatch, RenderInfo, RenderVertex};
use neocogi_ui::{Rect, Size, TextWrapping};
use std::{cell::Cell, rc::Rc};

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
    /// This test starts with an already prepared logical atlas and performs no installation.
    fn prepare_atlas(&self, _atlas: ComRef<'_, IRgbaImageAbi>) -> Result<(), NcStatus> {
        Ok(())
    }

    /// Copies both bounded arrays while the invocation token is valid.
    fn render(&self, info: RenderInfo, atlas: ComRef<'_, IRgbaImageAbi>, stream: ComRef<'_, IRenderStreamAbi>) -> Result<(), NcStatus> {
        let mut vertices = vec![RenderVertex::default(); info.vertex_count as usize];
        let mut batches = vec![RenderBatch::Atlas { first: 0, count: 3 }.into(); info.batch_count as usize];
        let vertex_count = stream.vertices(info.invocation, NcCopyToBufferOffset::new(0, &mut vertices))?;
        let batch_count = stream.batches(info.invocation, NcCopyToBufferOffset::new(0, &mut batches))?;
        self.observation.vertices.set(vertex_count);
        self.observation.batches.set(batch_count);
        self.observation.calls.set(self.observation.calls.get() + 1);
        self.observation.atlas.set(atlas.as_ptr() as usize);
        Ok(())
    }
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

/// Two UIs may use one graphics service, but constructing the second must not replace the
/// first UI's renderer resources behind its retained asset snapshot.
#[test]
fn shared_graphics_service_keeps_runtime_resources_independent() {
    let base = neocogi_demo_assets::load_bundle().unwrap();
    let revision = appearance_revision(base.as_com_ref());
    let base_image = base.query::<IRgbaImageAbi>().unwrap();
    let revision_image = revision.query::<IRgbaImageAbi>().unwrap();
    assert_ne!(base_image.as_ptr(), revision_image.as_ptr());
    let compositor = GfxCompositorObject::create(base.as_com_ref()).unwrap();
    let first = neocogi_ui_runtime::create_runtime_from_assets(base.as_com_ref(), compositor.as_com_ref()).unwrap();
    let _second = neocogi_ui_runtime::create_runtime_from_assets(revision.as_com_ref(), compositor.as_com_ref()).unwrap();
    let ui = neocogi_ui_nodes::bind_builtin_nodes(first).unwrap();
    let text = ui.widgets().text_block("First UI", TextWrapping::None).unwrap();
    ui.window("First UI", Rect { x: 4, y: 4, width: 160, height: 80 }, &text).unwrap();
    let size = Size { width: 200, height: 120 };
    ui.update(size).unwrap();
    let observation = Rc::new(Observation::default());
    let renderer: ComPtr<IRendererAbi> = CaptureRenderer::from_value(CaptureRenderer { observation: observation.clone() })
        .query()
        .unwrap();

    // Inspect the renderer boundary as well as the UI snapshot: the original bug left
    // Ui::assets correct while silently substituting the second revision during submit.
    assert_eq!(ui.assets().unwrap().as_ptr(), base.as_ptr());
    ui.render(size, renderer.as_com_ref()).unwrap();
    assert_eq!(observation.atlas.get(), base_image.as_ptr() as usize);
}

/// A production runtime must delegate final expansion and submit exactly one bulk renderer frame.
#[test]
fn production_runtime_uses_the_gfx_compositor() {
    let assets = neocogi_demo_assets::load_bundle().expect("embedded test assets must load");
    let compositor = GfxCompositorObject::create(assets.as_com_ref()).expect("valid assets must create a compositor");
    let runtime = neocogi_ui_runtime::create_runtime_from_assets(assets.as_com_ref(), compositor.as_com_ref())
        .expect("matching UI and GFX resources must create a runtime");
    let ui = neocogi_ui_nodes::bind_builtin_nodes(runtime).expect("the object must expose every UI capability");
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
    assert_eq!(
        result,
        Ok(()),
        "UI, GFX, and renderer must complete one frame: {}",
        compositor.diagnostic().expect("GFX must expose the underlying failure").to_string().unwrap()
    );

    assert_eq!(observation.calls.get(), 1);
    assert!(observation.vertices.get() > 0);
    assert!(observation.batches.get() > 0);
}
