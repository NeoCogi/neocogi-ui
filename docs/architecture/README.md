# Architecture

- [Widget and event model](WIDGET_EVENT_MODEL.md) — retained ownership, typed
  queues, composite/application inboxes, update scheduling and callback boundaries.
- [Component design](COM_DESIGN.md) — compile-time contracts, runtime activation,
  application behavior and deployment responsibilities.
- [Component/plugin boundaries](COMPONENTS.md) — ownership, dependency graph,
  resource policy and frame-cost invariants.
- [Retained UI implementation](../../crates/ui/docs/runtime/ARCHITECTURE.md) —
  forest, concrete surfaces, identity and ownership.
- [Layout](../../crates/ui/docs/runtime/LAYOUT.md),
  [rendering](../../crates/ui/docs/runtime/RENDER.md), and
  [backend ownership](../../crates/ui/docs/runtime/BACKENDS.md).
- [Widgets](../../crates/ui/docs/nodes/WIDGETS.md),
  [menus](../../crates/ui/docs/runtime/MENUS.md),
  [typography](../../crates/ui/docs/runtime/TYPOGRAPHY.md), and
  [skinning](../../crates/ui/docs/runtime/SKINNING.md).

Implementation examples explain provider internals. Applications and independent
plugins should start with the [author contracts](../contracts/COM_AUTHORS.md)
and working [examples](../guides/EXAMPLES.md).
