# Authored UI documents and generated forms

`neocogi-ui-document` reconstructs an authored widget tree through the existing
COM widget factories. JSON is editable source; the NCB persistence envelope is
suitable for embedding or deployment. The loader performs no file I/O. Both
representations use schema revision 7, and earlier revisions are rejected.

Binary save and load apply the same byte, collection, value, and nesting limits.
`Document::to_binary` and `Component::save` reject over-budget initialization
instead of returning bytes that their corresponding readers cannot accept under
the same policy. The 16 MiB byte ceiling does not override the one-million-value
limit: the persistence envelope stores the encoded document as `Vec<u8>`, so each
state byte counts as one collection element and one value, alongside the outer
record's other values. This is intentional collection accounting, not an implicit
binary-blob representation. See the [binary limit definitions](../../com/core/README.md#binary-resource-limits).

Typed `Loader::compile` uses `Document::validate_binary()` before cloning the recipe
or calling any widget factory. It checks both persistence layers without allocating
encoded output. `Document::to_binary()` uses the same preflight before its temporary
inner encoding. Shape validation alone (`Document::validate`) does not promise binary
saveability; the current outer value budget permits at most 999,979 encoded state
bytes. JSON source retains its independent 16 MiB policy.

Document JSON and catalog descriptions use bounded JSON writers/counters. Custom
settings are opaque NCB bytes, with a separate 64 KiB snapshot ceiling. Providers
use the same bounded NCB policy for encoding and decoding and validate their own
typed schema/revision. The entire candidate's storage is checked before construction;
providers also check direct calls. See [document resource admission](../../../docs/contracts/DOCUMENT_RESOURCE_ADMISSION.md).

A `Document` contains a tree of built-in widget constructors, styles, resources,
and parent-owned layout edges. Each node has a unique stable key. Its physical
event port count is zero or one. Every observed port transfers a weak origin and
an exclusive `ComBox<IEventAbi>` snapshot through the common `IEventReceiver`.
Text editors emit separate `ITextChangedEvent` and `ITextSubmittedEvent` snapshots
through that same port. The behavioral parent selects its receiver before the
loader constructs widgets, using `EventBindings`. An omitted receiver chooses
permanent non-delivery.

The application owns event behavior and handles accepted messages at its normal
update boundary. It queries each snapshot's concrete COM interface and reads its
captured getters. This is where button-to-textbox behavior, validation, state
transitions, and application-specific commands belong. Optional generated forms
provide checked widget fields; they contain no event handlers or dispatch code.
The current document schema has no serialized visual output-to-input graph,
route expression, or route editor.

## Loading and typed widget access

Use `Loader::new(ui, parent_window, resources, grid_limits)` or
`ReloadSlot::new(&mut reload_ids, ui, parent_window, grid_limits)`. The loader returns a `Component` whose
concrete children can be queried before mounting. Supply `EventBindings` to
`compile_with_events` or `load_with_events` before construction; plain `compile`
and `load` select permanently unobserved widgets. Delivery errors
propagate through the emitting operation; there is no relay diagnostic channel.

`grid_limits` is an application-selected `GridLimits { max_columns, max_rows }`.
It bounds each built-in Grid's explicit tracks and cumulative implicit placement.
The policy is immutable across reload attempts and is never read from serialized
content. JSON and binary recipes can be valid for a larger host while construction
under this host's policy returns `NC_E_OUT_OF_RANGE`; a failed candidate leaves the
published component intact. The designer preview and runtime-UI example select
64 columns by 256 rows. See the [Grid contract](../../../docs/contracts/GRID_PLACEMENT.md)
for placement, ownership-return, and resource semantics.

`Component::widget::<ButtonAbi>("save")` resolves an authored node key and checks
its concrete COM interface before returning a weak `Widget<ButtonAbi>` observation.
Generated forms use this lookup for every field. A wrong key or incompatible
interface fails during candidate preparation, before replacing a live form.
Getters and setters belong to those widget interfaces. An application can write
`form.widget_title_1.set_text("Saved")` and read its text through the same field.
There is no separate command record, setter receiver, named input lookup, or
`Form.inputs` object. Custom fields expose their own COM getters through `as_interface()` and setters
through `as_interface_mut()`; the designer does not maintain a catalog of custom setters.

These calls validate and apply changes synchronously and return their result.
Event queues acquire the original COM event owner and weak origin for deferred
handling. The application selects commands after dequeueing and checking the
origin. Getters and ordinary state setters do not publish user-interaction events.
Application code decides when a state change
should also trigger application behavior.

A component can advertise optional `IUiComponentMetadata`. Its
`replace_metadata` operation accepts a complete bounded recipe whose schema and
root constructor tree exactly match the authored snapshot, then atomically
publishes document name/output aliases. It changes no live widgets, event bindings,
resources or reload generation. The UI Designer uses this path only when texture
identities are unchanged and no opaque custom nodes are involved. Constructor,
resource and plugin settings edits use normal transactional reload. Publication
must run under normal host mutation admission; layout/paint reject it. Persistence
advances to the new authored metadata while interactive state remains independent.

A compiled component retains the recipe, weak lookup handles and one unmounted
root owner. `take_root()` moves that owner once; `root()` only observes it.
Mounting consumes the same root. Dropping metadata after transfer cannot remove
the mounted tree, and keeping metadata cannot prolong a removed tree. An optional
output alias is a display name, not a second endpoint or receiver replacement API.
Generated bindings resolve widgets by node key.

## Optional typed fields and common event handling

The designer's **File → Export Rust bindings…** command calls
`Document::rust_bindings()`. The command-line equivalent is:

```sh
cargo run -p neocogi-ui-document --example export_bindings -- form.ui.json > bindings.rs
```

The generated module contains `Form` with checked concrete widget fields and
`bind(component)` only. Regenerate when node keys or types change. Names include
ordinals to avoid punctuation/keyword collisions; runtime keys remain exact literals.
Event handling requires no generated queue, event enum or handler trait. Applications
can also use `component.widget::<ButtonAbi>("save")` directly without generating fields.

For reloadable documents, share a `Sender<DocumentEvent>` across attempts. Import
`document_receiver`, `EventBindings` and `DocumentEvent` from `neocogi_ui_document`,
`IActivatedEventAbi` from `neocogi_ui`. This example uses the `title` and `save` nodes created
by `Document::new`, their exported fields, and an already mounted `ReloadSlot`.
Construct a library receiver carrying each attempt's generation and bind all
emitting widgets:

<!-- checked-example: document-reload -->
```rust,ignore
let mut candidate_form = None;
slot.reload(&document, resources,
    |generation| {
        let queue = document_receiver(generation, sender.clone())?;
        EventBindings::all(&document, &registry, queue.as_com_ref())
    },
    |component, _| { candidate_form = Some(generated::bind(component)?); Ok(()) },
)?;
let mut form = candidate_form.unwrap();
```

At the application dispatch boundary, authenticate generation and current widget
identity before querying the concrete event:

<!-- checked-example: document-dispatch -->
```rust,ignore
for envelope in receiver.try_iter() {
    if let Some(message) = envelope.event_for(&slot, "save")? {
        if message.query::<IActivatedEventAbi>()?.is_some() {
            form.widget_title_1.set_text("Saved")?;
        }
    }
}
```

`event_for` rejects stale attempts before lookup, then rejects expired, service and
foreign origins. It returns a borrowed message while the envelope keeps the original
`ComBox<IEventAbi>` owner. Unsupported event queries return `None`; real COM errors
remain errors. The receiver adds only generation and a sender, with no widget or
application ownership. Disconnection returns the same event owner to the producer.

Use `EventBindings::insert_queue` to observe selected keys. Non-reloading consumers
can supply `events::channel()` directly. No generated Rust is required for either path.
The [walkthrough regression](../runtime/src/runtime/tests/documentation.rs)
compiles these exact blocks with fresh generated fields and executes activation,
source-based commands, replacement and stale-event rejection. Run it with
`cargo test --offline -p neocogi-ui-runtime --lib documented_form_reload`.

## Reload, ownership, and authored state

`ReloadSlot::take_widget()` transfers its stable container to the tree once; the
slot then observes it weakly. `reload(document, resources, make_events, check_fields)`
prepares receivers, validates contracts, constructs a candidate and checks concrete
fields before replacement. Failed preparation preserves the published component
and destroys the candidate, including when validation captured weak handles.
Successful replacement drops the previous owner. There is no teardown status.
Create one application-owned `ReloadIds::new()` for all slots whose messages
can meet in the same dispatch queue. Pass `&mut reload_ids` to every slot constructor,
including slots in separate UI runtimes or slots replacing destroyed ones. Preserve
that allocator across slot recreation; never restart it while older messages can
still arrive. Different allocators define separate domains and their tokens must
not be compared or mixed. No library-static counter supplies process-wide identity.

Every attempt gets a copied `Generation` containing `(slot_id, attempt)`, including
failed attempts. Slot IDs are never recycled; attempts advance before receiver
creation. Exhaustion returns an error without wrapping or changing the active form.
The token owns no widgets and adds no allocation/reference counting. `accepts`
compares the complete pair; `DocumentEvent::event_for` also validates the weak origin.
Allocator mutation uses `&mut self`. Concurrent slot creation must serialize access
to the same allocator. Copied tokens need no synchronization beyond the channel's
ordinary transport.
The [reload identity contract](../../../docs/contracts/RELOAD_GENERATION_IDENTITY.md) defines
allocator lifetime, reservation ordering, exhaustion, examples and regression coverage.

Mount a `Component`, its `take_root()` result, or a `ReloadSlot::take_widget()`
result. Removing those trees destroys their widget queues. Already posted facts
remain application-owned; generation and origin checks reject stale delivery.
Application edits belong outside layout and paint. `Component::save()` preserves
authored defaults, not live focus, scrolling or application state.

`Style.options = None` leaves the concrete constructor's requested flags intact;
`Some(0)` explicitly clears them. Only `Some(bits)` calls `replace_flags` during
loading. This applies to custom COM providers as well as built-ins. There is no
per-kind defaults table. Read `IWidget.requested_flags()` for current
constructor/setter configuration. See the [constructor configuration contract](../../../docs/contracts/WIDGET_CONSTRUCTOR_DEFAULTS.md)
for examples, designer behavior and the history of this distinction.

For surface enablement, call `clear_flags(WidgetOptions::NO_INTERACT)` to enable
or `set_flags(WidgetOptions::NO_INTERACT)` to disable. Both preserve other current
requested bits. Do not write `properties().options` back into settings: effective
properties may include temporary mouse-wheel or participation flags. Constructor
defaults initialize state once; `replace_flags` explicitly replaces the requested
set. `SplitView::set_position` preserves other live split settings in one COM
call. Combo `replace_items`/`replace_menu` atomically validate and replace the
model and selection. These guarantees belong to widgets, not document adapters.

This flag change applies to the widget surface, not recursively to its descendants.
Behavior-specific policy remains separate: for example, enabling the surface
does not turn on a disabled scroll model. The effective flags continue to reflect
both the current requested value and the widget's own behavior.

Fonts and icons resolve through the supplied immutable UI asset bundle. Images
use logical keys resolved by the caller's `Resources` map. Source documents do
not embed GPU handles, file paths selected by the loader, receivers, or plugin
code. Validation bounds document bytes, depth, node count, numeric settings,
child layout and event aliases. JSON rejects unknown fields, including the removed
`inputs` property. Binary envelopes and inner documents reject earlier revisions;
there is no migration or silent removal of old command declarations.

`Form { width, height }` is a fixed-coordinate container. Its children use
`Position { x, y, width, height }` edges in parent content pixels. Smaller
allocations clip rather than scale; sibling order controls paint and hit order.
Linear, Grid, Split, Scroll, and Disclosure remain ordinary nested containers.

## Custom COM widgets

A host installs custom widgets through `CustomWidgetRegistry`. A local
`CustomWidgetFactory` returns an owned `DesignWidgetManifest`, normally made by
`DesignWidgetManifest::from_source(kind, defaults, &CounterAbi::EVENT_SOURCE)`,
and a constructor accepting its optional COM receiver and returning `OwnedWidget<IWidgetAbi>`. Static conversion checks the
product identity and complete reachable event graph. The constructor receiver
is separate from the product's interface chain. A custom widget
uses one common receiver and advertises its concrete event interfaces, for
example `ui_event_sources! { CounterAbi => [IActivatedEventAbi, ICounterChangedEventAbi]; }`.

An independently built plugin exposes `IDesignWidgetCatalog` on its root factory.
The registry reads bounded version-12 `DesignWidgetManifest` records: event
identities, getter fields, their reachable value definitions, Rust ABI paths and
opaque NCB defaults. `create_widget` receives the common `EventBinding`. The host
checks the returned concrete interface before publication; registration validates
all kinds before publishing any. Older native event/receiver metadata is removed.

The document stores `Custom { kind, settings: Vec<u8> }`. The plugin interprets its
private NCB schema, including its version and semantic constraints. The host bounds
storage and resolves kinds; it does not duplicate provider field validation.
Malformed settings fail candidate construction, before replacing a live form.

For editing, `create_editor` optionally returns an exclusive
`IDesignSettingsEditor`. It receives the designer's ordinary `UiFactory`, creates
controls, and transfers their tree once through `take_view`. The inspector owns
that tree; the controller reads it through weak handles. `settings` validates a
complete draft and copies a stable bounded snapshot. `NOINTERFACE` from
`create_editor` means deliberate absence; other errors remain errors.

Apply prepares history and successfully constructs the preview before accepting
source. Failed drafts stay mounted; accepted snapshots support normal save/undo/
redo. No generic reflected setter invocation or full IDL reconstruction is needed.
See [the settings contract](../../../docs/contracts/PLUGIN_DESIGNER_SETTINGS.md) and the
[independent example](../../../examples/ui-extension/src/settings.rs).

`Document::rust_bindings_with(&registry)` generates optional concrete widget fields
using the owned IDL interface metadata from the registry. Event handling uses
common receivers and concrete COM getters independently of those fields.
The plain `rust_bindings()` helper and the command-line example use an empty
registry and therefore handle built-in documents only. The designer's Rust
export uses its active registry.

Custom widgets are currently leaf nodes in authored forms. A custom container
needs a separate child attachment contract before it can own authored children.
The Run-mode preview renders custom widgets but does not generically decode
plugin-defined events for its status trace. Applications receive the same common
COM stream and query plugin event interfaces directly.
