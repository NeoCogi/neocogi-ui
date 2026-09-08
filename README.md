# microui-redux

[![Crate](https://img.shields.io/crates/v/microui-redux.svg)](https://crates.io/crates/microui-redux)

`microui-redux` is a retained Rust GUI toolkit inspired by
[rxi/microui](https://github.com/rxi/microui). Applications use a thin, statically
linked typed facade. Concrete COM objects own widgets, containers, the UI runtime,
windows and renderer resources. Native deployments load a few independently built
plugins; browsers register the same interfaces with one static runtime.

This checkout is **0.9.0-alpha.0**, pre-alpha. Its API and binary contracts may change
without compatibility layers. The imported nc-com implementation is maintained in
this workspace. Concrete controls such as `Button` are callable COM class contracts;
shared protocols such as `IWidget` and `ILeafLayout` remain interfaces on the same
object and state.

| Windows 3.11 | Mac OS 9 |
| :--: | :--: |
| ![Windows 3.11 theme](res/microui-windows-3.11.png) | ![Mac OS 9 theme](res/microui-mac-os-9.png) |

## Run the demos

Install the assembly tool from this checkout, then build and run a complete native package:

```sh
cargo install --path tools/cargo-nc-com
cargo nc-com run --assembly full
```

Use `--variant vulkan` or `--variant wgpu` to select another renderer; the default
is Glow. Other assemblies are `simple`, `calculator`, `backend-frame-cube`, and
`smoke`. Use `--release` for an ordinary release package, or `build` instead of
`run` to stage without launching. The command prints the resulting directory.
Native assets are external by default; keep the staged `assets` and `themes`
directories with the executable. Add `--variant embedded` after the renderer
variant for an explicit embedded deployment. Assembly definitions live in
`[workspace.metadata.nc-com]` in [Cargo.toml](Cargo.toml).
The [tool reference](tools/cargo-nc-com/README.md) documents configuration,
versioned crates.io components, standalone TOML, and lockfiles.
The [package guide](docs/COM_NATIVE.md) covers dependencies, nightly min-size builds,
optional assets, actual GPU tests and executable-relative loading. A native demo
needs its adjacent runtime and plugin manifest; direct `cargo run` on the demo
package does not assemble those files.

For the browser:

```sh
scripts/web-demo.sh serve
```

The platform owns its WebGL2 canvas and DOM input/IME bridge. All four demo themes,
images and custom drawing examples remain available. See [WEB.md](docs/WEB.md) for
input limitations and the native-filesystem picker's browser restriction.

## Typed application code

Depend on the local facade while using this pre-alpha checkout:

```toml
[dependencies]
microui-redux = { path = "../microui-redux/crates/microui-redux" }
nc-com = { path = "../microui-redux/crates/com/nc-com" }
```

Once the host has activated `UiRuntimeAbi`, the facade creates and edits the actual objects:

```rust
use microui_redux::*;
use nc_com::{ComPtr, NcStatus};

fn install(ui: &Ui) -> Result<ComPtr<IWindowAbi>, NcStatus> {
    let hello = ui.widgets().button("Hello, world!")?;
    let content = ui.widgets().linear(FlowDirection::TOP_TO_BOTTOM, CrossSize::Stretch)?;
    content.insert(0, &hello, LinearEdge::new(LayoutTrack::Content))?.finish()?;
    ui.window("Hello", Rect { x: 40, y: 40, width: 300, height: 120 }, &content)
}
```

`UiFactory` is a dynamically implemented class used by the facade as a shorthand.
A host may instead construct `ButtonAbi` directly from its generated typed
arguments and the runtime's `IWidgetHost`; no Button-specific interface or
untyped argument object is involved.

`EventSession<State>` owns application state and concrete `EventHandler<State, Event>`
implementations. Producers do not retain that state. The host polls copied input,
calls `ui.update_with_events(size, &events)`, synchronizes the platform's text-input
area, then calls `ui.render(size, renderer)`. Updates automatically measure changed
geometry before subsequent input. Setters retain invalidation; custom widget updates
report `GeometryChange::CHANGED` only when measurement or placement inputs change.
Resize invalidation is independent of that return value.

External shared state, hidden/skipped updates and observational painting remain
explicit widget/app responsibilities. Read [COM_AUTHORS.md](docs/COM_AUTHORS.md)
for the complete requirements and examples. Local custom leaves implement
`LeafContent`; GPU scenes expose concrete interfaces such as `IMeshScene: IRenderItem`.
Backend-owned Rust GPU types stay inside their platform plugin.

The [documentation index](docs/README.md) links authoring, assets, deployment and
implementation references. [Examples](docs/EXAMPLES.md) maps each feature to working
source. [COM_MIGRATION.md](docs/COM_MIGRATION.md) records completed gates and remaining
cleanup; it distinguishes implementation progress from the accepted design.

## License

Project-authored code uses the [BSD 3-Clause License](LICENSE). Imported nc-com and
portions derived from rxi/microui retain their MIT terms. Bundled assets retain the
licenses recorded in [asset attribution](docs/ASSETS.md).
