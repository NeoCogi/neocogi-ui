//! End-to-end coverage for the deployable UI-to-GFX-to-renderer frame path.

use neocogi_com::{ComPtr, ComRef, NcCopyFromBufferOffset, NcStatus, NC_E_OUT_OF_MEMORY, com_object};
use neocogi_gfx_support::abi::{AssetAppearanceRevision, AssetPublisherAbi, GfxPluginFactoryAbi, IAssetBundleAbi};
use neocogi_platform_contracts::render::abi::{IRenderStreamAbi, IRenderer, IRendererAbi, IRendererVtblFor, RenderInfo};
use neocogi_ui::{Rect, Size, TextWrapping, Ui, UI_E_OWNER};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Identical drawable dimensions keep alternating frames comparable across independent UIs.
const SIZE: Size = Size { width: 200, height: 120 };

/// Maintenance runs through the real UI/GFX boundary before client camera uploads.
mod maintenance {
    use super::*;
    use neocogi_com::ComBox;
    use neocogi_platform_contracts::render::abi::*;
    use neocogi_ui::{GeometryChange, ITextMetricsAbi, LayoutLimits, LeafContent, Painter, WidgetEvent, WidgetOptions, WidgetPaint, WidgetUpdate};

    /// Shared observations bridge generated COM callbacks and the independently owned test caller.
    #[derive(Default)]
    struct Probe {
        /// Native host methods use shared receivers and cannot borrow the caller's owned counters.
        maintained: Cell<u32>,
        /// Proves a failed retirement never invokes widget paint.
        painted: Cell<u32>,
        /// Arms a recoverable owned native error after binding has succeeded.
        fail: Cell<bool>,
    }

    #[com_object(interfaces(IRenderHost, IRenderClientFactory))]
    /// Delegate authentic resource creation while observing maintenance admission.
    struct Host {
        /// Sole resource authority remains the canonical logical fixture device.
        inner: ComPtr<IRenderHostAbi>,
        /// Contains no UI or widget reference, preventing a capability ownership cycle.
        probe: Rc<Probe>,
    }
    impl IRenderDriverBound for Host {
        fn driver_identity(&self) -> Result<ComPtr<IRenderDriverIdentityAbi>, NcStatus> {
            self.inner.driver_identity()
        }
    }
    impl IRenderHost for Host {
        /// Failure returns the same owned report used by actual native retirement operations.
        fn maintain_result(&self) -> Result<ComPtr<IRenderOutcomeAbi>, NcStatus> {
            self.probe.maintained.set(self.probe.maintained.get() + 1);
            if self.probe.fail.get() {
                return Ok(operation_result(Err(RenderError::new(RENDER_E_BACKEND, "camera retirement failed: é"))));
            }
            self.inner.maintain_result()
        }
        fn import_texture_result(
            &self,
            resources: ComRef<'_, IRenderResourcesAbi>,
            image: ComRef<'_, ITextureAbi>,
        ) -> Result<ComBox<ITextureImportResultAbi>, NcStatus> {
            self.inner.import_texture_result(resources, image)
        }
        fn prepare_texture_result(
            &self,
            resources: ComRef<'_, IRenderResourcesAbi>,
            texture: ComRef<'_, IGpuBufferAbi>,
        ) -> Result<ComPtr<IRenderOutcomeAbi>, NcStatus> {
            self.inner.prepare_texture_result(resources, texture)
        }
        fn recording_broker(&self) -> Result<ComPtr<IRenderViewBrokerAbi>, NcStatus> {
            self.inner.recording_broker()
        }
    }
    impl IRenderClientFactory for Host {
        fn create_client(
            &self,
            limits: RenderClientLimits,
            surface: ComRef<'_, IRenderSurfaceAbi>,
            permissions: RenderClientPermissions,
        ) -> Result<ComBox<IRenderClientDomainAbi>, NcStatus> {
            self.inner.query::<IRenderClientFactoryAbi>()?.create_client(limits, surface, permissions)
        }
    }

