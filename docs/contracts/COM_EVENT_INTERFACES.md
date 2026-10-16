# COM event objects

The common event protocol lives in the ordinary
[COM event IDL](../../crates/com/api/idl/neocogi/com/events/events.ncidl).
`IEvent` is its root interface. Concrete events derive from this interface and
expose immutable snapshots through ordinary typed COM getters. Event data is
independent of the producer's subsequent edits and lifetime.

## Ownership and delivery

`IEventReceiver::post(origin, event)` transfers a **`ComBox<IEvent>`**. The producer,
queue and eventual handler move one exclusive event owner; they do not clone a
shared event reference. The receiver acquires the weak origin before committing
the transfer. Every error returns the same event owner, through the existing
checked COM owner-input protocol. A receiver that rejects a full or disconnected
inbox must return that event rather than destroy it and report failure.

`EventOrigin` explicitly distinguishes:

- `Widget(WeakComBox<IUnknown>)`, observing an exclusively owned widget;
- `Service(WeakComPtr<IUnknown>)`, observing a shared window or other service.

Neither case owns the producer or holds an execution lease. Expired origins
remain valid identities; only actual widget/service calls require a surviving
value. Queued facts remain readable after the producer disappears. The receiver
must enqueue during the producer call and defer application behavior until that
call returns. Visual ancestry does not determine event routing.

The Rust `EventMessage` holds this origin and the sole event owner. Its
`downcast::<ConcreteEventAbi>()` queries a typed **weak projection** of the event:
unsupported interfaces produce `Ok(None)`, while an ABI mismatch or other error
remains an error. A successful projection is not another owner. Its methods
return `NC_E_DROPPED` after the message's event owner is destroyed.

An event owner outside an envelope uses the ordinary
`event.query_weak::<ConcreteEventAbi>()` operation. It returns a weak projection
or the original COM error, including `NC_E_NOINTERFACE` for an unsupported type.
There is no free `downcast_event` function between these APIs. The envelope's
`downcast` keeps the filtering policy, mapping only `NC_E_NOINTERFACE` to `None`;
the same policy serves widget/service origin selection. Neither operation may
hide an ABI mismatch by converting all query failures to absence.

## Value transport and interpretation

The origin is an IDL value, not an untyped payload. Recursive `WeakComBox<T>` fields
retain only validated exclusive lifetime bookkeeping, including after removal.
Their checked decoder rejects null storage, a different interface/revision, and
shared-owner storage. A successful acquisition uses the provider's canonical
projection. `ComBox<T>` remains forbidden inside repeatable IDL value fields.

Weak fields cannot persist without an explicit reconstruction protocol. IDL
resolution and authoring metadata distinguish shared and exclusive weak fields;
the design catalog uses descriptor version 12, including concrete event getter metadata.

Rust can select concrete event interfaces with generic COM queries. Other hosts
can use the same IDL interfaces through the C ABI. This protocol does not prescribe
a script language, string-based field registry, per-form generated queue or
application message enum. Applications read the event getters directly and may construct their own domain commands.

## Concrete UI snapshots

The [core](../../crates/ui/core-contracts/idl/neocogi/ui/core/semantic_events.ncidl)
and [node](../../crates/ui/nodes-contracts/idl/neocogi/ui/nodes/semantic_events.ncidl)
contracts define event meaning independently of the producer's concrete class:

| Interface | Immutable information |
| --- | --- |
| `IActivatedEvent` | Captured label; shared by buttons, rows and menu commands |
| `ITextChangedEvent` | Complete text and its UTF-8 byte cursor |
| `ITextSubmittedEvent` | Submitted text |
| `ICheckboxChangedEvent` | Checked state |
| `INumericChangedEvent` | Finite numeric value; shared by slider and number controls |
| `IScrollbarChangedEvent` / `ISplitChangedEvent` | Nonnegative offset or divider position |
| `IWindowGeometryChangedEvent`, `IWindowMinimizedEvent`, `IWindowMaximizedEvent`, `IWindowRestoredEvent` | Captured window rectangle |
| `IWindowCloseRequestedEvent`, `IWindowHiddenEvent`, `IWindowDestroyedEvent`, `IPopupDismissedEvent` | Distinct lifecycle facts; the window root is a marker interface |
| `IChoiceChangedEvent` | Stable selection identity or explicit absence |
| `IFileAcceptedEvent` / `IFileCancelledEvent` | Exact native path or a distinct cancellation fact |

