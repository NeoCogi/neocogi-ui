# NeoCogi UI documentation

NeoCogi UI, formerly microui-redux, is a pre-alpha workspace for COM and
IDL, plugins, retained UI, rendering and authoring tools. Start with the
[project README](../README.md) for setup and examples.

## Find documentation by purpose

| Folder | Contents |
| --- | --- |
| [Issues](issues/README.md) | Unresolved correctness findings and incomplete contracts |
| [Optimization opportunities](optimizations/README.md) | Performance investigations and optional simplifications, with evidence and acceptance criteria |
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

Issues and optimization opportunities each have one maintained register. Fixes
update their owning contracts/tests and remove the resolved entry. Experiment
reports record the revision actually tested and distinguish prototypes from
applied changes. Measurements retain their workload, artifacts and limits.

New reviews go in `reviews/`. Once reconciled, move unresolved findings into the
appropriate register and retire the report; preserve useful reproduction data
under `external run artifacts`. Completed plans and duplicate historical review ledgers
do not remain as competing descriptions of current behavior.