    /// Runtime-exclusive leaf observes maintenance ordering at the actual widget paint boundary.
    struct Content(Rc<Probe>);
    impl LeafContent for Content {
        fn measure(&self, _: LayoutLimits, _: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
            Ok(Size { width: 80, height: 24 })
        }
        fn update(&mut self, _: WidgetUpdate, _: WidgetEvent, _: &str) -> Result<GeometryChange, NcStatus> {
            Ok(GeometryChange::NoChange)
        }
        fn paint(&mut self, state: WidgetPaint, painter: &mut Painter<'_>) -> Result<(), NcStatus> {
            assert!(self.0.maintained.get() > 0, "retirement must precede widget uploads, not wait for GFX replay");
            self.0.painted.set(self.0.painted.get() + 1);
            painter.fill(state.clip, 0xffffffff)
        }
    }

    /// A maintenance failure stops before paint, owns its original error and permits a later retry.
    #[test]
    fn maintenance_precedes_widget_paint_and_preserves_owned_errors() {
        let assets = neocogi_ui_support::test_support::test_asset_data_with_font_sizes(&[("body", 12)])
            .into_bundle()
            .unwrap();
        let gfx = neocogi_ui_support::test_support::gfx_factory(super::host_runtime());
        let probe = Rc::new(Probe::default());
        let host: ComPtr<IRenderHostAbi> = Host::from_value(Host {
            inner: neocogi_gfx_support::testing::host(),
            probe: probe.clone(),
        })
        .query()
        .unwrap();
        let mut runtime = nc_ui::create_runtime_from_assets(assets.as_com_ref(), gfx.as_com_ref()).unwrap();
        runtime
            .set_render_host(host.as_com_ref(), neocogi_gfx_support::testing::framebuffer().as_com_ref())
            .unwrap();
        let factory = nc_ui_nodes::UiFactory::bind(runtime.as_com_ref()).unwrap();
        let ui = Ui::from_interfaces(runtime, factory).unwrap();
        let root = ui.widgets().leaf(Content(probe.clone()), WidgetOptions::NONE).unwrap();
        let _window = ui.window("camera uploads", Rect { x: 0, y: 0, width: 180, height: 100 }, root).unwrap();
        ui.update(SIZE).unwrap();
        let observation = Rc::new(Observation::default());
        let renderer: ComPtr<IRendererAbi> = CaptureRenderer::from_value(CaptureRenderer { observation: observation.clone() })
            .query()
            .unwrap();
        ui.render(SIZE, renderer.as_com_ref()).unwrap();
        assert_eq!(probe.painted.get(), 1);
        assert_eq!(observation.calls.get(), 1);
        probe.fail.set(true);
        let failure = ui.render(SIZE, renderer.as_com_ref()).unwrap_err();
        assert_eq!(failure.status, RENDER_E_BACKEND);
        assert_eq!(failure.message, "camera retirement failed: é");
        assert_eq!(probe.painted.get(), 1);
        assert_eq!(observation.calls.get(), 1);
        probe.fail.set(false);
        ui.render(SIZE, renderer.as_com_ref()).unwrap();
        assert_eq!(probe.painted.get(), 2);
        assert_eq!(observation.calls.get(), 2);
        assert_eq!(failure.message, "camera retirement failed: é");
    }
}

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
    pixels: RefCell<Vec<u8>>,
    /// Recorded atlas shape is checked independently of the exposed UI asset metadata.
    size: Cell<Option<Size>>,
}

#[com_object(interfaces(IRenderer))]
/// Minimal renderer that consumes the complete stream without owning a native graphics backend.
struct CaptureRenderer {
    /// Shared assertions remain available after COM object publication and final release.
    observation: Rc<Observation>,
}

