# Testing the deployed UI path

UI runtime tests and deployed consumers use the same constructor, retained context,
COM node representation, scoped child-layout service and GFX compositor. Test
compilation can add fixtures and read-only observations; it must not select an
alternate runtime, renderer executor or object model.

## Fixture responsibilities

| Fixture | Responsibility | Ownership |
| --- | --- | --- |
| `test_support::runtime_from_assets` | Activate the real GFX factory and call the production UI constructor | The runtime creates and privately owns its compositor |
| Support `test_support::gfx_factory` | Register and activate the statically linked production GFX plugin | Fixtures share a stateless service; each construction returns independent mutable state |
| `test_support::headless::Headless` | Drive public `Ui` input, update and render; copy final renderer streams | One shared observation cell is required by `IRenderer`'s shared receiver |
| Nodes `test_support::algorithm::Algorithm<W>` | Keep a real host alive for focused tests of a concrete built-in algorithm | The host lives beside `W`; `W` retains its production representation and uses ordinary mutable borrows |
| Support `test_support/assets.rs` | Publish deterministic canonical input tables and expected revisions | Explicit pixel transfers happen here, outside deployed resource import |
| Runtime callback probes | Act as independent COM widgets, containers, renderers and resources, including deliberately failing/reentrant components | Concrete fields and typed interfaces; no `Any`, downcasting or native callback registry |

A renderer fixture reads `IRenderStream` block observers only while its invocation
token is active. The shared GFX test helper authenticates real generic snapshots and
decodes GFX's explicit vertex bytes and sampled bindings into copied geometry and
`UiBatch` observations for assertions. Those observations are not a renderer wire API. Tests that deliberately
retain the stream verify rejection after the renderer returns, including failures.
GFX owns that rejection status (`NC_E_INVALID_ARG`); UI scope errors do not replace
renderer contract behavior.

Fixtures construct `AssetData` and use the shared appearance compiler, then pass through
`AssetData::validate` before direct algorithm use. UI import itself reads only metadata and the
white-tile data required by portable validation, just as it does outside tests. Republishing a
fixture clones its canonical tables; it does not export a UI-specific atlas or skin representation.
Exports of already imported revisions explicitly read the source image and do not make deployed
runtime import copy full pixel buffers.

## Coverage migration

The former test suite included an independently maintained `Context`, generic UI
executor, external render-stream adapter, native widget/child owners, borrowed
child-layout adapter and application callback dispatcher. These implementations
and their supporting APIs have been removed.

The replacement coverage is organized by observable behavior:

| Behavior formerly checked through native fixtures | Current production-path coverage |
| --- | --- |
| Runtime construction, authentication, mount/close, failed/reentrant callbacks, scope expiry | `src/runtime/tests.rs` |
| Nested child ownership, cycle rejection, failed measurement/placement, scoped reentry and teardown | `src/runtime/tests/containers.rs` |
| Linear/grid relationships, spans, flexible tracks and changed metadata | `src/runtime/tests/containers.rs`, plus pure container arithmetic tests |
| Scroll composition, boundary bubbling, disclosure participation and multiline caret reveal | `src/runtime/tests/containers.rs`, plus focused editor tests |
| Event FIFO, immutable snapshots, handler errors, cancellation, recursion and bounded dispatch | `src/runtime/tests/events.rs` and facade event tests |
| Combo popup ownership, optional stable selection, serialized hierarchy validation, nested-path restoration, fallback and keyboard commit | Core `src/runtime/tests/combo.rs` and `src/choice_menu.rs`, nodes `src/widgets/combo.rs` |
| Nested menu navigation, live row edits, disabled entries, standalone popup submission and focus restoration | `src/runtime/tests/menus.rs` |
| Eventless phases, cached measurement, typed invalidation and independent auto-size axes | `src/runtime/tests/retained.rs` and `retained/policies.rs` |
| Layer order, root families, clipping, modal scopes, popup dismissal, chrome movement/resize and window events | `src/runtime/tests/retained/policies.rs` and pure `root_chrome` geometry tests |
| Tab order, exclusive capture, removal/replacement, cancellation and visibility | `src/runtime/tests/retained/policies.rs` |
| Partial-paint cleanup and subsequent successful submission | `src/runtime/tests.rs` and `retained/resources.rs` |
| Image ownership, failed edits, texture dimensions, stream ordering and clipping | `src/runtime/tests/images.rs`, `src/render/compositor/tests.rs` and `tests/image_api.rs` |
| Text fallback/newlines, nine-patches, triangle attributes and both windings, hidden draws and extreme integer rectangles | `src/render/compositor/tests.rs` |
| Atomic asset installation, catalog identity, measurement revision and failed upload | `src/assets/tests.rs` |
| Shared factory with distinct resource revisions or unrelated catalogs, failed/successful installation, independent closure, and overlapping standalone frames | `tests/compositor_boundary.rs` |
| Owned failure text/status through UI and GFX, renderer destruction, later failures and successful retry | `tests/compositor_boundary.rs` |
| Explicit compositor permissions and image-group rejection at the window framebuffer | GFX `src/compositor/tests.rs` and `tests/platform/src/native/generic_gpu.rs` |
| File-picker lifecycle and callback transactions | Nodes `src/dialogs/tests.rs` |

