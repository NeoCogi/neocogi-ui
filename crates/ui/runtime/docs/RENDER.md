# Rendering

Applications and extension widgets paint through the static `neocogi-ui` facade.
Its painter sends typed calls to the scoped COM paint context. The runtime temporarily
moves its one frame display list into that context while invoking a COM widget, so a
foreign call holds no Rust borrow and every primitive appends to the final operation
sequence immediately. Neither path lends a widget, tree, backend frame or Rust trait
object across the ABI.

Painter operations use mutable receivers. The concrete recorder stores its active
scope and optional frame context as ordinary fields, and neocogi-com's generated exclusive
call gate rejects retained or reentrant aliases while one operation mutates them.

```text
IWidget.paint -> scoped IPainter -> runtime frame display list
             -> coarse GfxCompositor operations -> one ordered neocogi.renderer block submission
```

The display list owns text, solid geometry and retained images and sealed graphics blocks.
There is no per-widget COM command queue and no replay into a second display list. UI sends
one typed compositor call per retained draw operation, never one call per glyph or vertex.
The GFX compositor clears reusable output storage after submission or failure. Adjacent
compatible geometry batches together; images and client graphics blocks preserve painter order.
Inputs are validated before entering the list and the completed frame is validated
before GPU work, including buffer bounds, finite values, resource provenance and
immutable clip roots and domain admission.

The runtime creates a private compositor through the shareable `GfxPluginFactory`.
Each successful factory call returns independent resource selection, frame state,
diagnostics, and stream storage, even when the immutable asset bundle is shared.
The factory is borrowed only during construction; the compositor stays inside its UI.
Standalone GFX clients may create their own sessions through the same factory.

The runtime, its dynamically linked widget factory, and its private GFX compositor
use the same immutable asset revision as every built-in product. Frame, icon, and
font slots therefore resolve against that revision without reconstructing an atlas or skin per widget.
The compositor retains the bundle's narrow `IRgbaImage` projection and uses its bound
`IRenderHost` to prepare a private generic texture. Native preparation must succeed
before an asset replacement is published; unchanged image identity reuses that texture.
`Ui::install_assets` takes only the candidate bundle. The final `IRenderer::render`
receives frame metadata and a sealed block stream, with no atlas or UI metadata.

## Update and frame validity

Deliver input and semantic changes before `Ui::update`. Each queued event receives an
eligible-tree update, typed event dispatch and geometry synchronization before the next
event is routed. Rendering observes the completed commit. Pending input, changed
geometry or a different drawable size requires a new update before submission.

Paint is observational. Widget setters, topology edits, event production and recursive
updates are prohibited during paint and layout. Scope checks reject calls made with a
retained expired painter/layout token; the runtime also rejects recursive update/render.
Private rendering caches may change without affecting authored state or geometry.
See [author obligations](../../../../docs/contracts/COM_AUTHORS.md) for error and partial-commit rules.

## Coordinates and clipping

Widget paint and pointer input use the same content-local origin. The recorder translates
to screen space once. Every operation owns an effective clip; nested paint scopes can
only narrow it. Root/child-window policy, container viewports and scrolling establish
those clips from committed geometry, so paint and hit testing share their boundaries.

The GFX compositor clips atlas rectangles and adjusts their sampling coordinates,
clips image quads while preserving UV projection, and clips solid triangles against
the operation clip and viewport. Empty geometry is discarded. Backends receive final
screen-space geometry inside GFX. `RenderVertex` belongs to the `neocogi.gfx`
contract and is reexported by the UI facade. GFX encodes its vertex layout explicitly
into generic buffers; native executors do not depend on that record's ABI or know
its meaning. Client graphics blocks retain their authoritative visible-client root
at the same ordered boundary.

The COM recorder appends semantic operations to UI-local storage. At frame execution UI sends
complete triangle, text, icon, and nine-patch inputs to GFX with one call per operation. A
`NinePatchDraw` contains only destination insets, center visibility, and closed flat-or-image
content; UI never derives its nine cell rectangles. GFX resolves bundle-local image sources,
partitions source and destination cells, removes empty cells, clips final geometry, and folds the
result into adjacent atlas work. GFX records that geometry and ordinary images as generic
commands in its private UI domain. Client blocks seal preceding UI work and retain their
original painter positions. The final stream exposes only sealed block observers.

The portable target is a positive drawable up to 8192 by 8192 pixels. Translated logical
edges must remain in `-32768..=40960`, allowing four maximum drawable spans off-screen.
Widget/app authors own that coordinate constraint. Positive-size validation does not
promise that larger drawables or arbitrary i32 edge sums work on every backend. Empty
and negative-extent rectangles emit no geometry.

## Images and scoped graphics

`IImage` owns immutable pixels uploaded through the window's texture factory. Widgets
retain the actual image interface and use explicit source UVs and tint. A same-sized
image replacement can be paint-only; a change to intrinsic image dimensions must dirty
measurement. Resource capabilities belong to the device that created them.

Custom graphics use `Painter::begin_render_view` with a restricted resource factory.
The widget defines its own encoded streams, shader inputs and primitive draws;
the runtime supplies its committed visible client root and ordered contribution.
Finish seals the contribution, while paint return revokes escaped views. Native
contexts, command encoders and render passes remain inside the renderer plugin.
See [GPU rendering](../../../../docs/contracts/GPU_RENDERING.md) for the complete
ownership, validation, viewport and work-budget rules.

## Implementation boundaries

| Source | Responsibility |
| --- | --- |
| `ui_node/paint_service.rs` | Scoped foreign access to the runtime-owned frame paint context |
| [crates/gfx/impl/src/nine_patch.rs](../../../gfx/impl/src/nine_patch.rs) | Nine-patch validation, source resolution, cell partitioning, and visibility |
| [crates/gfx/impl/src/compositor.rs](../../../gfx/impl/src/compositor.rs) | Final clipping, batching, and generic block transport |
| `render/painter.rs` | Private built-in geometry recording |
| `render/display_list.rs` | Owned ordered operations and reusable geometry |
| `render/compositor.rs` | Thin production replay into the GFX compositor |
| [crates/render/core](../../../render/core) | Backend-neutral frame contracts, texture identity, color, and upload validation |
| [crates/render/plugin](../../../render/plugin) | Backend plugin identity, window/device ownership, and checked COM GPU submission |
| [crates/render/{glow,vulkan,wgpu,webgl}](BACKENDS.md#backend-sources) | Concrete GPU implementations linked into one selected plugin |
| [crates/render/support](../../../render/support) | Shared frame preparation and resource validation |

A backend acquires one exclusive frame through `RendererBackend::frame`. Its quad and
triangle methods may batch geometry, `flush` preserves operation order, and consuming
`finish` reports submission/presentation failures. Drop abandons pending work. GPU state
and resource obligations are detailed in [BACKENDS.md](BACKENDS.md).

Headless tests inspect the actual compositor stream through `IRenderer`. They
cover command order, resource expiry, texture UVs, clipped triangle attributes,
text placement and physical nine-patch geometry. UI recording tests assert only
the retained semantic patch and never reproduce GFX's partitioning algorithm.
No second UI executor or clipping implementation is compiled for tests.
[TESTING.md](TESTING.md) describes these boundaries.

The warmed painter allocation assertion covers command recording only. Deployment
measurements must include COM overhead, the executable, shipped plugins, assets,
loaded memory, startup and actual tree shape.