String getters return ordinary immutable `INcString` capabilities. An event owns
its captured data, and its getters never consult the originating widget. Concrete
event allocations are exclusive even when a payload contains a shared immutable
string or path capability.

Rust constructors such as `events::text_changed(text, cursor)` and
`events::numeric_changed(value)` return `Result<ComBox<IEventAbi>, NcStatus>`
directly. They validate finite numeric values, nonnegative geometry and UTF-8
cursor boundaries. Unobserved producers skip construction before copying labels
or other snapshot data. Observed controls pass borrowed text directly to the
snapshot constructor, which captures it before delivery. String snapshots store
one immutable string capability; repeated getters add references without copying
its contents. Foreign getter results remain subject to the consumer's domain checks.
There are no native widget output enums or conversion traits.

A custom widget adds an `IEvent`-derived interface and lists it in `#[events(...)]`.
The compiler derives getter metadata directly; see [event metadata](IDL_EVENT_METADATA.md).

## Construction and native form conveniences

`events::channel()` returns one common receiver and an `EventMessage` inbox.
Applications and composites that only enqueue these messages use that channel
directly. A custom receiver is needed only when it adds behavior at the transport
boundary, such as attaching a reload generation or forwarding a public origin.
All observed factories accept that same receiver projection. `EventBindings`
registers it by authored key without a widget type parameter. The constructor
binding selects only delivery policy. It does not encode an event type.

`EventBinding::from_queue(Option<ComRef<IEventReceiverAbi>>)` returns a `Result`.
It retains a shared receiver using fallible acquisition: a valid borrow from an
exclusive receiver returns `NC_E_INVALID_ARG`, preserving its sole owner.
`None` succeeds as `Unobserved`; a concrete shared handle can also move directly
into `EventBinding::Observed`. UI constructors propagate acquisition failures
before publication. Window, child, dialog and popup facades return the original
unmounted root with the error, so the caller can retry with another receiver.

Optional generated forms contain typed widget fields and `bind(component)` only.
`document_receiver(generation, sender)` is a shared library receiver that tags the
same exclusive event with a reload generation. `EventBindings::all` can select all
emitting widgets without generated wiring. `DocumentEvent::event_for(slot, key)`
checks the generation before lookup, then validates weak origin liveness and exact
current widget membership. Unrelated or removed controls return `None`.
The standard channel returns `NC_E_CLOSED` with the original event owner when its
inbox is gone. Producer convenience APIs discard that rejected snapshot and return
the status without rolling back committed widget changes.

## Validation

[Generated ABI regressions](../../crates/com/idl/tests/event_interfaces.rs) exercise
both origin kinds, live/expired weak fields, missing event interfaces, ABI revision
mismatches, exclusive destruction and return of the owner after rejected posts.
The generated receiver and event getters are invoked through actual COM tables.

The [snapshot and channel regressions](../../crates/ui/api/tests/event_snapshots.rs)
also verify that the standard inbox preserves allocation identity, keeps facts
readable after source destruction, and restores the same owner on disconnection.
Each observed scalar event now requires an exclusive allocation. The existing
[allocation probe](../../examples/com-plugin/examples/event_allocations.rs), run
on October 4 with `--features monolith`, recorded 2 setup allocations and 10,323
allocations for 10,000 scalar events including channel storage. The unobserved
path performed 10,000 updates with zero allocations. These are
allocation counts, not an elapsed-time benchmark. See the
[migration validation](../validation/COM_EVENTS_ONLY_2026-10-04.md).

[Built-in list-item regressions](../../crates/ui/runtime/src/runtime/tests/list_item_events.rs)
also measure empty, nonempty and Unicode labels through real COM updates for
pointer, Enter and Space activation. They verify no silent capture allocations,
immutable labels after edits/destruction and state preservation on rejected delivery.

The [October 4 benchmark](../experiments/COM_EVENT_DOWNCAST_2026-10-04.md) measured
shared `ComPtr` queries. Its exact timings do not measure this exclusive event
transfer and weak-projection path.
