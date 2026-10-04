# Reload slot and attempt identity

A `ReloadSlot` manages a container whose child is one replaceable widget tree
constructed from a UI document. The container remains while successful reloads
replace its child. A construction attempt needs an identity before event receivers
are created: messages may already be queued when the application accepts or
rejects that attempt.

## The identity is a pair

`Generation` contains two private `u64` fields:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Generation {
    slot_id: u64,
    attempt: u64,
}
```

The application supplies slot IDs through one owned `ReloadIds`; each slot owns
its attempt counter. The token is 16 bytes and requires no token allocation,
reference counting, weak handle, pointer identity, or interior mutability.
Ordinary form construction and channel delivery retain their existing allocations.
No binary-size reduction is claimed from this representation change.

| Operation | Example token |
| --- | --- |
| Create slot A; its first load succeeds | `(41, 1)` |
| A's next load fails while preparing receivers | `(41, 2)`, never active |
| A reloads successfully | `(41, 3)` |
| Create slot B; its first load succeeds | `(42, 1)` |
| Destroy A, then create slot C | C's first attempt is `(43, 1)` |

`accepts` compares the whole pair with the currently published attempt. It returns
false before the first successful publication. A failed reload consumes an attempt
but leaves the previous active token and component unchanged. A successful reload
rejects earlier tokens; another slot never accepts them within the same domain.

## The application owns the ID domain

Create the allocator alongside the application's message flow, outside individual
slot/surface constructors. Reuse it for every slot whose tokens can be compared
by that application, including slots in different UI runtimes:

```rust,ignore
let mut reload_ids = ReloadIds::new();
let mut first = ReloadSlot::new(
    &mut reload_ids, ui.clone(), parent_window.clone(), grid_limits,
)?;
let mut second = ReloadSlot::new(
    &mut reload_ids, ui.clone(), parent_window.clone(), grid_limits,
)?;
```

Slot construction borrows the allocator briefly through `&mut`; the slot does not
retain an application reference or keep the allocator borrowed. Preserve that
allocator for future slot creation while old messages or producers can still feed
the same domain. Destroying a slot does not return its ID for reuse. Replacing the
allocator with `ReloadIds::new()` on surface replacement would violate this contract.

Separate allocators define separate domains and can issue equal numeric pairs.
Do not compare or merge their tokens. If several channels feed a dispatcher that
checks messages against the same slots, they need the same allocator too. Starting
a fresh allocator is valid for a genuinely new domain whose messages are isolated
from the old one; these IDs are not persistent document identities.

The designer retains `ReloadIds` in its application queue state and supplies it to
preview-surface creation. The runtime-UI example creates it in application startup,
alongside its channel. No slot constructor secretly creates an allocator.

Sequential creation needs no synchronization beyond `&mut self`. Concurrent slot
creation must serialize access to that same allocator using the application's
existing synchronization. Each thread or shared library must not introduce its
own counter. Copied `Generation` values can be sent to worker threads; this does
not make thread-local UI objects callable from those threads.

These are Rust application-helper values, not a new COM wire protocol. Independent
widget plugins keep using their existing COM factories and event queues. A static
counter compiled separately into each executable/shared object is deliberately
not used as a supposed process-wide authority.

## Reservation, failure and exhaustion

Slot construction validates its policy and then reserves a slot ID before calling
the container factory. Any failure after reservation consumes that ID permanently.
The private last-attempt state begins at zero; no application receives attempt zero.

Each reload reserves its next attempt before either the receiver callback or
widget construction. Receiver, construction, field-binding and publication failures
never make that token active or return its number for reuse. The active component
changes only after successful insertion/replacement through the owning container.

Both counters use checked increments. Their maximum value is usable once; subsequent
reservations return `NC_E_OUT_OF_RANGE` (as the reload API's string error for attempt
exhaustion). No counter wraps or restarts. Attempt exhaustion preserves the active
component and occurs before callbacks; exhausted slot allocation occurs before
calling the container factory. Existing slots can still reload after the allocator
has exhausted new slot IDs.

## Generations and widget origins answer different questions

The token identifies the accepted construction attempt. `DocumentEvent` also
contains a weak widget origin and the sole event owner. `event_for` checks generation
before any lookup/query, then origin liveness and current component membership:

```rust,ignore
if let Some(message) = message.event_for(&slot, "save")? {
    if message.downcast::<IActivatedEventAbi>()?.is_some() {
        app.save()?;
    }
}
```

Both checks matter: a current token cannot authenticate a foreign widget, and an
old attempt cannot target a replacement with the same authored key.

Application messages may intentionally retain only a generation and an owned
payload, such as an asynchronous result. Pair comparison protects those messages
without a widget-origin check. Dropping widgets still destroys them immediately
under the exclusive-ownership contract; it does not remove facts already copied
into the application queue. The application may deliberately consume historical
facts without calling `accepts`, but they must not be treated as current-form work.

## History and acceptance

`cf01e4a0` introduced a slot-local attempt counter for stable-container reloads.
That correctly distinguished successive loads of the same slot. Later event,
constructor and ownership changes retained it, while the public filter could also
be used with a channel shared across slots or retained across slot recreation.
The missing slot namespace caused both first attempts to compare equal. Existing
weak-origin checks already protected generated handler resolution; the standalone
payload-only filter remained incomplete.

The regression suite covers:

- Two live slots in different UI runtimes feeding one application channel.
- Slot destruction and recreation while an old payload-only message remains queued.
- Receiver, constructor and field-binding rejection with preserved active state.
- Same-slot successful replacement and rejection of all failed/previous tokens.
- Cross-thread token copies, the 16-byte representation, and permanent exhaustion
  of both counters without wrapping or invalidating existing slot counters.
- Common document receivers shared by two live forms, retaining independent origin checks.
- Designer preview-surface replacement using its retained application allocator.

Generated fixtures are regenerated, compiled and compared with current generator
output. Runtime/document suites, offscreen designer execution and native/WebGL
consumer compilation provide integration coverage. No browser execution or new
whole-repository audit is implied.
