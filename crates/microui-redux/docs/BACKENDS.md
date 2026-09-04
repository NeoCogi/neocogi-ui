# Backend frames and typed application rendering

## How `SelectedBackend::Frame<'a>` works

`SelectedBackend` is not a special type supplied by microui-redux. The examples define it as an
ordinary compile-time alias for one concrete renderer. Each renderer accepts an application
renderer type parameter, so the final backend contains both its UI state and the application's
native GPU state as one statically known type. The feature guards use WebGL, Glow, Vulkan, then
WGPU precedence so the aliases remain well-defined when additive Cargo tooling enables several
backend features:

```rust,ignore
// demo-host defines this alias independently for each backend feature.
type SelectedBackend<ApplicationRenderer = ()> =
    /* GLRenderer<ApplicationRenderer>, VulkanRenderer<ApplicationRenderer>, ... */;

// The full demo needs only one local name for its completely concrete backend.
type DemoBackend = SelectedBackend<DemoRenderer>;
```

`RendererBackend::Frame<'a>` is a generic associated type (GAT): each backend chooses its own
active-frame type, and that type may borrow the backend for `'a`. WebGL and native GL both select
`GlFrame<'a, ApplicationRenderer>` directly, WGPU selects `WgpuFrame<'a,
ApplicationRenderer>`, and Vulkan selects `VulkanFrame<'a, ApplicationRenderer>`. Applications do
not need to name that associated type: `register_custom_renderer` infers the concrete frame from
the `Context` receiving the registration.

The relevant part of the backend contract is:

```text
trait RendererBackend {
    type Frame<'a>: RendererFrame
    where
        Self: 'a;

    fn get_atlas(&self) -> AtlasHandle;
    fn replace_atlas(&mut self, atlas: AtlasHandle) -> Result<(), AtlasUploadError>;
    fn frame(&mut self, info: FrameInfo) -> Result<Self::Frame<'_>, FrameError>;
    // persistent Context-texture methods omitted
}
```

`Self: 'a` permits the concrete frame to contain a borrow of its backend. Because Cargo selects a
concrete `SelectedBackend`, the compiler monomorphizes the callback and its frame methods; this is
not a `dyn RendererBackend` or runtime backend switch. If a selected frame does not implement an
inherent extension method used by a callback, that backend selection fails at compile time.

There are two frame values and one shared frame trait in the public lifecycle:

| Type | What it represents | What ending it does |
| --- | --- | --- |
| `ContextFrame<'ctx, B, State = ()>` | The application-level paint/submission frame. It exclusively borrows `Context<B, State>` while committed retained UI is painted and recorded. | `render_ui(self)` paints and submits once. Dropping without submission cancels. |
| `B::Frame<'backend>` | The backend-level RAII frame. It exclusively borrows the concrete backend only while the recorded display list is executing. | `finish(self)` reports completion errors. WGPU/Vulkan submit and present there; GL flushes before the host swaps buffers. Dropping without finish abandons pending work. |
| `RendererFrame` | The common trait implemented by every `B::Frame<'_>`. | Defines atlas quads/triangles, flush boundaries, external textures, and explicit fallible completion. |

The lifetime is created by the borrow of `&mut B` in `RendererBackend::frame`; applications do not
choose it and should not try to make it `'static`. While the returned frame exists, it owns the
backend's exclusive mutable borrow. Safe Rust therefore prevents acquiring a second frame or
retaining the frame after rendering, and the API exposes no parallel mutable backend handle. This
is the ownership guarantee that replaces a stateful `begin`/`end` protocol.

`Context`, the selected backend, its frames, and registered render callbacks stay on their owning
thread. The rendering traits intentionally have no `Send` or `Sync` requirement. Cross-thread work
should deliver owned application data before `Context::frame`; custom callbacks execute
synchronously during display-list execution.

The native WGPU and Vulkan backends retain an SDL `Window` clone. SDL shares the existing native
window and its video subsystem through that clone; it does not create a second window. GPU surface
destruction precedes release of this retained owner, including on constructor failure. Applications
may therefore drop their own window handle before dropping the renderer without invalidating its
raw window/display handles. Keep native rendering and window operations on their owning thread.

Glow targets OpenGL ES 3 and WebGL 2 and draws UI to the default window/canvas framebuffer. Each
UI draw establishes its framebuffer, viewport, scissor, color mask, blend equation/factors, depth
and stencil policy, rasterization enables, program/uniforms, texture unit zero, sampler, and private
vertex array/buffers. Atlas, external-texture, and colored-triangle draws share that one setup.
Frame clears restore their target and write region too. CPU uploads select the texture unit and
reset unpack-buffer binding, alignment, row/image strides, and skips; browser uploads also disable
implicit flipping, premultiplication, and color-space conversion of decoded RGBA bytes.

Application renderer authors may leave those persistent settings changed. They must use their own
GPU objects, finish scoped operations (including queries and transform feedback), and restore any
extension-specific state outside the documented ES 3/WebGL 2 baseline. They must not modify or
delete backend-owned resources. These are author responsibilities: the backend cannot recover an
unfinished application operation or infer the meaning of arbitrary extension state. Application
commands should establish the state they consume, just as the UI backend does; UI rendering does
not promise to restore the application's previous state on return.

