//! Real GPU regressions for independent atlases and immutable surface snapshots.
use super::*;

#[com_object(interfaces(IRgbaImage))]
/// Asymmetric source pixels expose texture orientation errors.
struct Pixels {
    /// Two-by-two tightly packed RGBA pixels.
    bytes: [u8; 16],
    /// Shared observation crosses the immutable COM `&self` boundary only in this test.
    /// It measures actual pixel reads rather than inferring uploads from frame counts.
    reads: Rc<std::cell::Cell<u32>>,
}
impl IRgbaImage for Pixels {
    /// Returns the exact extent represented by the fixed source buffer.
    fn size(&self) -> Result<Size, NcStatus> {
        Ok(Size { width: 2, height: 2 })
    }
    /// Supplies only the requested bounded byte range.
    fn pixels(&self, mut output: NcOutputWriter<'_, u8>) -> Result<(), NcStatus> {
        self.reads.set(self.reads.get() + 1);
        let start = output.offset() as usize;
        let end = start + output.capacity();
        output.extend_from_slice(self.bytes.get(start..end).ok_or(NC_E_INVALID_ARG)?)?;
        Ok(())
    }
}

#[com_object(interfaces(IRenderStream))]
/// Owned UI stream for both offscreen submission and native composition.
struct SurfaceStream {
    /// Final copied geometry in physical pixels.
    vertices: Vec<RenderVertex>,
    /// Complete painter ordering.
    batches: Vec<RenderBatch>,
    /// Sampled images indexed by overall batch number.
    images: Vec<ComPtr<ITextureAbi>>,
}
impl IRenderStream for SurfaceStream {
    /// Copies the complete geometry requested by the production preparer.
    fn vertices(&self, _: u64, mut output: NcOutputWriter<'_, RenderVertex>) -> Result<u32, NcStatus> {
        output.extend_from_slice(&self.vertices)?;
        Ok(self.vertices.len() as u32)
    }
    /// Copies the complete ordered batch list.
    fn batches(&self, _: u64, mut output: NcOutputWriter<'_, RenderBatch>) -> Result<u32, NcStatus> {
        output.extend_from_slice(&self.batches)?;
        Ok(self.batches.len() as u32)
    }
    /// Retains the texture selected by this exact batch.
    fn texture(&self, _: u64, batch: u32) -> Result<ComPtr<ITextureAbi>, NcStatus> {
        self.images.get(batch as usize).cloned().ok_or(NC_E_INVALID_ARG)
    }
    /// UI-only fixtures contain no application commands.
    fn item(&self, _: u64, _: u32) -> Result<ComPtr<IRenderItemAbi>, NcStatus> {
        Err(NC_E_INVALID_ARG)
    }
}
impl SurfaceStream {
    /// Publishes counts and the corresponding stream together.
    fn publish(self, size: Size, clear: u32) -> (RenderInfo, ComPtr<IRenderStreamAbi>) {
        let info = RenderInfo {
            size,
            clear,
            invocation: 1,
            vertex_count: self.vertices.len() as u32,
            batch_count: self.batches.len() as u32,
        };
        (info, Self::from_value(self).query().unwrap())
    }
}

/// Converts logical top-left coordinates into GL window readback indexing.
fn pixel(pixels: &[u8], x: usize, y: usize) -> &[u8] {
    &pixels[((SIZE.height as usize - 1 - y) * SIZE.width as usize + x) * 4..][..4]
}

