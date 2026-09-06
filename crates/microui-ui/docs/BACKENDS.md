# Backend ownership and COM rendering

The platform plugin co-packages the native window, selected renderer, images, meshes,
and backend-specific application drawing. Its implementation links a concrete Glow,
Vulkan or WGPU backend. Browser builds statically register the same COM contracts and
use WebGL. Applications retain `IWindow`, `IRenderer`, `IImage` and `IRenderItem`
interfaces through the facade; native GPU objects remain inside the platform plugin.

`RendererBackend::Frame<'a>` is an implementation-local generic associated type. The
frame borrows its backend exclusively until `finish(self)` submits it, or Drop abandons
it. There is no production custom callback registration or native Context app API.
The actual implementation and compile fixtures demonstrate acquire/use/drop and reject
two simultaneous backend frames.

## Backend sources

Renderer implementation crates live alongside [the platform plugin](../../../plugins/platform)
under `plugins/`. The platform plugin links the selected backend and shared helpers
into its native library; browser builds link them into the wasm module. The renderer
crates are Rust libraries without separate COM plugin descriptors or shared libraries.

| Source | Responsibility |
| --- | --- |
| [plugins/platform](../../../plugins/platform) | Window, input and device ownership; COM renderer integration |
| [plugins/renderer-glow](../../../plugins/renderer-glow) | Glow rendering shared by native OpenGL and WebGL |
| [plugins/renderer-vulkan](../../../plugins/renderer-vulkan) | Vulkan rendering and UI shaders |
| [plugins/renderer-wgpu](../../../plugins/renderer-wgpu) | WGPU rendering |
| [plugins/renderer-webgl-canvas](../../../plugins/renderer-webgl-canvas) | Browser canvas backend using the Glow renderer |
| [plugins/renderer-support](../../../plugins/renderer-support) | Shared renderer and platform helpers |

## Ordered application drawing

A widget records an owned `IRenderItem` in its paint stream. After preceding UI batches
have flushed, the renderer invokes the item with copied content, final clip and frame
dimensions. The item issues validated mesh commands against the active draw context.
The platform then resumes ordinary UI drawing in painter order.

The generic application renderer types in the native backend crates are statically
linked implementation helpers. Their Rust command values, frame borrows and GPU
objects never cross the COM ABI. See [the demo renderer](../../../demos/demo-renderer/src)
for concrete backend implementations and [the application mesh component](../../../demos/demo-full/src/graphics.rs)
for owned COM rendering.

## Resource lifetime and thread rules

All window, renderer and UI operations run on their creating thread. Cross-thread
workers deliver owned data to that thread before an update. COM retention does not
make GPU contexts or widgets thread-safe.

Images and meshes are immutable objects authenticated to their creating window's
render device. Foreign-device resources are rejected before native work. Resource
references retain the device owner until release; retirement queues defer destruction
until the backend can safely perform it. Renderer destruction precedes native surface
and GL context destruction. GL context ownership uses `Rc` on the window thread.

Glow targets OpenGL ES 3 / WebGL 2. UI draws establish the framebuffer, viewport,
scissor, write mask, blending, depth/stencil policy, rasterization enables, program,
texture unit, sampler and private vertex objects. Uploads reset unpack state; browser
uploads additionally disable implicit flipping, premultiplication and color conversion.

Application GPU implementations must own their GPU objects, establish every setting
their commands consume, finish scoped operations such as queries or transform feedback,
and restore extension-specific state outside that baseline. They must not modify or
delete backend-owned resources. The backend cannot infer unfinished application work.
UI drawing does not promise to restore the application's previous GPU state.

WebGL context loss invalidates the current device. Recreate the window/device and its
resources after loss; retained COM pointers do not restore native resources automatically.
The browser platform owns canvas, text-input DOM state, IME and event normalization.

## Validation and packaging

[Native deployment](../../../docs/COM_NATIVE.md) documents separate builds, plugin staging,
load order, feature choices and smoke commands. [Author obligations](../../../docs/COM_AUTHORS.md)
cover phase authority, callback reentrancy, lifecycle and geometry invalidation.
Run `scripts/test-demo.sh BACKEND` for the full application and
`scripts/test-extension.sh BACKEND` for a separately built widget/container plugin.
