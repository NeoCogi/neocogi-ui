# Component and plugin boundaries

NeoCogi UI owns frontend protocols and composition.
[NeoCogi Platform](https://github.com/NeoCogi/neocogi-platform) owns the platform,
renderer, shader and native deployment packages consumed by this workspace.
Public responsibilities retain their independent deployment contracts:

| Responsibility | Contract | Deployable implementation |
| --- | --- | --- |
| Shader compilation | `neocogi.shader` | CPU-only `neocogi-shader-compiler` |
| Renderer | `neocogi.renderer` | Selected Glow, Vulkan, WGPU, or WebGL backend |
| Platform | `neocogi.platform` | Currently co-owned by the selected backend artifact |
| GFX resources | `neocogi.gfx` | `neocogi-gfx` |
| Skin authoring | `neocogi.skin` | `neocogi-skin` |
| Retained UI protocols | `neocogi.ui.core` | `neocogi-ui-runtime` |
| Built-in UI classes and standard composites | `neocogi.ui.nodes` | `neocogi-ui-nodes` |

The platform contract is independent even though its current SDL/DOM
implementation shares a binary and COM identity with the renderer. That
co-location is an ownership decision: native windows, graphics surfaces, and
device handles remain private instead of crossing an ABI as raw handles. A
future standalone platform implementation must introduce a typed surface
capability with explicit lifetime rules before these implementations become
separate libraries. The contract split allows that work without changing input,
polling, or IME types again.

The GFX plugin deliberately groups bounded PNG decoding, font rasterization,
icon loading, atlas packing, and renderer-neutral frame composition. Resource
construction operations share one CPU pixel pipeline and produce immutable resource
bundles or renderer-owned textures; the compositor consumes those same validated
tables without exposing them to UI or a backend. A UI receives the shareable
`GfxPluginFactory` and creates a private compositor for its lifetime. Immutable
bundles may be shared across runtimes; each runtime independently selects its
revision and owns its mutable frame session. GFX factories return fresh sessions,
and the core UI implementation depends only on their generated contracts.

The skin plugin groups typed palette/metric derivation and optional external JSON theme
compilation. Its theme loader receives explicit typed `AssetLoader` and
`AssetPublisher` capabilities from GFX and submits each bounded font or PNG
buffer to one transactional build session. Live style compilation requires only
the publisher, so embedded deployments omit the font builder. Skin therefore
does not compile another copy of GFX's font rasterizer, PNG decoder, atlas packer,
bundle reader, validator, or publisher. Source bytes, bundle-local image placements, complete appearance
records, and the completed `IAssetBundle` cross the boundary in coarse calls;
JSON syntax trees, parser callbacks, atlas drafts, pixels, and decoder state
remain private to their owners.

`neocogi-gfx-support` is an ordinary Rust support crate, not a plugin. It
publishes and validates concrete `IAssetBundle` objects for the GFX builder,
embedded resources, and the UI importer. Keeping this code below the UI facade
prevents a GFX plugin from linking the UI core contract merely to return its own
resource type.

The private implementation packages follow the same ownership split.
`neocogi-gfx-impl` contains decoding, rasterization, packing, bundle storage, and
publication. `neocogi-skin-impl` contains style policy and optional JSON parsing.
They share only `neocogi-appearance`, whose pure functions compile typed flat
appearance records for runtime Skin policy and offline default bundles. A change
to either heavyweight implementation therefore does not invalidate or relink
the other plugin.

The UI deployment has two ownership boundaries. `neocogi-ui-runtime` owns the retained forest,
layout traversal, routing, windows, command menus, generic choice-menu popup policy, and frame
commit. `neocogi-ui-nodes` owns concrete built-in leaves and containers plus standard composites
such as Combo and `FileDialog`. The nodes root binds through generated public capabilities and
never receives runtime storage, a root handle, or an implementation callback. Built-ins therefore
exercise the same cached `IWidget`, `ILeafLayout`, and `IContainerLayout` path as independently
compiled custom controls.

The Rust sources follow those boundaries: core implementation and activation live in
`crates/ui/runtime`; concrete widgets, containers, factory, picker, and node phase adapters live
in `crates/ui/nodes`. Neither provider has a production dependency on the other provider's
implementation. `neocogi-ui-support` holds only binary-local resource imports, pure geometry/text
algorithms, and phase-local recording helpers, with no retained tree, host authority, concrete
classes, runtime, or plugin getter. There is no shared implementation containing both halves,
and core's exclusion of concrete nodes does not rely on linker garbage collection.

## Dependency direction

The contract graph is acyclic and follows ownership toward lower layers:

```text
neocogi.ui.nodes ───────────> neocogi.ui.core
neocogi.ui.core ────────────> neocogi.gfx ────────> neocogi.platform
      ├─────────────────────> neocogi.platform             │
      └─────────────────────> neocogi.renderer <───────────┘
neocogi.skin ───────────────> neocogi.gfx
      └─────────────────────> neocogi.platform
```

`neocogi.gfx` owns the immutable bundle, fonts, icons, appearance records, and
their bounded bulk-copy interface. `neocogi.skin` authors new revisions of that
format; it does not own the runtime value. `neocogi.platform` owns native path
identity, avoiding a dependency from either loading plugin back into UI.

Contract build scripts carry these dependencies through Cargo's source graph.
Generated declaration signatures include their exact transitive revisions.
Plugins publish a complete root-factory signature; the runtime checks it before
bootstrap and checks subsequent COM pointer transfers. No contract libraries or
runtime module graphs are shipped.

At the provider level, `neocogi.ui.nodes.runtime` has a hard dependency on
`neocogi.ui.runtime`. The assembly planner loads core first, then activates the nodes root and calls
`bind(&UiRuntime)`. The core plugin advertises only `neocogi.ui.core`; the nodes plugin advertises
only `neocogi.ui.nodes`. Neither plugin imports the other's bootstrap entrypoint.

All polymorphism is expressed through named IDL interfaces. No cross-component
value uses `std::any::Any`, native Rust trait objects, or an untyped property bag.
IDL enums cross the ABI as generated explicit discriminators and payload layouts;
receivers validate unknown tags before reading a payload.

## Frame and loading costs

Plugin separation does not add a second per-widget dispatch layer or any per-draw provider lookup:

- Runtime mounting queries each node's public capabilities once. Update, measure, placement, and
  paint use those cached projections whether the object came from the built-in nodes provider or
  an application plugin. `NodesPluginFactory::bind` and `Ui::from_interfaces` run only during setup.

- UI records one retained display list and sends one coarse typed call per draw operation
  to its GFX compositor; there is no per-glyph, per-nine-patch-cell, or per-vertex ABI call.
  In particular, one `NinePatchDraw` carries destination insets, center policy, and either
  nine flat colors or one typed bundle-local image description.
- GFX validates and expands that complete nine-patch operation, including source slicing,
  tiny-destination partitioning, cell visibility, final clipping, and adjacent batching. It
  then encodes explicit generic resources and submits one bounded `IRenderStream` of
  sealed graphics blocks. UI and client blocks execute through the same native path.
- Skin authoring and GFX construction services run only while loading, rebuilding, or
  editing an immutable bundle. The GFX compositor remains active for coarse frame
  execution. A JSON theme crosses the Skin-to-GFX boundary once per
  referenced resource, once for atlas finalization, and once for bulk appearance
  publication; those authoring services are absent from ordinary update, layout, and paint.
- The UI imports font/icon/appearance tables once per installed revision. Its GFX
  compositor retains the independent `IRgbaImage` returned by `IAssetBundle::image()`.
  Appearance-only revisions retain its identity; rebuilt pixels have a new identity.
- GFX compares image identity before preparing its private generic texture through
  the graphics host. The renderer receives sealed blocks without an implicit atlas.
- Platform polling transfers normalized copied events. No SDL event, DOM object,
  or native handle enters UI code.

The remaining performance risks are oversized streams, redundant bundle
installation, texture churn, and excessive plugin activation. Bounded streams,
revision/identity checks, retained renderer resources, and activation-time
capability discovery address those cases. Benchmarks should measure commands,
vertices, uploads, and dynamic calls per frame so a future boundary change cannot
silently turn a bulk transaction into fine-grained IPC. Measure actual min-size code and relevant frame workloads before changing
this boundary.

## Resource deployment

Native assemblies stage fonts, icons, JSON themes, images, and meshes beside the
executable by default. Embedding is an explicit variant, never the default. The
GFX and skin plugins receive typed native paths or copied buffers and return
immutable capabilities; resource directories and JSON representations are not
part of the runtime renderer or UI abstractions.

Assembly manifests select these plugins independently. Simple, calculator, and
backend-frame examples stage GFX for their file-backed base bundle but omit Skin;
the full demo and smoke suite stage Skin because they exercise external themes.
Their application feature graphs omit the Skin contract descriptor and their
packages omit the corresponding authored theme directories as well.
Embedded applications keep the compact GFX compositor but can omit its builder and the
Skin plugin when their generated bundle is sufficient.

## Portable shader compilation

`neocogi-shader-core` owns generated contracts, plain reflection and bounded
artifact readers. Its only contract dependency is render-core's graphics stage;
render-core does not import shader-core. `neocogi-shader-compiler` privately owns
Naga admission, validation and guarded emission. Every renderer plugin requires
`neocogi.shader.compiler` and acquires its root before native construction.

Shader creation imports owned COM outputs into the existing immutable artifact
snapshot. Vulkan consumes guarded SPIR-V, Glow/WebGL ES300 and native names,
and WGPU returned validated WGSL. Native realization, target adaptation and
completion retention remain renderer responsibilities. Neither GPU resource
management nor Glow/Vulkan normally links Naga or the compiler implementation;
WGPU's checked native translator legitimately has its own Naga dependency.

Compiler reflection calls occur only during creation. No compiler object is
retained in pipeline snapshots or called during drawing/completion. See the
[shader contract](https://github.com/NeoCogi/neocogi-platform/blob/master/docs/contracts/SHADER_COMPILATION.md) for ownership, quota,
coordinate semantics and diagnostic behavior. Artifact and timing improvements
must be measured; extracting the provider alone establishes no size/speed claim.
