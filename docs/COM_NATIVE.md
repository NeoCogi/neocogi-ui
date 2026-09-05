# Native COM development package

The incremental migration is tracked in [COM_MIGRATION.md](COM_MIGRATION.md). The accepted final design is [COM_DESIGN.md](COM_DESIGN.md); this document describes the currently executable slice, not completion of that design.

Build and stage the runtime, UI assembly, SDL/GPU assembly and small host:

```sh
cargo run -p microui-redux-tools --bin com-build
./target/com-native/debug/glow/prebuilt/microui-demo-com
```

For release artifacts, pass `-- --release` to the driver and use `target/com-native/release/glow/prebuilt`. The driver uses the selected Cargo toolchain, builds each binary separately, and honors `CARGO_TARGET_DIR`. Its builds are offline, so dependencies must already be cached. It does not invoke Cargo from a build script.

Select the native backend explicitly; each package includes only the selected implementation and the demo's matching native mesh renderer:

```sh
cargo run -p microui-redux-tools --bin com-build -- --backend vulkan
cargo run -p microui-redux-tools --bin com-build -- --backend wgpu
```

Packages are staged in `target/com-native/{debug,release}/{glow,vulkan,wgpu}/{prebuilt,builder,theme-json}`. Each contains the same host, runtime, UI plugin and `microui-plugin-platform`, plus an explicit `plugins.txt`. A native display is required for the Vulkan/WGPU smoke tests. The ordinary Cargo `glow`/`vulkan`/`wgpu` feature priority matches the existing demo renderer when features are unified; the packaging driver selects exactly one with default features disabled.

The host reads the staged `plugins.txt` in order, loads both trusted libraries from beside its executable, and activates their concrete bootstrap classes. `microui-redux` contains only static typed clients and contracts. `microui-ui` contains retained implementation code. The small crates under `plugins/` own descriptors and explicitly selected exports; an ordinary implementation-library dependency never exports a plugin entrypoint accidentally.

On Linux the executable uses an `$ORIGIN` runtime search path. It can start outside the source tree without `LD_LIBRARY_PATH`. The driver includes native filename/search-path handling for macOS and Windows, but those packages have not yet passed the platform deployment gates. Native SDL 2.26+ and the selected graphics API system dependencies still need to be installed; this is not a system-library bundler.

A deterministic input/render/teardown smoke run is available:

```sh
cd /tmp
SDL_VIDEODRIVER=offscreen LIBGL_ALWAYS_SOFTWARE=1 \
  /absolute/path/to/target/com-native/debug/glow/prebuilt/microui-demo-com --smoke
```

This uses a real SDL GL context and Mesa rendering. It verifies separate-binary activation, real GPU painting, keyboard activation into application-owned handlers, inherited/composed controls, compact menus, native-path file-picker acceptance, invalidation, resize, explicit update-before-render and logical closure. Glow additionally has a GPU pixel-readback test for image/mesh/overlay order. Linux Vulkan and WGPU packages have passed the same smoke on the native Intel/Mesa display. The complete native input vocabulary and IME placement use the same COM input destination; browser and cross-platform deployment gates remain item 16.

The current slice has deliberate transitions with assigned removal milestones:

- The retained engine is moved intact to `microui-ui`. Existing demos and behavior tests temporarily use that implementation crate while their public integration is converted. Item 17 removes obsolete integration APIs and documentation.
- COM leaf nodes now own their widget/layout capabilities directly. The extra Bridge allocation, weak invalidation hook and duplicate mount-list widget owners are removed. Existing Node caches, window layout, routing, clipping, framing and display-list algorithms remain authoritative. Legacy demos still use local Node variants and public semantic constructors during migration. Item 17 removes them after the remaining container/composition/demo ports. Built-in COM classes own their concrete semantic values inline; temporary common-option copies inside the old algorithm structs must also be removed in that cleanup.
- The manager mounts arbitrary leaves exposing IWidget and ILeafLayout. Lifetime, common properties, input/update, invalidation and paint belong to IWidget; the separate activation/lifetime interfaces have been removed. The manager now exposes child windows, hidden modal dialogs, typed popups, visibility, layering, title/options and child clipping. The basic built-in leaves now have concrete editing capabilities. Linear/Grid and the real ScrollArea, Disclosure and TextArea compositions expose transactional topology and scoped layout services.
- Built-in buttons, checkboxes, textboxes, text blocks, sliders, numbers, color swatches, list entries and scrollbars reuse their complete semantic, measurement and theme-paint algorithms. Font/icon selection and button center-fill are concrete optional capabilities. Paint receives copied interaction state and records atlas geometry in bulk. The simple external ITextMetrics/IPainter text conveniences use body-font button appearance; independent widgets obtain full immutable metadata through IWidgetHost::assets. Finite and unbounded constraints are explicit copied records.
- The host supplies one immutable COM bundle to UI and native platform construction. Neither plugin embeds demo assets. Compiled tables require only facade/contracts at runtime; optional font/PNG/JSON construction lives in the asset plugin. See [COM_ASSETS.md](COM_ASSETS.md) for limits, catalog identity, atomic replacement and author obligations.
- The renderer copies one complete vertex buffer and one ordered batch buffer into reusable owned storage, then authenticates image/scene capabilities before opening its native frame. Atlas triangles, clipped image quads and real native 3D commands preserve source order. Texture/scene resources keep their native window/device alive; diagnostics preserve backend failure text. Runtime installation validates compatible resources and uploads before publishing the UI revision; each frame explicitly identifies its bundle. Temporary legacy-ID and custom-callback hooks disappear with the old direct Context API in item 17.
- The runtime accepts complete typed pointer/button/wheel/key/modifier/text/cancellation events through input(), then update(size) applies queued input and synchronizes layout. Widget updates receive copied router snapshots and content-local events. The native platform borrows the runtime’s inherited IInputSink during polling and forwards the complete SDL input vocabulary. After update, text_input_area()/text_input() synchronize committed IME placement without restarting active composition. See the platform author obligations.
- The host owns an `EventSession` and concrete handler. `update_with_events` dispatches between input phases, releases the context for ordinary callback edits, and commits their geometry before routing later input. All current leaf, window and popup event families use the same queue/lease transport. Combo/menu actions and standard native-path picker completion use the same transport; internal component callbacks share the finite boundary budget. The producer never owns the sink or lease.

