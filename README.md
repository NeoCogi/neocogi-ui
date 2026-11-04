# NeoCogi UI

A standalone Rust workspace for NeoCogi's retained UI library, built-in widgets,
document loading, composition, resources, themes, and UI examples. Components use
typed COM interfaces with independently deployable UI runtime, node, renderer,
GFX, and skin providers.

[NeoCogi Platform](https://github.com/NeoCogi/neocogi-platform) supplies native/browser
platform services, generic graphics resources, shader compilation and renderer
backends through the published crates.io release `0.1.0-alpha.1`.

The coordinated UI release is `0.1.0-alpha.1`. See the
[release guide](docs/project/RELEASING.md) for archive qualification and publishing.

See [UI and COM history](docs/project/HISTORY.md) for the matching dependency
revisions throughout the commit history.

## Workspace layout

| Directory | Purpose |
| --- | --- |
| `crates/build` | Packaged UI/GFX/Skin IDL sources and build helpers |
| `crates/ui/api` | Typed application clients and shared UI bindings |
| `crates/ui/runtime` | Retained forest, windows, surfaces, menus and dispatch |
| `crates/ui/nodes` | Built-in widgets, containers and file dialogs |
| `crates/ui/support` | Presentation algorithms and drawing helpers |
| `crates/ui/document`, `crates/ui/design` | Runtime document loading, serialization and extension manifests |
| `crates/gfx`, `crates/skin`, `crates/appearance` | Images, fonts, resource bundles, themes and appearance |
| `examples` | Full/basic/smoke applications, runtime loading and an independent UI extension |
| `tools` | Asset export |
| `tests/platform` | UI/GFX integration against platform/backend contracts |
| `docs` | UI contracts, architecture, deployment and attribution |

The [COM SDK, host implementation, IDL compiler and inspector](https://github.com/NeoCogi/neocogi-com)
are maintained together. All COM dependencies use the exact published crates.io
release `0.1.0-alpha.1`; see the
[dependency guide](docs/guides/COM_DEPENDENCY.md) for updates and validation.

## Build and run

The development workspace selects `nightly-2026-09-29` with `rust-src` for
Cargo artifact dependencies and the min-size profile. Native applications need
SDL2 and the libraries for their selected graphics backend; see
[backend prerequisites](crates/ui/runtime/docs/BACKENDS.md).
The checked-in lockfile fixes the standalone dependency graph.

All Platform and COM packages use exact `=0.1.0-alpha.1` registry requirements.
Cargo downloads their checked packages from crates.io; builds require no GitHub
credentials or companion checkout.

```sh
cargo fetch --locked
cargo run --locked -p neocogi-demo-basic --bin neocogi-demo-calculator
cargo run --locked -p neocogi-demo-full
cargo run --locked -p neocogi-demo-full --no-default-features --features monolith,glow,embed-assets
cargo run --locked -p neocogi-runtime-ui-example
```

Native defaults select dynamic providers and Glow. Build Vulkan or WGPU with
`--no-default-features --features dynamic,vulkan` or `dynamic,wgpu`.
Resources and providers are staged beside the executable by Cargo. The
[native guide](docs/guides/COM_NATIVE.md) covers deployment and debugging;
[examples](docs/guides/EXAMPLES.md) maps features to source.

For the browser, run `scripts/web-demo.sh serve` and follow the local URL.
The [WebGL guide](docs/guides/WEB.md) covers input and browser validation.

## Validation

```sh
python3 scripts/package_inputs.py --check
python3 -m unittest discover -s scripts -p 'test_*.py'
cargo test --locked -p neocogi-ui -p neocogi-ui-document -p neocogi-ui-runtime -p neocogi-ui-nodes -p neocogi-ui-support -p neocogi-ui-design --features neocogi-com/runtime --all-targets
```

[UI testing](crates/ui/runtime/docs/TESTING.md) and
[CI](.github/workflows/rust.yml) describe independent consumers, providers,
native rendering, and browser checks.

## Typed application code

Use the exact matching registry versions when integrating the alpha release:

```toml
[dependencies]
neocogi-ui = { version = "=0.1.0-alpha.1" }
neocogi-com = { version = "=0.1.0-alpha.1" }
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
`LeafContent`; custom graphics use restricted GPU resource factories and scoped
render views. Clients define their own vertex layouts, shaders and uniform bytes.
[GPU rendering contracts](https://github.com/NeoCogi/neocogi-platform/blob/master/docs/contracts/GPU_RENDERING.md) describe lifetime,
validation and viewport authority. Native GPU types stay inside their renderer plugin.

The [documentation index](docs/README.md) links authoring, assets, deployment and
implementation references. [Examples](docs/guides/EXAMPLES.md) maps each feature to working
source. [FEATURES.md](docs/guides/FEATURES.md) defines current feature ownership, while
[COM_DESIGN.md](docs/architecture/COM_DESIGN.md) defines the accepted component architecture.

## License

Project-authored code uses the [BSD 3-Clause License](LICENSE). The external
[neocogi-com SDK](https://github.com/NeoCogi/neocogi-com/blob/92fa66ebc70888f71f2d07f2bbc14335a2ffd4e6/LICENSE-MIT)
and portions derived from rxi/microui use MIT terms. The retained GFX allocation
probe and document test schema keep their [COM fixture MIT notice](LICENSES/COM-fixtures-MIT.txt). Bundled assets retain the
licenses recorded in [asset attribution](docs/project/ASSETS.md).
