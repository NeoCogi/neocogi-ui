# NeoCogi component design

A contract is a compile-time description of values and callable interfaces.
An object is private Rust state implementing those interfaces. A factory is an
interface with construction policy: it is the only role eligible for the
runtime's zero-argument bootstrap. A plugin packages implementations and one
root factory. A published event is concrete message data; generated port
connections carry it across COM into a consumer-owned inbox.

## Ownership

```text
.ncidl + imported source graphs
  -> build-time resolution and transitive SHA-256 signatures
  -> generated Rust layouts, vtables, wrappers, and fixed signatures
       -> compiled into each host/plugin that uses them

plugin: private Rust implementations + one factory descriptor
COM runtime: plugin registry + loaded-library lifetime + checked root activation
             + caller-owned persistence contexts
host: supplies paths/dependency order and calls typed factory methods
```

Contracts produce no native shared libraries, static description archives,
exported getters, runtime module inventories, or registration hooks. Their
Cargo `links` metadata carries source graphs between build scripts. It is a
build-time ownership/dependency mechanism, not a native linker requirement.
`neocogi.com.events` follows this same rule; it needs no runtime seeding.

`cargo-neocogi` remains an optional assembly driver. The executable example in
[examples/com-plugin](../examples/com-plugin/README.md) builds its plugin and
host with ordinary Cargo, including a monolith.

## Interfaces and construction

`interface` and `factory` use one callable model. Role, threading, required
projections, ordered methods and named message outputs are contract inputs.
The old IDL `trait` and `class` spellings are removed. Generated Rust still uses
traits as its statically typed implementation surface.

In `interface Button: IWidget + IActions`, the first interface supplies the
physical vtable prefix. Subsequent interfaces are required sibling projections
reachable through signature-checked `QueryInterface`. Requirements are
transitive. Interface cycles and weakening a required shared-threading promise
are rejected during IDL resolution.

`#[com_interface]` generates every callable projection, including the factory
marker. `ComInterface` owns the IID, signature, vtable, and minimum threading
promise. There is no second object identity, version constant, or description
trait. `#[com_object(Interface)]` supplies shared private storage; use `from_value`
with an explicitly initialized Rust value. `ownership = exclusive` selects unique
storage and `into_box` instead. The same option applies to `com_class`; a concrete
provider emits only its chosen constructor family. Ordinary unit objects have no
generated `create()` shortcut. Shared calls use the borrow gate alone; exclusive
calls additionally protect admitted execution against concurrent owner removal.
This choice affects provider-local Rust helpers, not the C interface layout.

`#[com_factory(Factory)]` additionally proves `ComFactoryInterface` and associates
the implementation with its exact interface. Unit factories receive a default
`ComFactoryBootstrap`; stateful roots implement this typed policy themselves.
Factory methods accept the actual constructor parameters and return owned COM
interfaces. No erased argument frame or runtime lookup by object ID is needed.

Mutating methods use `&mut self`. The existing allocation-wide call gate rejects
conflicting/reentrant access as `NC_E_BUSY`. Its internal `UnsafeCell` is the
single audited bridge from ABI pointers to a guarded Rust borrow; ordinary
implementation fields do not need `Cell` or `RefCell` for this purpose.

Named outputs carry concrete data through generated COM posting. A receiver
copies and maps each emitted fact into an application message and sends it through
`std::sync::mpsc`. A persistent `Controller<Model>` owns the receiver and ordinary
application state through a borrowed `IApplicationDispatcher`. Composite models
and receivers belong to the concrete widget, whose `IWidget::dispatch_next`
shares its normal borrow gate and lifecycle. The retained runtime schedules
widgets descendant-first. Removing an exclusive `ComBox` owner rejects new calls;
Drop waits for admitted calls and destroys the value before returning. Widgets
request self-removal for the runtime boundary after their callback returns.
Dispatch snapshots use `WeakComBox`.
The COM process runtime owns no event loop. `LocalBox` grants typed concrete
access for exclusive widgets; `LocalCom` remains for shared services. Both use
the same COM borrow gate. See [exclusive ownership](EXCLUSIVE_WIDGET_OWNERSHIP.md).
No `Any`, erased event payload, borrowed callback or Rust trait object crosses
the event ABI. See the [event guide](../crates/ui/runtime/docs/EVENTS.md).

## Compatibility at the call boundary

