# COM widget and application contracts

These are the author-facing rules for the component architecture defined in
[COM_DESIGN.md](COM_DESIGN.md). The contract crate contains fixed interfaces and
values; the application-facing Rust facade contains typed conveniences. Neither
exposes implementation state through `Any`, untyped properties, or native Rust GPU
objects.

## Linking and ownership

Native applications always link the Rust facade. A dynamic assembly links one shared COM runtime and loads the UI, GFX compositor, selected renderer, and optional Skin plugin. A monolith instead links one runtime implementation plus those component rlibs and registers their descriptors directly. Browser builds use the same static registration model. Never link a separate runtime implementation into each dynamic plugin. Loaded or statically registered plugins remain active for the process lifetime; this design does not support hot unloading.

A `ComPtr` keeps an allocation alive. It does not keep a widget attached, a logical window open, or a subscription connected. Closing a window/subtree ends its input, layout, editing, and painting eligibility even when the application retains typed pointers. Further operations report the documented closed status. Final Release frees allocation storage and must not submit GPU work, invoke application callbacks, or substitute for a fallible close/finish operation.

Initially each widget mounts once, under one parent in one runtime. Attachment checks duplicate membership, cycles, and owner compatibility before publishing an edit. Container track/span settings belong to the parent-child relationship. Do not place them in a generic widget property map. Foreign-runtime widgets and foreign-renderer resources are errors, even if their numerical IDs happen to match.

IDs are scoped to their owner. A static counter compiled into several shared libraries is several counters. Do not assume it provides process-wide identity. QueryInterface's canonical IUnknown projection identifies a live COM object; arbitrary interface-header addresses can differ on the same object.

Default UI and GPU pointers are thread-local. Atomic reference counts do not make UI methods, OS windows, or GPU contexts thread-safe. Do not strengthen a threading marker without the corresponding implementation guarantee, including final destruction on another thread.

## Update, measurement, and external state

The update result describes geometry: preferred measurement or child-placement inputs changed, or they did not. It is not a generic "something happened" flag. A widget may emit an event, change hover/focus painting, or update a paint cache while reporting no geometry change.

The runtime has additional independent invalidation sources:

- Programmatic setters record sticky geometry changes. Several edits before update may coalesce, but the change must survive until the runtime observes it.
- A different measurement constraint, including a resize, invalidates the corresponding cached measurement.
- A skin/font/atlas revision can change glyph, border, icon, padding, or other measurement inputs.
- Child insertion/removal and parent-owned track/span changes invalidate ancestor measurement and placement.
- A descendant's pending change must be collected before accepting a parent's cached result, including when that descendant's update was skipped.

Widget authors must report changes honestly. A no-change result cannot detect mutations to application-owned state read behind a separate shared reference. If a model, shared string, image geometry, font choice, or other external input changes without going through a widget setter, the author must explicitly synchronize the widget and request geometry invalidation before relying on layout. The runtime does not inspect arbitrary user state or infer its dependencies.

For example, changing a button from "Save" to "Save all documents" changes its preferred width. Its setter marks geometry dirty. Changing only a caret blink phase normally affects paint and does not change preferred size. A model containing the button's future text changing elsewhere is invisible until the application applies the new label or explicitly invalidates its measurement.

Do not rely on hidden, clipped, disabled, or otherwise ineligible widgets receiving an update call on every application frame. Update eligibility follows routing/traversal policy, not a timer service. Keep required model synchronization in application code, and carry its geometry invalidation until the runtime collects it. A widget returning no change does not suppress descendant updates or automatically certify the entire subtree's cache.

Call the runtime's update/synchronization operation after programmatic changes and resize before using layout results or rendering. Painting is observational: it may update private rendering caches, but it must not change semantic state, topology, focus/capture, measurement inputs, or publish application-coordination events. A missing or invalidated update commit causes rendering to report that update is required.

Measurement constraints and allocated rectangles have different meanings. Unbounded is different from bounded zero. A child reports desired size under constraints; the parent then assigns an exact rectangle. Children must not silently reapply a sizing policy to that final allocation.

## Borrowed calls, paint, and layout

Generated interfaces have one safe calling convention. `ComPtr<IFooAbi>` and
`ComRef<'_, IFooAbi>` both lend the generated `IFooClient` view, so callers use
`pointer.method()` without importing an interface-specific extension trait.
Derived client views dereference through their declared base views for inherited
method lookup. An owned base projection still requires checked `QueryInterface`;
method lookup alone never manufactures another COM reference.

`ComRef` borrows a live interface for a call; it can be retained with `add_ref()`.
Therefore it is not an exclusive, non-retainable Rust borrow. Never create a COM
implementation containing a stack `&mut Painter`, borrowed child storage, a
native frame borrow, or a borrowed application `&mut State` and assume a ComRef
prevents it escaping.

Paint/layout services own their storage and issue invocation tokens. The static `Painter<'a>`/layout convenience wrapper hides the token from ordinary drawing calls, but the implementation checks it on every ABI operation. The token expires on every exit, including errors. An old token never becomes valid again in a later frame. Retaining the object remains memory-safe and retaining its expired authority does not permit further painting/layout.

Scoped layout permits a container to measure/place its indexed children. It does not permit topology edits during layout or unrestricted access to the runtime tree. Recursive child calls restore the enclosing scope correctly before continuing the parent. Applications and plugins must not fabricate authority or treat an invocation token as a durable resource identifier.

Use `&mut self` for methods that change object state and store that state in ordinary fields. The generated object-wide call gate permits overlapping shared calls and returns `NC_E_BUSY` for conflicting mutable or recursive entry before it creates a Rust reference. Interior mutability is reserved for genuinely shared infrastructure such as event queues and runtime registries; release those short borrows before calling an external widget, container, painter, or application handler. Panics are implementation faults and follow the COM abort policy; they are not converted into recoverable statuses.

The same rule applies outside COM objects. Event-loop drivers and other values
with one clear owner use `&mut self` and ordinary fields. `Cell` or `RefCell` is
appropriate only when an interface requires shared observation, ownership is
genuinely shared, or state must be detached before a reentrant foreign call; the
field comment must name that constraint.

## Typed events and application state

The COM compiler treats the generated `neocogi.com.events` NCIDL module as its
canonical event-lease ABI: a foreign same-named lease cannot satisfy an `event`
port. That module owns `IEventSubscription` and `IEventDispatcher`;
`neocogi-com-events` owns the producer queue, weak endpoint, application session,
and local delivery adapter. Generated event bindings check the support API
version at compile time. This support crate does not depend on UI or on the
process-global `neocogi-com-runtime`. UI adds only domain validation and
typed port getters. `neocogi_ui::EventSession<State>` reexports the same
`neocogi_com_events::EventSession<State>` used by non-UI components.
`Ui::update_with_events` borrows its dispatcher; only the native facade sees the
state parameter, and the runtime receives a concrete COM capability.
Each handler is an owned
`EventHandler<State, Event>` implementation stored inline in a local generic COM
sink. Application handlers implement that trait on concrete structs or enums
whose dependencies are explicitly named fields; closures and function pointers
are not implicitly adapted into handlers. Handlers and Rust state never cross the ABI. Generic
Rust list data also stays in the application; cross-plugin domain data needs a
concrete domain interface and owner-scoped keys.

