# NeoCogi UI documentation

NeoCogi UI, formerly microui-redux, is a pre-alpha workspace for COM and
IDL, plugins, retained UI, rendering and authoring tools. Start with the
[project README](../README.md) for setup and examples.

## Find documentation by purpose

| Folder | Contents |
| --- | --- |
| [Issues](issues/README.md) | Navigation to findings and investigations in the consolidated review |
| [Optimization opportunities](optimizations/README.md) | Navigation to performance and simplification work in the consolidated review |
| [Contracts](contracts/README.md) | Current API, ABI, ownership, event, document and resource rules |
| [Experiments](experiments/README.md) | Prototype reports, hypotheses, outcomes and implementation status |
| Measurements | Historical performance/size results, raw data, fixtures and reproduction commands |
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

One consolidated review owns current findings, investigations, decisions and
optimization opportunities. Issue and optimization pages link into it. Fixes
update its disposition and the owning contracts/tests. Experiment reports record
the revision actually tested and distinguish prototypes from applied changes;
measurements retain workloads, artifacts and limits.

Reconcile future reviews into one current report under `reviews/` and archive
superseded snapshots. Retain useful evidence under `external run artifacts`; do not maintain
competing current ledgers.
