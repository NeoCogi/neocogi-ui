//! Coarse COM submission adapter over the existing display-list executor.
use super::*;
use crate::{ImageSource, TextureId};
use crate::render::{AtlasUploadError, FrameError, FrameInfo, RendererBackend, RendererFrame, TextureError, Vertex, RenderItemArgs};
use nc_com::NcCopyToBufferOffset;

/// Decodes canonical RGBA packing independently of machine byte order.
pub(super) fn color(rgba: u32) -> crate::render::Color {
    let [r, g, b, a] = rgba.to_le_bytes();
    crate::render::Color { r, g, b, a }
}

/// Owned frame storage is reused; invocation checks prevent stale readers observing reuse.
#[derive(Default)]
pub(super) struct Storage {
    /// Zero outside an active renderer call.
    active: Cell<u64>,
    /// Invocation sequence is never allowed to wrap.
    next: Cell<u64>,
    /// Ordered draw boundaries and owned capabilities, detached before any foreign Release.
    batches: RefCell<Vec<Batch>>,
    /// Final ordered geometry; no COM call per vertex or glyph.
    pub(super) vertices: RefCell<Vec<RenderVertex>>,
}

/// Each batch owns only the concrete resource kind needed by its draw operation.
enum Batch {
    /// A contiguous range of atlas triangles; adjacent ranges are merged.
    Atlas {
        /// First vertex in the frame's shared geometry buffer.
        first: u32,
        /// Complete triangle vertex count, merged with adjacent atlas operations.
        count: u32,
    },
    /// One clipped quad; Rc permits fetching its COM capability without AddRef under a borrow.
    Texture {
        /// First of the four clipped quad vertices.
        first: u32,
        /// One retained capability, borrowed through Rc before any foreign AddRef.
        texture: Rc<ComPtr<ITextureAbi>>,
    },
    /// Application work with final clipped geometry and no reference to a native frame.
    Item {
        /// Complete content rectangle, already translated to drawable coordinates.
        rect: Rect,
        /// Positive content/widget/viewport intersection.
        clip: Rect,
        /// One retained application capability with no native frame borrow.
        item: Rc<ComPtr<IRenderItemAbi>>,
    },
}

impl Batch {
    /// Copies pointer-free metadata; resource capabilities are fetched separately per non-atlas batch.
    fn record(&self) -> RenderBatchAbi {
        match self {
            Self::Atlas { first, count } => RenderBatch::Atlas { first: *first, count: *count },
            Self::Texture { first, .. } => RenderBatch::Texture { first: *first },
            Self::Item { rect, clip, .. } => RenderBatch::Item { rect: *rect, clip: *clip },
        }
        .into()
    }
}

impl Storage {
    /// Checks even empty reads so an expired stream cannot become valid through a zero-length query.
    fn check(&self, invocation: u64) -> Result<(), NcStatus> {
        if invocation == 0 || self.active.get() != invocation {
            Err(UI_E_EXPIRED_SCOPE)
        } else {
            Ok(())
        }
    }
    /// Releases resources with no storage borrow, preserving the batch allocation for the next frame.
    fn clear_batches(&self) {
        let mut batches = std::mem::take(&mut *self.batches.borrow_mut());
        batches.clear();
        *self.batches.borrow_mut() = batches;
    }
}

#[com_object(interfaces(IRenderStream))]
/// Retainable bulk-copy capability with no reference to a native borrowed frame.
struct Stream {
    /// Shared owned frame storage, still safe after the UI runtime is released.
    storage: Rc<Storage>,
}