Tests specific to deleted weak native handles, legacy texture IDs, native callback
registries and the alternate context's panic/borrow APIs are retired with those
APIs. Their public ownership and failure counterparts are checked through COM.
Legacy clipper implementation details are no longer asserted against a second
algorithm; final geometry is checked through GFX. Test counts therefore differ
from the old suite and are not a one-to-one measure of retained coverage.

The old runtime regression file was also disconnected from Rust's module tree.
`runtime.rs` now explicitly includes it. Earlier reports of 440 passing UI tests
did not include that file's failed-paint, scope and container regressions.

## Performance assertions

The warmed painter test measures production command recording and polygon scratch
reuse. Stable-frame tests also count actual foreign measurement callbacks, so a
cache regression cannot hide behind a native node adapter. The deleted executor's
allocation counters and ignored native benchmarks did not measure COM deployment
cost and are not a production performance baseline.

`SDL_VIDEODRIVER=offscreen cargo test --offline -p neocogi-platform-plugin --lib
measure_ui_and_mixed_client_costs -- --ignored --nocapture --test-threads=1`
measures a real retained Button and the same scene with a procedural client block
on the production GL executor. It reports setup/UI recording, cold preparation,
and 100 warm preparation/completed-execution samples at 64 × 64. Immutable resource
version counts describe the submitted workload. Separate typed test-only GL loader
wrappers count actual `glBufferData`, `glTexImage2D` and `glLinkProgram` calls in
setup/cold/warm phases; they do not count driver allocations. Native `glFinish`
is included, readback/presentation are excluded. This
small fixed scene is a reproducible qualification sample without timing thresholds.

Use `scripts/measure-deployment.py <fresh-app> --executable <name>
--configuration <profile-target-features-workload> --output <record.json>` to count
the executable and actual `lib/` providers, including the shader compiler. The
workspace-pinned compiler and exact hashes are recorded; assets/debug sidecars are
excluded. Comparisons permit fresh paths and report added/removed providers while
requiring a matching compiler and configuration key. Optional ELF section detail
does not limit portable byte/hash measurements.

## Validation commands

Run the isolated package tests first so workspace feature unification cannot hide
missing direct dependencies:

```sh
cargo test -p neocogi-ui-runtime --offline
cargo test -p neocogi-ui-nodes --offline
cargo test -p neocogi-ui-support --offline
cargo clippy -p neocogi-ui-runtime -p neocogi-ui-nodes -p neocogi-ui-support --offline --all-targets -- -D warnings
cargo check -p neocogi-ui-runtime-dylib --lib --locked --offline
cargo check -p neocogi-ui-nodes-dylib --lib --locked --offline
```

`--offline` assumes dependencies are already cached. Core retains headless runtime tests,
compositor integration, public API compile fixtures and guide doctests. Concrete node and picker
tests live in nodes; pure-helper tests live beside the production algorithms in support. Nodes
fixtures construct the production runtime through public interfaces and never observe private
core storage. The provider-boundary integration test activates both UI plugin roots and the GFX factory,
rejects cross-runtime factory binding, and delivers real node events through core routing.
The two plugin checks independently compile both loader roots. These commands
require no window system or GPU. Broader library validation selects the reusable
packages explicitly, as in [CI](../../../../.github/workflows/rust.yml):

```sh
cargo test -p neocogi-ui -p neocogi-ui-document --features neocogi-com/runtime --offline
```

Native applications select one linkage and renderer per build. For example,
check the dynamic and monolithic graphs separately:

```sh
cargo check -p neocogi-demo-full --no-default-features --features dynamic,glow --offline
cargo check -p neocogi-demo-full --no-default-features --features monolith,glow,embed-assets --offline
```

These native checks need the platform prerequisites in
[BACKENDS.md](BACKENDS.md). The [feature policy](../../../../docs/guides/FEATURES.md)
defines the supported separate graphs. An aggregate workspace test mixes dynamic
discovery exports with static/test providers and is not a supported substitute.
Actual native device and presentation checks use the backend-specific scripts;
successful compilation does not establish rendering acceptance.
