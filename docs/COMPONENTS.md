# Component and plugin boundaries

NeoCogi uses five public responsibilities without turning every COM class into
its own deployment unit:

| Responsibility | Contract | Deployable implementation |
| --- | --- | --- |
| Renderer | `neocogi.renderer` | Selected Glow, Vulkan, WGPU, or WebGL backend |
| Platform | `neocogi.platform` | Currently co-owned by the selected backend artifact |
| GFX resources | `neocogi.gfx` | `neocogi-gfx` |
| Skin authoring | `neocogi.skin` | `neocogi-skin` |
| Retained UI | `neocogi.ui.core` | `neocogi-ui-runtime` |

The platform contract is independent even though its current SDL/DOM
implementation shares a binary and COM identity with the renderer. That
co-location is an ownership decision: native windows, graphics surfaces, and
device handles remain private instead of crossing an ABI as raw handles. A
future standalone platform implementation must introduce a typed surface
capability with explicit lifetime rules before these implementations become
separate libraries. The contract split allows that work without changing input,
polling, or IME types again.

The GFX plugin deliberately groups bounded PNG decoding, font rasterization,
icon loading, and atlas packing. These operations share one CPU pixel pipeline
and produce immutable resource bundles or renderer-owned textures. The skin
plugin groups typed palette/metric derivation and optional external JSON theme
compilation. Its theme loader receives explicit typed `AssetLoader` and
`AssetPublisher` capabilities from GFX and submits each bounded font or PNG
buffer to one transactional build session. Live style compilation requires only
the publisher, so embedded deployments omit the font builder. Skin therefore
does not compile another copy of GFX's font rasterizer, PNG decoder, atlas packer, bundle reader, validator, or
publisher. Source bytes, bundle-local image placements, complete appearance
records, and the completed `IAssetBundle` cross the boundary in coarse calls;
JSON syntax trees, parser callbacks, atlas drafts, pixels, and decoder state
remain private to their owners.

`neocogi-gfx-support` is an ordinary Rust support crate, not a sixth plugin. It
publishes and validates concrete `IAssetBundle` objects for the GFX builder,
embedded resources, and the UI importer. Keeping this code below the UI facade
prevents a GFX plugin from linking the UI core contract merely to return its own
resource type.

The UI plugin remains one cohesive retained runtime. Nodes, layout, routing,
window management, menus, and `FileDialog` share tree ownership and commit
phases, so separating them would add dynamic calls to hot traversals without an
independent lifetime boundary. Their source modules remain separate and custom
widgets can still live in external plugins through typed widget capabilities.

## Dependency direction

The contract graph is acyclic and follows ownership toward lower layers:

```text
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

Generated shared contract libraries carry these exact dependencies. A plugin
lists only its directly implemented module, and runtime registration validates
the transitive fingerprints. Optional `gfx` and `skin` facade features prevent
unrelated clients from linking either authoring descriptor merely to use UI.

All polymorphism is expressed through named IDL interfaces. No cross-component
value uses `std::any::Any`, native Rust trait objects, or an untyped property bag.
IDL enums cross the ABI as generated explicit discriminators and payload layouts;
receivers validate unknown tags before reading a payload.

## Frame and loading costs

Plugin separation does not add per-widget or per-draw calls:

- UI records one retained display list and submits one bounded `IRenderStream`
  to the renderer per frame.
- Clipping, geometry generation, and adjacent batching happen before backend
  submission; vertices and batches transfer through bulk slices.
- Resource and skin plugins run only while loading, rebuilding, or editing an
  immutable bundle. A JSON theme crosses the Skin-to-GFX boundary once per
  referenced resource, once for atlas finalization, and once for bulk appearance
  publication; both plugins are absent from ordinary update, layout, and paint.
- The UI imports font/icon/appearance tables once per installed revision and
  caches the narrow `IRgbaImage` projection used by rendering.
- The renderer compares image identity before upload and retains GPU textures;
  it never asks the skin plugin to resolve a draw.
- Platform polling transfers normalized copied events. No SDL event, DOM object,
  or native handle enters UI code.

The remaining performance risks are oversized streams, redundant bundle
installation, texture churn, and excessive plugin activation. Bounded streams,
revision/identity checks, retained renderer resources, and activation-time
capability discovery address those cases. Benchmarks should measure commands,
vertices, uploads, and dynamic calls per frame so a future boundary change cannot
silently turn a bulk transaction into fine-grained IPC.

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
Embedded simple applications omit both plugins when their generated bundle is sufficient.
