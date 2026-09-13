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
             -> coarse GfxCompositor operations -> one bulk neocogi.renderer submission
```

The display list owns text, solid geometry and retained image/render-item interfaces.
There is no per-widget COM command queue and no replay into a second display list. UI sends
one typed compositor call per retained draw operation, never one call per glyph or vertex.
The GFX compositor clears reusable output storage after submission or failure. Adjacent
compatible geometry batches together; images and application render items preserve painter order.
Inputs are validated before entering the list and the completed frame is validated
before GPU work, including buffer bounds, finite values, resource provenance and
supported batch kinds.

The runtime, its dynamically linked widget factory, and its retained GFX compositor share one imported asset
value with every built-in product. Frame, icon, and font slots therefore resolve
against that existing value without reconstructing an atlas or skin per widget. The
import stores a separate `IRgbaImage` projection beside the UI bundle. Only that narrow
projection crosses into `IRenderer::prepare_atlas` and `GfxCompositor::submit`; GFX passes
it to `IRenderer::render`, and no renderer
can inspect the bundle's theme, font, icon, chrome, or catalog tables.

## Update and frame validity

Deliver input and semantic changes before `Ui::update`. Each queued event receives an
eligible-tree update, typed event dispatch and geometry synchronization before the next
event is routed. Rendering observes the completed commit. Pending input, changed
geometry or a different drawable size requires a new update before submission.

Paint is observational. Widget setters, topology edits, event production and recursive
updates are prohibited during paint and layout. Scope checks reject calls made with a
retained expired painter/layout token; the runtime also rejects recursive update/render.
Private rendering caches may change without affecting authored state or geometry.
See [author obligations](../../../../docs/COM_AUTHORS.md) for error and partial-commit rules.

## Coordinates and clipping

Widget paint and pointer input use the same content-local origin. The recorder translates
to screen space once. Every operation owns an effective clip; nested paint scopes can
only narrow it. Root/child-window policy, container viewports and scrolling establish
those clips from committed geometry, so paint and hit testing share their boundaries.

The GFX compositor clips atlas rectangles and adjusts their sampling coordinates,
clips image quads while preserving UV projection, and clips solid triangles against
the operation clip and viewport. Empty geometry is discarded. Backends receive final
screen-space UI vertices. `RenderVertex` from the standalone `neocogi.renderer`
contract module is the one canonical vertex representation
used by clipping, the COM stream, platform validation and concrete GPU backends; the
boundary performs its required bulk copy without packing and unpacking every vertex
into a duplicate native struct. Each validated atlas range is then presented to the native
frame as one slice. Contiguous buffered backends append it once, while indexed backends can
use the trait's per-triangle fallback. A render item additionally receives its authoritative
visible region because its own mesh drawing happens at that ordered boundary.

The COM recorder appends semantic operations to UI-local storage. At frame execution UI sends
complete triangle, text, and rectangle inputs to GFX with one call per operation. GFX expands
glyphs and quads locally, then exposes all final vertices and batch metadata to the renderer in
two bulk reads. Texture and custom-item capabilities are fetched only for their corresponding batches.

The portable target is a positive drawable up to 8192 by 8192 pixels. Translated logical
edges must remain in `-32768..=40960`, allowing four maximum drawable spans off-screen.
Widget/app authors own that coordinate constraint. Positive-size validation does not
promise that larger drawables or arbitrary i32 edge sums work on every backend. Empty
and negative-extent rectangles emit no geometry.

## Images and render items

`IImage` owns immutable pixels uploaded through the window's texture factory. Widgets
retain the actual image interface and use explicit source UVs and tint. A same-sized
image replacement can be paint-only; a change to intrinsic image dimensions must dirty
measurement. Resource capabilities belong to the device that created them.

Backend-specific rendering uses an owned `IRenderItem`, such as the full demo's mesh
scene. The platform invokes it after flushing preceding UI geometry and before later
commands. Native contexts, command encoders and render passes stay within that plugin.
`RenderItemArgs` is copied implementation geometry, with frame dimensions, full content
rectangle and final visible region. Render items are owned COM capabilities; no native callback registry participates in UI execution.

## Implementation boundaries

| Source | Responsibility |
| --- | --- |
| `ui_node/paint_service.rs` | Scoped foreign access to the runtime-owned frame paint context |
| [crates/gfx/impl/src/compositor.rs](../../../gfx/impl/src/compositor.rs) | Atlas expansion, final clipping, batching, and bulk frame transport |
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
text placement and nine-patch geometry. Pure painter/tessellation tests observe
production buffers directly. No second UI executor or clipping implementation is
compiled for tests. [TESTING.md](TESTING.md) describes these boundaries.

The warmed painter allocation assertion covers command recording only. Deployment
measurements must include COM overhead, the executable, shipped plugins, assets,
loaded memory, startup and actual tree shape.
