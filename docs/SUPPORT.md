# Support and compatibility

`microui-redux` is pre-alpha and currently intended for experimentation and early integration. This
page records what downstream users can rely on without implying guarantees the project does not yet
make.

## Rust toolchain

The crate uses Rust 2024 edition but does not publish a minimum supported Rust version (MSRV).
Development and CI target the current stable Rust toolchain. Dependency updates or a pre-1.0 minor
release may therefore require a newer stable compiler.

## Platforms and renderers

The library owns retained UI behavior but does not create an operating-system window, graphics
device, or event loop. Those responsibilities belong to the application through
`RendererBackend` and the input-forwarding methods on `Context`.

The repository includes SDL-based Glow, Vulkan, and WGPU integrations as native executable
examples, plus a `web-sys` WebGL 2 integration for an HTML canvas. The automated project
workflows test isolated core features, check each native backend on Linux, and build/test the
browser host on pull requests. Publishing to GitHub Pages is a separate trusted deployment. Windows and
macOS portability are intended, but they are not currently a CI-backed platform guarantee. Treat
the example integrations as reference code to adapt to an application's own renderer and
lifecycle.

## API compatibility

During pre-alpha, APIs and data formats may change whenever a simpler, consistent design warrants
it. There is no compatibility or long-term-support commitment. Pin the checkout revision used by
your application and review the [changelog](CHANGELOG.md) before upgrading. The manifest version
is not a promise that the unreleased checkout matches a published package.

## Reporting a problem

Use the [GitHub issue tracker](https://github.com/NeoCogi/microui-redux/issues). Include the crate
version or commit, enabled Cargo features, Rust version, operating system, rendering backend, and a
minimal reproduction when possible.
