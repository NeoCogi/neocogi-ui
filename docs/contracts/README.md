# Contracts

These guides define current behavior. For component responsibilities see
[architecture](../architecture/README.md); for setup and tooling see
[guides](../guides/README.md). Package-owned ABI specifications remain with COM.

## COM and IDL

- [COM author responsibilities](COM_AUTHORS.md).
- [Current COM ABI](../../crates/com/docs/README.md) and
  [local typed ownership](../../crates/com/api/README.md), including `WeakLocalCom`.
- [Object/interface storage](COM_INTERFACE_STORAGE.md) and
  [lifetime errors](COM_LIFETIME_ERRORS.md).
- [IDL language and generation](IDL_MODULES.md),
  [generic values](IDL_GENERIC_VALUES.md),
  [event metadata](IDL_EVENT_METADATA.md), and
  [generated identifier hygiene](COM_GENERATED_IDENTIFIER_HYGIENE.md).
- [Recursive wire values](RECURSIVE_VALUE_CONTRACT.md),
  [directional output buffers](COM_OUTPUT_BUFFERS.md), and
  [consuming-call output completion](COM_OUTPUT_COMPLETION.md).
- [NCB persistence](../../crates/com/core/README.md#why-persistence-is-ncb-only) and
  [independent-build compatibility](../../crates/com/docs/compatibility.md).

## Events and UI ownership

- [Exclusive COM event objects and the common receiver](COM_EVENT_INTERFACES.md).
- [Typed event queues and weak origins](CLASS_EVENT_STREAMS.md).
- [Owner-scoped widget origins](WIDGET_EVENT_ORIGINS.md).
- [Widget/application dispatch](WIDGET_EVENT_DISPATCH.md).
- [Exclusive widget ownership](EXCLUSIVE_WIDGET_OWNERSHIP.md).
- [Provider-owned constructor defaults](WIDGET_CONSTRUCTOR_DEFAULTS.md).
- [Runtime shutdown](UI_RUNTIME_SHUTDOWN.md) and
  [runtime event guide](../../crates/ui/runtime/docs/EVENTS.md).
- [Grid placement and bounds](GRID_PLACEMENT.md).

Widgets retain one exclusive owner. Private dispatch is an optional capability on
that allocation; snapshots are transient and actual descendants precede later
application/input work. Application behavior is `MessageProcessor`, its inbox
owner is `Dispatcher`, and deferred work reports `ApplyOutcome`. Unbounded
application channels and whole-module IDL identity are deliberate policies.

## Resources and authoring

- [Immutable assets and themes](COM_ASSETS.md).
- [Independent render surfaces](RENDER_SURFACES.md).
- [Document resource admission](DOCUMENT_RESOURCE_ADMISSION.md).
- [Reload generation identity](RELOAD_GENERATION_IDENTITY.md).
- [Plugin-owned designer settings](PLUGIN_DESIGNER_SETTINGS.md).
- [Theme editor transactions](THEME_EDITOR_TRANSACTIONS.md).

Provider sticky geometry and conservative host notification work together.
Explicit Reset/Open intent may require preview replacement even when recipes are
equal. Weak-local ownership is implemented; it does not imply a runtime-storage
contracts describe accepted behavior and must not present proposed changes as
already implemented.
