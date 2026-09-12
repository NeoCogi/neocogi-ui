# Support and compatibility

`neocogi-ui` is pre-alpha and currently intended for experimentation and early integration. This
page records what downstream users can rely on without implying guarantees the project does not yet
make.

## Rust toolchain

The crate uses Rust 2024 edition but does not publish a minimum supported Rust version (MSRV).
Development and CI target the current stable Rust toolchain. Dependency updates or a pre-1.0 minor
release may therefore require a newer stable compiler.

The [cargo-neocogi command](../tools/cargo-neocogi/README.md) builds ordinary native
assemblies with stable or explicitly selected nightly Cargo. Its bundled `min-size`
preset requires nightly plus `rust-src` and currently targets x86_64 Linux GNU.
Foreign-target assembly is rejected because descriptor validation loads native
plugins. Registry components are supported through ordinary Cargo resolution;
the repository's pre-alpha component releases must be published separately before
users can select those versions from crates.io.

## Platforms and renderers

The UI plugin owns retained widget/window-manager behavior. The renderer plugin
owns the OS window, device and native renderer together. Applications retain typed
COM capabilities and drive the copied input/update/render sequence; Rust SDL/GPU
owners stay inside the renderer implementation.

The workspace includes native SDL/Glow, Vulkan and WGPU assemblies and a static
browser assembly using WebGL2 and DOM input/IME. Linux CI runs the real packaged
full-demo and independent-extension tests for all three native renderers. Browser
CI checks actual Chromium input and lifecycle behavior. macOS/Windows packaging
jobs are configured, but local Linux results do not establish those OSes' runtime
behavior. The [native guide](COM_NATIVE.md) records dependencies and driver limits;
the [browser guide](WEB.md) records text input and native-filesystem restrictions.
GitHub Pages publication is a separate trusted deployment workflow.

## API compatibility

During pre-alpha, APIs and data formats may change whenever a simpler, consistent design warrants
it. There is no compatibility or long-term-support commitment. Pin the checkout revision used by
your application and every plugin together, and review the [changelog](CHANGELOG.md) before upgrading. The manifest version
is not a promise that the unreleased checkout matches a published package.

The neocogi-com sources in `crates/com` are part of this checkout. Core ABI 7,
runtime API table version 2, and IDL module-fingerprint format version 1 are
separate discriminators.
Use the workspace sources for both producer and consumer; a crates.io dependency
with the same pre-alpha package version is not a substitute for this fork. Native
modules stay loaded for the process lifetime; hot reload/unload is unsupported.

## Reporting a problem

Use the [GitHub issue tracker](https://github.com/NeoCogi/neocogi-ui/issues). Include the crate
version or commit, enabled Cargo features, Rust version, operating system, rendering backend, and a
minimal reproduction when possible.