The complete sequence is:

```text
application/resource updates + ordered input calls
        |
Context::update_ui[_state](...)       drain FIFO; full update + layout after each event
        |
Context::frame(FrameInfo)             logical ContextFrame
        |
ContextFrame::render_ui(self)         paint-only internal display-list recording
        |
Context executor preflight           validate texture/custom-render keys
        |
RendererBackend::frame(&mut backend)  acquire SelectedBackend::Frame<'_>
        |
Internal display-list execution       RendererFrame calls + typed custom callbacks
        |
Application command handling          GL records immediately; WGPU/Vulkan prepare then record
        |
backend_frame.finish(self)            complete/submit/present or return FrameError; release borrow
        |
drop logical frame                    release the exclusive Context borrow
```

Context's executor acquires the backend frame only after resource preflight succeeds. Normal UI
operations use the backend-neutral `RendererFrame` methods. Immediately before a custom-render
operation, the executor closes the current UI batch and invokes the registered callback with
the concrete `&mut B::Frame<'_>`. Later UI commands continue after that callback, preserving
painter order. Acquisition failures are returned as `FrameError` before callbacks execute. Once a
frame exists, `finish(self)` completes it and propagates command/submission/presentation failures
through `RenderError::Frame` to `ContextFrame::render_ui`. An unfinished frame's destructor only
abandons pending work and must not submit or present. Application-renderer panics unwind normally;
they are not converted to success or swallowed by a finalizing destructor.

This concrete frame type is useful because it may expose inherent methods that are deliberately
absent from `RendererFrame`. `RendererFrame` remains the small UI contract; it does not grow into a
lowest-common-denominator graphics API. The example backends expose
`enqueue_colored_vertices(CustomRenderArgs, Vec<Vertex>)` for the small CPU-projected cube example:

```rust,ignore
let cube_renderer = ctx.register_custom_renderer({
    let angle = angle.clone();
    move |frame, args: CustomRenderArgs| {
        let vertices = build_cube_vertices(args.content_area, white_uv, angle.get());

        // This is an inherent concrete-frame method, not part of RendererFrame.
        frame.enqueue_colored_vertices(args, vertices);
    }
})?;

let cube = CubeBuilder::create_widget(CubeParameters);
let tree = Node::custom_render(cube, cube_renderer);
let _root = ctx.ui().create_window(Window::new("Cube", rect(40, 40, 360, 360), tree));
```

That colored-vertex helper is not the application-rendering abstraction. It is merely a compact
demo convenience. Three-dimensional meshes and more complex GPU work use the typed native
application renderer described below.

`register_custom_renderer` accepts a callback valid for every frame borrow lifetime. In expanded
form its important bound is `for<'frame> FnMut(&mut B::Frame<'frame>, CustomRenderArgs)`. That
higher-ranked lifetime means the callback can use the active frame but cannot save it in captured
state. The returned `CustomRenderHandle<B>` is tagged with `B`, so registration and removal through
a Context using another backend type fail at compile time. `Node::custom_render` erases the handle
to its backend-neutral registry key after checking the backend type; a key originating from any
other Context retains a different concrete owner identity and is rejected during Context
preflight before a backend frame is acquired.

`CustomRenderArgs` carries the geometry needed at execution time:

- `content_area` is the full screen-space rectangle allocated to the custom widget;
- `view` is the authoritative final visible rectangle after operation, content-area, viewport,
  window, and scroll clipping;
- `dimensions` is the validated drawable size of the active frame.

The executor does not invoke the callback when that intersection is empty. Do not intersect
`content_area` and `view` again inside the callback.

## The application renderer is native and concrete

Each reusable backend is generic over one backend-specific application renderer trait:

| Backend | Application trait | Preparation capabilities | Ordered graphics capabilities |
| --- | --- | --- | --- |
| Glow and WebGL 2 | `GlApplicationRenderer` | The callback executes immediately with the shared `Arc<glow::Context>`, framebuffer dimensions, and `CustomRenderArgs`. | Any operation supported by that GL context: application programs, buffers, textures, framebuffer state, and native draw calls. |
| WGPU | `WgpuApplicationRenderer` | `WgpuApplicationPrepareContext` exposes `Device`, `Queue`, a pre-pass `CommandEncoder`, surface format, and extent. Applications can create/upload resources and encode transfer or compute work before graphics. | `render` receives the active color/depth `RenderPass` and can bind arbitrary compatible pipelines, bind groups, buffers, viewports, scissors, and direct or indirect draws. |
| Vulkan | `VulkanApplicationRenderer` | `VulkanApplicationPrepareContext` exposes the logical device, an active primary command buffer outside the render pass, compatible formats/render pass, extent, memory properties, fenced frame slot, and swapchain generation. Applications can create resources and record transfer, compute, barriers, queries, or other legal pre-pass commands. | `render` receives the same primary command buffer inside the shared graphics render pass and can record arbitrary compatible native graphics commands. |