impl neocogi_platform_contracts::render::abi::IRenderDriverBound for CaptureRenderer {
    fn driver_identity(&self) -> Result<neocogi_com::ComPtr<neocogi_platform_contracts::render::abi::IRenderDriverIdentityAbi>, neocogi_com::NcStatus> {
        Err(neocogi_com::NC_E_NOINTERFACE)
    }
}
impl IRenderer for CaptureRenderer {
    /// Copies both bounded arrays while the invocation token is valid.
    fn render_result(
        &self,
        info: RenderInfo,
        stream: ComRef<'_, IRenderStreamAbi>,
    ) -> Result<ComPtr<neocogi_platform_contracts::render::abi::IRenderOutcomeAbi>, NcStatus> {
        let result = (|| -> Result<(), NcStatus> {
            let (vertices, batches) = neocogi_gfx_support::testing::read(info, stream)?;
            self.observation.vertices.set(vertices.len() as u32);
            self.observation.batches.set(batches.len() as u32);
            self.observation.calls.set(self.observation.calls.get() + 1);
            let blocks = neocogi_gfx_support::testing::blocks(info, stream)?;
            let atlas = blocks.iter().flat_map(|block| block.commands()).find_map(|command| {
                if let neocogi_platform::gpu::command::Command::Draw(draw) = command {
                    Some(draw.textures[0].texture.clone())
                } else {
                    None
                }
            });
            if let Some(atlas) = atlas {
                self.observation.size.set(Some(atlas.desc().size));
                *self.observation.pixels.borrow_mut() = atlas.pixels().unwrap().as_slice().to_vec();
            }
            Ok(())
        })();
        Ok(neocogi_platform_contracts::render::abi::operation_result(
            result.map_err(neocogi_platform_contracts::render::abi::RenderError::from),
        ))
    }
}

/// Constructs and mounts an ordinary UI through the public production entrypoint.
fn create_ui(assets: ComRef<'_, IAssetBundleAbi>, gfx: ComRef<'_, GfxPluginFactoryAbi>) -> Ui {
    create_ui_with_failure(assets, gfx, Rc::new(Cell::new(None)))
}
/// Failure injection belongs to private native preparation rather than the final renderer sink.
fn create_ui_with_failure(assets: ComRef<'_, IAssetBundleAbi>, gfx: ComRef<'_, GfxPluginFactoryAbi>, failure: Rc<Cell<Option<NcStatus>>>) -> Ui {
    let mut runtime = nc_ui::create_runtime_from_assets(assets, gfx).unwrap();
    runtime
        .set_render_host(
            neocogi_gfx_support::testing::host_with_preparation_failure(failure).as_com_ref(),
            neocogi_gfx_support::testing::framebuffer().as_com_ref(),
        )
        .unwrap();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime).unwrap();
    let text = ui.widgets().text_block("Independent UI", TextWrapping::None).unwrap();
    ui.window("Independent UI", Rect { x: 4, y: 4, width: 160, height: 80 }, text).unwrap();
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
    let size = image.size().unwrap();
    let len = neocogi_platform_contracts::render::abi::rgba_bytes(size).unwrap();
    let mut pixels = neocogi_com::NcOutputBuffer::new();
    image.pixels(pixels.output(0, len)).unwrap();
    assert_eq!(observation.size.get(), Some(size));
    assert_eq!(observation.pixels.borrow().as_slice(), pixels.as_slice());
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
    let base = neocogi_ui_support::test_support::test_asset_data_with_font_sizes(&[("body", 12)])
        .into_bundle()
        .unwrap();
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
    let gfx = neocogi_ui_support::test_support::gfx_factory(host_runtime());
    let first = create_ui(base.as_com_ref(), gfx.as_com_ref());
    let failure = Rc::new(Cell::new(None));
    let mut second = create_ui_with_failure(revision.as_com_ref(), gfx.as_com_ref(), failure.clone());
    let observation = Rc::new(Observation::default());
    let renderer: ComPtr<IRendererAbi> = CaptureRenderer::from_value(CaptureRenderer { observation: observation.clone() })
        .query()
        .unwrap();

    // Alternate one renderer between both runtimes to expose stale shared resource state.
    assert_frame(&first, base.as_com_ref(), renderer.as_com_ref(), &observation);
    assert_frame(&second, revision.as_com_ref(), renderer.as_com_ref(), &observation);
    assert_frame(&first, base.as_com_ref(), renderer.as_com_ref(), &observation);

    let styled = appearance_revision(revision.as_com_ref());
    let next = neocogi_ui::assets::AssetData::read(styled.as_com_ref())
        .unwrap()
        .into_revision(revision.as_com_ref())
        .unwrap();
    failure.set(Some(NC_E_OUT_OF_MEMORY));
    assert_eq!(second.install_assets(next.as_com_ref()).map_err(|error| error.status), Err(NC_E_OUT_OF_MEMORY));
    assert_frame(&second, revision.as_com_ref(), renderer.as_com_ref(), &observation);
    assert_frame(&first, base.as_com_ref(), renderer.as_com_ref(), &observation);

    failure.set(None);
    second.install_assets(next.as_com_ref()).unwrap();
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
    let base = neocogi_ui_support::test_support::test_asset_data_with_font_sizes(&[("body", 12)])
        .into_bundle()
        .unwrap();
    let unrelated = neocogi_ui_support::test_support::test_asset_data_with_font_sizes(&[("body", 12)])
        .into_bundle()
        .unwrap();
    assert_ne!(base.catalog().unwrap().as_raw().object, unrelated.catalog().unwrap().as_raw().object);
    let gfx = neocogi_ui_support::test_support::gfx_factory(host_runtime());
    let mut first = create_ui(base.as_com_ref(), gfx.as_com_ref());
    let second = create_ui(unrelated.as_com_ref(), gfx.as_com_ref());
    let observation = Rc::new(Observation::default());
    let renderer: ComPtr<IRendererAbi> = CaptureRenderer::from_value(CaptureRenderer { observation: observation.clone() })
        .query()
        .unwrap();

    assert_frame(&first, base.as_com_ref(), renderer.as_com_ref(), &observation);
    assert_frame(&second, unrelated.as_com_ref(), renderer.as_com_ref(), &observation);
    assert_eq!(first.install_assets(unrelated.as_com_ref()).map_err(|error| error.status), Err(UI_E_OWNER));
    assert_frame(&first, base.as_com_ref(), renderer.as_com_ref(), &observation);
    assert_frame(&second, unrelated.as_com_ref(), renderer.as_com_ref(), &observation);
}

