# Backend ownership and COM rendering

Each renderer is a plugin: `neocogi.platform.glow`, `neocogi.platform.vulkan`,
`neocogi.platform.wgpu`, or `neocogi.platform.webgl`. Enabling `platform` with one
backend in `neocogi-platform` selects one identity and its concrete renderer.
GPU-only SDK consumers can combine native backend features. Browser
builds statically register the WebGL descriptor; native assemblies dynamically load
one of the other three.

The selected plugin also owns its native window, event pump, images and generic graphics resources. Those are implementation details of a rendering
device, not separate deployment units. Keeping them together avoids exposing native
window/graphics handles and avoids another dynamic dispatch boundary during every
frame. Applications retain `IPlatformWindow`, `IRenderer`, `ITexture` and scoped GPU
interfaces through the facade; native GPU objects never leave the plugin.

Renderer-neutral ABI declarations live in the independent `neocogi.renderer`
contract module under `crates/platform-contracts/src/render`. The module owns `Size`, `Rect`,
`Color`, `IRgbaImage`, texture/resource capabilities, and the sealed block stream.
It has no widget, font, icon, skin, chrome, palette, or platform-window declarations.
`neocogi.ui.core` imports the small set of renderer types needed by its public UI
methods instead of owning duplicate declarations.

Normalized input, native-window polling, and IME placement live in the separate
`neocogi.platform` contract module under `crates/platform-contracts/src/render`. Its factory
creates windows from a title and size, without UI assets or a UI-core dependency. The current backend artifacts implement
the platform and renderer projections on one object; this keeps native handles
private while preserving their distinct ABI projections.

`RendererBackend::Frame<'a>` is an implementation-local generic associated type. The
frame borrows its backend exclusively until `finish(self)` submits it, or Drop abandons
it. There is no production custom callback registration or native Context app API.
The actual implementation and compile fixtures demonstrate acquire/use/drop and reject
two simultaneous backend frames.

## Backend sources

Vulkan window presentation requires the surface to advertise opaque composition.
Construction and swapchain rebuild reject unsupported surfaces before native
swapchain creation, with an opaque-composition diagnostic. Other composition
modes are not transparent-window fallbacks; they need their own pixel-alpha
contract. A failed destructive rebuild leaves presentation unavailable.

GPU backends live in `crates/platform/src/gpu/backends`. The platform plugin under
`crates/platform/src/platform` owns window/input assembly and selects one backend. Core
and support remain ordinary Rust libraries linked into that plugin artifact.

| Source | Responsibility |
| --- | --- |
| [crates/platform/src/platform](https://github.com/NeoCogi/neocogi-platform/blob/master/crates/platform/src/platform) | Plugin descriptor plus window, input, and device ownership |
| [crates/platform-contracts/src/render](https://github.com/NeoCogi/neocogi-platform/blob/master/crates/platform-contracts/src/render) | Backend-neutral frame, texture, color, and validated upload SPI used by renderer implementations and native test fixtures |
| [crates/platform/src/gpu/backends/glow](https://github.com/NeoCogi/neocogi-platform/blob/master/crates/platform/src/gpu/backends/glow) | Glow rendering shared by native OpenGL and WebGL |
| [crates/platform/src/gpu/backends/vulkan](https://github.com/NeoCogi/neocogi-platform/blob/master/crates/platform/src/gpu/backends/vulkan) | Vulkan target and command execution |
| [crates/platform/src/gpu/backends/wgpu](https://github.com/NeoCogi/neocogi-platform/blob/master/crates/platform/src/gpu/backends/wgpu) | WGPU rendering |
| [crates/platform/src/gpu/backends/webgl](https://github.com/NeoCogi/neocogi-platform/blob/master/crates/platform/src/gpu/backends/webgl) | Browser canvas backend using the Glow renderer |
| [crates/platform/src/platform](https://github.com/NeoCogi/neocogi-platform/blob/master/crates/platform/src/platform) | Shared frame preparation and resource validation |

The contracts crate supplies the same ABI and Rust values to callers and backends.
Concrete resource/command implementations and opt-in CPU fixtures belong to
`neocogi_platform::gpu`. Renderer providers do not link UI implementations.
Production UI-to-GFX and GFX-to-renderer boundaries use generated contracts and one
complete COM frame.

## Ordered application drawing

A widget owns buffers, sampled textures, samplers, shaders and pipelines created
through its restricted factory. It begins a render view through the current
painter and records bindings, persistent uniform buffer ranges, primitive draws and bounded clears.
Finish seals its contribution; the runtime preserves the position reserved at
begin. No native callback, mesh or camera object crosses the graphics ABI.

Preparation authenticates every sealed block, snapshots exact immutable versions
and realizes native allocations before acquiring a frame. Final domain checks
follow the last foreign callback. Every draw and clear retains its immutable
client root; viewport mapping and raster clipping are independent. See the
[GPU contract](https://github.com/NeoCogi/neocogi-platform/blob/master/docs/contracts/GPU_RENDERING.md) and
[Suzanne's client-owned pipeline](../../../../examples/full/src/mesh.rs).

## Resource lifetime and thread rules

All window, renderer and UI operations run on their creating thread. Cross-thread
workers deliver owned data to that thread before an update. COM retention does not
make GPU contexts or widgets thread-safe.

Ordinary images are immutable. GPU buffers have a fixed Vertex, Index, Uniform or
Texture shape. Static buffers are initialized once; dynamic buffers preserve
unselected bytes, and streamed buffers reuse three slots while clearing unwritten
tails. Synchronous uploads publish a slot only when its recorded and native owners
have retired; Busy rejects without growing storage. Each draw captures the slot it
actually consumes. All capabilities
are authenticated to their creating device and client domain. Foreign-device resources are rejected before native work. Resource
references retain the device owner until release; retirement queues defer destruction
until the backend can safely perform it. Renderer destruction precedes native surface
and GL context destruction. GL context ownership uses `Rc` on the window thread.

Glow targets OpenGL ES 3 / WebGL 2. UI draws establish the framebuffer, viewport,
scissor, write mask, blending, depth/stencil policy, rasterization enables, program,
texture unit, sampler and private vertex objects. Uploads reset unpack state; browser
uploads additionally disable implicit flipping, premultiplication and color conversion.

The common executor establishes complete fixed-function, layout and resource state
for each draw. Client commands select no framebuffer, native state object, encoder
or queue. The read-only shader profile and immutable raster root bound attachment
writes; this in-process API does not bound arbitrary shader time or isolate device faults.

WebGL context loss invalidates the current device. Recreate the window/device and its
resources after loss; retained COM pointers do not restore native resources automatically.
The browser platform owns canvas, text-input DOM state, IME and event normalization.

## Validation and packaging

[Native deployment](../../../../docs/guides/COM_NATIVE.md) documents separate builds, plugin staging,
load order, feature choices and smoke commands. [Author obligations](../../../../docs/contracts/COM_AUTHORS.md)
cover phase authority, callback reentrancy, lifecycle and geometry invalidation.
Run `scripts/test-demo.sh BACKEND` for the full application and
`scripts/test-extension.sh BACKEND` for a separately built widget/container plugin.

GFX owns UI shader sources, explicit vertex encoding and atlas selection. Its private
`IRenderHost` imports authenticated immutable ordinary images into a UI-only domain
and completes fallible native preparation before publishing resource changes. The
platform creates windows without assets. Native executors accept generic blocks only;
no backend has an implicit atlas, UI vertex stream or separate UI pipeline. Each GFX
session reuses its atlas when the immutable image identity is unchanged.