impl IRenderStream for Stream {
    /// Copies a checked prefix of the requested range only during the active invocation.
    fn vertices(&self, invocation: u64, mut output: NcCopyToBufferOffset<'_, RenderVertex>) -> Result<u32, NcStatus> {
        self.storage.check(invocation)?;
        let offset = usize::try_from(output.offset()).map_err(|_| NC_E_INVALID_ARG)?;
        let vertices = self.storage.vertices.try_borrow().map_err(|_| UI_E_BUSY)?;
        let source = vertices.get(offset..).ok_or(NC_E_INVALID_ARG)?;
        let count = source.len().min(output.len());
        output.as_mut_slice()[..count].copy_from_slice(&source[..count]);
        u32::try_from(count).map_err(|_| nc_com::NC_E_OUT_OF_RANGE)
    }
    /// Copies ordered metadata without invoking any resource method while storage is borrowed.
    fn batches(&self, invocation: u64, mut output: NcCopyToBufferOffset<'_, RenderBatchAbi>) -> Result<u32, NcStatus> {
        self.storage.check(invocation)?;
        let offset = usize::try_from(output.offset()).map_err(|_| NC_E_INVALID_ARG)?;
        let batches = self.storage.batches.try_borrow().map_err(|_| UI_E_BUSY)?;
        let source = batches.get(offset..).ok_or(NC_E_INVALID_ARG)?;
        let count = source.len().min(output.len());
        for (output, batch) in output.as_mut_slice()[..count].iter_mut().zip(source) {
            *output = batch.record();
        }
        Ok(count as u32)
    }
    /// Retains the checked texture after releasing the batch borrow; foreign AddRef may reenter.
    fn texture(&self, invocation: u64, batch: u32) -> Result<ComPtr<ITextureAbi>, NcStatus> {
        self.storage.check(invocation)?;
        let texture = match self.storage.batches.try_borrow().map_err(|_| UI_E_BUSY)?.get(batch as usize) {
            Some(Batch::Texture { texture, .. }) => texture.clone(),
            _ => return Err(NC_E_INVALID_ARG),
        };
        Ok(texture.as_ref().clone())
    }
    /// Uses the same detached ownership boundary for application GPU work.
    fn item(&self, invocation: u64, batch: u32) -> Result<ComPtr<IRenderItemAbi>, NcStatus> {
        self.storage.check(invocation)?;
        let item = match self.storage.batches.try_borrow().map_err(|_| UI_E_BUSY)?.get(batch as usize) {
            Some(Batch::Item { item, .. }) => item.clone(),
            _ => return Err(NC_E_INVALID_ARG),
        };
        Ok(item.as_ref().clone())
    }
}

/// One concrete geometry backend for complete COM frame submission.
pub(super) struct Backend {
    /// Runtime submissions observe the first component failure before calling the renderer.
    host: Rc<HostState>,
    /// Current installed assets accompany the complete frame submission.
    resources: super::widget::SharedAssets,
    /// Owned scratch survives across frames and is never lent through a COM object.
    storage: Rc<Storage>,
    /// Stable stream object reused for every synchronous submission.
    stream: ComPtr<IRenderStreamAbi>,
    /// Installed only for the duration of the caller's explicit render operation.
    pub(super) target: Rc<RefCell<Option<Rc<ComPtr<IRendererAbi>>>>>,
}

impl Backend {
    /// Creates reusable complete-frame storage and its stable stream capability.
    pub(super) fn new(host: Rc<HostState>, resources: super::widget::SharedAssets) -> Self {
        let storage = Rc::new(Storage::default());
        let stream = Stream::from_value(Stream { storage: storage.clone() })
            .query()
            .expect("Stream declares IRenderStream");
        Self {
            host,
            resources,
            storage,
            stream,
            target: Rc::default(),
        }
    }
}

/// Private native borrowed frame; it never crosses the shared-library boundary.
pub(super) struct Frame<'a> {
    /// Exclusive backend access prevents overlapping submissions in safe Rust.
    backend: &'a mut Backend,
    /// Validated drawable size and clear color.
    info: FrameInfo,
    /// Earlier unsupported/failing commands must prevent submission.
    error: Option<FrameError>,
}

impl Frame<'_> {
    /// Appends one complete local atlas primitive sequence and extends its shared batch.
    fn push_atlas_vertices(&mut self, vertices: &[Vertex]) {
        // Only the trait's triangle and quad entry points call this helper, so every accepted slice
        // is nonempty, forms complete triangles, and fits the bounded count conversion below.
        debug_assert!(matches!(vertices.len(), 3 | 6));
        if self.error.is_some() {
            return;
        }
        let count = vertices.len() as u32;
        let storage = &self.backend.storage;
        // Hold each distinct local buffer once. No foreign code runs while either guard exists.
        let mut output = storage.vertices.borrow_mut();
        let first = output.len() as u32;
        let mut batches = storage.batches.borrow_mut();
        let joins_atlas = matches!(batches.last(), Some(Batch::Atlas { .. }));
        if first > MAX_RENDER_VERTICES - count || (!joins_atlas && batches.len() >= MAX_RENDER_BATCHES as usize) {
            self.error = Some(FrameError::new("frame geometry exceeds the shared allocation limit"));
            return;
        }
        output.extend_from_slice(vertices);
        if let Some(Batch::Atlas { count: batch_count, .. }) = batches.last_mut() {
            *batch_count += count;
        } else {
            batches.push(Batch::Atlas { first, count });
        }
    }
}

/// Expires frame access even when a renderer fails.
struct ReadScope<'a>(&'a Storage);
impl Drop for ReadScope<'_> {
    /// A retained old stream/token can never read a later frame.
    fn drop(&mut self) {
        self.0.active.set(0);
    }
}