```rust
use neocogi_ui::{ButtonWidget, EventHandler, EventSession};
use neocogi_com::NcStatus;
use neocogi_ui::events::Activation;

/// Submission behavior with an explicitly named widget dependency.
struct Submitted {
    /// Owned button capability updated after each delivered activation.
    button: ButtonWidget,
}
impl EventHandler<u32, Activation> for Submitted {
    /// Updates local state and the concrete control from the captured label.
    fn handle(&mut self, count: &mut u32, event: &Activation) -> Result<(), NcStatus> {
        *count += 1;
        // The event owns the label captured at activation, even after this setter.
        self.button.set_label(&format!("{}: {}", event.label, count))
    }
}

let events = EventSession::new(0u32);
let lease = events.subscribe(button.activated(), Submitted { button: button.clone() })?;
ui.update_with_events(drawable_size, &events)?;
println!("{} submissions", *events.state()?);
ui.render(drawable_size, renderer.as_com_ref())?;
lease.cancel()?; // Explicit cancellation; dropping the session also cancels its leases.
```

The event capabilities are separate from editing capabilities and may be queried
once when subscribing. Pass these ports to the same session API:

| Port | Producers | Captured payload |
| --- | --- | --- |
| `activated()` | Button, ListItem, ListBox | Submitted label |
| `changed()` | Checkbox | Checked boolean |
| `changed()` | Slider, Number | Finite committed value |
| `changed()` | Scrollbar | Nonnegative clamped offset |
| `changed()` | Textbox, TextArea | Normalized text and matching UTF-8 byte cursor |
| `submitted()` | Textbox, TextArea | Submitted normalized text |
| `window_events(window)?` | Logical window | Typed lifecycle/chrome event with copied geometry where relevant |
| `popup_events(popup)?` | Popup | Dismissal fact |
| `changed()` | Combo | NCIDL-generated committed selection |
| `activated()` | Compact menu row | Submitted label |
| `completed()` | Standard file picker | Exact accepted native path or cancellation |

Independent producers share `neocogi_com_events::EventSource<E>` with built-ins;
the UI facade reexports this type for UI authors.
Standard UI snapshots are generated from `event` NCIDL declarations, and their
source subscription methods and typed port values are generated from `event`
ports. Use one `session.subscribe(port, handler)` operation for built-in and
independently loaded controls. UI getters such as `button.activated()` and
`editor.changed()` select the exact generated port and any required domain check.
A raw generated port constructor accepts an owned source projection; its
`.checked(validate)` method installs a concrete predicate before state borrowing.
The port releases its source after setup; only the returned lease is retained.

`EventPort` associates each source with its concrete owned `Event` type.
Subscription claims the registration transaction before the port creates its
sink or calls the foreign subscription method. Failure restores registration
order and releases uncommitted ownership. Native sources may implement the same
trait; declared NCIDL ports need no handwritten setup adapter.

Event sinks use `&mut self`, letting the generated COM call gate protect a plain
handler field. The shared state still needs one `RefCell` because independent
sinks borrow the same owner. Rust binding protocol 3 checks compiler/support
agreement. Event delivery signatures change accordingly; plugin descriptor and
runtime table ABIs remain v1. Rebuild contracts and plugins together.

Commit semantic state, then
call `emit` with its owned snapshot. Implement the event capability by returning
`source.subscribe(sink)`, and close ordinary sources when the widget logically closes.
The [extension example](../examples/ui-extension) demonstrates the full object,
its inherited editing interface, and an application callback across separate binaries.
Raw queue leases are available only through the COM support crate's `test-support`
feature for algorithm fixtures; ordinary applications use COM subscriptions.

A source permits one subscriber per port. Editor changed/submitted ports are
independent. Subscribing to an occupied port returns `UI_E_BUSY`; subscribing to
a closed producer returns `UI_E_CLOSED`. A failed registration does not replace
the current subscriber. TextArea forwards both capabilities to its actual editor;
there is one queue for each event, without an outer duplicate.

Producer queues contain immutable snapshots and never own application callbacks,
sinks, leases or sessions. A subscription owns its queue connection and typed
sink; the session owns subscriptions. The runtime **borrows** `IEventDispatcher`
synchronously during `update_with_events` and must not retain it. State and
handlers may therefore own runtime/widget capabilities. Application-created
ownership cycles still require explicit cancellation. `EventSession` destruction
cancels every registered lease, including leases retained by other app code, and
releases their sinks. A retained dispatcher subsequently reports `UI_E_CLOSED`.
Final subscription release disconnects; final producer release closes its queue.
Neither invokes application handlers.

A composite widget with private child handlers exposes the optional
`IComponentEventProvider` UI capability. Its `private_dispatcher` returns a
separate COM allocation—not the widget's own `IUnknown` identity—so callback
edits cannot collide with a held widget call gate. The runtime validates that
identity before the one-way mount, caches the driver on the mounted node, and
copies child-first driver tickets after traversal. Private component sessions
run before the application session at every finite callback boundary. A ticket
is retired when its node is removed, so a callback that closes another
component prevents that removed component's snapshot from firing later in the
same sweep. No composite-specific branch or private widget type enters the
window-root policy layer.

```rust
// In the composite's NCIDL contract module:
// event IChoiceSink { fn chosen(&mut self, item: u64) -> Result<()>; }
// interface ChoicePanel: IWidget + IContainerLayout + IComponentEventProvider {
//     event subscribe_choice(&mut self): IChoiceSink;
// }

// In the producer plugin binary, after committing a choice:
source.emit(IChoiceSinkEvent { item });
// Its IComponentEventProvider implementation returns an owned reference from
// a component-owned private EventSession, not the ChoicePanel COM identity:
fn private_dispatcher(&self) -> Result<ComPtr<IEventDispatcherAbi>, NcStatus> {
    Ok(self.private_session.as_dispatcher().add_ref())
}

// In an application or another plugin's State:
let session = neocogi_com_events::EventSession::new(state);
session.subscribe(choice_panel_subscribe_choice_port(panel.clone()), ChoiceHandler)?;
ui.update_with_events(size, &session)?;
```

Private composite logic can translate these deliveries into one local message enum
and `Model::update(&mut self, message)`. The file picker demonstrates this for row
commands, text changes/submission, and window lifecycle. Translation runs directly
inside the existing delivery; it adds no inbox or second queue. Private events reach
parents only through the composite's explicitly declared public outputs.

Nested components are scheduled at arbitrary tree depth. A child mounted during a
callback joins the next sweep at that same boundary. Removing an ancestor retires
its descendant tickets before teardown, including tickets already collected for
later delivery. Runtime tests exercise three nested component levels, automatic
private dispatch without an application session, insertion during a callback, and
ancestor removal from a descendant handler.

The example uses one generated domain payload and sink on both sides of the
ABI. Private handlers and application handlers remain native, typed values in
their respective binaries. A component without private handlers needs only
its `event` source port and does not implement `IComponentEventProvider`.

Dispatch sweeps sources in registration order, with FIFO inside each source.
Each source captures a bounded delivery window, then pops one event before each
callback. Unattempted events stay queued; no producer, retained tree, registry
or sink-slot borrow crosses the callback. Cross-source order is
registration order, not a global timestamp order. For example, if B was
registered before A, A produced `a1, a2`, and B produced `b1`, a sweep delivers
`b1, a1, a2`. Callback emissions participate in subsequent source visits or
sweeps until the queues are empty. Registration, session cancellation, and
recursive dispatch during a session transaction return `UI_E_BUSY`; cancelling
an individual lease from its own handler is supported and stops the remainder
of its batch. A cancelled old lease cannot disconnect a later replacement.

