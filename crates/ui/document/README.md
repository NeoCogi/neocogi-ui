# Experimental runtime UI documents

`neocogi-ui-document` reconstructs authored UI components through the COM
persistence factory and the existing runtime-bound widget factories. It depends
on public contracts and clients, never the concrete UI or nodes implementations.
The same document works with shared plugins, a monolith, and WebAssembly.

`Document` is the shared typed authoring schema. It describes the real ownership
tree, constructor values, common options, logical resources, layout edges, and
public input/output declarations. JSON is the editable source; NCB binary is the
COM persistence envelope for deployment. `Document::to_binary` and
`Document::from_binary` are the export/import boundary. Bytes can come from a
file, an embedded `include_bytes!`, or a browser fetch; the loader does no I/O.

Revision one supports Button, Checkbox, Textbox, TextBlock, TextArea, Slider,
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
When loading a node with exported outputs, construction supplies its generated
`exported_observer()` before the node factory returns. That one observer also
implements `INamedOutputs`; the component export table retains this capability
and maps public names to concrete observer ports. Nodes without exports receive
no observer. Installing a named target checks its exact receiver interface and
updates only the downstream destination; the widget's observer remains fixed.
An exported post uses one additional concrete COM forwarding call, with no
per-post name lookup and no intermediate owned message copy.

Use `neocogi_com::events::bind_output` with the concrete event type declared by
the component's public contract. Wrong types fail at connection time. Inputs
include text, checked, numeric value, interaction enablement, expansion, and
combo-model replacement. `SetChoices` uses UTF-8 text plus checked exclusive
label offsets, avoiding a nested untyped payload. Calls are ordinary synchronous
setters; send them at the application's normal safe update boundary.

Internal node keys are unique names, independent of text and sibling position.
They identify editor selection and document binding targets. Applications bind
public names; they do not acquire private widgets or interpret their paths.
Moving a node preserves its key and therefore its authored connections. Each
concrete event port has one destination and may be exported only once.

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

Retain `Component` for the mounted view's lifetime, or let `ReloadSlot` retain
it. Releasing its final persistence owner closes its root. Call these APIs at
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
to revision one; incompatible changes require a revision and an explicit
migration. Limits bound document bytes, traversal depth, node count, and decoded
values. Validation rejects unknown fields in JSON, unknown revisions, malformed
endpoints, unsupported resources, and invalid numeric/layout policies.

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

These variants append to the experimental revision-one enum tables. Existing
revision-one documents retain their binary encoding and load in the new reader;
older readers reject the new variants instead of misinterpreting them. Applications
using forms must deploy matching updated node contracts and runtime providers.
