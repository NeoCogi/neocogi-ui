# Three-crate UI layout, 2026-11-09

The reusable UI release advances to `0.2.0-alpha.2` and follows Platform's
contracts/static/dylib model. The previous nineteen release packages are replaced
by exactly three. Platform remains pinned to `0.2.0-alpha.2`, rs-math3d to `0.21.0`
and COM/IDL to `0.1.0`; the preceding dependency/API migration remains in effect.
This record supersedes earlier package/deployment descriptions, without changing
the evidence or measurements in dated reports.

## Ownership and activation

`neocogi-ui-contracts` owns canonical UI, node, GFX, Skin and document declarations,
typed clients/events, bounded immutable resource and appearance values, design
metadata and the optional packaged schema/build API. It has no retained engine,
concrete widgets, PNG/font construction, native renderer or theme-file parser.
Its 32 shared NCIDL sources include the new typed library-root contract.

`neocogi-ui` reexports those contracts and owns feature-selected `runtime`, `nodes`,
`support`, `gfx`, `skin`, `authoring` and `document` modules. Defaults remain empty.
Private retained/test protocols and concrete CPU-device test fixtures belong here.
The merged unit suites share one opt-in thread-local allocation probe; existing
runtime and dialog allocation assertions still observe the same operations.

`neocogi-ui-dylib` is a thin `nc_ui` cdylib forwarding implementation features and
the discovery call. It contains no second factory or descriptor implementation.
Examples, tools, sample contracts and the independent extension remain separate
unpublished Cargo packages. There are no compatibility wrapper crates.

The sole UI identity is `neocogi.ui`, with a canonical `UiLibraryFactory` root.
Its `ui()`, `nodes()`, `gfx()` and `skin()` getters return fresh, independently owned
factories; disabled services return `NC_E_NOT_FOUND`. Dropping the root does not
revoke a returned factory. Existing optional builder/theme methods retain their
capability errors. The root has no hard implementation-plugin dependencies.

Nodes still binds public runtime construction capabilities and authenticates
canonical widget-host identity. GFX publication, Skin compilation, document
construction, exclusive widget ownership, deferred event delivery and frame
commit boundaries retain their existing typed protocols. Package consolidation
introduces no untyped registry, hidden runtime pointer or duplicated asset model.

Native compositions stage UI and Platform DLLs, plus the independent extension
when selected. Static/browser compositions register the same service roots. The
host executable owns one COM runtime and supplies access through bootstrap.
Former runtime/node/GFX/Skin plugin identities and DLLs are removed. Feature
selection and native target-profile ownership remain explicit; no size or speed
improvement is claimed from the crate merge alone.

## Consumers and package qualification

Examples, the independent extension, asset exporters, generated bindings,
native artifact selection, browser fixtures, CI and current guides use the new
owners. Contract-only event and editor-navigation consumers depend directly on
`neocogi-ui-contracts`. Form-export tests live with document implementation in
the SDK rather than creating an implementation dependency in contracts tests.

Release qualification admits exactly the three public packages, requires exact
internal registry versions, inspects their normalized archives and compiles only
extracted internal source. Regression fixtures are package-contained derivatives
of canonical repository sources. The SDK carries the example ABI/schema needed
by its own document tests, without publishing a fourth sample package. The old
post-extraction dev-dependency restoration mechanism is removed.

Implementation unit suites explicitly select the `test-support` fixture bundle.
Separate service integration tests compile the SDK as an ordinary dependency
under each production feature selection, including omitted services. The
UI compile-contract fixture declares the `ui` feature explicitly in an
independent temporary consumer. Current nightly Cargo's output layout prevents
trybuild's legacy fingerprint lookup from discovering parent features. Explicit
selection keeps the positive and negative API cases meaningful in both checkout
and archive tests without enabling UI in default-feature library tests.

## Validation

All results below are fresh checks of this refactor. Logs are retained under
`/tmp/ui-three-*.log`, outside Cargo targets; historical qualification logs were
not reused.

| Boundary | Actual validation |
| --- | --- |
| Contracts | `cargo test --offline -p neocogi-ui-contracts --all-features`: 33 unit tests plus generated consumers, integration tests, borrowed-event compile rejection and doctests passed. |
| SDK | All-feature unit/integration/doctest suites passed; the final unit run has 575 passes and one existing ignored case. The fixture-only selection has 530 passes and one ignored case. |
| Feature selection | Service integration consumers passed with defaults, `ui`, `publisher`, `gfx`, `builder`, `skin`, `authoring`, `theme-json`, `document` and `save-to-rust`; getters, optional atlas construction and factory lifetime were exercised through real COM calls. |
| Public compile API | `cargo test --offline -p neocogi-ui --all-features --test ui` passed all positive and negative ownership/borrow/privacy cases with the independent consumer explicitly selecting `ui`. |
| Warnings | All three public crates passed all-feature/all-target Clippy with `-D warnings`. Full/basic/Platform consumers passed the same warning policy independently for Glow, Vulkan and WGPU; full/Platform consumers passed it for wasm WebGL. |
| Native activation | Host tests and real Cargo DLL discovery/runtime-catalog tests passed, including the independent extension, null descriptor output and renamed DLL layouts. |
| Native behavior | `SDL_VIDEODRIVER=offscreen scripts/test-demo.sh glow`: 23 full-demo tests and nine Platform regressions passed. Extension smoke executed successfully; the COM benchmark compiled. |
| Deployment | `scripts/test-native-build.py` passed both target profiles. `scripts/test_package_native.py --production` passed exact two-library membership, Skin/backend cache switches, private artifact capture, concurrent publication and required-input failures. Private-path inspection and relocated full-demo startup passed; removing a required UI/Platform DLL or font failed without fallback. |
| Independent consumers | Three event consumers and the generated-only renderer control passed. The target-only COM facade retained exactly `alloc` in the control. All 13 actual rust-analyzer definitions and exact authored IDL origins passed. |
| Source export | `scripts/test-asset-export.py` passed dev/min-size generation, byte/field comparison and compiled COM bundle consumers. |
| Browser | The debug WebGL application built; both real headless Chromium `input` and `full` suites passed, including IME, theme/image actions, context/resource behavior and teardown. Wasm all-target Clippy passed with warnings denied. |
| Archives | All three actual archives passed member/schema/dependency inspection, extracted all-feature unit/integration/compile-contract/doctest suites, independent linked and real DLL consumers, and consumer Clippy with warnings denied. The linked consumer also passed on stable Rust 1.98.1. Archives and checksums are retained in `/tmp/neocogi-ui-three-archives`; the final gate log is `/tmp/ui-three-archives-low-memory.log`. |
| Tooling | All 78 Python regressions, derived-input freshness and `cargo fmt --all --check` passed. |

The archive gate uses `--offline --allow-dirty --stable-toolchain stable` for
development qualification of this change; no publication was requested or run.
Its final run uses a disk-backed `TMPDIR` outside the checkout,
`CARGO_BUILD_JOBS=1`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`
and `CARGO_INCREMENTAL=0`. An interrupted attempt had retained 9.2 GiB on the
RAM-backed `/tmp`; that abandoned target was removed before the rerun.

Live native rendering was exercised with Glow. Vulkan/WGPU were compiled and
linted here; macOS/Windows execution remains covered by CI rather than a local
run. Existing ignored performance/schematic examples were retained, and no
performance improvement or compiler floor is inferred from these checks.

The [feature guide](../guides/FEATURES.md), [component ownership](../architecture/COMPONENTS.md)
and [release procedure](RELEASING.md) define the current normative contracts.