`EVENT_DISPATCH_LIMIT` is 1,000,000 callbacks per runtime dispatch boundary,
including mounted private components, runtime-owned dialog services, application
sources, and cascade sweeps. `dispatch_bounded(limit)` selects an
explicit smaller budget for manual delivery. Exhaustion reports
`UI_E_EVENT_LIMIT` (-1007) and preserves unattempted events. Delivering exactly
the limit succeeds when nothing remains; limit zero succeeds only for empty
queues. A handler error consumes the attempted event, stops delivery, and
leaves unattempted events queued ahead of events emitted by that callback.
Retry never replays the failed attempt. Changes already made by a handler are
not rolled back. Panics follow the COM abort policy and are not recoverable
handler errors. A custom dispatcher must report its actual successful callback
count and honor the supplied bound.

Ordinary widget setters are silent; changing a value does not manufacture a
user submission. Window visibility transitions and popup dismissal explicitly
publish lifecycle facts even when initiated by application code. Logical source
closure discards ordinary queued work, including the remainder of an active
delivery window. Window destruction preserves exactly one terminal `Destroyed` fact for
its existing subscriber; it can be delivered after the window is closed. A new
subscriber cannot replay historical events. Cancellation discards terminal facts
too. Terminal delivery is attempted once, even if its handler returns an error.

`Ui::update_with_events` initially synchronizes geometry, dispatches already
queued events, then processes each native input followed by semantic dispatch
and layout before routing the next input. With no native input, eligible widgets
receive one ordinary update. At handler boundaries the context is available for
ordinary setters, window creation/removal, topology edits, and input enqueueing.
Recursive update/render reports `UI_E_BUSY`. Callback failure leaves the frame
uncommitted, so render reports `UI_E_UPDATE_REQUIRED`; another update can finish
remaining queued input. If a handler closes the runtime, the outer update
reports `UI_E_CLOSED`.

`Ui::update` drives mounted private components automatically, without supplying
an application dispatcher. `EventSession::dispatch`
only delivers semantic events: it neither processes native input nor commits
layout. After manual callbacks mutate geometry, call update before rendering.
Directly dispatching a raw lease bypasses session ordering and its global budget;
applications using this lower-level API own those obligations themselves.

Finish `state()`/`state_mut()` borrows before dispatch or an eventful update. A
handler needs mutable access to the same state; incompatible borrows produce
`UI_E_BUSY`. As with any handler error, an event attempted while that state is
unavailable is consumed. Do not hide borrowed stack state inside a retainable
COM object to work around this rule.

### Geometry synchronization does not repeat ordinary widget behavior

A setter invoked by a handler can run after that widget's ordinary update for
the current input. The following layout sees its dirty geometry, but the runtime
does not automatically rerun already visited widgets. Requests implemented by
ordinary update, such as TextArea caret reveal and preferred-column reset, wait
for that widget's next eligible update. If immediate caret visibility is required
after a handler changes cursor, text, font or wrapping, perform another update
after the outer update returns (or allow a later queued input to supply that
update). Hidden/ineligible editors retain requests until eligible again.

This is an author obligation: retain such requests explicitly and distinguish
geometry invalidation from behavior that needs another update. A geometry commit
alone cannot certify completion of arbitrary pending model work. External/shared
state still requires explicit synchronization, as described above; neither a
handler return nor `Changed | NoChange` observes undocumented dependencies.

## Paths, buffers, resources, and errors

An immutable COM string is UTF-8 display text. It is not a native filesystem path. Native path capabilities distinguish Unix bytes from Windows UTF-16 code units, and the static facade converts those to `Path`/`PathBuf`. Preserve exact native selection independently of its potentially lossy display label. Directory reads and asset replacement validate their complete input before publishing a new live state.

Copy buffers are typed, borrowed for one call, and define explicit offset/count semantics. A whole-string replacement requires offset zero, validates UTF-8, and copies before returning. Validate bounds and overflow before writes or allocations. Immutable returned snapshots own their storage and release through their producer's COM reference counting.

The complete resource, replacement and authoring contract is documented in [COM_ASSETS.md](COM_ASSETS.md). Obtain the current bundle through `IWidgetHost::assets` and refresh any independent widget cache when its identity changes. Logical asset metadata and font tables may be cached per immutable bundle revision. Do not introduce a COM call per glyph or per vertex. Rendering crosses the boundary in bulk, ordered frames/batches; resource references are typed capabilities with validated renderer ownership. Preserve source ordering across ordinary UI geometry and custom render items.

Native GPU integrations belong in the same platform/render plugin as their backend. Rust `wgpu::RenderPass`, `CommandEncoder`, Ash owners, SDL windows, and `Rc<glow::Context>` do not become ABI-safe because the application facade is generic. Keep the backend-specific Rust integration local and expose concrete scene/mesh/camera/render-item interfaces outward. OS-window/context lifetime must cover GPU resource destruction.

A frame finishes explicitly and reports submission/presentation errors. Dropping an unfinished frame abandons it. A successful upload/replacement is published only after validation and backend preparation succeed. On failure, the previous valid bundle remains active. NcStatus reports the recoverable failure category; preserve useful backend diagnostics through the documented diagnostic capability.


Window capabilities expose current manager state through copied snapshots. A child inherits its family root's fixed layer; changing a managed child/dialog layer fails. Hiding a parent excludes its structural family, and showing it restores child-local visibility intent without reopening modal dialogs. Popups have a distinct interface, obey the active owner's modal policy, and retain their definition when hidden. Explicit close destroys the corresponding subtree and closes its widgets even when the application retains handles. Parent capabilities are authenticated by their manager-owned COM identity before a widget's mount-once claim is committed. Unknown option bits, malformed visibility/clip tags, invalid dimensions and invalid fixed layers are rejected before mutation.

## Widget class descriptions and shared protocols

Declare each concrete produced control as an IDL class, for example
`Button: IWidget + ILeafLayout`. Put its control-specific editing methods on
that class and its construction parameters on a factory method. Keep an interface only when unrelated classes need
the same independently queryable protocol. The implementation uses
`#[com_object(Button)]` on the concrete Rust widget type and stores its semantic
fields directly beside the common runtime helper; it
does not declare a matching `IButton` or repeat a list of projections.

Query `IWidget` when an API needs the common node protocol and `ILeafLayout` when
the layout runtime needs leaf measurement. Those projections preserve the class
identity and refer to the same allocation and state. The static facade hides these
checked projections from application code. IDL class definitions across crates should
name protocol supertraits through their module path when needed so generated
companion types resolve together.

Use the generated `UiFactory` as the typed construction surface. The core plugin's root creates
`UiRuntime`; the separately activated nodes plugin returns `NodesPluginFactory`, whose typed
`bind(&UiRuntime)` method creates `UiFactory`. `UiRuntime::construction_host()` returns only the core
`IConstructionHost`, allowing binding and facade construction to authenticate the exact host
identity without making core expose a concrete nodes projection. Do not model construction as a
redundant `IUiFactory`, class-ID lookup, or private runtime pointer. Hosts call typed generated
methods, and `Ui::from_interfaces` rejects a factory bound to a different runtime.

## Common state, input and node ownership