/// Reusing the same bundle must still allocate independent frame state for every factory call.
#[test]
fn standalone_compositor_frame_does_not_block_runtime_construction_or_rendering() {
    let assets = neocogi_ui_support::test_support::test_asset_data_with_font_sizes(&[("body", 12)])
        .into_bundle()
        .unwrap();
    let gfx = neocogi_ui_support::test_support::gfx_factory(host_runtime());
    let mut standalone = gfx.create_compositor(assets.as_com_ref()).unwrap();
    standalone
        .bind_render_host(
            neocogi_gfx_support::testing::host().as_com_ref(),
            neocogi_gfx_support::testing::framebuffer().as_com_ref(),
        )
        .unwrap();
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
    let assets = neocogi_ui_support::test_support::test_asset_data_with_font_sizes(&[("body", 12)])
        .into_bundle()
        .expect("valid complete fixture assets");
    let gfx = neocogi_ui_support::test_support::gfx_factory(host_runtime());
    let mut runtime = nc_ui::create_runtime_from_assets(assets.as_com_ref(), gfx.as_com_ref()).expect("matching UI and GFX resources must create a runtime");
    runtime
        .set_render_host(
            neocogi_gfx_support::testing::host().as_com_ref(),
            neocogi_gfx_support::testing::framebuffer().as_com_ref(),
        )
        .unwrap();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime).expect("the object must expose every UI capability");
    let text = ui
        .widgets()
        .text_block("GFX boundary", TextWrapping::None)
        .expect("the built-in text block must construct");
    ui.window("Boundary", Rect { x: 4, y: 4, width: 160, height: 80 }, text)
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

// This final integration-test executable owns one embedded runtime.
extern crate neocogi_com_host as _;

