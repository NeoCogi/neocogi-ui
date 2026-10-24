# Support and compatibility

NeoCogi UI uses version **0.0.0** across its workspace packages and is
pre-alpha, intended for experimentation and early integration. This page records
what downstream users can rely on without implying guarantees the project does
not yet make.

## Rust toolchain

The workspace uses Rust 2024 edition. The COM core, facade, runtime, macros and
IDL compiler declare Rust **1.85** as their minimum supported Rust version. The
inspector and normative ABI snippets are also exercised by the
[minimum-version suite](../../crates/com/scripts/test-msrv.sh), which has a
dedicated CI job. Compile-test tooling is pinned to a floor-compatible version.
The development workspace pins nightly Cargo because its native compositions use
artifact dependencies. External COM/IDL consumers retain the Rust 1.85 floor.
The stable/MSRV and cross-compiler suites copy the library source boundary into
a temporary test workspace, excluding development-only artifact manifests.
This fixture builds no production assembly. UI and renderer dependency versions
are not covered by the COM compiler floor.

The [native guide](../guides/COM_NATIVE.md) documents ordinary Cargo application
builds and their current constraints. IDL generation is an in-process Cargo
build dependency; the obsolete package-scaffolding CLI is removed. The
workspace's separate min-size alias requires nightly and `rust-src`.

## Platforms and renderers

The UI plugin owns retained widget/window-manager behavior. The renderer plugin
owns the OS window, device and native renderer together. Applications retain typed
COM capabilities and drive the copied input/update/render sequence; Rust SDL/GPU
owners stay inside the renderer implementation.

The workspace includes native SDL/Glow, Vulkan and WGPU assemblies and a static
browser assembly using WebGL2 and DOM input/IME. Linux CI runs the real Cargo-built
full-demo and independent-extension tests for all three native renderers. Browser
CI checks actual Chromium input and lifecycle behavior. macOS/Windows native Cargo build
jobs are configured, but local Linux results do not establish those OSes' runtime
behavior. The [native guide](../guides/COM_NATIVE.md) records dependencies and driver limits;
the [browser guide](../guides/WEB.md) records text input and native-filesystem restrictions.
GitHub Pages publication is a separate trusted deployment workflow.

## API compatibility

During pre-alpha, APIs and data formats may change whenever a simpler, consistent design warrants
it. There is no compatibility or long-term-support commitment. Pin the checkout revision used by
your application and every plugin together, and review the [changelog](CHANGELOG.md) before upgrading. The manifest version
is not a promise that the unreleased checkout matches a published package.

The neocogi-com sources in `crates/com` are part of this checkout. Core, plugin
descriptor and runtime table ABIs remain at version 1 during pre-alpha.
The [COM ABI-v1 specification](../../crates/com/docs/abi-v1.md) describes that
baseline; the [core version constant](../../crates/com/core/src/metadata/contract.rs)
is its numeric authority. The compiler owns the separate
[module fingerprint grammar](../contracts/IDL_MODULES.md#canonical-signatures)
and [encoder](../../crates/com/idl/src/resolve.rs). None promises
compatibility across pre-alpha checkout revisions.
Use the workspace sources for both producer and consumer; a crates.io dependency
with the same pre-alpha package version is not a substitute for this fork. Native
modules stay loaded for the process lifetime; hot reload/unload is unsupported.

## Reporting a problem

Use the [NeoCogi UI issue tracker](https://github.com/NeoCogi/neocogi-ui/issues). Include the crate
version or commit, enabled Cargo features, Rust version, operating system, rendering backend, and a
minimal reproduction when possible.
