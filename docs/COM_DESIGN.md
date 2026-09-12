# NeoCogi component design

NeoCogi uses generated C-shaped interfaces for Rust components built as
independent artifacts. Public contracts live in Rust-like IDL modules. Plugins
implement one typed root factory; the shared runtime validates and connects
artifacts but does not construct arbitrary classes.

## Architecture

```text
.ncidl source
  -> generated Rust contract crate
       -> ABI layouts and typed wrappers compiled into host/plugin
       -> MODULE reference imports the exact native description getter
  -> fingerprint-qualified contract library
       -> one compact immutable identity graph

plugin implementation
  -> private Rust state + generated COM object storage
  -> NcFactoryPluginDescription + root activation function

host + plugin + contract libraries
  -> libnc_com_shared_runtime
       -> validates identities/fingerprints/signatures
       -> retains loaded libraries and registry state
       -> returns the requested typed root factory
```

The split is deliberate:

- The contract crate gives Rust code types it can compile and call.
- The native contract library is the single runtime source of descriptive
  identity and dependency metadata for one exact module revision. It does not
  expose the original IDL source.
- The plugin owns executable implementation and construction policy.
- The runtime owns cross-artifact validation, registration, inspection, and
  module lifetime.

No layer uses Rust trait objects or allocator-owned Rust values across the
binary boundary.

## IDL descriptions

An IDL `class` is a public object description, not an implementation record:

```rust
#[version(1)]
trait IWidget {
    fn enabled(&self) -> Result<bool>;
    fn set_enabled(&mut self, enabled: bool) -> Result<()>;
}

#[version(1)]
class Button: IWidget {
    fn label(&self) -> Result<String>;
}

#[version(1)]
factory UiFactory {
    fn create_button(&self, label: &str) -> Result<Button>;
}
```

The generated `Button` description owns its derived identity, version,
threading capability, callable surface, and required `IWidget` projection. It
has no constructor. The generated `UiFactory` description defines the typed
construction call.

Names are exact. If this source is `neocogi/ui/core.ncidl`, the declarations are
`neocogi.ui.core.IWidget`, `neocogi.ui.core.Button`, and
`neocogi.ui.core.UiFactory`.
`#[version(1)]` derives identity from that name and version;
`#[id(1, "UUID")]` pins an identity when migration or external compatibility
requires it. IDL members are public by definition and do not use `pub`.

## Implementations

Implementation types contain only private state and method bodies:

```rust,ignore
#[com_object(neocogi_ui_core_contracts::Button)]
struct ButtonObject {
    enabled: bool,
    label: String,
}

impl IWidget for ButtonObject {
    fn enabled(&self) -> Result<bool, NcStatus> {
        Ok(self.enabled)
    }

    fn set_enabled(&mut self, enabled: bool) -> Result<(), NcStatus> {
        self.enabled = enabled;
        Ok(())
    }
}

impl Button for ButtonObject {
    fn label(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        NcString::from_str(&self.label)
    }
}

#[com_factory(neocogi_ui_core_contracts::UiFactory)]
struct UiFactoryObject;

impl UiFactory for UiFactoryObject {
    fn create_button(
        &self,
        label: NcUtf8View,
    ) -> Result<ComPtr<ButtonAbi>, NcStatus> {
        let label = label.to_owned()?;
        ButtonObject::from_value(ButtonObject { enabled: true, label }).query()
    }
}
```

The precise generated Rust lowering may use ABI wrapper types such as borrowed
UTF-8 views. What matters is that construction remains statically typed from
the factory method through the concrete implementation. There is no runtime
argument bag and no `Any` downcast.

Mutating methods should use `&mut self`. Generated object storage supplies the
exclusive call gate, so implementation state should remain plain fields unless
interior mutability represents a real independent synchronization requirement.

## Plugin root

One implementation is selected as the plugin root:

```rust,ignore
neocogi_com::factory_plugin! {
    name = "neocogi.ui.runtime",
    version = "1.0.0",
    contracts = [neocogi_ui_core_contracts::MODULE],
    factory = UiFactoryObject,
}

neocogi_com::export_factory_plugin!();
```

The resulting `NcFactoryPluginDescription` stores plugin identity/version, hard
plugin dependencies, exact linked contract references, root factory
identity/threading, and one no-argument activation pointer. The
final DSO exports `nc_get_factory_plugin_description` exactly once.

