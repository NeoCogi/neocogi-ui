# Documentation

The root [README](../README.md) gives a short project overview and quick start. The guides here
cover the retained UI model, integration details, and repository workflows.

## COM authors and deployment

- [Author contracts](COM_AUTHORS.md) — change reporting, external state, phase limits, scopes, ownership and error recovery.
- [Native packaging](COM_NATIVE.md) — independent plugin builds, startup order and actual renderer tests.
- [Assets](COM_ASSETS.md) — immutable catalogs, generated resources and transactional installation.
- [Local COM implementation](../crates/com/README.md) and [ABI specification](../crates/com/docs/abi-v3.md).
- [Accepted design](COM_DESIGN.md) and [implementation progress](COM_MIGRATION.md).

## Core concepts

- [Architecture](../crates/microui-ui/docs/ARCHITECTURE.md) — retained trees, concrete surfaces, ownership, and stable identity.
- [Events](../crates/microui-ui/docs/EVENTS.md) — typed widget events, dispatch ordering, roots, and retained services.
- [Built-in widgets](../crates/microui-ui/docs/WIDGETS.md) — control and container selection, typed events, ownership, and
  retained mutation.
- [Layout](../crates/microui-ui/docs/LAYOUT.md) — measurement, tracks, placement, invalidation, and update/paint boundaries.
- [Application menus](../crates/microui-ui/docs/MENUS.md) — concrete registered items, retained composition, popup
  coordination, and Windows-style keyboard/submenu navigation.
- [Typography](../crates/microui-ui/docs/TYPOGRAPHY.md) — semantic font roles, UTF-8 behavior, glyph coverage, and atlas setup.
- [Skin architecture](../crates/microui-ui/docs/SKINNING.md) — concrete roles and states, stable resources, atomic bundles,
  one installed bundle revision, and cache identity.
- [JSON themes](../crates/microui-ui/docs/THEMES.md) — typed appearance roles and states, PNG nine-patches, flat fallbacks,
  bundled classic themes, and window chrome recipes.

## Rendering and integration

- [Rendering](../crates/microui-ui/docs/RENDER.md) — painter and display-list architecture, clipping, textures, backend
  implementation, and performance validation.
- [Backend ownership and typed application rendering](../crates/microui-ui/docs/BACKENDS.md) — plugin-local backend frames,
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