Every trait has an associated `Command` type. The backend stores that type directly in its frame
command enum; it never stores `dyn Any`, asks for a type identifier, or downcasts. An application
with several operations defines an ordinary enum and matches it exhaustively, just as it would for
a typed event:

```rust,ignore
enum DemoCommand {
    DrawMesh(MeshSubmission),
    DrawIndirect(IndirectSubmission),
}

struct DemoRenderer {
    // Backend-specific pipelines, buffers, descriptors, and frame-slot resources.
}

impl WgpuApplicationRenderer for DemoRenderer {
    type Command = DemoCommand;

    fn prepare(
        &mut self,
        context: WgpuApplicationPrepareContext<'_>,
        commands: &[&DemoCommand],
    ) -> Result<(), String> {
        // Allocate/upload resources and optionally encode compute or transfer work.
        Ok(())
    }

    fn render<'pass>(
        &'pass self,
        pass: &mut wgpu::RenderPass<'pass>,
        args: CustomRenderArgs,
        command_index: usize,
        command: &'pass DemoCommand,
    ) -> Result<(), String> {
        match command {
            DemoCommand::DrawMesh(mesh) => {
                // Bind the application's pipeline/resources and issue the native indexed draw.
            }
            DemoCommand::DrawIndirect(draw) => {
                // Bind application state and issue the native indirect draw.
            }
        }
        Ok(())
    }
}
```

The host constructs the fully specialized backend with
`Application::new_with_renderer(atlas, DemoRenderer::new(), initialize_state)`. A registered
microui callback then bridges the retained display-list position to the application's concrete
command:

```rust,ignore
let renderer = ctx.register_custom_renderer(
    move |frame, args: CustomRenderArgs| {
        frame.enqueue_application(
            args,
            DemoCommand::DrawMesh(MeshSubmission {
                mesh: mesh.clone(),
                pvm: camera.pvm(),
                view_model: camera.view_matrix(),
            }),
        );
    },
)?;
```

The full demo keeps `DemoCommand`, `MeshVertex`, `MeshBuffers`, `MeshSubmission`, shader assets,
and all three native pipeline implementations under `demos/demo-renderer`. Reusable renderers know
only the associated command selected by the application. UI-only construction uses a public
uninhabited command type, making application dispatch impossible rather than representing “no
command” with a nullable or dynamically typed value.

Painter order is preserved without constraining GPU behavior. Glow flushes previous UI geometry
and invokes the application immediately. WGPU and Vulkan retain commands until the frame is fully
known, run a preparation phase, and replay application graphics at the exact positions where their
custom-render nodes appeared. A following UI draw explicitly restores its own complete pipeline
state, so application code does not depend on private UI bindings.

Custom callbacks receive no input and should not acquire another frame, mutate the atlas, or
finalize/present the backend frame. Update application/widget state before `Context::frame`; inside
the callback, read that state and record work on the supplied frame. Use `Painter` instead when the
drawing can be expressed with portable UI primitives.

The documented [backend-frame cube demo](../../../demos/basic/src/bin/backend-frame-cube.rs) is an
intentionally small example of inherent frame methods. It transforms and depth-sorts a cube on the
CPU, then submits colored UI vertices. The full demo's Suzanne viewport is the GPU example: it
submits `DemoCommand::DrawMesh`, and the application renderer performs vertex transformation,
depth testing, shading, and indexed drawing on the selected graphics API. Run the small cube with
exactly one backend:

```bash
cargo run -p microui-redux-basic-demos --bin backend-frame-cube --no-default-features --features glow
cargo run -p microui-redux-basic-demos --bin backend-frame-cube --no-default-features --features vulkan
cargo run -p microui-redux-basic-demos --bin backend-frame-cube --no-default-features --features wgpu
```

The same custom-frame path is available in the WebGL 2 canvas backend. Build the complete browser
site with `./scripts/web-demo.sh build`; see the [browser build guide](../../../docs/WEB.md).


Frame failure is observable, but GPU work is not transactional. Glow executes commands immediately;
earlier draws and clears may already have happened when a later command fails. It latches the first
application error and ignores later frame commands. WGPU abandons an encoder on application prepare
or render failure instead of presenting it. Vulkan marks any post-acquire failure or abandonment
fatal, because the acquire semaphore and image may still be outstanding; construct a new renderer
to recover. Hosts decide how to log, retry, recreate the backend, or stop after `render_ui` fails.
A successful finish reports the backend's synchronous completion boundary, not eventual GPU health.

The native WGPU backend validates atlas and user-texture dimensions against the selected device's
2D limit. Texture creation, upload, view creation, and bind-group creation run inside validation,
out-of-memory, and internal-error scopes. All scopes are resolved before a candidate replaces a
live atlas or enters the texture map. Failure therefore preserves the previous resource identity
and returns through `AtlasUploadError` or `TextureError`; it cannot publish an invalid placeholder.
