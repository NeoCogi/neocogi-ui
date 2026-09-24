# Experimental runtime UI documents

`neocogi-ui-document` reconstructs authored UI components through the COM
persistence factory and the existing runtime-bound widget factories. It depends
on public contracts and clients, never the concrete UI or nodes implementations.
The same document works with shared plugins, a monolith, and WebAssembly.

`Document` is the shared typed authoring schema. It describes the real ownership
tree, constructor values, common options, logical resources, layout edges, and
public input/output declarations plus executable private connections. JSON is the editable source; NCB binary is the
COM persistence envelope for deployment. `Document::to_binary` and
`Document::from_binary` are the export/import boundary. Bytes can come from a
file, an embedded `include_bytes!`, or a browser fetch; the loader does no I/O.

Revision three supports Button, Checkbox, Textbox, TextBlock, TextArea, Slider,
Number, ColorSwatch, ListItem, ListBox, Scrollbar, flat Combo, Linear, Grid,
SplitView, ScrollArea, and Disclosure. Container edges preserve tracks, spans,
child order, and exact cross sizes. A component is the unit of persistence:
its complete private initialization tree is typed state owned by one
`IPersistable` provider. Ordinary widget classes do not gain automatic memory
serialization. Custom widget schemas, nested independently provided components,
window/menu declarations, and hierarchical combo models need explicit future
schema additions; arbitrary Rust objects or plugin code cannot be loaded from
file contents.

## Loading and application contracts

Create `Loader::new(ui, parent_window, resources)` and call `load(binary)` or
`prepare(&document)`. It returns an opaque `Component`; the application sees
only `root()`, `outputs()`, typed `input::<SetText>("title")`, and `save()`.
For each observed child, construction installs exactly one typed observer
connected to its containing form. The form owns the child event queue, matches each event
variant, executes matching visual actions, and may relay the complete event to a
public application binding. A public output gets a separate, form-owned
`exported_observer()` proxy; that proxy is never installed on the child. The
component export table maps one public name per observed child to the proxy's
sole physical port. Rebinding replaces its one downstream application receiver,
while the child's construction-time observer remains the form. Invalid names or
receiver types preserve the previous application destination. The form's child
observer applies the same typed domain checks as a standalone widget observer
before it queues an event, including cursor boundaries and finite or nonnegative
numeric fields from independently supplied node providers. Unobserved children
receive no observer and allocate no event adapter.

Use `neocogi_com::events::bind_output` with the concrete event type declared by
the component's public contract. Wrong types fail at connection time. Inputs
include text, checked, numeric value, interaction enablement, expansion, and
combo-model replacement, combo selection, scrollbar offset, and split position. `SetChoices` uses UTF-8 text plus checked exclusive
label offsets, avoiding a nested untyped payload. Calls are ordinary synchronous
setters; send them at the application's normal safe update boundary.

Internal node keys are unique names, independent of text and sibling position.
They identify editor selection and document binding targets. Applications bind
public names; they do not acquire private widgets or interpret their paths.
Moving a node preserves its key and therefore its authored connections. Each
child event port has exactly one consumer, the form, and may have at most one
public relay.

## Reload and ownership

`ReloadSlot` is a real single-child Linear container. Insert `widget()` into the
host's layout once. `reload(document, resources, bind)` validates the document,
constructs an unmounted tree, prepares named endpoints, and runs application
binding before replacing the old child. A failed preparation preserves the old
component. Candidate cleanup closes partial trees and their combo popups.
The stable parent window provides popup ownership during construction; no
multi-window atomic-publication claim is made.

Each attempt receives a distinct `Generation`. Tag queued application messages
with that value and check `slot.accepts(generation)` before acting. Failed
attempts never reuse a generation. Old input handles fail after their widgets
close; reconnect inputs and outputs for each replacement. The returned
`Reloaded.teardown` is a post-commit diagnostic: an error closing old content
must not be reported as a failed publication of the new content.

Mount `Component` itself, or use `ReloadSlot`, so the runtime retains both its
view and its event owner. `root()` is an inspection capability, not a replacement
for mounting the component's behavior. Unpublished candidates close on release;
mounted owners retire with their subtree at the runtime's safe update boundary. Call these APIs at
ordinary application mutation boundaries, never during layout or paint.

## Resources and authored state

Fonts and icons use names in the supplied UI's immutable asset bundle. Images
use logical keys resolved by the caller's `Resources` directory to
renderer-compatible textures. Missing names fail construction. Files, GPU
handles, bundle indices, and application receivers are not serialized. Resource
file transport and packaging belong to the application/designer.

`Component::save()` preserves authored defaults. User edits in the running form,
focus, scroll positions, popup visibility, and application input commands do
not modify its document. The designer edits `Document` and reloads the preview;
application session persistence should use an explicit application state model.

