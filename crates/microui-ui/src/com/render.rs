//! Coarse COM submission adapter over the existing display-list executor.
use super::*;
use crate::{AtlasHandle, TextureId};
use crate::render::{AtlasUploadError, FrameError, FrameInfo, RendererBackend, RendererFrame, TextureError, Vertex};
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
    /// Final ordered triangles; no COM call per vertex or glyph.
    pub(super) vertices: RefCell<Vec<RenderVertex>>,
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
        if invocation == 0 || self.storage.active.get() != invocation {
            return Err(UI_E_EXPIRED_SCOPE);
        }
        let offset = usize::try_from(output.offset()).map_err(|_| NC_E_INVALID_ARG)?;
        let vertices = self.storage.vertices.try_borrow().map_err(|_| UI_E_BUSY)?;
        let source = vertices.get(offset..).ok_or(NC_E_INVALID_ARG)?;
        let count = source.len().min(output.len());
        output.as_mut_slice()[..count].copy_from_slice(&source[..count]);
        u32::try_from(count).map_err(|_| nc_com::NC_E_OUT_OF_RANGE)
    }
}

/// Final destination of geometry prepared by the single concrete backend type.
#[derive(Copy, Clone, Eq, PartialEq)]
enum Destination {
    /// A built-in paint invocation returns its owned bulk vertices to a painter.
    Recording,
    /// A complete retained frame calls the selected external renderer once.
    Renderer,
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
    pub(super) target: Rc<RefCell<Option<ComPtr<IRendererAbi>>>>,
}

impl Backend {
    /// Creates recording storage without acquiring a GPU frame.
    pub(super) fn new(atlas: AtlasHandle, host: Rc<HostState>) -> Self {
        let storage = Rc::new(Storage::default());
        let stream = Stream::from_value(Stream { storage: storage.clone() })
            .query()
            .expect("Stream declares IRenderStream");
        Self {
            destination: Destination::Renderer,
            atlas,
            host,
            storage,
            stream,
            target: Rc::default(),
        }
    }
    /// Reuses the same backend/executor monomorphization for local widget geometry.
    pub(super) fn recording(atlas: AtlasHandle, host: Rc<HostState>) -> (Self, Rc<Storage>) {
        let mut backend = Self::new(atlas, host);
        backend.destination = Destination::Recording;
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
    /// Atlas replacement is wired to the asset/renderer transaction in the assets milestone.
    fn replace_atlas(&mut self, _atlas: AtlasHandle) -> Result<(), AtlasUploadError> {
        Err(AtlasUploadError::new("COM atlas replacement is not installed"))
    }
    /// Clears reused scratch; no native command executes before finish.
    fn frame(&mut self, info: FrameInfo) -> Result<Self::Frame<'_>, FrameError> {
        self.storage.vertices.borrow_mut().clear();
        Ok(Frame { backend: self, info, error: None })
    }
    /// External texture capabilities are added with the full renderer integration.
    fn create_texture(&mut self, _id: TextureId, _pixels: &[u8]) -> Result<(), TextureError> {
        Err(TextureError::backend("COM textures are not installed"))
    }
    /// No external texture can have been created by this slice adapter.
    fn destroy_texture(&mut self, _id: TextureId) {}
}

impl RendererFrame for Frame<'_> {
    /// Submits the prepared frame once; dropping the frame never submits anything.
    fn finish(self) -> Result<(), FrameError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if let Some(error) = self.backend.host.error.get() {
            return Err(FrameError::new(error.to_string()));
        }
        if self.backend.destination == Destination::Recording {
            return Ok(());
        }
        let storage = &self.backend.storage;
        let invocation = storage.next.get().checked_add(1).ok_or_else(|| FrameError::new("frame invocation exhausted"))?;
        let vertex_count = u32::try_from(storage.vertices.borrow().len()).map_err(|_| FrameError::new("too many vertices"))?;
        storage.next.set(invocation);
        let renderer = self.backend.target.borrow().clone().ok_or_else(|| FrameError::new("renderer not selected"))?;
        storage.active.set(invocation);
        let _scope = ReadScope(storage);
        let size = self.info.dimensions();
        let clear = self.info.clear();
        renderer
            .render(
                RenderInfo {
                    size: Size { width: size.width, height: size.height },
                    clear: u32::from_le_bytes([clear.r, clear.g, clear.b, clear.a]),
                    invocation,
                    vertex_count,
                },
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
        self.backend.storage.vertices.borrow_mut().extend(v.map(vertex));
    }
    /// Atlas-only triangles preserve their order without introducing an extra draw boundary.
    fn flush(&mut self) {}
    /// Rejects unsupported external resources instead of silently drawing with the wrong texture.
    fn draw_texture(&mut self, _id: TextureId, _vertices: [Vertex; 4]) {
        self.error = Some(FrameError::new("COM texture batch is not installed"));
    }
}