Each concrete widget embeds one private `WidgetRuntime`. That non-generic helper
contains `neocogi_ui_core_contracts::widget::WidgetState` plus host-backed resource and
rendering machinery; it cannot store a semantic widget implementation. `WidgetState`
owns the host authority, common options, mount-once claim, logical closed flag, and
sticky geometry flag. Neither helper owns a runtime tree, application callback, or
additional widget allocation. Concrete class contracts require `IWidget`, whose
protocol includes host/mount/close, properties/set_options, update, take_change,
and paint. Leaf measurement remains the shared `ILeafLayout` protocol. Button
activation and button-specific styling live directly on `Button`; no
activation-only, style-only, or lifetime interface is required for that class.

`WidgetState` contains plain fields. Mutating contract methods receive `&mut self`,
and the generated COM object provides the exclusivity needed to reach them. Do not
wrap ordinary widget fields in `RefCell<Option<T>>`, temporarily remove the value,
or add a second implementation-level busy flag; those mechanisms duplicate the
receiver contract and make state flow harder to follow.

A semantic setter checks `state.writable()` before borrowing its data, validates
all replacement inputs, commits atomically, and calls
`state.record_change(GeometryChange::Changed)` when measurement/placement inputs
actually differ. Common option setters use `state.set_options`. Return the
update's own geometry result; NO_CHANGE must never erase a prior setter marker.
The runtime collects markers across the authoritative forest, including hidden
surfaces and descendants, before using measurement caches. Hidden/clipped widgets
still skip their ordinary update; authors must not rely on that update to notice
externally shared mutations or to flush a setter's pending marker.

The app queues raw input and then updates explicitly:

```rust,ignore
ui.input(UiInput::Key(KeyStroke {
    key: KeyCode::Tab, value: 0, pressed: true, repeat: false, modifiers: 0,
}), "")?;
ui.input(UiInput::Text, "complete Unicode commit ☃")?;
ui.update(drawable_size)?;
ui.render(drawable_size, renderer.as_com_ref())?;
```

UiInput uses drawable coordinates and carries no invented drag or pointer delta.
The runtime preserves queue order, computes held state/deltas, selects recipients
and localizes WidgetEvent pointer payloads to content coordinates. Modifier bits
are Alt=1, Ctrl=2, Shift=4, Super=8; printable logical keys are separate from text
commits. Function keys use numbers 1–24. Key-repeat is valid only on presses.
A cancel event clears held/focus/capture state in order; widgets must also handle
an eventless update with changed focus/active snapshots. Update every visible
widget even when it received no routed event: animation, shared-state
synchronization and editor blur commits can change geometry then.

Widget update implementations call `decode_widget_event(event, text)` before
using input. This checks the ABI tag, complete UTF-8 with offset zero, key and
button values, and the rule that only Text has a nonempty text argument. A text
borrow expires on return; copy it if it must be retained. WidgetUpdate is an
owned snapshot of content geometry, clipping, interaction and the runtime's
shared semantic action. It contains no painter or pointer into the runtime.
Properties are refreshed after update even when geometry is unchanged, so
mode-dependent focus/text-input policy is visible to the next routed event.

The runtime takes ownership of its retained context for each transaction and restores it
on every return path. No runtime RefCell guard spans a foreign call. Reentrant
runtime operations receive BUSY while the slot is occupied; ordinary peer
widget setters remain available during update. Measurement, painting and
property/change observation reject mutation through the common host authority.
Explicit close also respects observational phases. Node removal invokes logical
widget closure once; retained COM handles then report CLOSED. A component close
failure is returned by explicit window/runtime teardown after the logical
removal has committed. Final COM destruction cannot report an error and must
infallibly mark its event source closed, including when it happens during an
observational phase.

A failed measurement does not publish a zero-size preference. The runtime reports
the component error, leaves the node and its ancestor path dirty, and retries on
a later update. Widget authors should return an error for temporarily unavailable
measurement inputs and resume returning a valid nonnegative size after recovery.
Do not return a fabricated successful size to hide a failed resource lookup.

## Built-in editing and painting

Built-in leaf and container classes are their concrete semantic allocations.
Concrete `Textbox`, `TextBlock`, `Slider`, and `Number` operations live directly
on their callable class projections. Shared protocols such as
`ITextContent` and `ITextEditor` remain only where independently handled internal
content needs the same behavior. Every call edits the same state used by input,
measurement, and painting.

Linear and Grid keep their topology edits and parent-owned relationship metadata
on their concrete class projections. They require `IContainerLayout` only for
the runtime's shared measurement and placement protocol; `IContainer` remains a
reusable protocol for independently defined domain containers.

A Textbox removes CR and LF during construction, replacement and committed text
input. Its cursor is a byte offset at a Unicode scalar boundary, not a glyph,
grapheme or visual column. Replacing text moves the cursor to the end even if
normalization leaves the text unchanged; that cursor-only operation does not
invalidate measurement. TextBlock normalizes CR/CRLF to LF. Word wrapping breaks
at ASCII spaces and keeps an oversized word intact. These are explicit existing
policies, not promises of grapheme editing or a full Unicode line-break engine.

Numeric constructors validate finite values, finite ascending slider spans,
nonnegative steps and precision through `f32::DIGITS`. Setters reject nonfinite
values; sliders clamp valid values to their range. Checkbox checked state, swatch
color, scrollbar offset/range and button center-fill selection affect paint only.
Programmatic setters do not manufacture user-originated events. Label, font,
icon, wrapping and numeric-label changes retain a sticky geometry marker.

Button and ListBox expose `set_image(texture, fit_width)` directly on their
class contracts. The operation
retains an immutable `ITexture`, validates its positive dimensions once and then
commits the edit. A failed dimension query leaves the previous content intact;
recursive image editing during that query reports BUSY. Measurement and paint
use the immutable size and do not call back into the image. Image setters dirty
geometry but emit no activation. `clear_image` preserves the label and font;
selecting a button icon replaces its image, and selecting an image replaces its
icon. A retained widget can retain its image after logical window closure; clear
the image before closure or release the remaining widget handles to retire it.

Width fitting preserves aspect ratio using the allocated content width. Ordinary
inline placement uses the source dimensions bounded by available width/height,
matching the existing control layout policy. Images always use white tint,
independently of the theme's label foreground. For example:

```rust,ignore
let images = host.renderer.query::<ITextureFactoryAbi>()?;
let image = images.create_texture(size, NcCopyFromBufferOffset::new(0, &rgba))?;
let mut button = host.ui.widgets().button("Preview")?;
button.set_image(image.as_com_ref(), true)?;
// The button owns a reference; this local image reference may now be dropped.
```

Font and icon capabilities select semantic roles or resolve an exact named
resource once in the widget's runtime bundle. Unknown names leave the previous
selection intact. Normal measurement and painting retain stable resolved
identities and perform no COM calls per glyph. FillStates controls unframed
button center fill; the outer FRAME option remains owned by retained traversal.

Call `WidgetUpdate::validate()` or `WidgetPaint::validate()` before consuming a
copied phase snapshot, and decode event/text input with `decode_widget_event`.
Painting receives effective clipping, enabled/hover/focus/press state and logical
window activation as copied values. It has no mutable runtime context.