The binary format and schema are experimental. Ordinals and field order belong
to revision three; older revisions are rejected rather than migrated. Limits
bound document bytes, traversal depth, node count, and decoded values. Validation rejects unknown fields in JSON, unknown revisions, malformed
endpoints, unsupported resources, and invalid numeric/layout policies.

`WidgetSpec::outputs()` returns zero or one physical event stream per widget.
`WidgetSpec::event_variants()` lists the cases the form may match for visual
actions. Textbox and TextArea expose one physical `event` port carrying a whole
`TextboxEvent` or `TextAreaEvent` enum, while `changed` and `submitted` are
logical cases within that enum. Rust handler generation exports the whole enum
under one public name. Validation and the designer share these declarations.
Font/icon/image capability queries are shared by validation and inspector construction; concrete resource setters remain
explicit typed dispatch. Generated Rust symbols collapse punctuation runs and
retain declaration ordinals for collision avoidance. Public endpoint names remain
exact string literals.

Constructor option defaults belong to the authored schema revision and are shared
with the designer. They exclude effective routing bits added by scrolling or
container participation. `SetEnabled` changes the exported surface's interaction
bit against that authored baseline; it is not recursive subtree disablement.

`Form { width, height }` provides fixed-coordinate layout for visually authored
forms. Its children use `Position { x, y, width, height }` edges, expressed in
parent content pixels. Coordinates are nonnegative, sizes positive, and sums
must fit in `i32`. A smaller allocation clips children; it does not scale them.
Sibling order determines painting and hit priority. Linear/Grid/Split policies
remain available inside a positioned control or as an application's root.

## Typed preparation and endpoint cardinality

`Loader::prepare` validates an in-memory document and constructs through the same
concrete service context as binary restoration. It does not serialize and decode
an editable document merely to rebuild a preview. `load` retains bounded COM
persistence decoding for external and embedded binary files.

Each `(node, input kind)` has exactly one public name, matching the inspector's
single endpoint field. Duplicate aliases are rejected before construction or
editing; changing a binding can no longer silently delete another accepted alias.

`SetSelection { selected }` uses a combo ordinal, with `-1` clearing selection.
Unknown ordinals and other negative values fail without changing selection.
`SetOffset { offset }` clamps to the scrollbar's current lengths.
`SetPosition { position }` changes the requested split extent and retains its live
axis, pane minima, and divider width. These commands are silent programmatic
changes; they do not publish user events or rewrite authored defaults. All input
kinds and their inspector labels come from `InputKind::ALL` and `InputKind::label`.

## Executable connections

Revision three requires a `connections` array. Each action names `source`, `event`,
`target`, and a typed `action`, for example:

```json
{"source":"name","event":"changed","target":"title","action":{"Forward":"Text"}}
```

`Forward` accepts compatible payload fields: checkbox state to checked/enabled/
expanded, text edits or submissions to text, numeric values to value, pixel
positions/offsets to position/offset, and combo selection to selection. Constant
variants (`Text`, `Checked`, `Value`, `Enabled`, `Expanded`, `Selection`, `Offset`,
`Position`) work with any source event, including button activation. An
activation also carries the source control's captured label, so an activation
route can forward that string to a `SetText` destination. Model replacement
remains an application input rather than an event transformation.

The child event port has one consumer: its form. A form may apply multiple
actions to one event case and also forward the complete event through one public
export. These are form effects, not additional child-port consumers. Validation
rejects multiple public names for one child, missing endpoints, incompatible
forwarded payloads, and invalid constants before construction. Target widgets
still apply their current range policies; a forwarded selection absent from the destination model reports an error.

Observers only capture facts into the mounted component's queue. Its controller
applies resolved concrete commands after traversal releases widget borrows. Silent
setters prevent cyclic connections from recursively publishing new user events.
Closing/replacing the subtree retires its controller and discards pending work at
the next safe update boundary. No arbitrary code or erased Rust values enter the
serialized graph.

## Generated application bindings

`Document::rust_bindings()` emits a standalone Rust module. The designer exposes
this as **File → Export Rust bindings…**; the command-line equivalent is:

```sh
cargo run -p neocogi-ui-document --example export_bindings -- form.ui.json > bindings.rs
```

The module contains `Event`, generation-tagged `Message`, concrete `Inputs`, a
required `Handlers` trait, and `bind`. Use `bind` in the `ReloadSlot::reload`
preparation callback with a channel sender. At the application's normal update
boundary, check `slot.accepts(message.generation)` and then call
`message.event.dispatch(&mut handlers)`. Implement `Handlers` in application code
outside the generated file; missing handlers are compile errors. Reconnect input
handles after every replacement.

Public names are preserved exactly as escaped Rust string literals. Generated
method/field identifiers use a role prefix, normalized name, and declaration
ordinal, so keywords, punctuation, and normalization collisions remain valid.
Regenerate after changing public declarations. Private visual connections need no
application handlers and are not exported into the binding module.