/// Initializes the selected implementation owned by this test executable.
#[cfg(test)]
fn host_runtime() -> neocogi_com::Runtime {
    // SAFETY: this explicit host owns the complete immutable process-lived graph.
    unsafe { neocogi_com::Runtime::__from_host_api(neocogi_com_host::initialize_process()) }.expect("compatible test host")
}

#[com_object(interfaces(IRenderer))]
/// A real COM renderer publishes a different owned native failure for each fixture instance.
struct ReportedFailure {
    /// Immutable operation data is copied into the result rather than borrowed by the UI.
    error: neocogi_platform_contracts::render::abi::RenderError,
    /// Proves that returned failures do not retain the renderer or its device facade.
    dropped: Rc<Cell<bool>>,
}
impl neocogi_platform_contracts::render::abi::IRenderDriverBound for ReportedFailure {
    /// The fixture consumes authentic CPU blocks without publishing native resources.
    fn driver_identity(&self) -> Result<ComPtr<neocogi_platform_contracts::render::abi::IRenderDriverIdentityAbi>, NcStatus> {
        Err(neocogi_com::NC_E_NOINTERFACE)
    }
}
impl IRenderer for ReportedFailure {
    /// Validate the production stream before publishing this invocation's independent failure.
    fn render_result(
        &self,
        info: RenderInfo,
        stream: ComRef<'_, IRenderStreamAbi>,
    ) -> Result<ComPtr<neocogi_platform_contracts::render::abi::IRenderOutcomeAbi>, NcStatus> {
        let blocks = neocogi_gfx_support::testing::blocks(info, stream)?;
        assert!(blocks.iter().any(|block| !block.commands().is_empty()));
        Ok(neocogi_platform_contracts::render::abi::operation_result(Err(self.error.clone())))
    }
}
impl Drop for ReportedFailure {
    /// Observation retains no COM owner and can be checked after the native-shaped boundary is gone.
    fn drop(&mut self) {
        self.dropped.set(true);
    }
}

/// Native -> GFX -> UI must preserve exact status/text through repeated failures and successful retry.
#[test]
fn operation_errors_survive_renderers_later_failures_and_successful_retry() {
    use neocogi_platform_contracts::render::abi::{RenderError, RENDER_E_BACKEND};
    let assets = neocogi_ui_support::test_support::test_asset_data_with_font_sizes(&[("body", 12)])
        .into_bundle()
        .unwrap();
    let gfx = neocogi_ui_support::test_support::gfx_factory(host_runtime());
    let ui = create_ui(assets.as_com_ref(), gfx.as_com_ref());
    let mut errors = Vec::new();
    for (status, message) in [
        (RENDER_E_BACKEND, "original GPU submission failed: é"),
        (NC_E_OUT_OF_MEMORY, "a later resource retirement failed"),
    ] {
        let dropped = Rc::new(Cell::new(false));
        let renderer: ComPtr<IRendererAbi> = ReportedFailure::from_value(ReportedFailure {
            error: RenderError::new(status, message),
            dropped: dropped.clone(),
        })
        .query()
        .unwrap();
        let error = ui.render(SIZE, renderer.as_com_ref()).unwrap_err();
        drop(renderer);
        assert!(dropped.get(), "an operation error cannot retain its renderer");
        assert_eq!(error.status, status);
        assert_eq!(error.message, message);
        errors.push(error);
    }
    let observation = Rc::new(Observation::default());
    let renderer: ComPtr<IRendererAbi> = CaptureRenderer::from_value(CaptureRenderer { observation: observation.clone() })
        .query()
        .unwrap();
    assert_frame(&ui, assets.as_com_ref(), renderer.as_com_ref(), &observation);
    drop((renderer, ui, gfx, assets));
    assert_eq!(errors[0].status, RENDER_E_BACKEND);
    assert_eq!(errors[0].message, "original GPU submission failed: é");
    assert_eq!(errors[1].status, NC_E_OUT_OF_MEMORY);
    assert_eq!(errors[1].message, "a later resource retirement failed");
}