Every `IPainter` operation has a mutable receiver. A widget therefore uses a
mutable `Painter` facade while recording, and neocogi-com rejects conflicting or
reentrant access through another retained projection. The concrete recorder owns
ordinary invocation and frame-context fields; it does not use interior-mutable
slots to simulate mutation through `&self`.

`IPainter::triangles` remains available for custom widgets that already own a
complete local atlas batch. The input offset is zero, its length is a multiple
of three, and every position and UV component is finite. Built-in classes do not
pre-expand their visuals through that path. They issue compact frame, icon, text,
and image operations, which the runtime appends to its one display list and
expands during the single final renderer pass.


## Local custom leaves and solid drawing

The skin plugin's `StyleCompiler` class supports typed palette and metric editors.
`compiler.derive_style(base, metrics, PaletteEdit::Preserve.into())` keeps the base
theme artwork; `PaletteEdit::Flat(palette)` compiles complete flat appearance tables.
Both return a new immutable bundle in the current catalog without installing it
or changing the retained tree. Call `ui.install_assets(candidate, renderer)` through
a mutable `Ui` reference to publish it through the same validation and GPU preparation
as any other revision.
Invalid metrics or unknown palette tags leave the installed resources unchanged.
The shared `Color` and `FlatPalette` records are plain contract values; they carry
no renderer or parser implementation. `Ui` and `WidgetFactory` clones retain their
existing COM projections. Event handlers may prepare a candidate through shared access;
the host installs it through a mutable `Ui` reference after dispatch returns.

Derived styles use the ordinary portable bundle publication/install path, including
its pixel copy and renderer preparation. This API does not promise a palette-only
fast path. Applications should derive once per committed edit rather than every
animation frame, and preserve image-backed artwork when only metrics are edited.

`WidgetFactory::leaf(content, options)` stores a concrete `LeafContent` inline in
one COM allocation implementing `IWidget` and `ILeafLayout`. Its common state
supplies the same mount-once, closure, options and sticky invalidation rules as
built-ins. Content methods use static Rust dispatch; there is no second widget
tree or erased application value. Use a direct COM class with `WidgetState` when
the component also needs its own public editing interfaces.

Content is stored directly in the COM allocation. Widget phases receive an
ordinary mutable object reference, while the adapter may call a content method
with the narrower receiver declared by `LeafContent`. Recursive entry through any
projection returns `NC_E_BUSY` while that outer mutable COM phase is active. A
failed update records a conservative geometry change
because application code may have partly changed its model. Successful updates
must still report CHANGED truthfully. The external state, hidden-widget and
skipped-update obligations above apply equally to this helper. Animation that
changes only drawing can return NO_CHANGE.

`Painter::colored_triangles` takes local positions and packed RGBA colors. Solid
content never needs the atlas's white-pixel location: the UI supplies it while
executing its existing solid geometry commands. `stroke_line` and `fill_polygon`
share that geometry path. All points must be finite; line widths are nonnegative,
and triangle counts are multiples of three. Polygon filling retains the painter's
existing simple-polygon policy; applications must supply a non-self-intersecting
boundary in order.

`let inner = painter.clipped(rect)?;` creates a borrowed painter value with a
narrower clip. Commands issued through `inner` intersect that clip with the real
widget clip; commands issued through the original painter keep the original clip.
There is no mutable clip stack, callback, or restoration operation. The copied
scope still expires after the widget paint call, including scopes or raw painter
interfaces retained by a component. Negative extents are rejected and empty clips
draw nothing. Coordinates remain content-local after clipping.

## Container topology and edge metadata

An independently implemented container exposes `IWidget` and `IContainerLayout`
on one COM object. It exposes exactly one layout family: adding `ILeafLayout`
to that object is rejected. Obtain a private `IChildrenEditor` from the widget's
host during construction, retain it in the container, and return its `IChildren`
view from `IContainerLayout::children`. That view's canonical identity must stay
unchanged for the container's lifetime. The runtime accepts only collections
issued by that exact host. One collection can belong to only one mounted
container. Arbitrary look-alike collection objects and shared collections are
rejected before attachment.

The view and editor deliberately have different COM identities over one actual
child sequence. Querying the view cannot recover edit authority. Expose concrete
editing methods that keep Linear tracks, Grid spans or application-specific edge
metadata synchronized; do not return the private editor to callers who are not
responsible for that metadata. Child references do not transfer tree ownership.
Insertion consumes the child's sole attachment opportunity after validation.
Removal closes the entire subtree permanently. Retained application references
remain memory-safe and report closure; they cannot detach or reparent a widget.
Indices, owner identity, layout capabilities, collection ownership and prospective
cycles are checked before a claim commits. Reentrant topology edits during
attachment validation return `UI_E_BUSY`.

`ChildEditResult` separates a successful topology commit from a removed child's
fallible teardown. An editor call returning `Err` did not change topology. An
editor call returning `Ok(outcome)` did change topology, even when
`outcome.teardown` is an error. Publish matching metadata before calling
`outcome.finish()` or returning the outcome. For example, the essential order
inside a concrete replacement method is:

```rust
// The concrete model guard owns its state; it holds no RefCell borrow.
let mut model = self.take_model()?;
let edge = validate_edge(edge)?;
let outcome = self.editor.replace(index, child)?;
model.edges[index as usize] = edge;
// Metadata now describes the new child even if the old child's close failed.
outcome.finish()
```

Validate metadata before calling the editor. Acquire the concrete state guard
before any operation can commit. Do not attempt to acquire fallible state access
after the topology commit, and do not propagate teardown failure before updating
metadata. A concrete read or edit that reenters an already active semantic state
may return `UI_E_BUSY`. Common lifetime authority must remain separate from that
state. Closing a container during an active semantic edit must reject before
partially changing its lifetime.

Final release of the private editor immediately closes the collection, including
retained read-only views. If a layout invocation currently owns its children,
those nodes are retired when the owned transaction returns and actually closed
after the outer observational phase ends. They are no longer live tree edges.
The runtime does not permit semantic mutation during layout to make teardown
possible. Releasing the last editor during layout makes that layout result fail
with `UI_E_CLOSED`; the host must not submit it as a valid frame. Explicit
container removal closes its child storage even if a custom widget's own close
implementation subsequently reports an error.

## Scoped container layout

`IContainerLayout::measure` and `place` receive an `ILayoutService` and a
`LayoutScope`. The service owns the active child sequence. It never stores a
borrowed runtime node, mutable tree reference, or pointer to the caller's stack.
The container receives indexed operations and copied geometry only. The provided
scope is valid for that synchronous call; retaining the COM service does not
extend the scope. Tokens never wrap or become valid again.

Measurement permits `child_count` and `measure_child` only. Placement also permits
exact child allocation, copied content-extent reads, responsive participation and
explicit reporting of a changed child placement dependency. Placement-only calls
during measurement return `UI_E_BUSY`. A valid zero constraint remains different
from an unbounded constraint. Preferred child sizes include each child's own
frame. The container resolves tracks and relationships before passing an exact
rectangle to `place_child`; the runtime does not reinterpret that policy.

Nested child containers receive a fresh token. Their parent's token returns
`UI_E_EXPIRED_SCOPE` while the child token is active, then becomes valid again
when the child call returns. Direct reentry while a service method temporarily
owns its frame returns `UI_E_BUSY`. After the overall invocation ends, every
method rejects the old scope, including empty or otherwise harmless reads. All
success and failure paths restore the parent frame and authority.

