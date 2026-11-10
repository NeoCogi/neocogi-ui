# Support and compatibility

NeoCogi UI uses version **0.2.0-alpha.2** across its workspace packages and is
an alpha release, intended for experimentation and early integration. This page records
what downstream users can rely on without implying guarantees the project does
not yet make.

## Rust toolchain

The workspace uses Rust 2024 edition. The COM core, facade, runtime, macros and
IDL compiler declare Rust **1.85** as their minimum supported Rust version. The
inspector and normative ABI snippets are also exercised by the
[minimum-version suite](https://github.com/NeoCogi/neocogi-com/blob/92fa66ebc70888f71f2d07f2bbc14335a2ffd4e6/tools/xtask/README.md), which has a
dedicated upstream CI job. Compile-test tooling is pinned to a floor-compatible version.
The development workspace pins nightly Cargo because its native compositions use
artifact dependencies. External COM/IDL consumers retain the Rust 1.85 floor.
The upstream Rust task runner owns compiler-floor, extracted-archive and
independent host/provider checks. UI CI checks the pinned SDK through
its own UI, renderer and application consumers. UI and renderer dependency
versions are not covered by the COM compiler floor. The UI archive consumer is
separately exercised with current stable Rust; this does not establish an MSRV.

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
The [release guide](RELEASING.md) defines the independent archive and consumer
checks. GitHub Pages publication is a separate trusted deployment workflow.

## API compatibility

During the alpha series, APIs and data formats may change whenever a simpler, consistent design warrants
it. There is no compatibility or long-term-support commitment. Pin the same exact release for
your application and every plugin, and review the [changelog](CHANGELOG.md) before upgrading. The manifest version
is not a promise that the unreleased checkout matches a published package.

The COM SDK, host implementation and IDL compiler use the exact published
crates.io release `0.1.0`; this checkout owns their UI/render integration. Core, plugin
descriptor and runtime table ABIs remain at version 1 during pre-alpha.
The [COM ABI-v1 specification](https://github.com/NeoCogi/neocogi-com/blob/92fa66ebc70888f71f2d07f2bbc14335a2ffd4e6/docs/abi-v1.md) describes that
baseline; the [core version constant](https://github.com/NeoCogi/neocogi-com/blob/92fa66ebc70888f71f2d07f2bbc14335a2ffd4e6/crates/core/src/metadata/contract.rs)
is its numeric authority. The compiler owns the separate
[module fingerprint grammar](https://github.com/NeoCogi/neocogi-com/blob/92fa66ebc70888f71f2d07f2bbc14335a2ffd4e6/docs/contracts/IDL_MODULES.md#canonical-signatures)
and [encoder](https://github.com/NeoCogi/neocogi-com/blob/92fa66ebc70888f71f2d07f2bbc14335a2ffd4e6/crates/idl/src/resolve.rs). None promises
compatibility across pre-alpha checkout revisions.
Use the same exact registry release for hosts and providers; a Git or path dependency
with the same package version has a different Cargo identity. Native
modules stay loaded for the process lifetime; hot reload/unload is unsupported.

## Reporting a problem

Use the [NeoCogi UI issue tracker](https://github.com/NeoCogi/neocogi-ui/issues). Include the crate
version or commit, enabled Cargo features, Rust version, operating system, rendering backend, and a
minimal reproduction when possible.
