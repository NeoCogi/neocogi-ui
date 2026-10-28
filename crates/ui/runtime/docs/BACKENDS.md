# Backend ownership and COM rendering

Each renderer is a plugin: `neocogi.render.glow`, `neocogi.render.vulkan`,
`neocogi.render.wgpu`, or `neocogi.render.webgl`. A build of `neocogi-render-plugin`
produces exactly one of those identities and links its concrete renderer. Browser
builds statically register the WebGL descriptor; native assemblies dynamically load
one of the other three.

The selected plugin also owns its native window, event pump, images, meshes, and
backend-specific application drawing. Those are implementation details of a rendering
device, not separate deployment units. Keeping them together avoids exposing native
window/graphics handles and avoids another dynamic dispatch boundary during every
frame. Applications retain `IPlatformWindow`, `IRenderer`, `ITexture`, and `IRenderItem`
interfaces through the facade; native GPU objects never leave the plugin.

Renderer-neutral ABI declarations live in the independent `neocogi.renderer`
contract module under `crates/render/core`. The module owns `Size`, `Rect`,
`Color`, `IRgbaImage`, texture/resource capabilities, and the bulk renderer stream.
It has no widget, font, icon, skin, chrome, palette, or platform-window declarations.
`neocogi.ui.core` imports the small set of renderer types needed by its public UI
methods instead of owning duplicate declarations.

Normalized input, native-window polling, and IME placement live in the separate
`neocogi.platform` contract module under `crates/render/core`. Its factory
accepts `IRgbaImage`, so a platform/renderer implementation does not link the UI
core contract merely to create a window. The current backend artifacts implement
the platform and renderer projections on one object; this keeps native handles
private while leaving their ABI ownership separate for the staged implementation split.

`RendererBackend::Frame<'a>` is an implementation-local generic associated type. The
frame borrows its backend exclusively until `finish(self)` submits it, or Drop abandons
it. There is no production custom callback registration or native Context app API.
The actual implementation and compile fixtures demonstrate acquire/use/drop and reject
two simultaneous backend frames.

## Backend sources

Renderer sources live together under `crates/render`. The `plugin` package is the
deployment shell and owns the platform/device integration. The other packages are
ordinary Rust implementation libraries linked into the selected plugin artifact; they
are not additional runtime boundaries.

| Source | Responsibility |
| --- | --- |
| [crates/render/plugin](../../../render/plugin) | Plugin descriptor plus window, input, and device ownership |
| [crates/render/core](../../../render/core) | Backend-neutral frame, texture, color, and validated upload SPI used by renderer implementations and native test fixtures |
| [crates/render/glow](../../../render/glow) | Glow rendering shared by native OpenGL and WebGL |
| [crates/render/vulkan](../../../render/vulkan) | Vulkan rendering and UI shaders |
| [crates/render/wgpu](../../../render/wgpu) | WGPU rendering |
| [crates/render/webgl](../../../render/webgl) | Browser canvas backend using the Glow renderer |
| [crates/render/support](../../../render/support) | Shared frame preparation and resource validation |

`core` is a statically linked Rust SPI, not a COM component or deployment plugin. It
lets native test fixtures and concrete backends share renderer implementation types
without making a renderer plugin link the UI implementation. Production UI-to-GFX and
GFX-to-renderer boundaries use generated contracts and one complete COM frame.

## Ordered application drawing

A widget records an owned `IRenderItem` in its paint stream. After preceding UI batches
have flushed, the renderer invokes the item with copied content, final clip and frame
dimensions. The item issues validated mesh commands against the active draw context.
The platform then resumes ordinary UI drawing in painter order.

The generic application renderer types in the native backend crates are statically
linked implementation helpers. Their Rust command values, frame borrows and GPU
objects never cross the COM ABI. See [the mesh renderer](../../../render/mesh/src)
for concrete backend implementations and [the application mesh component](../../../../examples/full/src/mesh.rs)
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
GFX-composed UI drawing does not promise to restore the application's previous GPU state.

WebGL context loss invalidates the current device. Recreate the window/device and its
resources after loss; retained COM pointers do not restore native resources automatically.
The browser platform owns canvas, text-input DOM state, IME and event normalization.

## Validation and packaging

[Native deployment](../../../../docs/guides/COM_NATIVE.md) documents separate builds, plugin staging,
load order, feature choices and smoke commands. [Author obligations](../../../../docs/contracts/COM_AUTHORS.md)
cover phase authority, callback reentrancy, lifecycle and geometry invalidation.
Run `scripts/test-demo.sh BACKEND` for the full application and
`scripts/test-extension.sh BACKEND` for a separately built widget/container plugin.

GPU atlas input is a borrowed `ImageSource` containing positive dimensions and
packed RGBA bytes. Backends validate it, upload a complete candidate texture and
publish that texture only on success. They retain no native atlas, font/icon IDs
or skin. `RendererBackend::replace_atlas` uses this raw input inside the selected backend;
the GFX compositor owns final atlas geometry in both production and tests. The host queries
`IRgbaImage` once through its initial bundle's `image()` accessor and gives that capability to the platform.
Subsequent renderer preparation and frame
submission accept only that projection, whose `size` and `pixels` methods cannot
leak UI metadata into a renderer implementation.
