# NeoCogi component design

A contract is a compile-time description of values and callable interfaces.
An object is private Rust state implementing those interfaces. A factory is an
interface with construction policy: it is the only role eligible for the
runtime's zero-argument bootstrap. A plugin packages implementations and one
root factory. An event is an interface role with immutable delivery rules and
generated typed adapters.

## Ownership

```text
.ncidl + imported source graphs
  -> build-time resolution and transitive SHA-256 signatures
  -> generated Rust layouts, vtables, wrappers, and fixed signatures
       -> compiled into each host/plugin that uses them

plugin: private Rust implementations + one factory descriptor
runtime: plugin registry + loaded-library lifetime + checked root activation
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

`interface`, `factory`, and `event` lower to the same `Interface` model. Role,
threading, required projections, and ordered methods are contract inputs.
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
trait. `#[com_object(Interface)]` supplies private storage; use `from_value` with
an explicitly initialized Rust value. Ordinary unit objects have no generated
`create()` shortcut.

`#[com_factory(Factory)]` additionally proves `ComFactoryInterface` and associates
the implementation with its exact interface. Unit factories receive a default
`ComFactoryBootstrap`; stateful roots implement this typed policy themselves.
Factory methods accept the actual constructor parameters and return owned COM
interfaces. No erased argument frame or runtime lookup by object ID is needed.

Mutating methods use `&mut self`. The existing allocation-wide call gate rejects
conflicting/reentrant access as `NC_E_BUSY`. Its internal `UnsafeCell` is the
single audited bridge from ABI pointers to a guarded Rust borrow; ordinary
implementation fields do not need `Cell` or `RefCell` for this purpose.

Event roles retain copied payload snapshots, exact typed sinks, subscription
leases, application-owned sessions, and generated port connectors. They use the
same interface ABI and signature checks. Mutable application handlers run under
the session's ownership policy; no `Any`, erased event payload, or Rust trait
object crosses the ABI.

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
`nc_get_factory_plugin_description_v2` export. A monolith links that same rlib,
registers its Rust `factory_plugin_description()` result, and links one runtime
with `static-runtime`. Both paths share factory activation and pointer checks.

Core object ABI remains v7. The changed plugin descriptor is independently
versioned as v2, with a versioned getter so an old layout is never read as the
new one. Removing the contract-registry slot changes runtime discovery to v3;
old table getters are removed. These experimental interfaces require rebuilding
hosts and plugins; there is no compatibility adapter retaining the deleted
mechanisms.

Inspection schema 9 contains plugin/dependency information and complete root
factory compatibility data. Full module/declaration inventories remain available
from IDL source tooling at build time, rather than shipping a parallel runtime
graph solely for inspection.

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