Static assemblies omit the export and pass `factory_plugin_description()`
directly to `Runtime::register_factory_plugin_description`.

## Dynamic contract linking

Each contract crate's `build.rs` generates both Rust bindings and the native
contract artifact. A full ABI fingerprint appears in the native library name
and getter symbol. Generated `MODULE` references bind directly to that exact
getter.

If the host uses UI/U1 and a plugin uses incompatible UI/U2, the operating
system can map both because their filenames differ. Registration then sees:

```text
logical module ID: neocogi.ui.core
host fingerprint:  U1
plugin fingerprint: U2
result: NC_E_CONTRACT_MISMATCH
```

A stable unversioned `contract.so` would allow loader coalescing to hide the
conflict. Fingerprint-qualified native identity is what makes the mismatch
observable and deterministic.

Contract dependencies are also exact native getter dependencies. On ELF they
appear as `DT_NEEDED`, on Mach-O as `LC_LOAD_DYLIB`, and on PE through the DLL
import table. The canonical description retains dependency module IDs and
fingerprints so the runtime verifies the graph rather than trusting filename
resolution alone.

NeoCogi's optional asset-authoring contract is one concrete example. Its
`neocogi/ui_assets.ncidl` source imports portable values and interfaces from
`neocogi/ui/core/`; generated signatures refer to those original UI types, and
`contract_neocogi_ui_assets` links to the exact fingerprinted
`contract_neocogi_ui_core` description. The assets plugin needs to name only the
asset-authoring module directly because registration follows that dependency
edge to the UI module.

## Runtime activation

Dynamic host code has one bootstrap sequence:

```rust,ignore
let runtime = Runtime::global();
let loaded = unsafe { runtime.load_plugin_library(plugin_path) };
let factory = runtime.activate_plugin_factory::<UiFactoryAbi>(
    loaded.plugin_id(),
)?;
let button = factory.create_button("OK")?;
```

Loading maps trusted code, resolves the factory-description symbol, validates
the complete contract graph and plugin dependencies, and publishes all records
atomically. Activation checks the requested root factory ID, ABI signature, and
threading capability before invoking plugin code. Every later product is
created through a typed factory method.

The dynamic `runtime` feature is intentionally unified across the assembly.
Its Rust facade and `runtime-sys` dependency contain only calls and table
declarations; registry ownership remains exclusively in the one
`libnc_com_shared_runtime` selected by the platform loader. This must not be
confused with `static-runtime`, which belongs only in a single monolithic image.

The runtime never discovers plugin paths recursively. An application or
`cargo neocogi` supplies a dependency-first plugin list. Required dynamic-load
failure is fatal and diagnosed; a partial plugin is never returned. Loaded
libraries remain mapped for process lifetime, so description and function
pointers cannot dangle.

## Inspection and storage

The runtime stores only accepted immutable descriptions, relationship indexes,
and retained module provenance. It does not copy implementation state,
constructors, or generated Rust bindings. Offline inspection borrows a validated
factory description during a synchronous callback and copies its structured
metadata into a factory-native schema-8 artifact.

The contract library stores:

- module name/ID and full ABI fingerprint;
- declaration name/ID/version/kind/threading summaries;
- exact dependency getters.

It does not store:

- original `.ncidl` source;
- private Rust fields or object instances;
- implementation vtables or factory code;
- runtime registry state;
- Cargo/workspace paths;
- class constructors or erased arguments.

The plugin library stores its implementation code, plugin description, direct
contract references, and root bootstrap. The host and plugin both compile the
generated ABI types they use, but the canonical runtime description is owned by
the linked contract library.

## Design boundaries

- Add a reusable IDL `trait` only for a capability shared or queried
  independently across object kinds.
- Use an IDL `class` for the complete public surface of one produced object.
- Use an IDL `factory` for construction, service acquisition, or implementation
  selection.
- Keep private state and policy in implementation crates.
- Pass owned variable-size values as COM objects and borrowed bulk data through
  bounded views.
- Treat dynamic libraries as trusted native code; descriptor validation is not
  a sandbox.
- Prefer coarse, useful ABI operations over chatty per-element calls.

This design removes the old duplicated class-description/implementation table
and makes ownership clear: IDL describes what can be called, factories decide
what gets created, and the runtime only validates and connects them.
