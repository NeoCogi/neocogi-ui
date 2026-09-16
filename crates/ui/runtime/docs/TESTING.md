# Testing the deployed UI path

UI runtime tests and deployed consumers use the same constructor, retained context,
COM node representation, scoped child-layout service and GFX compositor. Test
compilation can add fixtures and read-only observations; it must not select an
alternate runtime, renderer executor or object model.

## Fixture responsibilities

| Fixture | Responsibility | Ownership |
| --- | --- | --- |
| `test_support::runtime_from_assets` | Create an independent real GFX compositor and call the production two-argument UI constructor | Each runtime gets its own compositor |
| `test_support::headless::Headless` | Drive public `Ui` input, update and render; copy final renderer streams | One shared observation cell is required by `IRenderer`'s shared receiver |
| `test_support::algorithm::Algorithm<W>` | Keep a real host alive for focused tests of a concrete built-in algorithm | The host lives beside `W`; `W` retains its production representation and uses ordinary mutable borrows |
| `test_support/assets.rs` | Publish deterministic canonical input tables and expected revisions | Explicit pixel transfers happen here, outside deployed resource import |
| Runtime callback probes | Act as independent COM widgets, containers, renderers and resources, including deliberately failing/reentrant components | Concrete fields and typed interfaces; no `Any`, downcasting or native callback registry |

A renderer fixture reads `IRenderStream` only while its invocation token is active.
It may keep copied vertices and batches for assertions. Tests that deliberately
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
| Combo popup ownership, optional stable selection, serialized hierarchy validation, nested-path restoration, fallback and keyboard commit | `src/runtime/tests/combo.rs`, `src/combo_box.rs` |
| Nested menu navigation, live row edits, disabled entries, standalone popup submission and focus restoration | `src/runtime/tests/menus.rs` |
| Eventless phases, cached measurement, typed invalidation and independent auto-size axes | `src/runtime/tests/retained.rs` and `retained/policies.rs` |
| Layer order, root families, clipping, modal scopes, popup dismissal, chrome movement/resize and window events | `src/runtime/tests/retained/policies.rs` and pure `root_chrome` geometry tests |
| Tab order, exclusive capture, removal/replacement, cancellation and visibility | `src/runtime/tests/retained/policies.rs` |
| Partial-paint cleanup and subsequent successful submission | `src/runtime/tests.rs` and `retained/resources.rs` |
| Image ownership, failed edits, texture dimensions, stream ordering and clipping | `src/runtime/tests/images.rs`, `src/render/compositor/tests.rs` and `tests/image_api.rs` |
| Text fallback/newlines, nine-patches, triangle attributes and both windings, hidden draws and extreme integer rectangles | `src/render/compositor/tests.rs` |
| Atomic asset installation, catalog identity, measurement revision and failed upload | `src/assets/tests.rs` |
| File-picker lifecycle and callback transactions | `src/runtime/dialogs/tests.rs` |

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

## Validation commands

Run the isolated package tests first so workspace feature unification cannot hide
missing direct dependencies:

```sh
cargo test -p neocogi-ui-impl --offline
cargo clippy -p neocogi-ui-impl --offline --lib --tests -- -D warnings
cargo check -p neocogi-ui-runtime -p neocogi-ui-nodes --offline --no-default-features --features dynamic-plugin
```

`--offline` assumes dependencies are already cached. The implementation suite includes
headless runtime tests, compositor integration, public API compile fixtures and
guide doctests. The two plugin checks independently compile both loader roots. These commands
require no window system or GPU. Broader workspace validation
uses `cargo test --workspace --offline`; actual native device and presentation
checks still use the platform-specific scripts described in `BACKENDS.md`.
