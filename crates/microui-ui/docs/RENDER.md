# Rendering

Applications and extension widgets paint through the static `microui-redux` facade.
Its painter sends typed calls to the scoped COM paint context. The runtime temporarily
moves its one frame display list into that context while invoking a COM widget, so a
foreign call holds no Rust borrow and every primitive appends to the final operation
sequence immediately. Neither path lends a widget, tree, backend frame or Rust trait
object across the ABI.

Painter operations use mutable receivers. The concrete recorder stores its active
scope and optional frame context as ordinary fields, and nc-com's generated exclusive
call gate rejects retained or reentrant aliases while one operation mutates them.

```text
IWidget.paint -> scoped IPainter -> runtime frame display list
             -> one clipping/expansion pass -> one ordered IRenderer submission
```

The display list owns text, solid geometry and retained image/render-item interfaces.
There is no per-widget COM command queue and no replay into a second display list. The
executor clears reusable storage after submission or failure. Adjacent compatible
geometry batches together; images and application render items preserve painter order.
Inputs are validated before entering the list and the completed frame is validated
before GPU work, including buffer bounds, finite values, resource provenance and
supported batch kinds.

The runtime and its dynamically linked widget factory also share one imported asset
value with every built-in product. Frame, icon, and font slots therefore resolve
against that existing value without reconstructing an atlas or skin per widget.

## Update and frame validity

Deliver input and semantic changes before `Ui::update`. Each queued event receives an
eligible-tree update, typed event dispatch and geometry synchronization before the next
event is routed. Rendering observes the completed commit. Pending input, changed
geometry or a different drawable size requires a new update before submission.

Paint is observational. Widget setters, topology edits, event production and recursive
updates are prohibited during paint and layout. Scope checks reject calls made with a
retained expired painter/layout token; the runtime also rejects recursive update/render.
Private rendering caches may change without affecting authored state or geometry.
See [author obligations](../../../docs/COM_AUTHORS.md) for error and partial-commit rules.

## Coordinates and clipping

Widget paint and pointer input use the same content-local origin. The recorder translates
to screen space once. Every operation owns an effective clip; nested paint scopes can
only narrow it. Root/child-window policy, container viewports and scrolling establish
those clips from committed geometry, so paint and hit testing share their boundaries.

The final executor clips atlas rectangles and adjusts their sampling coordinates,
clips image quads while preserving UV projection, and clips solid triangles against
the operation clip and viewport. Empty geometry is discarded. Backends receive final
screen-space UI vertices. `RenderVertex` is the one canonical vertex representation
used by clipping, the COM stream, platform validation and concrete GPU backends; the
boundary performs its required bulk copy without packing and unpacking every vertex
into a duplicate native struct. Each validated atlas range is then presented to the native
frame as one slice. Contiguous buffered backends append it once, while indexed backends can
use the trait's per-triangle fallback. A render item additionally receives its authoritative
visible region because its own mesh drawing happens at that ordered boundary.

The COM recorder appends each generated triangle or six-vertex quad with one bounds check and
one borrow of each reused output buffer. A quad is not replayed through two recorder calls.

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
rectangle and final visible region. There is no production native callback registry.

## Implementation boundaries

| Source | Responsibility |
| --- | --- |
| `ui_node/paint_service.rs` | Scoped foreign access to the runtime-owned frame paint context |
| `render/external.rs` | Bulk frame transport to an external renderer |
| `render/painter.rs` | Private built-in geometry recording |
| `render/display_list.rs` | Owned ordered operations and reusable geometry |
| `render/renderer.rs` | Atlas expansion, final clipping and execution |
| `render/backend.rs` | Concrete backend frame contracts |
| [plugins/platform](../../../plugins/platform) | Window/device ownership and checked COM GPU submission |
| [plugins/renderer-*](BACKENDS.md#backend-sources) | Concrete GPU backends linked into the platform plugin |
| [plugins/renderer-support](../../../plugins/renderer-support) | Shared renderer and platform helpers |

A backend acquires one exclusive frame through `RendererBackend::frame`. Its quad and
triangle methods may batch geometry, `flush` preserves operation order, and consuming
`finish` reports submission/presentation failures. Drop abandons pending work. GPU state
and resource obligations are detailed in [BACKENDS.md](BACKENDS.md).

Native renderer ordering/allocation fixtures live under test support. They preserve
algorithm regression coverage but do not measure deployed COM overhead. Deployment
measurements must include the executable, every shipped plugin and runtime, required
assets, loaded memory, startup, allocation/reference activity and actual tree shape.
