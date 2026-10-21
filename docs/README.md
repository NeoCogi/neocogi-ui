# NeoCogi UI documentation

NeoCogi UI, formerly microui-redux, is a pre-alpha workspace for COM and
IDL, plugins, retained UI, rendering and authoring tools. Start with the
[project README](../README.md) for setup and examples.

## Find documentation by purpose

| Folder | Contents |
| --- | --- |
| [Optimization opportunities](optimizations/README.md) | Navigation to performance and simplification work in the open review backlog |
| [Contracts](contracts/README.md) | Current API, ABI, ownership, event, document and resource rules |
| [Experiments](experiments/README.md) | Consolidated prototype outcomes, retained decisions and implementation status |
| [Architecture](architecture/README.md) | Component boundaries and implementation structure |
| [Guides](guides/README.md) | Authoring, deployment, examples and tooling instructions |
| [Validation](validation/README.md) | Coverage, compatibility and maintained acceptance checks |
| [Project](project/README.md) | Changelog, support policy, attribution and licenses |

Crate-specific documentation stays beside its implementation and is linked from
these indexes. Repository-wide documents belong in the folder matching their
purpose; this directory contains only the main index.

## Common starting points

  owns current findings, investigations, decisions and simplification work.
- [Widget and event architecture](architecture/WIDGET_EVENT_MODEL.md) explains
  ownership, message flow and how an update reaches a renderable state.
- [COM author contracts](contracts/COM_AUTHORS.md),
  [IDL language](contracts/IDL_MODULES.md), and
  [current COM ABI](../crates/com/docs/README.md).
- [Application and widget dispatch](contracts/WIDGET_EVENT_DISPATCH.md),
  [typed event queues](contracts/CLASS_EVENT_STREAMS.md), and
  [exclusive widget ownership](contracts/EXCLUSIVE_WIDGET_OWNERSHIP.md).
- [Native deployment](guides/COM_NATIVE.md), [examples](guides/EXAMPLES.md),
  [WebGL](guides/WEB.md), and [cargo neocogi](../tools/cargo-neocogi/README.md).
- [Theme editor](../tools/theme-editor/README.md) and
  [UI designer](../tools/ui-designer/README.md).

## Keeping the categories current

One open review backlog owns remaining findings, investigations, decisions and
optimization opportunities. The optimization index links into it. Fixes
record verification, update owning contracts/tests and remove completed entries.
Experiment reports record the revision actually tested and distinguish prototypes
from applied changes and retain workload limits. Raw run artifacts stay outside Git.

validation. Keep raw run artifacts outside version control; do not maintain
competing current ledgers.
