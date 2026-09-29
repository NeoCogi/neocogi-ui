# Authored UI documents and generated forms

`neocogi-ui-document` reconstructs an authored widget tree through the existing
COM widget factories. JSON is editable source; the NCB persistence envelope is
suitable for embedding or deployment. The loader performs no file I/O. Both
representations use schema revision 6, and earlier revisions are rejected.

Binary save and load apply the same byte, collection, value, and nesting limits.
`Document::to_binary` and `Component::save` reject over-budget initialization
instead of returning bytes that their corresponding readers cannot accept under
the same policy. The 16 MiB byte ceiling does not override the one-million-value
limit: the persistence envelope stores the encoded document as `Vec<u8>`, so each
state byte counts as one collection element and one value, alongside the outer
record's other values. This is intentional collection accounting, not an implicit
binary-blob representation. See the [binary limit definitions](../../com/core/README.md#binary-resource-limits).

A `Document` contains a tree of built-in widget constructors, styles, resources,
and parent-owned layout edges. Each node has a unique stable key. Its physical
event port count is zero or one. A text editor's single port carries the whole
`TextboxEvent` or `TextAreaEvent` enum, including `Changed` and `Submitted`.
An event stream has one receiver selected by the behavioral parent during
construction. Generated forms prepare a shared typed queue before the loader creates
widgets. There is no later attachment method, intermediate loader queue, or form
relay. An omitted receiver chooses permanent non-delivery.

The application owns event behavior. Every event-producing node is represented
by one required method in the generated `Handlers` trait. The application
implements that trait in its own source file and dispatches accepted events at
its normal update boundary. This is where button-to-textbox behavior, validation,
state transitions, and application-specific commands belong. Revision 6 has no
serialized visual output-to-input graph, route expression, or route editor.

## Loading and typed widget access

Use `Loader::new(ui, parent_window, resources)` or
`ReloadSlot::new(ui, parent_window)`. The loader returns a `Component` whose
concrete children can be queried before mounting. Supply `EventBindings` to
`compile_with_events` or `load_with_events` before construction; plain `compile`
and `load` select permanently unobserved widgets. Delivery errors
propagate through the emitting operation; there is no relay diagnostic channel.

`Component::widget::<ButtonAbi>("save")` resolves an authored node key and checks
its concrete COM interface before returning an owned `Widget<ButtonAbi>` handle.
Generated forms use this lookup for every field. A wrong key or incompatible
interface fails during candidate preparation, before replacing a live form.
Getters and setters belong to those widget interfaces. An application can write
`form.widget_title_1.set_text("Saved")` and read its text through the same field.
There is no separate command record, setter receiver, named input lookup, or
`Form.inputs` object. Custom fields expose their own COM methods through
`as_interface()`; the designer does not maintain a catalog of custom setters.

These calls validate and apply changes synchronously and return their result.
Event queues have a different role: they acquire emitted facts and enqueue the
behavioral parent's mapped messages. Getters and ordinary state setters do not
publish user-interaction events. Application code decides when a state change
should also trigger application behavior.

A compiled component retains the recipe and typed lookup handles. It creates no
lifetime guard, scheduled event owner or event relay. Dropping it releases those
references, without explicitly closing a root retained by the application or
runtime. An otherwise unowned root is destroyed normally and closes descendants. An optional document output alias supplies a
trace/display name; it does not create a separate bindable endpoint or permit
receiver replacement. Generated application bindings resolve widgets by node key.

## Generated application contract

The designer's **File → Export Rust bindings…** command calls
`Document::rust_bindings()`. The command-line equivalent is:

```sh
cargo run -p neocogi-ui-document --example export_bindings -- form.ui.json > bindings.rs
```

The generated module contains `Form` with checked concrete fields for all
built-in nodes, an `Event` enum with one variant per emitting node, a
`QueuedEvent` with a weak origin and owned event per distinct queue contract, a
reload-generation-tagged `Message`, and a `Handlers` trait with one required
method per emitting node. No handler body is generated. Regenerate after
changing node keys or widget types and keep `impl Handlers` in application-owned
source. A missing trait method is a Rust compile error when the application
implements the generated trait. Names are normalized with a role prefix and
node ordinal to avoid keyword and punctuation collisions; authored keys remain
exact escaped literals for runtime lookup.

For a form containing a button `save` and text block `title`, an application
handler can use the generated typed fields directly:

```rust
impl generated::Handlers for generated::Form {
    fn on_save_2(
        &mut self,
        event: neocogi_ui::button::Event,
    ) -> Result<(), neocogi_com::NcStatus> {
        match event {
            neocogi_ui::button::Event::Activated(value) => self.widget_title_1.set_text(&value.label),
        }
    }
}
```

The actual method and field ordinals come from the generated file. First use
`generated::events(generation, sender)` in the reload receiver callback. Then use
`generated::bind(component)` in the field-validation callback and retain that
`Form` only after successful publication. The first function constructs one application queue object for all emitting fields;
the second resolves widget fields without changing event ownership.

```rust,ignore
let mut candidate_form = None;
let outcome = slot.reload(&document, resources,
    |generation| generated::events(generation, sender.clone()),
    |component, _| { candidate_form = Some(generated::bind(component)?); Ok(()) },
)?;
let mut form = candidate_form.unwrap();
```

At the application's safe update boundary, first check the reload generation,
then resolve the weak origin against the current form's concrete fields:

```rust,ignore
for message in receiver.try_iter() {
    if slot.accepts(message.generation) {
        if let Some(event) = message.event.resolve(&form)? {
            event.dispatch(&mut form)?;
        }
    }
}
```

`resolve` finishes its immutable form borrow before the mutable handler runs.
When behavior lives on `App { form, model }`, resolve against `&app.form`, then
call `event.dispatch(&mut app)`. Two buttons share one queue receiver while their
origins select separate required handlers. Expired or foreign origins return
`None`; no current widget is substituted. Weak promotion alone does not reject
a closed widget retained by an old form, so the generation check remains required.

Generated forms allocate one queue for the whole construction attempt and none
when there are no events. The queue owns only a sender and generation, not form
fields or application state. It acquires each event and weak origin once. Code
interested only in an old captured result can inspect `QueuedEvent` directly;
resolving it into a current widget handler is an explicit application decision.

## Reload, ownership, and authored state

`ReloadSlot` owns one stable container.
`reload(document, resources, make_events, check_fields)` prepares receivers,
validates all keys and exact queue contracts, constructs the unmounted candidate,
checks concrete fields, and only then replaces the old child. Failed preparation preserves the
published component. Candidate cleanup closes partial trees and combo popups.
The returned teardown status reports old-content cleanup after publication; it
cannot turn a successful replacement into an unpublished one. Each attempt gets
a distinct generation, even when preparation fails.

Mount `Component` itself, its actual `root()`, or use `ReloadSlot`; each path
passes the same widget to the runtime. Closing or replacing that subtree closes
its streams. A rejected `ReloadSlot` candidate is explicitly closed even if a
failing validation callback retained one of its widget handles. Application messages
already queued remain owned snapshots; generation checks reject stale ones. Application edits belong at ordinary mutation boundaries, outside
layout and paint. `Component::save()` preserves authored defaults rather than
user interaction, focus, scrolling, or live application state.

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
`DesignWidgetManifest::from_idl(kind, defaults, &CounterAbi::IDL_INTERFACE)`,
and a constructor accepting its optional COM receiver and returning `Widget<IWidgetAbi>`. Static conversion checks the
product identity and complete reachable event graph. The constructor receiver
is separate from the product's interface chain. A custom widget
may expose zero or one event port; multiple logical events belong in one
`enum`, such as `CounterEvent::Activated` and
`CounterEvent::Changed`.

An independently built plugin can expose `IDesignWidgetCatalog` on its root COM
factory. The registry reads bounded, versioned descriptions through the catalog's
`describe` method, copies the IDL struct/enum/message shape and typed property
defaults into owned data, and retains the catalog for `create_widget` calls.
Catalog revision 8 describes the constructor queue IID/signature and its public
Rust implementation trait path. COM owns the shared path-validation policy. `create_widget`
receives `WidgetEventBinding`; the plugin must query its exact typed queue before
allocating the widget. Mismatched queues fail before publication.
The shared `neocogi-ui-design` crate contains only this metadata and property
codec, so plugin implementations do not link the document runtime or designer.
`register_catalog` validates all kinds transactionally before publishing any.
The COM catalog accepts one widget index, the runtime's `IWidgetHost`, and the
validated property values encoded with the shared codec. The returned object's
exact class and stream IIDs and ABI signatures are checked before the document owns it.

The document stores only `Custom { kind, properties }`. It does not store Rust
paths, event traits, message types, executable code, or a copy of the property
schema. Loading resolves the kind against the installed registry, validates
property names and types against its defaults, constructs the widget, and
checks that the returned COM object implements the exact advertised interface
identity and ABI signature before publishing it. Unknown kinds and mismatched
properties fail preparation. The generic designer inspector can edit text,
boolean, integer, and finite floating-point constructor values; providers may
impose stricter domain bounds in `create`.

`Document::rust_bindings_with(&registry)` generates custom form fields and
required handler methods using the owned IDL interface and message metadata
read through the registry. The generated field uses the concrete COM projection;
the generated handler receives the declared message enum or struct. The custom
receiver is a projection of the generated application queue, supplied during construction;
subsequent form binding only checks and retains the concrete widget fields.
The plain `rust_bindings()` helper and the command-line example use an empty
registry and therefore handle built-in documents only. The designer's Rust
export uses its active registry.

Custom widgets are currently leaf nodes in authored forms. A custom container
needs a separate child attachment contract before it can own authored children.
The Run-mode preview renders custom widgets but does not generically decode
plugin-defined events for its status trace; the generated application binding
still receives the complete typed stream.
