//! Real CPU allocation failures cross generated texture calls and the production frame importer.
use neocogi_com::{ComPtr, ComRef, NcCopyFromBufferOffset, NcOutputBuffer, NcStatus, NC_E_BUSY, NC_E_INVALID_ARG, NC_E_OUT_OF_MEMORY, com_object};
use neocogi_gfx_support::abi::{GfxCompositorAbi, SolidVertex};
use neocogi_gfx_impl::GfxCompositorObject;
use neocogi_render_support::com::{Batch, PreparedFrame, Resources};
use neocogi_render_core::abi::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

// This GFX regression owns its allocation-free, thread-local fault injector.
#[path = "support/allocation_probe.rs"]
mod allocation_probe;
// The shared fixture includes a table-copy helper used only by other test executables.
// This test needs its immutable bundle, with no optional builder, filesystem, or GPU.
#[allow(dead_code)]
#[path = "../src/test_support.rs"]
mod fixture;

#[com_object(interfaces(ITexture))]
/// An existing renderer capability whose final COM release is independently observable.
struct Texture {
    /// No test observer owns the texture; only this scalar counter is shared.
    drops: Rc<Cell<u32>>,
}
impl ITexture for Texture {
    /// Positive immutable dimensions require no GPU or allocation during a draw.
    fn size(&self) -> Result<Size, NcStatus> {
        Ok(Size { width: 1, height: 1 })
    }
}
impl Drop for Texture {
    /// Counts destruction once the last real COM owner releases the capability.
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
/// Creates a real shared COM texture and an observer that cannot extend its lifetime.
fn texture() -> (ComPtr<ITextureAbi>, Rc<Cell<u32>>) {
    let drops = Rc::new(Cell::new(0));
    let value = Texture::from_value(Texture { drops: drops.clone() }).query().unwrap();
    (value, drops)
}

/// CPU import retains concrete texture owners; this fixture does not model device authentication.
struct OwnedTextures;
impl Resources for OwnedTextures {
    /// Imported batches own the real COM texture until CPU preparation is cleared.
    type Texture = ComPtr<ITextureAbi>;
    /// Retains the actual capability until imported-frame cleanup.
    fn texture(&self, texture: ComPtr<ITextureAbi>) -> Result<Self::Texture, NcStatus> {
        Ok(texture)
    }
}

/// Observations retain copied geometry and identities, without keeping textures alive.
#[derive(Default)]
struct Capture {
    /// Number of successfully validated frames.
    calls: Cell<u32>,
    /// Complete geometry copied by the production importer.
    vertices: RefCell<Vec<RenderVertex>>,
    /// Pointer-free batch records copied while the stream token remains active.
    batches: RefCell<Vec<RenderBatch>>,
    /// Actual retained texture identities resolved only after partition validation.
    textures: RefCell<Vec<usize>>,
    /// Optional renderer error occurs after CPU validation, independently of allocation injection.
    failure: Cell<Option<NcStatus>>,
}
#[com_object(interfaces(IRenderer))]
/// Validates the actual CPU stream before capturing it; no native GPU is needed for this defect.
struct Renderer {
    /// Independent observation storage is mutated only during a synchronous submission.
    capture: Rc<Capture>,
}
impl IRenderer for Renderer {
    /// Atlas preparation is outside this CPU frame-partition regression.
    fn prepare_atlas(&self, _: ComRef<'_, IRgbaImageAbi>) -> Result<(), NcStatus> {
        Ok(())
    }
    /// The shared production importer rejects orphan vertices before resolving any texture.
    fn render(&self, info: RenderInfo, _: ComRef<'_, IRgbaImageAbi>, stream: ComRef<'_, IRenderStreamAbi>) -> Result<(), NcStatus> {
        let mut frame = PreparedFrame::<ComPtr<ITextureAbi>>::default();
        frame.read(info, stream, &OwnedTextures)?;
        let mut records = NcOutputBuffer::new();
        if stream.batches(info.invocation, records.output(0, info.batch_count as usize))? != info.batch_count {
            return Err(NC_E_INVALID_ARG);
        }
        *self.capture.vertices.borrow_mut() = frame.vertices().to_vec();
        *self.capture.batches.borrow_mut() = records.into_vec();
        *self.capture.textures.borrow_mut() = frame
            .batches()
            .iter()
            .filter_map(|batch| match batch {
                Batch::Texture { texture, .. } => Some(texture.as_raw().object as usize),
                _ => None,
            })
            .collect();
        self.capture.calls.set(self.capture.calls.get() + 1);
        frame.clear();
        match self.capture.failure.get() {
            Some(status) => Err(status),
            None => Ok(()),
        }
    }
}

/// Distinguishes the two recoverable reservations using the real draw's allocation sequence.
#[derive(Clone, Copy, Debug)]
enum Failure {
    /// The first quad outgrows the vertex buffer after the preceding solid triangle.
    Vertices,
    /// Three earlier quads fill batch capacity; the next draw grows both vectors.
    Batches,
}
impl Failure {
    /// Places the next draw at the desired growth boundary with earlier valid work recorded.
    fn prefix(self) -> usize {
        match self {
            Self::Vertices => 0,
            Self::Batches => 3,
        }
    }
    /// Rc ownership allocates first, followed by vertex growth and then batch growth if needed.
    /// An observation-only control checks this profile before any request is denied.
    fn allocation(self) -> usize {
        match self {
            Self::Vertices => 2,
            Self::Batches => 3,
        }
    }
}

/// One active recording and real COM renderer, with no test-owned earlier texture capability.
struct Recording {
    /// Mutable compositor session exercised solely through generated ABI calls.
    compositor: ComPtr<GfxCompositorAbi>,
    /// Calls the common production partition validator.
    renderer: ComPtr<IRendererAbi>,
    /// Captures values independently of compositor and texture ownership.
    capture: Rc<Capture>,
    /// Earlier texture destruction proves cleanup even while the compositor remains alive.
    earlier_drops: Rc<Cell<u32>>,
}
/// Positive drawable bounds admit all fixture geometry without clipping it away.
const CLIP: Rect = Rect { x: 0, y: 0, width: 16, height: 16 };
/// Creates fresh, identically sized buffers for control and failure runs.
fn recording(failure: Failure) -> Recording {
    let bundle = fixture::test_bundle();
    let mut compositor = GfxCompositorObject::create(bundle.as_com_ref()).unwrap();
    compositor.begin(Size { width: 16, height: 16 }, 0xff000000).unwrap();
    let solid = [
        SolidVertex { x: 0., y: 0., rgba: 0xff0000ff },
        SolidVertex { x: 1., y: 0., rgba: 0xff0000ff },
        SolidVertex { x: 0., y: 1., rgba: 0xff0000ff },
    ];
    compositor.solid_triangles(CLIP, NcCopyFromBufferOffset::new(0, &solid)).unwrap();
    let (earlier, earlier_drops) = texture();
    for _ in 0..failure.prefix() {
        draw(&mut compositor, earlier.as_com_ref()).unwrap();
    }
    drop(earlier);
    assert_eq!(earlier_drops.get(), u32::from(failure.prefix() == 0));
    let capture = Rc::new(Capture::default());
    let renderer = Renderer::from_value(Renderer { capture: capture.clone() }).query().unwrap();
    Recording {
        compositor,
        renderer,
        capture,
        earlier_drops,
    }
}
/// Records one visible texture quad; normalized source and scalar inputs need no conversion allocation.
fn draw(compositor: &mut ComPtr<GfxCompositorAbi>, texture: ComRef<'_, ITextureAbi>) -> Result<(), NcStatus> {
    compositor.texture(
        CLIP,
        Rect { x: 2, y: 2, width: 4, height: 4 },
        TextureRegion { u0: 0., v0: 0., u1: 1., v1: 1. },
        0xffffffff,
        texture,
    )
}
/// Verifies the control allocation profile, then denies exactly the selected request in a fresh recording.
fn fail_draw(failure: Failure) -> Recording {
    let mut control = recording(failure);
    let (candidate, drops) = texture();
    let probe = allocation_probe::Probe::start(None);
    let result = draw(&mut control.compositor, candidate.as_com_ref());
    let report = probe.finish();
    assert_eq!(result, Ok(()));
    assert_eq!(report.attempts, failure.allocation(), "unexpected allocation profile for {failure:?}");
    assert_eq!(report.denied, 0);
    drop(candidate);
    assert_eq!(drops.get(), 0, "successful recording must retain its texture");
    control.compositor.submit(control.renderer.as_com_ref()).unwrap();
    assert_frame(&control.capture, failure.prefix() + 1);
    assert_eq!(drops.get(), 1);
    assert_eq!(control.earlier_drops.get(), 1);

    let mut failed = recording(failure);
    let (candidate, drops) = texture();
    let probe = allocation_probe::Probe::start(Some(failure.allocation()));
    let result = draw(&mut failed.compositor, candidate.as_com_ref());
    let report = probe.finish();
    assert_eq!(report.denied, 1);
    assert_eq!(report.attempts, failure.allocation());
    assert_eq!(result, Err(NC_E_OUT_OF_MEMORY));
    drop(candidate);
    assert_eq!(drops.get(), 1, "a failed draw must release its temporary owner");
    assert_eq!(failed.compositor.begin(Size { width: 16, height: 16 }, 0), Err(NC_E_BUSY));
    failed
}
/// Exact counts, ordering and geometry preserve the earlier triangle plus complete texture quads.
fn assert_frame(capture: &Capture, textures: usize) {
    assert_eq!(capture.calls.get(), 1);
    assert_eq!(capture.vertices.borrow().len(), 3 + textures * 4);
    let records = capture.batches.borrow();
    assert_eq!(records.len(), 1 + textures);
    assert!(matches!(records[0], RenderBatch::Atlas { first: 0, count: 3 }));
    for (index, record) in records[1..].iter().enumerate() {
        assert!(matches!(record, RenderBatch::Texture { first } if *first as usize == 3 + index * 4));
    }
    let vertices = capture.vertices.borrow();
    assert_eq!(
        vertices[..3].iter().map(|v| (v.x, v.y, v.rgba)).collect::<Vec<_>>(),
        [(0., 0., 0xff0000ff), (1., 0., 0xff0000ff), (0., 1., 0xff0000ff)]
    );
    for quad in vertices[3..].chunks_exact(4) {
        assert_eq!(
            quad.iter().map(|v| (v.x, v.y, v.u, v.v, v.rgba)).collect::<Vec<_>>(),
            [
                (2., 2., 0., 0., 0xffffffff),
                (6., 2., 1., 0., 0xffffffff),
                (6., 6., 1., 1., 0xffffffff),
                (2., 6., 0., 1., 0xffffffff)
            ]
        );
    }
    assert_eq!(capture.textures.borrow().len(), textures);
}

/// Real vertex and batch allocation errors preserve a submit-ready earlier frame and release owners.
#[test]
fn texture_reservation_errors_preserve_earlier_frame_and_ownership() {
    for failure in [Failure::Vertices, Failure::Batches] {
        let mut failed = fail_draw(failure);
        failed.compositor.submit(failed.renderer.as_com_ref()).unwrap();
        assert_frame(&failed.capture, failure.prefix());
        assert_eq!(failed.earlier_drops.get(), 1);
    }
}

/// A later successful draw in the same frame follows earlier work without a gap or stale texture.
#[test]
fn texture_reservation_errors_allow_same_frame_retry() {
    for failure in [Failure::Vertices, Failure::Batches] {
        let mut failed = fail_draw(failure);
        let (retry, drops) = texture();
        let identity = retry.as_raw().object as usize;
        draw(&mut failed.compositor, retry.as_com_ref()).unwrap();
        drop(retry);
        assert_eq!(drops.get(), 0);
        failed.compositor.submit(failed.renderer.as_com_ref()).unwrap();
        assert_frame(&failed.capture, failure.prefix() + 1);
        assert_eq!(failed.capture.textures.borrow().last(), Some(&identity));
        assert_eq!(drops.get(), 1);
        assert_eq!(failed.earlier_drops.get(), 1);
        assert_new_frame(&mut failed);
    }
}

/// New begin resets contents and budget; one fresh quad starts at zero and releases its owner.
fn assert_new_frame(recording: &mut Recording) {
    let calls = recording.capture.calls.get();
    recording.compositor.begin(Size { width: 16, height: 16 }, 0xff000000).unwrap();
    let (next, drops) = texture();
    let identity = next.as_raw().object as usize;
    draw(&mut recording.compositor, next.as_com_ref()).unwrap();
    drop(next);
    assert_eq!(drops.get(), 0);
    recording.compositor.submit(recording.renderer.as_com_ref()).unwrap();
    assert_eq!(recording.capture.calls.get(), calls + 1);
    assert_eq!(recording.capture.vertices.borrow().len(), 4);
    let batches = recording.capture.batches.borrow();
    assert_eq!(batches.len(), 1);
    assert!(matches!(batches[0], RenderBatch::Texture { first: 0 }));
    assert_eq!(&*recording.capture.textures.borrow(), &[identity]);
    assert_eq!(drops.get(), 1);
}

/// Cancellation releases earlier capabilities, stays idempotent and permits a clean next frame.
#[test]
fn texture_reservation_errors_allow_cancel_and_new_begin() {
    for failure in [Failure::Vertices, Failure::Batches] {
        let mut failed = fail_draw(failure);
        failed.compositor.cancel().unwrap();
        assert_eq!(failed.earlier_drops.get(), 1);
        assert_eq!(failed.capture.calls.get(), 0);
        failed.compositor.cancel().unwrap();
        assert_new_frame(&mut failed);
    }
}

/// A renderer error after a recoverable draw failure still ends the frame and releases its owners.
#[test]
fn texture_reservation_errors_allow_failed_submit_and_next_frame() {
    for failure in [Failure::Vertices, Failure::Batches] {
        let mut failed = fail_draw(failure);
        failed.capture.failure.set(Some(neocogi_com::NC_E_NOT_FOUND));
        assert_eq!(failed.compositor.submit(failed.renderer.as_com_ref()), Err(neocogi_com::NC_E_NOT_FOUND));
        assert_frame(&failed.capture, failure.prefix());
        assert_eq!(failed.earlier_drops.get(), 1);
        failed.capture.failure.set(None);
        assert_new_frame(&mut failed);
    }
}

// This final integration-test executable owns one embedded runtime.
extern crate neocogi_com_host as _;