Use the scoped service for child layout access. The separate `IChildren` view can
report its stable copied count while a layout owns the sequence, but `child(index)`
returns `UI_E_BUSY` then. A built-in algorithm validates the scoped count before
running, so an access error cannot be mistaken for an empty child sequence while
its edge metadata still contains entries.

Placement returns one `ContainerGeometry`: logical content extent, descendant
viewport and translation, and overflow propagation policy. The runtime clips the
viewport to the container's content allocation and publishes the result only
after the call succeeds. `HIDDEN` participation preserves ownership while gating
update, paint, input and focus for the entire subtree; `DISABLED` preserves update
and paint while excluding subtree input. Hidden children do not contribute active
overflow. Setter-change collection still visits hidden owned descendants.

Report every changed measurement or placement dependency through a truthful
update result or sticky setter marker. This includes shared scrolling offsets,
responsive participation policy, expanded/collapsed state, and layout inputs
changed indirectly by another object. `child_placement_changed` is for an
already active placement pass whose descendant depends on shared state despite
receiving the same rectangle; it does not replace the writer's responsibility to
request the initial layout. Independent plugins must honor these same ownership,
phase and invalidation rules.

A failed child measurement cannot become a cached zero preference. Failed
container measurement and placement retain invalidation and require a successful
retry before rendering. An implementation may have placed some children before
an error, but that partial pass is never a valid renderable frame. Restore owned
semantic state on all error paths and do not retain references obtained from
short local context wrappers after returning through COM.

## Composed controls and indirect invalidation

A composed control owns ordinary COM children. It shares explicit scalar data
only where two of those children need the same state. ScrollArea's viewport and
its real scrollbars share offset/range data; Disclosure's header and layout share
expansion data. TextArea's actual editor requests caret reveal through that same
scroll model. These models hold no parent widget, runtime, application, or COM
subscription reference. A child must not need to upgrade a weak parent handle to
perform an ordinary edit.

Composition does not reinstate the removed wrapper design. The registered outer
class still owns its own runtime, topology, and outer behavior directly. Each child
is a separately targetable retained role with its own identity. Do not introduce a
generic core containing the outer class's algorithm, or duplicate a child's state
on the parent merely to make forwarding convenient.

The public capabilities preserve each composition's structural roles:

- The `ScrollArea` class permits replacing its one application content root and
  requires the shared `IScrolling` protocol. It does not expose `IContainer`
  methods that could remove the transform surface or either scrollbar.
- The `Disclosure` class returns its actual text header and actual Linear body.
  Edit body children through the returned Linear capability, and edit the label
  or header options through the returned text capability. The fixed header/body
  pair itself cannot be removed through a generic container interface.
- The `TextArea` class is the composed root. Its direct editing calls forward to
  one actual editor child. Querying this root for `IScrolling` returns the same
  outer identity; querying it for the `ScrollArea` class projection fails. There
  is no operation that replaces its fixed editor with
  arbitrary content. The child exposes `IMultilineEditor`, has its own ordinary
  widget identity, and is the composition's single keyboard Tab stop.

For example, a static client can edit actual child capabilities without mirroring
plugin state:

```rust
let section = ui.widgets().disclosure("Details", DisclosureStyle::Tree, false)?;
let body = section.body()?;
let editor = ui.widgets().text_area("first line\nsecond line", TextWrapping::Word, true)?;
body.insert(0, &editor, LinearEdge::new(LayoutTrack::Content))?.finish()?;
section.header()?.set_text("Editable details")?;
section.set_expanded(true)?;
editor.set_cursor(0)?;
// Synchronize geometry and pending caret reveal before rendering.
ui.update(drawable_size)?;
```

`Changed | NoChange` from an ordinary update does not automatically detect changes
in shared models. In particular, clicking a disclosure header need not change the
header's own preferred size, and moving a text cursor need not change text size.
Nevertheless, expansion and caret reveal can change an ancestor's placement.
The writer of that shared model must retain a sticky notification, and the
owning layout widget must collect it through `take_change`. That notification
must survive skipped updates, collapsed ancestors, and intermediate render
preflight checks. The built-in models do this explicitly; independent plugins
must supply the same discipline for their own shared data.

A requested scroll offset may temporarily exceed the previous maximum. Placement
clamps it against freshly measured content. This lets `scroll_to_end` accompany a
content replacement without losing the new end to the old range. `ScrollInfo`
therefore distinguishes current requests from the last installed ranges and
viewport. Programmatic setters are silent: they do not fabricate user events.
The standalone scrollbar offset setter clamps immediately to its installed range.
Disabling a scroll composite clears its requests and ranges and changes the
structural children's participation on the next placement.

TextArea normalizes CRLF and lone CR to LF. Cursor offsets count UTF-8 bytes and
clamp to the preceding Unicode scalar boundary. Word wrapping breaks at ASCII
spaces and preserves an oversized word as one span. Text replacement, including
a text-identical replacement, requests caret reveal; so does cursor positioning.
These setters conservatively invalidate geometry because reveal can move the
viewport even when text dimensions are unchanged. Font and wrapping changes also
reset the preferred visual column and request reveal. Visible ordinary updates
resolve those pending requests; hidden editors retain them until eligible again.
Widget/application writers must run update before rendering after these edits.

Closing a composite closes its real descendants. A retained header, body, editor,
or bar reference remains memory-safe and reports logical closure; it cannot keep
an orphaned interactive subtree alive or be remounted elsewhere. Keeping such a
reference never transfers the parent's topology authority.

## Combo and compact menus

Create a combo through `Ui::combo_box(parent, items, selection)`, not the ordinary
widget factory. The runtime needs the parent up front because it constructs one
private popup definition with a real structural owner at the same time as the
unmounted header. The caller then inserts the returned `ComboWidget` into that
window's retained tree. The popup capability, row presentation, and coordination
events are deliberately not exposed.

The control owns its display labels and represents the committed stable leaf ID with
the NCIDL-generated `ChoiceSelection`. `ChoiceSelection::None` is not leaf zero: it
renders the runtime-owned `Select…` fallback unless `set_placeholder` replaces it.
The facade consumes and returns that generated contract type directly instead of
declaring a parallel Rust selection record. The flat `combo_box` and `replace_items`
helpers assign IDs equal to item ordinals. `combo_menu` and `replace_menu` accept the
generated core `ChoiceMenuEntry` preorder model and shared UTF-8 text directly, allowing
arbitrary stable IDs and nested submenus without another handwritten public tree:

```rust,no_run
use neocogi_ui::{ChoiceMenuEntry, ChoiceSelection, MenuTextRange};

let text = "FruitAppleBanana";
let entries = [
    ChoiceMenuEntry::Submenu {
        label: MenuTextRange { first: 0, count: 5 },
        children: 2,
    },
    ChoiceMenuEntry::Choice {
        id: 100,
        label: MenuTextRange { first: 5, count: 5 },
    },
    ChoiceMenuEntry::Choice {
        id: 200,
        label: MenuTextRange { first: 10, count: 6 },
    },
];
let combo = ui.combo_menu(window.as_com_ref(), text, &entries,
    ChoiceSelection::Selected(100))?;
```

