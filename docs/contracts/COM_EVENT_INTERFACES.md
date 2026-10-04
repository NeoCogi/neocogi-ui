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

## Value transport and interpretation

The origin is an IDL value, not an untyped payload. Recursive `WeakComBox<T>` fields
retain only validated exclusive lifetime bookkeeping, including after removal.
Their checked decoder rejects null storage, a different interface/revision, and
shared-owner storage. A successful acquisition uses the provider's canonical
projection. `ComBox<T>` remains forbidden inside repeatable IDL value fields.

Weak fields cannot persist without an explicit reconstruction protocol. IDL
resolution and authoring metadata distinguish shared and exclusive weak fields;
the design catalog uses descriptor version 11 for that distinction.

Rust can select concrete event interfaces with generic COM queries. Other hosts
can use the same IDL interfaces through the C ABI. This protocol does not prescribe
a script language, string-based field registry, per-form generated queue or
application message enum. Any native payload conversion is consumer convenience,
not a prerequisite for transporting an event through COM.

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
| `IWindowEvent` / `IPopupDismissedEvent` | Captured lifecycle and geometry facts |
| `IChoiceChangedEvent` | Stable selection identity or explicit absence |
| `IFileAcceptedEvent` / `IFileCancelledEvent` | Exact native path or a distinct cancellation fact |

String getters return ordinary immutable `INcString` capabilities. An event owns
its captured data, and its getters never consult the originating widget. Concrete
event allocations are exclusive even when a payload contains a shared immutable
string or path capability.

Rust's optional `EventPayload` conversion moves a native snapshot into its concrete
COM event or reads a compatible event through those same public getters. It
validates semantic fields on construction and after reading foreign getters.
Existing native event enums can select these conversions without becoming the
common queue's transported representation. A custom widget can add a derived
event interface, as the demo's `ICounterChangedEvent` does, without adding a new
receiver interface.

## Validation

[Generated-code regressions](../../crates/com/idl/tests/event_interfaces.rs)
exercise both origin kinds, live and expired weak fields, successful and missing
event interfaces, mismatched ABI revisions, exclusive event destruction, and
return of the original event owner after a rejected post. The generated receiver
and event getters are invoked through actual COM tables.

The [October 4 benchmark](../experiments/COM_EVENT_DOWNCAST_2026-10-04.md) measured
shared `ComPtr` queries. Its exact timings do not measure this exclusive event
transfer and weak-projection path.