Painter and render-stream objects own storage. Their monotonically increasing invocation identities expire on every return path and are never reused; retaining an interface is safe but retaining an old token does not extend authority. Tests also exercise reentrant update, mutation during observation, failed paint before backend submission, malformed input, foreign ownership, duplicate mounts and retained handles after closure. See [COM_AUTHORS.md](COM_AUTHORS.md) for the complete author obligations.

The native resource/ownership test is explicit because it creates real OS windows:

```sh
SDL_VIDEODRIVER=offscreen LIBGL_ALWAYS_SOFTWARE=1 \
  cargo test -p microui-plugin-platform --lib -- --ignored
cargo test -p microui-plugin-platform --no-default-features --features vulkan --lib -- --ignored
cargo test -p microui-plugin-platform --no-default-features --features wgpu --lib -- --ignored
```

`demos/com-slice/src/gpu.rs` demonstrates an independently implemented host widget with inline `WidgetState`, a typed immutable image and an inherited mesh render item. Its application logic uses only the facade and contracts. The demo's native shaders/pipelines remain co-packaged with the selected renderer. Factory `host()` supplies widget authority directly, without constructing a dummy built-in control.

On the tested Mesa 25.2.8 installation, combining offscreen EGL, accessible hardware
devices and `LIBGL_ALWAYS_SOFTWARE=1` crashed in `driCreateNewScreen3` during SDL window
creation, before renderer construction. Normal hardware-display tests pass; the
headless software tests pass with hardware devices unavailable. Use the ordinary
native-display command on a desktop instead of forcing that EGL combination.

Optional asset packaging uses an explicit mode:

```sh
cargo run -p microui-redux-tools --bin com-build -- --assets builder
cargo run -p microui-redux-tools --bin com-build -- --assets theme-json
./target/com-native/debug/glow/builder/microui-demo-com --smoke --rebuild-assets
./target/com-native/debug/glow/theme-json/microui-demo-com --smoke --theme themes/windows-95/theme.json
```

The default mode is `prebuilt`. Optional modes add one asset library; JSON mode also
stages the bundled theme/font sources. Relative theme paths resolve against the
executable directory. Raw compiled atlas pixels occupy 512 KiB for the demo, before
metadata; this explicit storage cost is included in the final installed-size budget.


The platform assembly also builds with `--no-default-features --features webgl`
for `wasm32-unknown-unknown`. It statically registers the same UI/platform COM
contracts, with one linked nc-com runtime. DOM listeners, native composition,
canvas ownership and WebGL stay in this assembly; bulk frame validation and
texture/mesh ownership are shared with all native backends. Each platform window
creates an independent canvas appended to the document body; applications supply
page CSS. Releasing the public window removes the canvas/listeners while retained
GPU resources keep their WebGL context alive until final release.

Run `scripts/test-browser-input.sh` after installing the matching wasm-bindgen CLI
(the script accepts `MICROUI_WASM_BINDGEN` and `CARGO_TARGET_DIR`). It checks actual
DOM composition/paste, chorded pointer input, wheel scaling and teardown, then
registers both static descriptors, edits a real COM textbox, positions native IME
and renders WebGL frames before/after a CSS resize. Headless Chromium uses its
software WebGL implementation so this is a real GPU-API execution gate rather
than a compile-only check. The main demo host migration remains in progress.

A lost WebGL context reports a renderer error before upload or submission. The
application must recreate the platform window and its GPU resources; automatic
context restoration and reusing old device capabilities are unsupported. This
follows the ordinary renderer ownership contract instead of hiding a second,
partially restored native device under existing texture identities.