Every submenu is followed immediately by its declared number of direct children;
nested descendants follow their owning child branch. Construction and replacement
atomically reject malformed counts, empty branches, invalid UTF-8 ranges, duplicate
leaf IDs, nesting beyond 32 branches, or an unknown selected ID. Programmatic edits
remain silent; `combo.changed()` yields immutable `ChoiceChanged` snapshots containing the selection
only after a pointer or keyboard choice commits. Display labels remain independently
available through `choice_label` and `display_text`; generic application data is
mapped from the committed stable ID.

Opening restores the retained submenu path containing the committed leaf and copies
that leaf into the transient keyboard candidate. The popup paints pointer hover,
then keyboard candidate, then committed selection: leaving the rows therefore
restores the keyboard/committed fallback. Arrows move only the candidate, Enter or a
row press commits, and Escape/outside dismissal preserves the prior selection.

Popup placement uses the header's complete outer border box rather than its inset
content rectangle. Its horizontal extent remains fixed to that box while row
content determines height, so theme-specific frames cannot shift the left edge or
extend the right edge.

The header and composition manager share one runtime-private typed state capability.
That allocation owns ordinary text, generated preorder entries, stable selection,
placeholder, revision, and lifecycle fields; every mutation has an exclusive
`&mut self` receiver. The manager associates it with a generic `PopupHandle`, while
the root forest receives only compact choice-menu values and ordinary relational
submenu edges. No generic Rust payload, `Any`, `Rc<RefCell<_>>` semantic model,
public popup handle, or mirrored menu tree crosses the boundary. Applications map
the committed stable ID back to their domain values.

Menus retain their compact representation: one surface owns direct row values,
with no synthetic widget tree per row. The runtime exposes `IMenus` alongside its
factory and window manager. A root `MenuBuilder` owns one native declaration;
child builders address append-only submenus inside it. Keep the root alive until
mounting. Dropping an unconsumed root expires its child builders and row sources.
Only root builders can be mounted, in their creating runtime, once. Failed owner,
shape or destination validation leaves the complete declaration usable.

```rust
let menus = ui.menus()?;
let bar = menus.create("")?;
let file = bar.submenu("File")?;
let open = file.item("Open")?;
open.set_shortcut_hint("Ctrl+O")?; // Presentation text, not a shortcut binding.
open.set_mark(MenuMark::Checked { checked: false })?;
file.separator()?;
file.submenu("Recent")?.item("Document")?;
menus.attach_bar(window.as_com_ref(), &bar)?;
// The actual compact rows now belong exclusively to the manager's forest.
open.set_label("Open file")?;
events.subscribe(open.activated()?, ConcreteOpenHandler)?;
```

A bar's direct entries must be submenus. `attach_bar` installs one intrinsic bar
on a window; trying to attach another reports `UI_E_POLICY`. Dynamic bar
replacement is not exposed. For an ordinary standalone popup, build a root with
items/separators/submenus and call `menus.popup(window, &builder)`, then use the
returned `IPopup` capability's `show()` method for current-pointer placement or
`show_at(anchor)` for an exact screen anchor. Its hide/close methods retain their
ordinary transient-lifetime behavior. Successful mounting consumes declarations;
subsequent builder appends report `UI_E_CLOSED`.

Menu item interfaces resolve the same native row before and after that transfer.
They expose label, enabled state, check/radio role, hint and validated font
selection. Retaining an item keeps its COM projection allocated without keeping
a menu surface alive. Closing its popup/window makes further edits report
`UI_E_CLOSED` and ends its activation source. Radio-group exclusivity, command
shortcuts and application actions remain application policy; presentation edits
are silent and do not implement those behaviors implicitly.

F10 and an unchorded Alt tap enter a window's bar; arrows navigate headings and
submenus, skipping disabled rows and separators. Enter/Space submits the selected
row and closes the active path, restoring ordinary application focus. A shown
standalone menu starts with no row selected: Down selects its first actionable
row before Enter. Submission captures the actual label at activation and uses
the shared `Activation` payload. Cross-source registration order still applies.

## Native paths and standard file pickers

`NativePath` owns an immutable `INativePath` snapshot. Its `unit_count` counts
Unix bytes or Windows UTF-16 code units, excluding any trailing terminator.
The two copy methods validate their exact requested ranges before writing and
reject a mismatched encoding. Unix invalid UTF-8 and Windows unpaired surrogates
are preserved. Decoding a different platform encoding is an error. `display()`
is explicitly display-only UTF-8; two distinct native names may produce identical
replacement-character labels and must remain distinct selections.

The native picker is a UI-owned COM component containing actual COM buttons,
textboxes, list rows, Linear containers and scrolling composites. Its internal
concrete handlers reuse the static `EventSession` implementation with weak
runtime ownership. The runtime keeps active modal controllers functional even if
the application releases a picker editing pointer. Close the picker or its owner
to end that logical component. Retaining a picker cannot keep a closed runtime
alive. The static facade still depends only on contracts and COM support; the UI
plugin uses its ordinary owned adapters internally without passing Rust model
state across the ABI.

Create the component and subscribe before dispatch, then open it from an ordinary
application handler:

```rust
let picker = ui.file_dialog(window.as_com_ref())?;
events.subscribe(picker.completed()?, ConcreteSelectionHandler)?;
let mut request = FileDialogRequest::new(native_directory);
request.title = "Choose a document".to_owned();
picker.open(&request)?;
ui.update_with_events(drawable_size, &events)?;
```

The handler receives the generated `IFileDialogSinkEvent::Accepted { path }`
or `Cancelled`. `neocogi_ui::events::completed_file_path` copies the exact
host-native `PathBuf` when the application needs it; display text is never
reconstructed as a lookup key.
Acceptance is lexical: application code decides whether the path must exist or
have a particular type before using it. A selected row retains its exact native
path while the filename field displays a possibly lossy basename. A real user
filename edit clears that selection; a programmatic display update does not.
Leading and trailing spaces are filename content. An unedited lossy directory
field still denotes the committed native directory; a real path edit makes its
UTF-8 text authoritative for the next navigation request.

Directory reads are synchronous and transactional. Every directory entry and
metadata lookup must succeed before either new column is published. Broken
symlinks and unreadable entries reject the whole candidate. A failed initial
open reports `UI_E_IO` (-1008), leaves the picker idle with its prior listing,
and exposes the OS error through `diagnostic()`. Failed navigation displays its
diagnostic while preserving the previous native directory, rows and selected
path. Successful navigation replaces only dynamic content; fixed controls and
scroll models remain, with offsets clamped against the new content on layout.
Overlapping opens report `UI_E_BUSY` without changing the active request.
Browser targets have no native filesystem access; native directory operations
there report the platform's unsupported-operation error rather than presenting
a fabricated filesystem picker.

Internal control events are swept in component creation order before application
sources, and both share the runtime's finite callback budget. Navigation rebuilds
rows after the component's current registration transaction ends. Application
callbacks may open an already prepared picker through its ordinary interface;
the following layout establishes its modal scope before later queued input.
The picker owns its window's exclusive lifecycle subscription, so use its
completion capability for application outcomes. Its returned window remains
available for ordinary geometry, visibility and destruction operations.

Cancellation hides the reusable modal and completes one active request. Reopening
before a previously queued Hidden notification is delivered does not cancel the
new visible request. Hiding/destroying an active modal or its owner cancels it
once. Explicit picker close and runtime teardown also cancel an active request
without invoking application callbacks during destruction.

