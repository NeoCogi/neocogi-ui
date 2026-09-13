# NeoCogi Platform

[![Crate](https://img.shields.io/crates/v/neocogi-ui.svg)](https://crates.io/crates/neocogi-ui)

NeoCogi Platform is a retained Rust GUI toolkit inspired by
[rxi/microui](https://github.com/rxi/microui). Applications use a thin, statically
linked typed facade. Concrete COM objects own widgets, containers, the UI runtime,
windows and renderer resources. Native deployments load a few independently built
plugins; browsers register the same interfaces with one static runtime.

This checkout is **0.9.0-alpha.0**, pre-alpha. Its API and binary contracts may change
without compatibility layers. The imported neocogi-com implementation is maintained in
this workspace. Concrete controls such as `Button` are callable COM class contracts;
shared protocols such as `IWidget` and `ILeafLayout` remain interfaces on the same
object and state.

| Windows 3.11 | Mac OS 9 |
| :--: | :--: |
| ![Windows 3.11 theme](res/microui-windows-3.11.png) | ![Mac OS 9 theme](res/microui-mac-os-9.png) |

## Run the demos

Install the assembly tool from this checkout, then build and run a complete native package:

```sh
cargo install --path tools/cargo-neocogi
cargo neocogi run --assembly full
```

Use `--variant vulkan` or `--variant wgpu` to select another renderer; the default
is Glow. Other assemblies are `simple`, `calculator`, `backend-frame-cube`, and
`smoke`. Use `--release` for an ordinary release package, or `build` instead of
`run` to stage without launching. The command prints the resulting directory.
Native assets are external by default; keep the staged `assets` and `themes`
directories with the executable. Add `--variant embedded` after the renderer
variant for an explicit embedded deployment. Assembly definitions live in
`[workspace.metadata.neocogi]` in [Cargo.toml](Cargo.toml).
The [tool reference](tools/cargo-neocogi/README.md) documents configuration,
versioned crates.io components, standalone TOML, and lockfiles.
The [package guide](docs/COM_NATIVE.md) covers dependencies, nightly min-size builds,
optional assets, actual GPU tests and executable-relative loading. A native demo
needs its adjacent runtime and plugin manifest; direct `cargo run` on the demo
package does not assemble those files.

## Workspace layout

Platform-owned COM infrastructure lives under `crates/com`: `core` defines the
ABI vocabulary, `macros` generates typed projections, `api` is the safe client
surface, and `runtime` plus `runtime-sys` own process-wide loading and linkage.
Build-time IDL tooling lives under `tools/idl`: `core` owns parsing, resolution,
and fingerprints, while `build` is the reusable contract-crate `build.rs`
integration. It remains separate from `tools/cargo-neocogi` so ordinary Cargo
builds can regenerate contracts without invoking the assembly CLI.
UI code follows the same role-based layout under `crates/ui`: `core-contracts` contains
UI-specific cross-component types, `api` is the application facade, and `runtime` is the one
deployable implementation containing retained nodes, built-in widgets and
containers, surfaces, and the window manager.

The UI runtime is deliberately one plugin. Source modules separate these concerns,
but update, layout, paint, and window traversal stay in one optimization unit.
External leaves and containers implement the same typed contracts; their
capabilities are discovered once at mount and cached for every hot-path phase.

Platform-normalized input, polling, native text-input placement, and window
construction live in `crates/platform/contracts`. The `neocogi.platform` ABI
depends only on `neocogi.renderer`; it accepts a narrow `IRgbaImage` and contains
no widget, skin, font, icon, or asset-catalog declaration. The current backend
plugin still co-locates platform and renderer implementations so no raw native
window or graphics handle crosses a dynamic boundary.

Rendering follows a deployment-oriented layout under `crates/render`. Glow,
Vulkan, WGPU, and WebGL builds have distinct plugin identities. Each renderer plugin
also owns the native window/device integration it needs, so graphics handles stay
private and a frame does not cross an extra plugin boundary. The backend-specific
Rust crates and shared helpers remain separate source concerns within that artifact.
`crates/render/contracts` owns the independent `neocogi.renderer` ABI: geometry,
raw RGBA images, GPU texture capabilities, and one bounded bulk frame stream. It
contains no UI asset or skin declarations; `IAssetBundle` merely exposes an
`IRgbaImage` projection to the renderer.
`crates/render/core` is their small, statically linked backend SPI; it is not another
plugin and does not depend on the UI runtime implementation.

`crates/assets` is the optional decoding and authoring plugin. User-defined UI
plugins belong outside the platform crates; `examples/ui-extension` demonstrates an
independently compiled leaf and container. A COM object is not automatically a
deployment plugin: buttons, containers, windows, menus, and file dialogs remain
individual typed objects implemented inside the cohesive UI runtime plugin.

For the browser:

```sh
scripts/web-demo.sh serve
```

The WebGL renderer plugin owns its canvas and DOM input/IME bridge. All four demo themes,
images and custom drawing examples remain available. See [WEB.md](docs/WEB.md) for
input limitations and the native-filesystem picker's browser restriction.

## Typed application code

Depend on the local facade while using this pre-alpha checkout:

```toml
[dependencies]
neocogi-ui = { path = "../neocogi-platform/crates/ui/api" }
neocogi-com = { path = "../neocogi-platform/crates/com/api" }
```

Once the host has activated `UiRuntimeAbi`, the facade creates and edits the actual objects:

```rust
use neocogi_ui::*;
use neocogi_com::{ComPtr, NcStatus};

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
Backend-owned Rust GPU types stay inside their renderer plugin.

The [documentation index](docs/README.md) links authoring, assets, deployment and
implementation references. [Examples](docs/EXAMPLES.md) maps each feature to working
source. [COM_MIGRATION.md](docs/COM_MIGRATION.md) records completed gates and remaining
cleanup; it distinguishes implementation progress from the accepted design.

## License

Project-authored code uses the [BSD 3-Clause License](LICENSE). Imported neocogi-com and
portions derived from rxi/microui retain their MIT terms. Bundled assets retain the
licenses recorded in [asset attribution](docs/ASSETS.md).
