# neocogi-ui-build

Canonical UI, GFX and Skin interface sources and Cargo build helpers for the
NeoCogi UI `0.1.0-alpha.1` release. This is the only authored owner of those IDL
files; the matching `neocogi-platform-build` package supplies Platform and COM.

Use it as a build dependency with the exact release version. `sources(&[])`
imports the standard contract libraries. Add application sources with
`include_root` and `module`, then pass the request to `build` to generate Rust
and publish `IDL_<MODULE>` include paths. Contract-owner crates select their
local emissions with the typed `Module` enum.

Sources are materialized below the caller's `OUT_DIR`. Unchanged files preserve
their timestamps, changed bytes are restored and obsolete derived inputs are
removed only within the owned `neocogi-ui-sources` child. Errors retain operation,
path and IO context. No repository checkout, global source cache or source-tree
writes are required.

The [release guide](https://github.com/NeoCogi/neocogi-ui/blob/master/docs/project/RELEASING.md)
documents archive ownership, independent consumers and publication.