/// Retained frames survive later rendering and surface release without changing pixels.
#[test]
#[ignore = "requires SDL GL; run with SDL_VIDEODRIVER=offscreen"]
fn independent_surface_snapshots_keep_orientation_and_device_lifetime() {
    let platform = PlatformObject::default();
    let assets = neocogi_demo_assets::load_bundle().unwrap();
    let atlas = assets.image().unwrap();
    let native = platform.window("surface acceptance", SIZE, atlas.as_com_ref()).unwrap();
    let weak = Rc::downgrade(&native.device);
    let window = WindowObject::from_value(native);
    let renderer: ComPtr<IRendererAbi> = window.query().unwrap();
    let factory: ComPtr<IRenderSurfaceFactoryAbi> = window.query().unwrap();
    let reads = Rc::new(std::cell::Cell::new(0));
    let preview_atlas: ComPtr<IRgbaImageAbi> = Pixels::from_value(Pixels {
        reads: reads.clone(),
        bytes: [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255],
    })
    .query()
    .unwrap();
    let extent = Size { width: 32, height: 32 };
    let surface = factory.create_surface(extent, preview_atlas.as_com_ref()).unwrap();
    let preview: ComPtr<IRendererAbi> = surface.query().unwrap();
    assert_eq!(reads.get(), 1, "surface creation performs one initial atlas upload");
    preview.prepare_atlas(preview_atlas.as_com_ref()).unwrap();
    assert_eq!(reads.get(), 1, "a metadata-only install retaining the image performs no upload");
    assert_eq!(surface.completed_image().unwrap_err(), neocogi_com::NC_E_NOT_FOUND);
    let vertices = quad(Rect { x: 0, y: 0, width: 32, height: 32 });
    let (first_info, first_stream) = SurfaceStream {
        vertices: vec![vertices[0], vertices[1], vertices[2], vertices[0], vertices[2], vertices[3]],
        batches: vec![RenderBatch::Atlas { first: 0, count: 6 }],
        images: vec![],
    }
    .publish(extent, 0xff000000);
    preview.render(first_info, preview_atlas.as_com_ref(), first_stream.as_com_ref()).unwrap();
    let old = surface.completed_image().unwrap();
    let (second_info, empty) = SurfaceStream {
        vertices: vec![],
        batches: vec![],
        images: vec![],
    }
    .publish(extent, 0xffff00ff);
    preview.render(second_info, preview_atlas.as_com_ref(), empty.as_com_ref()).unwrap();
    let new = surface.completed_image().unwrap();
    assert_ne!(identity(&old).unwrap(), identity(&new).unwrap());
    assert_eq!(old.size().unwrap(), extent);
    // This portable-size image exceeds the device's texture-axis limit. Failed
    // native allocation must preserve the last successfully completed snapshot.
    let oversized = Size { width: 131072, height: 1 };
    surface.resize(oversized).unwrap();
    assert_eq!(
        preview.render(RenderInfo { size: oversized, ..second_info }, preview_atlas.as_com_ref(), empty.as_com_ref()),
        Err(RENDER_E_BACKEND)
    );
    assert_eq!(identity(&surface.completed_image().unwrap()).unwrap(), identity(&new).unwrap());
    surface.resize(extent).unwrap();
    assert_eq!(surface.resize(Size { width: -1, height: 32 }), Err(NC_E_INVALID_ARG));
    assert_eq!(surface.size().unwrap(), extent);
    surface.resize(Size { width: 0, height: 0 }).unwrap();
    assert_eq!(
        preview.render(second_info, preview_atlas.as_com_ref(), empty.as_com_ref()),
        Err(NC_E_INVALID_ARG)
    );
    assert_eq!(identity(&surface.completed_image().unwrap()).unwrap(), identity(&new).unwrap());
    surface.resize(Size { width: 16, height: 16 }).unwrap();
    preview
        .render(
            RenderInfo {
                size: Size { width: 16, height: 16 },
                ..second_info
            },
            preview_atlas.as_com_ref(),
            empty.as_com_ref(),
        )
        .unwrap();
    assert_eq!(old.size().unwrap(), extent);
    assert_eq!(new.size().unwrap(), extent);
    assert_eq!(reads.get(), 1, "rendering, failures, and resizing never re-read an unchanged atlas");
    // The two old outputs remain valid after destroying their rendering session.
    drop(preview);
    drop(surface);
    let (window_info, stream) = SurfaceStream {
        vertices: quad(Rect { x: 0, y: 0, width: 32, height: 32 })
            .into_iter()
            .chain(quad(Rect { x: 32, y: 0, width: 32, height: 32 }))
            .collect(),
        batches: vec![RenderBatch::Texture { first: 0 }, RenderBatch::Texture { first: 4 }],
        images: vec![old.clone(), new.clone()],
    }
    .publish(SIZE, 0xff333333);
    renderer.render(window_info, atlas.as_com_ref(), stream.as_com_ref()).unwrap();
    {
        let owner = weak.upgrade().unwrap();
        let mut access = owner.access().unwrap();
        assert_eq!(access.state().atlas.as_raw().object, atlas.as_raw().object, "surface upload must preserve the window atlas");
        let pixels = access.state().gpu.last_pixels();
        assert_eq!(pixel(pixels, 4, 4), [255, 0, 0, 255]);
        assert_eq!(pixel(pixels, 24, 4), [0, 255, 0, 255]);
        assert_eq!(pixel(pixels, 4, 24), [0, 0, 255, 255]);
        assert_eq!(pixel(pixels, 24, 24), [255, 255, 0, 255]);
        assert_eq!(pixel(pixels, 40, 4), [255, 0, 255, 255]);
        assert_eq!(pixel(pixels, 40, 40), [51, 51, 51, 255]);
    }
    drop(stream);
    drop(renderer);
    drop(factory);
    drop(window);
    assert!(weak.upgrade().is_some(), "retained textures keep their device alive");
    drop(old);
    drop(new);
    assert!(weak.upgrade().is_none(), "surface sessions must not create a device ownership cycle");
}