impl RendererBackend for Backend {
    type Frame<'a> = Frame<'a>;
    /// Pixel replacement belongs to the external platform renderer, never this recorder backend.
    fn replace_atlas(&mut self, _atlas: ImageSource<'_>) -> Result<(), AtlasUploadError> {
        // This backend records geometry only. The platform already performed the GPU transaction.
        Ok(())
    }
    /// Clears reused scratch; no native command executes before finish.
    fn frame(&mut self, info: FrameInfo) -> Result<Self::Frame<'_>, FrameError> {
        self.storage.clear_batches();
        self.storage.vertices.borrow_mut().clear();
        Ok(Frame { backend: self, info, error: None })
    }
    /// Native IDs cannot cross COM; applications create owned images through ITextureFactory.
    fn create_texture(&mut self, _id: TextureId, _pixels: &[u8]) -> Result<(), TextureError> {
        Err(TextureError::backend("create COM textures through the selected renderer ITextureFactory"))
    }
    /// COM images are released by their owned capabilities, never by legacy native IDs.
    fn destroy_texture(&mut self, _id: TextureId) {}
}

impl RendererFrame for Frame<'_> {
    /// Submits the prepared frame once; dropping the frame never submits anything.
    fn finish(mut self) -> Result<(), FrameError> {
        if let Some(error) = self.error.take() {
            return Err(error);
        }
        if let Some(error) = self.backend.host.error.get() {
            return Err(FrameError::new(error.to_string()));
        }
        let storage = &self.backend.storage;
        let invocation = storage.next.get().checked_add(1).ok_or_else(|| FrameError::new("frame invocation exhausted"))?;
        let vertex_count = storage.vertices.borrow().len() as u32;
        let batch_count = storage.batches.borrow().len() as u32;
        storage.next.set(invocation);
        let renderer = self.backend.target.borrow().clone().ok_or_else(|| FrameError::new("renderer not selected"))?;
        storage.active.set(invocation);
        let _scope = ReadScope(storage);
        let size = self.info.dimensions();
        let clear = self.info.clear();
        let revision = self.backend.resources.borrow();
        renderer
            .render(
                RenderInfo {
                    size: Size { width: size.width, height: size.height },
                    clear: u32::from_le_bytes([clear.r, clear.g, clear.b, clear.a]),
                    invocation,
                    vertex_count,
                    batch_count,
                },
                revision.source(),
                self.backend.stream.as_com_ref(),
            )
            .map_err(|error| FrameError::new(error.to_string()))
    }
    /// Appends a quad as its two triangles in one checked buffer operation.
    fn push_quad(&mut self, v: [Vertex; 4]) {
        self.push_atlas_vertices(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
    }
    /// Appends to the reusable final vertex buffer.
    fn push_triangle(&mut self, v: [Vertex; 3]) {
        self.push_atlas_vertices(&v);
    }
    /// Atlas-only triangles preserve their order without introducing an extra draw boundary.
    fn flush(&mut self) {}
    /// Records a complete owned texture draw; only the selected GPU renderer can authenticate it.
    fn draw_com_texture(&mut self, texture: ComPtr<ITextureAbi>, vertices: [Vertex; 4]) -> Result<(), FrameError> {
        let storage = &self.backend.storage;
        let first = storage.vertices.borrow().len() as u32;
        if first > MAX_RENDER_VERTICES - 4 || storage.batches.borrow().len() >= MAX_RENDER_BATCHES as usize {
            return Err(FrameError::new("frame geometry exceeds the shared allocation limit"));
        }
        storage.vertices.borrow_mut().extend(vertices);
        storage.batches.borrow_mut().push(Batch::Texture { first, texture: Rc::new(texture) });
        Ok(())
    }
    /// Records final custom geometry at the current painter-order boundary.
    fn draw_com_item(&mut self, item: ComPtr<IRenderItemAbi>, args: RenderItemArgs) -> Result<(), FrameError> {
        let storage = &self.backend.storage;
        if storage.batches.borrow().len() >= MAX_RENDER_BATCHES as usize {
            return Err(FrameError::new("frame batches exceed the shared allocation limit"));
        }
        storage.batches.borrow_mut().push(Batch::Item {
            rect: copied_rect(args.content_area),
            clip: copied_rect(args.view),
            item: Rc::new(item),
        });
        Ok(())
    }
    /// Rejects unsupported external resources instead of silently drawing with the wrong texture.
    fn draw_texture(&mut self, _id: TextureId, _vertices: [Vertex; 4]) {
        self.error = Some(FrameError::new("native texture IDs cannot cross COM; submit an owned ITexture"));
    }
}

impl Drop for Frame<'_> {
    /// Failed, abandoned, and completed submissions release retained frame capabilities here.
    fn drop(&mut self) {
        self.backend.storage.clear_batches();
    }
}
