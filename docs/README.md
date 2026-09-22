# Documentation

The root [README](../README.md) gives a short project overview and quick start. The guides here
cover the retained UI model, integration details, and repository workflows.

## COM authors and deployment

- [Author contracts](COM_AUTHORS.md) — change reporting, external state, phase limits, scopes, ownership and error recovery.
- [Native packaging](COM_NATIVE.md) — shared component builds, startup order and actual renderer tests.
- [cargo neocogi](../tools/cargo-neocogi/README.md) — installable assembly command, typed TOML configuration, local/registry components, and build policies.
- [Assets](COM_ASSETS.md) — immutable catalogs, generated resources and transactional installation.
- Measured deployment and performance — complete package sizes, memory, startup, tree costs and external counters.
- Assembly build performance — compiler reuse, shared-library LTO, and measured build times.
- COM crate consolidation measurements — five-crate ownership, verified linkage, and before/after Vulkan min-size sizes.
- [IDL contract modules](IDL_MODULES.md) — Rust-like contract syntax, Cargo generation, explicit Rust locations, exact native linking, and factory-only activation.
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

- [Changelog](CHANGELOG.md)
- [Support and compatibility](SUPPORT.md) — Rust toolchain, platform, renderer, and API stability
  expectations.
- [Bundled asset attribution and licenses](ASSETS.md)
