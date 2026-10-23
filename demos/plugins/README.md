# Native demo composition

This crate declares the native DLL artifacts used by the existing demo host.
Cargo owns resolution, compilation, caching and exact provider paths.
Compiler-only staging copies provider DLLs into `lib/` beside the executable;
`register(runtime)` loads those filenames in provider order through access
supplied by the host. The executable initializes its ordinary runtime rlib through
`#[com_main]`; there is no COM runtime DLL or native-link bridge. The full ownership
contract is in the [native guide](../../docs/guides/COM_NATIVE.md).

Keep artifact dependencies here and normal Rust provider dependencies in the
host's static/browser path. Cargo rejects separate artifact/library aliases for
the same package inside one dependent package. Keeping the boundaries separate
also keeps artifact declarations out of reusable COM/IDL manifests.

The owning application selects `glow`, `vulkan` or `wgpu`; this crate deliberately
has no default backend. `skin` and `extension` select additional providers.

```sh
cargo test -p neocogi-demo-plugins --features glow,skin,extension
cargo test -p neocogi-demo-plugins --features vulkan,skin,extension
```

The regression loads a separate Cargo-built probe DLL. Its ordinary service
interface reads the shared catalog and publishes another static descriptor.
The host observes both operations through its own facade, then acquires the
published descriptor after dropping the probe owner. This catches accidental
private runtime instances and lost library pinning. It also checks the selected
renderer identity and repeat registration, making backend switches testable
without constructing an SDL window. It copies and renames the real test host's
application directory and re-executes it with Unix loader-path environment
variables removed before making those ABI calls. The probe uses a fixed test-only C interface;
IDL-generated closed contracts and generic roots are tested by the compiler suite.

Absolute Cargo paths are compiler inputs only. Runtime code holds filenames and
uses the executable's adjacent `lib/`. `register(runtime)` returns executable-location I/O
errors; mandatory plugin loading retains the runtime's fatal validation policy.
Staging tracks both input and copied files, so deleting a DLL or switching to a
cached renderer restores the selected bytes. Unused old files can remain in a
shared profile; only the compiled selection is registered.
