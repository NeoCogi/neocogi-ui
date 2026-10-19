# NeoCogi UI

[![Crate](https://img.shields.io/crates/v/neocogi-ui.svg)](https://crates.io/crates/neocogi-ui)

NeoCogi UI, formerly **microui-redux**, grew from
[rxi/microui](https://github.com/rxi/microui) into a broader workspace for modular
Rust applications. It now includes a COM component model and IDL tooling, a
shared-library and plugin architecture, retained UI and rendering, a
[theme editor](tools/theme-editor/README.md), and an experimental
[UI designer](tools/ui-designer/README.md).

Applications use a thin, statically linked typed facade. Concrete COM objects
own widgets, containers, the UI runtime, windows, and renderer resources. Native
deployments load independently built shared libraries and plugins, with an
optional monolithic build; browsers register the same interfaces with one static
runtime. The editors use these same components for live theme and UI previews.

All workspace packages use version **0.0.0**, pre-alpha. APIs and binary contracts may change
without compatibility layers. The imported neocogi-com implementation is maintained in
this workspace. Concrete controls such as `Button` have callable COM interface contracts;
shared protocols such as `IWidget` and `ILeafLayout` remain interfaces on the same
object and state.

The project is **NeoCogi UI**, hosted at
[`NeoCogi/ui`](https://github.com/NeoCogi/neocogi-ui). Cargo packages keep
descriptive `neocogi-*` names, while deployed libraries use the short `nc_*`
targets (`nc` means NeoCogi). See the [shared-library mapping](docs/guides/COM_NATIVE.md#profiles-and-output)
and [executable mapping](docs/guides/EXAMPLES.md) for exact outputs.

| Windows 3.11 | Mac OS 9 |
| :--: | :--: |
| ![Windows 3.11 theme](res/microui-windows-3.11.png) | ![Mac OS 9 theme](res/microui-mac-os-9.png) |

## Run the demos

The checkout pins nightly for native artifact/linkage integration and target
standard-library builds. Build and run the selected application directly:

```sh
cargo run -p neocogi-demo-full
cargo run -p neocogi-demo-full --release --no-default-features --features dynamic,vulkan
cargo run -p neocogi-demo-full --profile min-size --no-default-features --features dynamic,vulkan
cargo run -p neocogi-demo-basic --bin neocogi-demo-calculator
cargo run -p neocogi-demo-smoke -- --smoke
```

Cargo builds the shared runtime and selected plugins, copies them into `lib/`
beside the executable, and records relative Unix loader paths. The complete
min-size profile includes a size-optimized standard library and immediate abort.
Native defaults are `dynamic,glow`;
features are additive, so another renderer or `monolith` requires disabling
defaults. For a static application with compiled resources:

```sh
cargo run -p neocogi-demo-full --no-default-features --features monolith,glow,embed-assets
```

File-backed resources are copied into adjacent `assets/` and `themes/` directories;
`embed-assets` compiles them into the executable instead. Copy the executable,
`lib/` and those resource directories together to relocate the application
(also the adjacent `nc_com.dll` on Windows). Build paths remain compiler inputs;
debug unremapping sidecars must stay on the build machine. Distribution
installation remains separate. The [native guide](docs/guides/COM_NATIVE.md) explains ownership,
linkage, resources, browser builds and remaining Cargo costs. The optional
[cargo-neocogi tool](tools/cargo-neocogi/README.md) manages NCIDL module authoring;
it no longer builds assemblies.

For VS Code, install the recommended CodeLLDB extension and select **Full demo:
Vulkan (shared, debug)** or **Full demo: Glow (shared, debug)** in Run and Debug.
The workspace automatically restores local source mappings for copied plugins
and generated COM code. See [native debugging](docs/guides/COM_NATIVE.md#debugging-in-vs-code).

## Workspace layout

COM infrastructure lives under `crates/com`: `core` defines the
ABI vocabulary, `macros` generates typed projections, `api` is the safe client
surface, and `runtime` owns process-wide loading; the facade selects shared or static linkage.
Build-time IDL tooling lives in `neocogi-idl` under `crates/com/idl`, which owns
parsing, resolution, fingerprints, Rust generation, and reusable contract-crate
`build.rs` integration. Contract examples live under `tools/idl`. The reusable tooling remains
separate from `tools/cargo-neocogi` so ordinary Cargo builds can regenerate
contracts without invoking an external compiler command.
UI code follows the same role-based layout under `crates/ui`: `core-contracts` contains
shared node, layout, popup, menu, and window protocols; `nodes-contracts` owns the concrete
built-in widget/container classes and their typed factory; and `api` is the application facade.
`runtime` implements and deploys the retained forest, surfaces, generic menus, and window manager.
`nodes` deploys built-in leaves, containers, Combo headers, and standard file-dialog
compositions as a separate provider. Their concrete `.rs` implementations live under
`nodes/src/widgets`, `nodes/src/containers`, and `nodes/src/dialogs.rs`; core contains no built-in
catalog or concrete factory. `support` holds only binary-local resource imports, pure presentation
algorithms, and phase-local drawing helpers. Neither provider has a production dependency on the
other provider's Rust implementation, and there is no extra runtime-plugin wrapper crate.

The core runtime and built-in nodes are deliberately separate plugins. A host creates the core
runtime, activates `neocogi.ui.nodes.runtime`, and binds its `NodesPluginFactory` to that exact
runtime. The core never exposes or constructs `UiFactory`; host identity validation prevents a
interface bound to one runtime from being paired with another. External leaves and containers use
the same typed protocols and cached hot-path capabilities as built-ins.

Platform-normalized input, polling, native text-input placement, and window
construction live in `crates/platform/contracts`; immutable native paths live
there as well because both file dialogs and resource loaders consume them. The `neocogi.platform` ABI
depends only on `neocogi.renderer`; it accepts a narrow `IRgbaImage` and contains
no widget, skin, font, icon, or asset-catalog declaration. The current backend
plugin still co-locates platform and renderer implementations so no raw native
window or graphics handle crosses a dynamic boundary.

Rendering follows a deployment-oriented layout under `crates/render`. Glow,
Vulkan, WGPU, and WebGL builds have distinct plugin identities. Each renderer plugin
also owns the native window/device integration it needs, so graphics handles stay
private. UI sends coarse painter-order operations to GFX, which submits one bulk
frame to the selected renderer. The backend-specific
Rust crates and shared helpers remain separate source concerns within that artifact.
`crates/render/contracts` owns the independent `neocogi.renderer` ABI: geometry,
raw RGBA images, GPU texture capabilities, and one bounded bulk frame stream. It
contains no UI asset or skin declarations; `IAssetBundle` merely exposes an
`IRgbaImage` capability returned by `IAssetBundle::image()` to the renderer.
`crates/render/core` is their small, statically linked backend SPI; it is not another
plugin and does not depend on the UI runtime implementation.

`crates/gfx` owns the `neocogi.gfx` decoding and resource-baking plugin, while
`crates/skin` owns the `neocogi.skin` appearance and external-theme plugin.
The GFX contract owns complete portable bundles and the renderer-neutral compositor,
so both UI and Skin depend on GFX instead of resource construction or final graphics
execution depending backward on the retained UI.
Their concrete compiler machinery lives in separate private GFX and Skin
implementation crates. Portable bundle publication and validation live in
non-plugin `crates/gfx/support`, while a small appearance crate contains only
pure typed appearance compilation. None of these support crates publishes a
plugin identity. The split keeps PNG, font, and atlas services out of skin-only
clients, and keeps JSON/schema services out of GFX clients. User-defined UI plugins belong outside the platform crates;
`examples/ui-extension` demonstrates an
independently compiled leaf and container. A COM object is not automatically a
deployment plugin: the complete built-in family shares one nodes provider, while logical windows,
generic menus, and popup policy share one core runtime provider.

See [component and plugin boundaries](docs/architecture/COMPONENTS.md) for the complete
dependency graph, ownership rationale, resource policy, and frame-cost invariants.

For the browser:

```sh
scripts/web-demo.sh serve
```

The WebGL renderer plugin owns its canvas and DOM input/IME bridge. All four demo themes,
images and custom drawing examples remain available. See [WEB.md](docs/guides/WEB.md) for
input limitations and the native-filesystem picker's browser restriction.

## Typed application code

Depend on the local facade while using this pre-alpha checkout. These paths
assume the repository is cloned into a sibling directory named `ui`:

```toml
[dependencies]
neocogi-ui = { path = "../neocogi-ui/crates/ui/api" }
neocogi-com = { path = "../neocogi-ui/crates/com/api" }
```

With an initialized `Ui`, the facade creates and edits the actual objects:

<!-- checked-example: application-install -->
```rust
use neocogi_ui::*;
use neocogi_com::{ComPtr, NcStatus};

fn install(ui: &Ui) -> Result<ComPtr<IWindowAbi>, NcStatus> {
    let hello = ui.widgets().button_unobserved("Hello, world!")?;
    let mut content = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch)?;
    content.insert(0, hello, LinearEdge::new(LayoutTrack::Content))
        .map_err(|(status, _returned_widget)| status)?;
    ui.window("Hello", Rect { x: 40, y: 40, width: 300, height: 120 }, content)
        .map_err(|(status, _returned_root)| status)
}
```

The `neocogi.ui.nodes.runtime` plugin provides `UiFactory`. Its
`NodesPluginFactory::bind` method consumes a borrowed `UiRuntime`, queries the public
`IConstructionHost`, `IChoiceMenus`, window, and resource capabilities it needs, and returns one
host-bound factory. `Ui::from_interfaces(runtime, factory)` verifies that both sides expose the
same canonical `IWidgetHost` identity before the facade permits construction. No class ID,
untyped argument object, private runtime pointer, or root identifier crosses that boundary.

Use `events::channel()` to obtain one common `IEventReceiver` and its inbox.
Pass the same receiver to observed widgets through `*_with_queue`. Each message
owns a `ComBox<IEventAbi>` and a weak origin; query the concrete immutable event
interface when handling it. The channel only transfers ownership. Application
behavior runs later in the dispatcher, with ordinary mutable state:

<!-- checked-example: application-events -->
```rust
use neocogi_ui::events::{channel, Dispatcher, EventMessage, MessageProcessor};

/// Application state changed at the deferred handler boundary.
struct Counter {
    /// Accepted activations from a still-live button.
    count: u32,
}
impl MessageProcessor for Counter {
    type Message = EventMessage;
    /// This application ignores queued work from a removed button.
    fn handle(&mut self, message: EventMessage) -> Result<(), NcStatus> {
        let Some(origin) = message.origin.widget::<ButtonAbi>()? else { return Ok(()); };
        if origin.is_alive() && message.query::<IActivatedEventAbi>()?.is_some() {
            self.count += 1;
        }
        Ok(())
    }
}

// Inside application setup, with an initialized `ui: &Ui`:
let (queue, messages) = channel();
let button = ui.widgets().button_with_queue("Count", Some(queue.as_com_ref()))?;
let window = ui.window("Counter", Rect { x: 20, y: 20, width: 400, height: 200 }, button)
    .map_err(|(status, _unmounted)| status)?;
let app = Dispatcher::into_local(Dispatcher::new(Counter { count: 0 }, messages));
let dispatcher = app.query::<IApplicationDispatcherAbi>()?;
```

Retain `app` and `dispatcher` in application state. Once per frame, after forwarding
platform input, call the outer update with the current `size: Size`, then synchronize
text-input geometry and render:

<!-- checked-example: application-update -->
```rust
ui.update_with_events(size, dispatcher.as_com_ref())?;
```

This handler chooses to ignore removed buttons; their captured event facts remain
readable. `is_alive()` is an observation, and a later widget call still checks its
lifetime. `*_unobserved` chooses permanent non-delivery. The
[executed Button walkthrough](crates/ui/api/docs/button_events.rs) covers activation
and a queued event after removal; [event architecture](docs/contracts/CLASS_EVENT_STREAMS.md)
defines delivery and ownership. The README blocks also execute against real providers.

Nested components own their inbox inside the widget and implement
`IWidgetDispatcher::dispatch_next`; the runtime settles descendants before
application messages and later raw input. Deferred application publication uses
`MessageProcessor::apply_pending_changes` with an explicit `ApplyOutcome`; it runs at the guarded application boundary.
Setters retain invalidation; custom widget updates report `GeometryChange::Changed`
when measurement or placement inputs change. Resize invalidation is independent.

External shared state, hidden/skipped updates and observational painting remain
explicit widget/app responsibilities. Read [COM_AUTHORS.md](docs/contracts/COM_AUTHORS.md)
for the complete requirements and examples. Local custom leaves implement
`LeafContent`; GPU scenes expose concrete interfaces such as `IMeshScene: IRenderItem`.
Backend-owned Rust GPU types stay inside their renderer plugin.

The [documentation index](docs/README.md) links authoring, assets, deployment and
implementation references. [Examples](docs/guides/EXAMPLES.md) maps each feature to working
source. [FEATURES.md](docs/guides/FEATURES.md) defines current feature ownership, while
[COM_DESIGN.md](docs/architecture/COM_DESIGN.md) defines the accepted component architecture.

## License

Project-authored code uses the [BSD 3-Clause License](LICENSE). Imported neocogi-com and
portions derived from rxi/microui retain their MIT terms. Bundled assets retain the
licenses recorded in [asset attribution](docs/project/ASSETS.md).
