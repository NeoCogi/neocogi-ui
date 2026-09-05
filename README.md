# microui-redux

[![Crate](https://img.shields.io/crates/v/microui-redux.svg)](https://crates.io/crates/microui-redux)

`microui-redux` is a retained Rust GUI toolkit inspired by
[rxi/microui](https://github.com/rxi/microui). Applications use a thin, statically
linked typed facade. Concrete COM objects own widgets, containers, the UI runtime,
windows and renderer resources. Native deployments load a few independently built
plugins; browsers register the same interfaces with one static runtime.

This checkout is **0.9.0-alpha.0**, pre-alpha. Its API and binary contracts may change
without compatibility layers. The imported nc-com implementation is maintained in
this workspace; ordinary widget inheritance such as `IButton: IWidget` uses one
object and shared state, including inherited interface calls.

| Windows 3.11 | Mac OS 9 |
| :--: | :--: |
| ![Windows 3.11 theme](res/microui-windows-3.11.png) | ![Mac OS 9 theme](res/microui-mac-os-9.png) |

## Run the demos

Build a complete native package and run its staged executable:

```sh
cargo com-build --demo full --backend glow
./target/com-native/debug/glow/prebuilt/full/microui-redux-demo-full
```

Other backends are `vulkan` and `wgpu`; other demos are `simple`, `calculator`,
`backend-frame-cube` and `smoke`. Use `--release` for an ordinary release package.
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

Once a provider has created `Ui`, the facade creates and edits the actual objects:

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