Each payload on the completion source is already a terminal outcome of a request.
This source therefore finishes by preserving **all committed request outcomes**,
including the unattempted remainder of an active dispatch window. It rejects further emission and
new subscriptions after finishing; its existing lease can drain outcomes after
the controller/runtime has been destroyed. Failed handlers consume the attempted
outcome and preserve unattempted outcomes for retry. Cancellation of the lease
still discards pending outcomes. Ordinary widget/window sources retain their
separate rule: discard obsolete ordinary work and preserve only their explicitly
specified terminal fact. No producer stores callbacks or retains the application.


## Complete frames, images and application GPU work

`IRenderer::render` prepares and explicitly finishes one complete frame synchronously.
There is no cloneable COM frame or borrowed native render-pass object. UI records translated
semantic operations in painter order. Each nine-patch crosses into the retained GFX compositor
as one closed typed operation; GFX owns its source lookup, nine-cell expansion, visibility,
final clipping, and atlas batching alongside text and icon expansion. It then publishes the owned
`IRenderStream`. A retained stream/token expires
on success and failure, including zero-length reads and indexed resource queries.

A stream contains a bulk vertex buffer and ordered `RenderBatch` records. Atlas
ranges contain positive multiples of three vertices; image ranges contain four
vertices. Ranges must partition the buffer without gaps, overlap or unused trailing
vertices. Custom items contain positive content/clip rectangles, with the clip inside
both content and viewport. Unknown tags, invalid rectangles, non-finite vertex
components, truncated copies, zero invocation tokens and excess counts fail before
GPU execution. Limits are 16M vertices and 1M batches per complete frame. Adjacent
atlas triangles are coalesced. The native plugin makes two bulk reads, plus one owned
resource fetch for each image/custom batch, and reuses its CPU allocations.

The backend detaches its complete mutable state before calling a foreign stream or
querying resource identities. Native render/upload/scene-creation reentry returns
`UI_E_BUSY`; scene edits also reject preparation/execution reentry. All source reads
and resource authentication finish before a native frame is acquired. A failed
preparation discards already resolved resource snapshots and allows retry. GPU
execution failure is reported by explicit finish and does not present the failed
frame. Immediate GL work may already have executed: abandonment is not GPU rollback,
and native device/surface loss can require constructing a replacement renderer.

Create immutable images through the chosen renderer's `ITextureFactory`. Uploads
must have offset zero and exactly width*height*4 RGBA bytes, with positive dimensions
satisfying the common image validation/allocation policy. Publication follows a
successful upload. `ITexture::size` reports immutable source dimensions; a painter's
destination rectangle changes scaling without changing that image. No public native
texture ID is accepted across COM.

`Painter::texture` draws the complete source. `texture_region` accepts an explicit
normalized `TextureRegion`: finite, ordered edges in `[0, 1]`. Destination clipping
interpolates within that source region. Built-in paint forwarding preserves ordered
atlas/image boundaries and cropped source coordinates through the same executor;
it releases queued image references on forwarding errors and after completed
frames. Keeping a copied frame's vertices does not keep the image alive.

Resources belong to one native window/renderer instance, even when two instances
use the same GPU API. The native plugin authenticates canonical COM identity against
its own weak registry; a matching ABI or a claimed numeric owner does not prove
membership. Foreign images and scenes report `UI_E_OWNER` directly from the renderer.
The UI's frame API reports backend failures as `UI_E_RENDER`; query
`IRenderDiagnostics::diagnostic` on the renderer for the underlying preparation or
native-driver detail. Returned diagnostic strings are immutable snapshots.

An image keeps its native allocation, window and context alive after the public
window interface is released. A prepared draw keeps its image alive until finish or
abandonment. Final COM release immediately removes registry authentication before
that address can be reused; final native-image release deletes while idle, or queues
deletion until the active ownership transaction exits. Deletion selects the owning
GL context, and destructors never submit or present. Idle frame storage retains no
resource references, so it cannot form a device/resource cycle. Plugins remain loaded
for the process lifetime; hot unloading is unsupported.

The example platform exposes `IDemoScenes::create_scene` and `IMeshScene: IRenderItem`.
The scene is one COM object. It accepts complete finite mesh records and triangle
indices, plus atomically replaced column-major projection/view/model transforms.
Offsets are zero; limits are 4M vertices and 12M indices, every index must name a
vertex, and two empty buffers clear the mesh. Invalid mesh or transform edits leave
the prior scene intact. The native renderer snapshots shared immutable mesh buffers
before preparing actual depth-tested GPU work; it does not substitute CPU UI
triangles for application rendering.

Use `Painter::texture(rect, rgba, image)` and `Painter::item(rect, scene)` at the
intended source position. The full example is `demos/smoke/src/gpu.rs`. Scene edits
normally change paint only: render again; a UI update is needed if widget geometry or
other retained inputs also changed. Perform scene edits before painting. The GPU
owner can detect its own active preparation/execution, but cannot detect a separate
UI plugin's earlier paint phase. Widget/application authors must uphold observational
paint and synchronize externally shared scene/model changes; no automatic dependency
tracking is implied by `GeometryChange::NoChange`.

Backend-specific application renderers are compiled into the selected GPU plugin.
Their concrete command enums, GL context, Vulkan command buffers, WGPU encoders and
render passes remain private Rust values inside that binary. An independently
loaded GPU extension cannot borrow these native Rust objects through the generic
COM API. Such an extension would need a separately designed backend-specific ABI;
this migration intentionally provides typed scene/resource capabilities instead.

## Platform input and native composition

`UiRuntime: IInputSink` exposes the same ordered input queue through inherited
calls or a separately retained input projection. Cache that projection once in
the host loop. `IPlatformWindow::poll(destination)` borrows it only for the
synchronous call; the platform never owns an application input destination.
An application interceptor can implement `IInputSink` as a concrete COM class,
retain the runtime input projection and forward accepted events explicitly.

Drain `PollStatus::Event` until `IDLE`; `CLOSE` requests host shutdown. A failed
input delivery consumes the attempted event and preserves later queued events.
Do not replay the failed event automatically: a destination may have committed
part of its work before returning an error. Recursive polling of the same window
returns `UI_E_BUSY`, including during delivery. Native queues are per-window;
process quit is broadcast. Consecutive pointer motion may be coalesced within one
window's queue, but a button/key/text transition always separates motion runs.

After draining, read `size()`, call `update`/`update_with_events`, synchronize
`text_input(ui.text_input_area()?)`, and render. A changed physical size triggers
measurement through the ordinary update contract. Skip rendering zero-sized
or minimized drawables. Pointer positions and IME rectangles use physical
pixels, including captured positions outside the drawable; the platform converts
logical SDL or CSS coordinates at its own boundary.

`text_input_area()` reports only committed editable focus. Pending input,
geometry edits and removed focus must not expose stale rectangles. It is a
placement hint, not editor content or an editable text buffer. Synchronize it
after successful updates. Repeated activation preserves an in-progress native
composition. SDL's process-wide IME is controlled only by the focused native
window, so synchronizing an unfocused window cannot disable another's editor.

Logical key presses/releases carry the complete modifier snapshot and repeat
state. They never synthesize committed text. SDL TextInput and the DOM text
commit are authoritative, including complete Unicode strings; composition
previews remain platform-owned. Widget/app authors must not insert key scalars
again on the text path. Focus/capture cancellation is queued in input order and
resets held state. Fractional wheel motion is accumulated in physical pixels;
focus cancellation discards the previous gesture's remainder.
