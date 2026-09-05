//! Coarse COM submission adapter over the existing display-list executor.
use super::*;
use crate::{AtlasHandle, TextureId};
use crate::render::{AtlasUploadError, FrameError, FrameInfo, RendererBackend, RendererFrame, TextureError, Vertex, CustomRenderArgs};
use nc_com::NcCopyToBufferOffset;

/// Decodes canonical RGBA packing independently of machine byte order.
pub(super) fn color(rgba: u32) -> crate::render::Color {
    let [r, g, b, a] = rgba.to_le_bytes();
    crate::render::Color { r, g, b, a }
}

/// Packs a legacy vertex once at the private implementation boundary.
fn vertex(value: Vertex) -> RenderVertex {
    let p = value.position();
    let t = value.tex_coord();
    let c = value.color();
    RenderVertex {
        x: p.x,
        y: p.y,
        u: t.x,
        v: t.y,
        rgba: u32::from_le_bytes([c.x, c.y, c.z, c.w]),
    }
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

#[com_class(interfaces(IRenderStream))]
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

/// Final destination of geometry prepared by the single concrete backend type.
enum Destination {
    /// A built-in paint invocation returns its owned bulk vertices to a painter.
    Recording,
    /// A complete retained frame calls the selected external renderer once.
    Renderer(Rc<super::widget::Resources>),
}

/// One concrete geometry backend for local paint recording and complete COM frame submission.
pub(super) struct Backend {
    /// Selects local recording or final submission without duplicating the executor type.
    destination: Destination,
    /// Current validated resource atlas used by both measurement and display-list execution.
    atlas: AtlasHandle,
    /// Widget recording failures prevent submitting even a partially built frame.
    host: Rc<HostState>,
    /// Owned scratch survives across frames and is never lent through a COM object.
    storage: Rc<Storage>,
    /// Stable stream object reused for every synchronous submission.
    stream: ComPtr<IRenderStreamAbi>,
    /// Installed only for the duration of the caller's explicit render operation.
    pub(super) target: Rc<RefCell<Option<Rc<ComPtr<IRendererAbi>>>>>,
}

impl Backend {
    /// Creates recording storage without acquiring a GPU frame.
    pub(super) fn new(atlas: AtlasHandle, host: Rc<HostState>, resources: Rc<super::widget::Resources>) -> Self {
        Self::create(atlas, host, Destination::Renderer(resources))
    }
    /// Creates the same executor with explicit recording or resource-owning submission behavior.
    fn create(atlas: AtlasHandle, host: Rc<HostState>, destination: Destination) -> Self {
        let storage = Rc::new(Storage::default());
        let stream = Stream::from_value(Stream { storage: storage.clone() })
            .query()
            .expect("Stream declares IRenderStream");
        Self {
            destination,
            atlas,
            host,
            storage,
            stream,
            target: Rc::default(),
        }
    }
    /// Reuses the same backend/executor monomorphization for local widget geometry.
    pub(super) fn recording(atlas: AtlasHandle, host: Rc<HostState>) -> (Self, Rc<Storage>) {
        let backend = Self::create(atlas, host, Destination::Recording);
        let storage = backend.storage.clone();
        (backend, storage)
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
    /// Returns the same resource allocation used by the retained engine.
    fn get_atlas(&self) -> AtlasHandle {
        self.atlas.clone()
    }
    /// Publishes local geometry resources after Runtime has prepared the fallible GPU upload.
    fn replace_atlas(&mut self, atlas: AtlasHandle) -> Result<(), AtlasUploadError> {
        self.atlas = atlas;
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
        if matches!(self.backend.destination, Destination::Recording) {
            return Ok(());
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
        let Destination::Renderer(resources) = &self.backend.destination else {
            unreachable!("recording returned above")
        };
        let revision = resources.current();
        renderer
            .render(
                RenderInfo {
                    size: Size { width: size.width, height: size.height },
                    clear: u32::from_le_bytes([clear.r, clear.g, clear.b, clear.a]),
                    invocation,
                    vertex_count,
                    batch_count,
                },
                revision.assets.source(),
                self.backend.stream.as_com_ref(),
            )
            .map_err(|error| FrameError::new(error.to_string()))
    }
    /// Expands a quad into the same two triangles used by native backends.
    fn push_quad(&mut self, v: [Vertex; 4]) {
        self.push_triangle([v[0], v[1], v[2]]);
        self.push_triangle([v[0], v[2], v[3]]);
    }
    /// Appends to the reusable final vertex buffer.
    fn push_triangle(&mut self, v: [Vertex; 3]) {
        if self.error.is_some() {
            return;
        }
        let storage = &self.backend.storage;
        let first = storage.vertices.borrow().len() as u32;
        let mut batches = storage.batches.borrow_mut();
        let joins_atlas = matches!(batches.last(), Some(Batch::Atlas { .. }));
        if first > MAX_RENDER_VERTICES - 3 || (!joins_atlas && batches.len() >= MAX_RENDER_BATCHES as usize) {
            self.error = Some(FrameError::new("frame geometry exceeds the shared allocation limit"));
            return;
        }
        storage.vertices.borrow_mut().extend(v.map(vertex));
        if let Some(Batch::Atlas { count, .. }) = batches.last_mut() {
            *count += 3;
        } else {
            batches.push(Batch::Atlas { first, count: 3 });
        }
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
        storage.vertices.borrow_mut().extend(vertices.map(vertex));
        storage.batches.borrow_mut().push(Batch::Texture { first, texture: Rc::new(texture) });
        Ok(())
    }
    /// Records final custom geometry at the current painter-order boundary.
    fn draw_com_item(&mut self, item: ComPtr<IRenderItemAbi>, args: CustomRenderArgs) -> Result<(), FrameError> {
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
    /// Abandonment and completed submission both release queued resources without submitting work.
    fn drop(&mut self) {
        self.backend.storage.clear_batches();
    }
}
