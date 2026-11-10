# COM dependency

NeoCogi UI consumes the published COM SDK, host implementation and IDL compiler
at exactly `0.1.0` from crates.io. The root `Cargo.toml` selects
`=0.1.0` for `neocogi-com`, `neocogi-com-host` and `neocogi-idl`.
There is no local SDK fork, Git dependency or path override.

`neocogi-com` supplies caller/provider contracts, owned handles, object macros
and optional supplied-host service clients. Final hosts and test executables
explicitly link `neocogi-com-host`, whose Rust crate is `neocogi_com_host`.
`#[neocogi_com::com_main]` initializes that implementation. Providers use their
supplied `Runtime` handle; enabling `neocogi-com/runtime` selects clients only.
`neocogi-idl` generates bindings from the subsystem-owned `.ncidl` sources.
The SDK's core and macros resolve transitively from the same published release.

## Fetching packages

Ordinary Cargo builds download the public packages from crates.io. A GitHub SSH
identity is not required for the COM dependencies or the inspector. Run
`cargo fetch --locked` to populate the local registry cache; callers with a
complete cache can select `CARGO_NET_OFFLINE=true`.

## Platform dependencies

The three Platform packages use exact `=0.2.0` requirements from crates.io.
Math uses `rs-math3d = "=0.21.0"`. Update all three packages together:

- `neocogi-platform-contracts` owns renderer, shader, audio and platform contracts,
  shared validation, upload producers and typed GPU buffers. Build dependencies enable
  `build-support` for consumer-owned IDL generation and native staging.
- `neocogi-platform` implements renderers and their fixed-format shader compilers.
  Static hosts enable `platform` and exactly one hosting backend.
- `neocogi-platform-dylib` exports the final `nc_platform` provider with catalog
  identity `neocogi.platform.<backend>`. The compiler is owned by that provider.

The UI helper combines the contracts package's embedded schemas with UI-owned
sources. Each consumer generates in its own `OUT_DIR`. Resource build scripts
and the native artifact compiler hook use
`neocogi_platform_contracts::build::layout` and track both source and deployed
output paths for cached rebuilds. The hook preserves normal Cargo artifact edges
and their application compilation profiles; it owns no filesystem staging policy.

Rendering explicitly binds each compositor to a framebuffer or image surface.
Resource groups select permissions at creation; UI permits color/resource writes
and image import, with all depth operations disabled. Custom 3D groups retain
their separately selected permissions. See [rendering](../../crates/ui/docs/runtime/RENDER.md)
for ownership and per-operation errors. These pre-release interfaces change their
fingerprints: rebuild hosts and every provider together.

## Updating the release

Update all three exact `version` requirements in the root `Cargo.toml` together,
then update `Cargo.lock`. Preserve one SDK source and version across hosts and
providers: a Git/path dependency and a registry dependency have different Rust
package identities even when their version strings match. Independent Rust/Python
fixture builders copy the root declarations instead of maintaining their own pins.
Build and deploy peers together after changing the compiler or SDK.

The committed lockfile records registry checksums and the complete dependency
graph. Keep it current when changing the explicit COM versions, and validate
with `--locked`. Historical Git revision links remain recorded in
[UI and COM history](../project/HISTORY.md); switching the current dependency
source does not rewrite those snapshots.

Run these integration checks from NeoCogi UI, sequentially:

```sh
python3 -m unittest discover -s scripts -p 'test_*.py'
python3 scripts/check-contract-consumers.py
cargo test --locked -p neocogi-ui-contracts -p neocogi-ui --all-features --all-targets
cargo test --locked -p neocogi-demo-plugins --test shared_runtime --features neocogi-demo-plugins/glow,neocogi-demo-plugins/skin,neocogi-demo-plugins/extension
cargo check --locked -p neocogi-demo-full
cargo check --locked -p neocogi-demo-full --no-default-features --features monolith,glow,embed-assets
cargo check --locked -p neocogi-demo-full --target wasm32-unknown-unknown --no-default-features --features webgl
```

The document receiver schema belongs to `crates/ui/schemas`.
SDK unit/ABI/compiler-floor/archive/sanitizer checks belong to the companion
workspace and its [Rust task runner](https://github.com/NeoCogi/neocogi-com/blob/92fa66ebc70888f71f2d07f2bbc14335a2ffd4e6/tools/xtask/README.md).

## Inspector

The inspector is published with the SDK. Install the matching release:

```sh
cargo install neocogi-inspect --version '=0.1.0' --locked
neocogi-inspect dump path/to/plugin.so
```

See the [inspector guide](https://github.com/NeoCogi/neocogi-com/blob/92fa66ebc70888f71f2d07f2bbc14335a2ffd4e6/tools/neocogi-inspect/README.md)
for metadata, compatibility and load-order commands. The UI examples exercise
this workspace as callers and providers.
