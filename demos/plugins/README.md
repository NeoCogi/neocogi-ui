# Native demo composition

This crate declares the native DLL artifacts used by the existing demo host.
Cargo owns resolution, compilation, caching and exact paths. `build.rs` only
adds the resolved shared runtime directory to native linking; `register()` loads
selected artifacts in provider order. No build subprocess or assembly planner is
present. The full ownership contract is in the [native guide](../../docs/guides/COM_NATIVE.md).

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
without constructing an SDL window. The probe uses a fixed test-only C interface;
IDL-generated closed contracts and generic roots are tested by the compiler suite.

This is development composition. Embedded artifact paths are not an installed
application manifest; `cargo run`/`cargo test` supply loader paths.
