# COM dependency

NeoCogi UI consumes the shared COM SDK, host implementation and IDL compiler
from [`NeoCogi/neocogi-com`](https://github.com/NeoCogi/neocogi-com), pinned to
`4d8d7228efbcc400f63a016f9d04ee81f6f5baf8` in the root `Cargo.toml`.
There is no local SDK fork or path override.

`neocogi-com` supplies caller/provider contracts, owned handles, object macros
and optional supplied-host service clients. Final hosts and test executables
explicitly link `neocogi-com-host`, whose Rust crate is `neocogi_com_host`.
`#[neocogi_com::com_main]` initializes that implementation. Providers use their
supplied `Runtime` handle; enabling `neocogi-com/runtime` selects clients only.
`neocogi-idl` generates bindings from the subsystem-owned `.ncidl` sources.
The SDK's core and macros resolve transitively from the same Git source.

## Credentials

The Git URL uses SSH and the caller's existing GitHub credentials. The workspace
sets `net.git-fetch-with-cli = true`, so ordinary Cargo builds use Git's SSH
configuration. For commands outside this workspace, including installation of
the inspector, select that behavior in the environment:

```sh
export CARGO_NET_GIT_FETCH_WITH_CLI=true
cargo fetch
```

Both CI workflows set that Cargo option. Their runners also need an SSH identity
with read access to the dependency repository; the checkout token for
NeoCogi UI alone does not grant access to a different private repository.
Credential provisioning belongs to the runner environment. No new secret or
credential-management action is introduced here.

## Updating the revision

Update all three Git dependency `rev` values in the root `Cargo.toml` together,
then run `cargo fetch`. Preserve one SDK source and revision across hosts and
providers; two Cargo sources with the same pre-alpha version still produce
different Rust types. Independent Rust/Python fixture builders copy the root
declarations instead of maintaining their own pins. Build and deploy every peer
again after changing the compiler or SDK; the extracted compiler contains ABI
and generation fixes beyond the former in-tree implementation.

The committed root lockfile fixes the complete standalone graph. Keep it current
when changing the explicit COM revisions, and validate with `--locked`.

Run these integration checks from NeoCogi UI, sequentially:

```sh
python3 -m unittest discover -s scripts -p 'test_*.py'
python3 scripts/check-contract-consumers.py
cargo test -p neocogi-ui -p neocogi-ui-document -p neocogi-ui-runtime -p neocogi-ui-nodes -p neocogi-ui-support -p neocogi-ui-design --features neocogi-com/runtime --all-targets
cargo test -p neocogi-demo-plugins -p neocogi-ui-support --test shared_runtime --features neocogi-demo-plugins/glow,neocogi-demo-plugins/skin,neocogi-demo-plugins/extension
cargo check -p neocogi-demo-full
cargo check -p neocogi-demo-full --no-default-features --features monolith,glow,embed-assets
cargo check -p neocogi-demo-full --target wasm32-unknown-unknown --no-default-features --features webgl
```

The document receiver schema now belongs to `crates/ui/document/schemas`.
SDK unit/ABI/compiler-floor/archive/sanitizer checks belong to the upstream
workspace and its [Rust task runner](https://github.com/NeoCogi/neocogi-com/blob/4d8d7228efbcc400f63a016f9d04ee81f6f5baf8/tools/xtask/README.md).

## Inspector

The inspector is maintained with the SDK. Install the same revision rather than
building a second local copy:

```sh
cargo install --git ssh://git@github.com/NeoCogi/neocogi-com.git --rev 4d8d7228efbcc400f63a016f9d04ee81f6f5baf8 --locked neocogi-inspect
neocogi-inspect dump path/to/plugin.so
```

See the [upstream inspector guide](https://github.com/NeoCogi/neocogi-com/blob/4d8d7228efbcc400f63a016f9d04ee81f6f5baf8/tools/neocogi-inspect/README.md)
for metadata, compatibility and load-order commands. The UI examples exercise this workspace as callers and providers.
