# NeoCogi UI documentation

NeoCogi UI was formerly **microui-redux**. Version **0.0.0** is a
pre-alpha workspace covering COM components and IDL tooling, shared-library and
plugin deployment, retained UI and rendering, the
[theme editor](../tools/theme-editor/README.md), and the experimental
[UI designer](../tools/ui-designer/README.md).

The root [README](../README.md) gives the project history and quick start. These
guides cover architecture, authoring tools, integration, and repository workflows.

## COM authors and deployment

- [Author contracts](COM_AUTHORS.md) — change reporting, external state, phase limits, scopes, ownership and error recovery.
- [Native packaging](COM_NATIVE.md) — shared component builds, startup order and actual renderer tests.
- [cargo neocogi](../tools/cargo-neocogi/README.md) — installable assembly command, typed TOML configuration, local/registry components, and build policies.
- [Assets](COM_ASSETS.md) — immutable catalogs, generated resources and transactional installation.
- Measured deployment and performance — complete package sizes, memory, startup, tree costs and external counters.
- Assembly build performance — compiler reuse, shared-library LTO, and measured build times.
- COM crate consolidation measurements — five-crate ownership, verified linkage, and before/after Vulkan min-size sizes.
- [IDL contract modules](IDL_MODULES.md) — Rust-like contract syntax, Cargo generation, explicit Rust locations, exact native linking, and factory-only activation.
- [IDL source maps](IDL_SOURCE_MAPS.md) — generated Rust provenance, readable annotations, sidecar format and source lookup.
- [Local COM implementation](../crates/com/README.md) and [ABI specification](../crates/com/docs/abi-v7.md).
- [Accepted design](COM_DESIGN.md), [feature ownership](FEATURES.md), and [unreleased changes](CHANGELOG.md).

## UI implementation notes

The guides below describe the UI plugin's internal algorithms and native behavior
fixtures. Their native node/context examples are implementation references, not the
current application facade API. Application and independent plugin authors should
start with [COM_AUTHORS.md](COM_AUTHORS.md) and the working [examples](EXAMPLES.md).

- [Architecture](../crates/ui/runtime/docs/ARCHITECTURE.md) — retained trees, concrete surfaces, ownership, and stable identity.
- [Events](../crates/ui/runtime/docs/EVENTS.md) — typed widget events, dispatch ordering, roots, and retained services.
- [Built-in widgets](../crates/ui/nodes/docs/WIDGETS.md) — control and container selection, typed events, ownership, and
  retained mutation.
- [Layout](../crates/ui/runtime/docs/LAYOUT.md) — measurement, tracks, placement, invalidation, and update/paint boundaries.
- [Class event streams](CLASS_EVENT_STREAMS.md) — generic COM contracts, one lifetime consumer, generated forms, ownership and state diagrams.
- Owned event controllers — persistent model ownership, validation, and dynamic/monolithic min-size comparison.
- [Application menus](../crates/ui/runtime/docs/MENUS.md) — concrete registered items, retained composition, popup
  coordination, and Windows-style keyboard/submenu navigation.
- [Typography](../crates/ui/runtime/docs/TYPOGRAPHY.md) — semantic font roles, UTF-8 behavior, glyph coverage, and atlas setup.
- [Skin architecture](../crates/ui/runtime/docs/SKINNING.md) — concrete roles and states, stable resources, atomic bundles,
  one installed bundle revision, and cache identity.
- [JSON themes](../crates/ui/runtime/docs/THEMES.md) — typed appearance roles and states, PNG nine-patches, flat fallbacks,
  bundled classic themes, and window chrome recipes.

## Rendering and integration

- [Rendering](../crates/ui/runtime/docs/RENDER.md) — painter and display-list architecture, clipping, textures, backend
  implementation, and performance validation.
- [Backend ownership and typed application rendering](../crates/ui/runtime/docs/BACKENDS.md) — plugin-local backend frames,
  ordered COM render items, resource authentication and explicit device lifetimes.
- [Examples](EXAMPLES.md) — running the demos, backend selection, asset loading, and size-focused
  builds.
- [WebGL demo](WEB.md) — building, serving, and publishing the HTML canvas demo.
- [Cargo features](FEATURES.md) — crate features and atlas-loading combinations.

## Project information

- [COM/UI adversarial review (2026-09-26)](COM_UI_ADVERSARIAL_REVIEW_2026-09-26.md) — full COM and widget/event findings, reproduced cases, contract decisions, and the persistence-limit fix follow-up.
- [Whole-repository adversarial review (2026-09-25)](ADVERSARIAL_REVIEW_2026-09-25.md) — 16 source-based findings, diagrams, contract decisions, and acceptance tests.
- [Changelog](CHANGELOG.md)
- [Support and compatibility](SUPPORT.md) — Rust toolchain, platform, renderer, and API stability
  expectations.
- [Bundled asset attribution and licenses](ASSETS.md)
