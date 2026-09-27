# Authored UI documents and generated forms

`neocogi-ui-document` reconstructs an authored widget tree through the existing
COM widget factories. JSON is editable source; the NCB persistence envelope is
suitable for embedding or deployment. The loader performs no file I/O. Both
representations use schema revision 5, and earlier revisions are rejected.

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
An event stream has exactly one lifetime consumer, attached by the behavioral
parent. The loader constructs unbound widgets; generated form bindings attach
their typed sinks directly to the application's message queue. There is no
intermediate loader queue or form relay. A second attachment is rejected.

The application owns event behavior. Every event-producing node is represented
by one required method in the generated `Handlers` trait. The application
implements that trait in its own source file and dispatches accepted events at
its normal update boundary. This is where button-to-textbox behavior, validation,
state transitions, and application-specific commands belong. Revision 5 has no
serialized visual output-to-input graph, route expression, or route editor.

## Loading and typed widget access

Use `Loader::new(ui, parent_window, resources)` or
`ReloadSlot::new(ui, parent_window)`. The loader returns a `Component` whose
concrete children can be queried and bound before mounting. Delivery errors
propagate through the emitting operation; there is no relay diagnostic channel.

`Component::widget::<ButtonAbi>("save")` resolves an authored node key and checks
its concrete COM interface before returning an owned `Widget<ButtonAbi>` handle.
Generated forms use this lookup for every field. A wrong key or incompatible
interface fails during candidate preparation, before replacing a live form.
Widget methods remain the actual COM class methods, so an application can write
`form.widget_title_1.set_text("Saved")` rather than route through a serialized
command graph. Optional named `InputHandle<SetText>` endpoints remain available
for message-based integrations; they are independent of the generated widget
fields.

A loaded component owns the widget tree and its publication/retirement guard.
It creates no event adapters. An optional document output alias supplies a
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

The actual method and field ordinals come from the generated file. Bind the
candidate inside `ReloadSlot::reload`, retain the returned `Form` for the
published generation, and queue generated `Message` values. Before dispatch,
check `slot.accepts(message.generation)`; stale events from replaced candidates
must not update the current form. Run `message.event.dispatch(&mut form)` only at
the application's safe update boundary.

## Reload, ownership, and authored state

`ReloadSlot` owns one stable container. `reload(document, resources, bind)`
validates and constructs an unmounted candidate, runs its binding callback, and
only then replaces the old child. Failed construction or binding preserves the
published component. Candidate cleanup closes partial trees and combo popups.
The returned teardown status reports old-content cleanup after publication; it
cannot turn a successful replacement into an unpublished one. Each attempt gets
a distinct generation, even when preparation fails.

Mount `Component` itself or use `ReloadSlot` so the runtime retains both view and
event owner. `root()` alone is an inspection capability. Closing or replacing a
subtree retires its lifetime guard and closes its streams. Application messages
already queued remain owned snapshots; generation checks reject stale ones. Application edits belong at ordinary mutation boundaries, outside
layout and paint. `Component::save()` preserves authored defaults rather than
user interaction, focus, scrolling, or live application state.

Fonts and icons resolve through the supplied immutable UI asset bundle. Images
use logical keys resolved by the caller's `Resources` map. Source documents do
not embed GPU handles, file paths selected by the loader, receivers, or plugin
code. Validation bounds document bytes, depth, node count, numeric settings,
child layout, event aliases, and command endpoints. JSON rejects unknown fields.

`Form { width, height }` is a fixed-coordinate container. Its children use
`Position { x, y, width, height }` edges in parent content pixels. Smaller
allocations clip rather than scale; sibling order controls paint and hit order.
Linear, Grid, Split, Scroll, and Disclosure remain ordinary nested containers.

## Custom COM widgets

A host installs custom widgets through `CustomWidgetRegistry`. A local
`CustomWidgetFactory` returns an owned `DesignWidgetManifest`, normally made by
`DesignWidgetManifest::from_idl(kind, defaults, &CounterAbi::IDL_INTERFACE)`,
and a constructor returning `Widget<IWidgetAbi>`. Static conversion checks the
generated resolved stream against the complete required interface chain. A custom widget
may expose zero or one event port; multiple logical events belong in one
`enum`, such as `CounterEvent::Activated` and
`CounterEvent::Changed`.

An independently built plugin can expose `IDesignWidgetCatalog` on its root COM
factory. The registry reads bounded, versioned descriptions through the catalog's
`describe` method, copies the IDL struct/enum/message shape and typed property
defaults into owned data, and retains the catalog for `create_widget` calls.
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
port is bound directly to the application queue after the form has been checked.
The plain `rust_bindings()` helper and the command-line example use an empty
registry and therefore handle built-in documents only. The designer's Rust
export uses its active registry.

Custom widgets are currently leaf nodes in authored forms. A custom container
needs a separate child attachment contract before it can own authored children.
The Run-mode preview renders custom widgets but does not generically decode
plugin-defined events for its status trace; the generated application binding
still receives the complete typed stream.