The build helper computes a canonical module fingerprint including exact
transitive dependency revisions. Each declaration signature is SHA-256 over a
domain tag, that fingerprint, and its declaration ID. Thus changing a record or
an imported module changes the root factory signature even if its IID remains
unchanged. Comments, source-part names, and formatting do not affect signatures.

Canonical module encoding v2 gives the unified callable representation its own
hash domain, preventing any ambiguity with earlier encodings.

A plugin descriptor contains its name/ID/version, hard plugin dependencies,
complete factory IID/signature, threading promise, and bootstrap pointer. The
runtime validates and publishes this metadata atomically, then compares the
host's complete factory identity **before invoking bootstrap**. A signature
mismatch returns `NC_E_ABI_VERSION` without constructing an object. Returned
projections and later COM pointer transfers are checked too.

Different unconnected revisions may coexist in one process. They cannot be
connected through a mismatched factory or interface. There is no process-wide
one-revision-per-contract-module rule. This intentionally moves compatibility
from global module registration to the actual typed connection boundary.

Plugin dependencies still determine registration order. They identify required
providers; they do not discover paths or load code recursively. Library loading,
getters, bootstrap calls, and diagnostics run outside the catalog write lock.
Retained libraries keep borrowed descriptors and vtables alive for process life.
Required live-load failures remain diagnosed and fatal; activation mismatches
are returned as statuses.

## Dynamic and static builds

A plugin normally builds as an `rlib`. The final dynamic build overrides its
crate type and enables `dynamic-plugin`, which gates the single
`nc_get_factory_plugin_description_v1` export. A monolith links that same rlib,
registers its Rust `factory_plugin_description()` result, and links one runtime
with `static-runtime`. Both paths share factory activation and pointer checks.

Plugin descriptor and runtime table ABIs both remain v1 during pre-alpha; their
exported getters and Rust table types use that version consistently. Core object
ABI is v8. These version numbers are not a compatibility promise across
pre-alpha revisions: build hosts, runtimes, and plugins from the same checkout.
There is no compatibility adapter retaining the deleted mechanisms.

Inspection schema 9 contains plugin/dependency information and complete root
factory compatibility data. Full module/declaration inventories remain available
from IDL source tooling at build time. A widget catalog additionally transports
the finite structural value graph needed to author independently built widgets;
see [recursive values and inspection](RECURSIVE_VALUE_CONTRACT.md#com-inspection-graph).

## Why the previous mechanisms existed

The implementation history explains the guarantees retained here:

| History | Original purpose | Current representation |
|---|---|---|
| `a557404b`, `3c2ed874` | Callable classes, then separation of contracts from private implementations | One interface contract plus private object storage |
| `4b4428bb` | Require sibling capabilities beyond the physical base | Common interface requirements and recursive generated projections |
| `58d854d0`, `e6b03b65`, `3795f0dc` | Separate descriptions from constructors, then make runtime activation factory-only | Factory role and typed zero-argument bootstrap; no object constructor metadata |
| `ea6566b1`, `0aa02e32` | Give runtime registration exact native contract graphs | Complete transitive signatures checked at factory/interface boundaries |
| `2498bb71`, `8be012fe`, `3418246a` | Try embedded graphs, revert them, then restore one shared descriptor per module | Delete runtime graphs entirely; no embedded replacement graph |
| `57cd9101` | Precompute declaration signatures from transitive module fingerprints | Retained build-time signature derivation |
| `7f42429b`, `9a812a9f`, `6698f944` | Typed event generation, builtin contract seeding, and typed registration connectors | Keep generated typed delivery/connectors; remove only runtime seeding |

The revert of `2498bb71` records no detailed rationale. This change does not
infer one: it removes the runtime inventory that both storage alternatives
were trying to own.

## COM crate and linkage boundaries

Five production crates own COM: `core` supplies portable ABI/data support, `api`
supplies object helpers, built-in event contracts/adapters, and runtime clients,
`runtime` owns loading/registries/persistence execution, `macros` expands Rust
objects, and `idl` owns parsing through Cargo integration. Runtime declarations
are internal facade code, and event contracts are published from the facade's
IDL source graph with the Rust path `neocogi_com::events`.

Only the process runtime has a deployed COM shared-library form (`libnc_com.so`
on Linux). The facade, contracts, and event adapters compile into their consumers.
Shared runtime callers import its six C discovery functions; static/native and
browser consumers link the `nc_com` rlib. Procedural macros and IDL generation run
on the build machine and do not become target runtime dependencies. An rlib used
as an intermediate when building a shared library does not imply static linkage
of the runtime into the final host.
