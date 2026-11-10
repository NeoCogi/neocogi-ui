# Platform 0.2 and rs-math3d 0.21 migration

The UI workspace consumes published `rs-math3d = 0.21.0` and the three coordinated
Platform `0.2.0-alpha.2` packages. Exact registry requirements and checksums are in
[Cargo.toml](../../Cargo.toml) and [Cargo.lock](../../Cargo.lock). COM/IDL remain
at `0.1.0`; UI packages remain at `0.1.0-alpha.2`. Unrelated locked packages retain
their previous versions. No sibling checkout, Git source or registry override is required.

This baseline supersedes the dependency selection in the historical
[November 4 qualification](PLATFORM_ALPHA2_QUALIFICATION_2026-11-04.md), which remains
an account of Platform `0.1.0-alpha.2` and math `0.20.1`.

Implementation commit: `63d5bc7f5b17c5d2d51504f3823a68f53bb191ee`.

## Ownership and public APIs

| Package | Responsibility in UI |
| --- | --- |
| `neocogi-platform-contracts` | Canonical renderer, shader, audio and platform bindings; upload producers; typed GPU buffer authoring. Build dependencies enable `build-support` for schemas and native layout. |
| `neocogi-platform` | Concrete backends, renderer-owned shader compilers and optional hosting. Static hosts enable `platform` and one backend; CPU fixtures enable `test-support`. |
| `neocogi-platform-dylib` | The final `nc_platform` library and selected `neocogi.platform.<backend>` identity. |

Generated consumers import `neocogi.audio` as well as renderer/platform namespaces.
Compiler interfaces now belong to `neocogi.renderer`. Rendering constructs its own
fixed-format compiler; the independent compiler DLL and descriptor registration
are removed. Native shader-failure coverage uses public resource creation and
checks that diagnostics outlive the window, replacing the old injectable-window
compiler fixture.

Keyboard conversion preserves `KeyCode::Character(u32)`, `KeyCode::Function(u8)`
and `KeyAction::{Pressed { repeat }, Released}`. Canonical validation precedes
conversion to retained `char` values and modifier flags. Tests cover every named
key, all function keys, representative Unicode scalars, all valid modifier/action
combinations and invalid payloads/bits. A release has no repeat payload.

Surface descriptions use their enum variants. Uniform binding lengths belong to
`GpuBindingKind::Uniform`, and blend equations carry only their applicable factors.
These are shared generated contracts; no compatibility representations are retained.
All providers and generated consumers must be rebuilt together.

## Math and typed GPU authoring

OBJ normals and triangle cross products use shared vector operations, including
scale-aware checked normalization. Mesh framing uses the math crate's bounding-box
operations in widened coordinates. Matrix uniforms use `to_cols_array`; vector
component conversion uses `map`, `to_array` and `from_array`.

The mesh client authors vertices, indices and streamed camera elements through
`TypedGpuBufferFactory` and `TypedGpuBuffer`. Plain scalar arrays declare its shader
layout without a local unsafe cast or repeated byte/element range calculations.
Camera uploads use a fixed array rather than allocating an encoded vector per paint.
Real recorded commands and native rendering exercise that layout.

UI coordinate admission, half-open hit regions and positive-area intersection
remain UI policy layered over checked library arithmetic. The math crate's inclusive
rectangle containment is not the UI hit-testing contract. Widened color interpolation
also retains its explicit finite-endpoint clipping policy.

## Native and browser compilation boundaries

Resource recipes call `neocogi_platform_contracts::build::layout::stage_resources`
in build scripts. Both input and deployed output paths are tracked; unchanged
normalized theme JSON preserves its timestamp.

Normal Cargo artifact dependencies expose their paths to the consuming compiler,
not its build script. Moving providers to build artifact dependencies gives them
host-tool profiles, including with `target = "target"`; the actual Cargo unit-graph
regression reproduced this for release and min-size. The development-only
`tools/native-artifacts` compiler hook therefore delegates library staging to
Platform's `build::layout::{output_root, stage_library}` and tracks both endpoints.
It emits a filename and owns no filesystem copy/layout policy. Published UI libraries
do not depend on this nightly hook. Providers retain ordinary artifact dependency
edges and their application's target profile.

The browser input export awaits Platform's asynchronous input/GPU qualification.
Its JavaScript caller awaits the resulting promise before reporting success.

## Verification

Fresh checks used the pinned `nightly-2026-09-29` toolchain and the final registry
lockfile. Cargo builds sharing a target directory ran sequentially.

| Check | Result |
| --- | --- |
| UI/API/document/runtime/nodes/support/design and GFX implementation, `--all-targets` | 614 passed; one manual performance measurement ignored. |
| UI/API/document/runtime/nodes/support/design doctests | 13 passed; two illustrative snippets ignored. |
| Python script regressions | All 78 passed. |
| Independent contract consumers | Three UI event consumers executed; generated-only renderer compiled with target COM facade features limited to `alloc`. |
| Clippy with `-D warnings` | Core UI/GFX and native artifact hook; full/basic/platform tests for Glow, Vulkan and WGPU; full/platform WebAssembly targets. |
| Native full demo, all tests included, offscreen Glow | All 23 passed, including typed mesh authoring and camera framing. |
| Public Platform integration, offscreen Glow | Nine passed; the cost-measurement harness was excluded. |
| Separately linked provider descriptors and relocated host/runtime catalog | Both passed; the catalog has no independent compiler provider. |
| Actual Cargo release/min-size unit graphs | Both profile checks passed. |
| Independent editor-navigation probe | Compiled against the current generated source graph. |
| Monolithic embedded Glow composition and COM benchmark | Both compiled. |
| Real Chromium input and full-demo suites | Both passed, including deferred GPU work and final DOM/animation cleanup. |
| Canonical resource staging fixture | Restored deleted outputs, removed an optional input, rejected a missing required input and preserved the prior package. |
| Exact native distribution | Packaged with the canonical staging APIs; the renamed application rendered three frames without loader-path overrides. Removing the adjacent platform library failed without a build-tree fallback. |

The migration does not claim Windows/macOS execution, Vulkan/WGPU GPU execution,
a new performance benchmark or a complete published-archive release qualification.
Native execution here used Linux offscreen OpenGL. Previous dated performance and
release reports retain their original scope.
