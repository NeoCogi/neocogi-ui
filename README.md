# microui-redux

[![Crate](https://img.shields.io/crates/v/microui-redux.svg)](https://crates.io/crates/microui-redux)

`microui-redux` is a retained, backend-agnostic Rust GUI toolkit inspired by
[rxi/microui](https://github.com/rxi/microui). Applications assemble uniquely owned `Node` trees,
retain typed weak handles for later access, and register windows, dialogs, popups, and menus in a
`Context`. Ordered input and typed widget events update that retained state; painting records a
backend-neutral display list for the application's `RendererBackend`.

> **Development status:** this checkout is pre-alpha. Its manifests use `0.8.0`, which is
> marked unreleased in the changelog. APIs and data formats may change without compatibility layers.

Compared with [microui-rs](https://github.com/neocogi/microui-rs), this crate embraces standard
library types, reusable retained trees, and richer widgets such as custom rendering callbacks,
dialogs, and a file dialog.

| Windows 3.11 | Mac OS 9 |
| :--: | :--: |
| ![Retained UI demo using the Windows 3.11 theme](res/microui-windows-3.11.png) | ![Retained UI demo using the Mac OS 9 theme](res/microui-mac-os-9.png) |

## Getting started

To use the API documented in this checkout, clone the repository and point your application at
the local core package. For an application beside the checkout:

```toml
[dependencies]
microui-redux = { path = "../microui-redux/crates/microui-redux" }
```

`microui-redux` does not create a native window or graphics device. Applications provide a
`RendererBackend`; the repository examples contain SDL-based Glow, Vulkan, WGPU, and browser
WebGL 2 integrations. Non-UI graphics stay application-owned: each example backend can be
specialized by a concrete command type and native renderer, giving application code direct GPU
preparation and recording access without `Any`, downcasting, or a portable pseudo-renderer. See
[backend frames and typed application rendering](crates/microui-redux/docs/BACKENDS.md) for the
complete ownership and command flow.

Clone the repository and run the full demo with one backend:

```bash
cargo run -p microui-redux-demo-full --no-default-features --features glow
```

See [Examples](docs/EXAMPLES.md) for the other backends, asset requirements, and size-focused
builds.

The full demo also runs in an HTML canvas. Once the repository's Pages workflow has been deployed,
open the [live WebGL demo](https://neocogi.github.io/microui-redux/), or build and serve it locally
with the Rust toolchain:

```bash
./scripts/web-demo.sh serve
```

## Minimal retained UI

Most application code starts with `microui_redux::prelude`. Construct each widget once, move its
unique node into a container, then register the completed tree in the context:

```rust
# use microui_redux::prelude::*;
fn install<B: RendererBackend>(context: &mut Context<B>) -> WindowHandle {
    let (_, hello) = Button::create(ButtonParameters::new("Hello, world!"));
    let (_, content) = Linear::create(LinearParameters::vertical([
        LinearItem::content(hello),
    ]));

    context
        .ui()
        .create_window(Window::new("Hello", rect(40, 40, 300, 120), content))
}
```

Forward platform input to `Context`, synchronize the retained UI once, then render the committed
state through the backend frame:

```rust
# use microui_redux::prelude::*;
# use microui_redux::render::RenderError;
fn draw<B: RendererBackend>(
    context: &mut Context<B>,
    dimensions: Dimensioni,
    frame_info: FrameInfo,
) -> Result<(), RenderError> {
    context.update_ui(dimensions);
    context.frame(frame_info).render_ui()
}
```

Contexts with application state use `update_ui_state` instead. Call the same update method after
every programmatic widget mutation before depending on its result. Custom widgets return
`Change::Changed` when update changes measurement or placement inputs; updates commit geometry
automatically. See the [widget author contract](crates/microui-redux/docs/WIDGETS.md#reporting-geometry-changes-during-update)
for external-state, visibility, ordering, and paint responsibilities. The complete
[`simple` demo](demos/basic/src/bin/simple.rs) supplies SDL windowing, input translation, atlas setup, and a
selectable example renderer.

## Documentation

The [documentation index](docs/README.md) routes each topic to one canonical guide. Useful starting
points are [built-in widgets](crates/microui-redux/docs/WIDGETS.md), [typed events](crates/microui-redux/docs/EVENTS.md),
[layout and synchronization](crates/microui-redux/docs/LAYOUT.md), [rendering](crates/microui-redux/docs/RENDER.md), and the
[example catalog](docs/EXAMPLES.md), and [browser build guide](docs/WEB.md). Release and adoption details live in the
[changelog](docs/CHANGELOG.md) and [support policy](docs/SUPPORT.md).

Applications normally import `microui_redux::prelude`; `microui_redux::retained` is available for
explicit imports of the retained core. Backend contracts live under `microui_redux::render`, atlas
construction lives under `microui_redux::atlas::builder`, and the complete generated API reference
for published releases is available on [docs.rs](https://docs.rs/microui-redux). Build the reference
for this checkout with `cargo doc -p microui-redux --no-deps --open`.

## License

Project-authored code and assets are licensed under the [BSD 3-Clause License](LICENSE). Portions
derived from rxi/microui retain its MIT terms, and bundled fonts, icons, and demo assets retain the
terms recorded in [asset attribution and licenses](docs/ASSETS.md).
