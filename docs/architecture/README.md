# Architecture

- [Component design](COM_DESIGN.md) — compile-time contracts, runtime activation,
  application behavior and deployment responsibilities.
- [Component/plugin boundaries](COMPONENTS.md) — ownership, dependency graph,
  resource policy and frame-cost invariants.
- [Retained UI implementation](../../crates/ui/runtime/docs/ARCHITECTURE.md) —
  forest, concrete surfaces, identity and ownership.
- [Layout](../../crates/ui/runtime/docs/LAYOUT.md),
  [rendering](../../crates/ui/runtime/docs/RENDER.md), and
  [backend ownership](../../crates/ui/runtime/docs/BACKENDS.md).
- [Widgets](../../crates/ui/nodes/docs/WIDGETS.md),
  [menus](../../crates/ui/runtime/docs/MENUS.md),
  [typography](../../crates/ui/runtime/docs/TYPOGRAPHY.md), and
  [skinning](../../crates/ui/runtime/docs/SKINNING.md).

Implementation examples explain provider internals. Applications and independent
plugins should start with the [author contracts](../contracts/COM_AUTHORS.md)
and working [examples](../guides/EXAMPLES.md).
