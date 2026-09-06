# Native COM packages and browser assembly

Build every native binary independently and stage a complete executable-relative package:

```sh
cargo com-build --demo full --backend glow
./target/com-native/debug/glow/files/full/microui-redux-demo-full
```

`--demo` selects `full`, `simple`, `calculator`, `backend-frame-cube`, or `smoke`
(the default). `--backend` selects exactly one of `glow`, `vulkan`, or `wgpu`.
`--release` selects ordinary release builds. The driver honors `CARGO_TARGET_DIR`,
the selected Cargo toolchain, and `CARGO_NET_OFFLINE`; it does not run Cargo from a
build script. Native demo feature flags are markers: implementation selection
belongs to the separately built platform package.

The [platform package](../plugins/platform) selects its backend from the renderer
crates under `plugins/`; see the [backend source map](../crates/microui-ui/docs/BACKENDS.md#backend-sources).
The selected renderer and shared support code are compiled into the platform plugin's
library (`libmicroui_plugin_platform.so` on Linux).

A direct `cargo build -p microui-redux-demo-full --features vulkan --release`
builds only the host executable. It does not build the shared runtime, configure
its link search path or stage implementation plugins. `unable to find library
-lnc_com_runtime` means that native link dependency is missing. Use:

```sh
cargo com-build --demo full --backend vulkan --release
./target/com-native/release/vulkan/files/full/microui-redux-demo-full
```

`cargo com-build` is a workspace alias for the driver invocation above. For a
custom build system, build `nc-com-runtime` first, set `NC_COM_RUNTIME_LINK_DIR`
to the directory containing its shared/import library when linking the host,
and configure the executable-relative runtime lookup. Independently build the
selected UI/platform/asset plugins and stage them with their descriptor-planned
manifest. Supplying a linker search path alone does not assemble a runnable demo.

Packages live in `target/com-native/{debug,release,min-size}/{backend}/{assets}/{demo}`.
They contain the selected executable, one nc-com runtime library, UI and platform
plugins, and a `plugins.txt` manifest. The default `files` package also contains the
asset plugin with atlas/theme construction, adjacent font/icon inputs and theme
files. The full demo's photograph and mesh are external files too. All paths resolve
against the executable directory; the host contains no embedded demo assets.
Decoding, font rasterization and theme parsing run in the asset plugin. Call
`microui_redux_demo_host::initialize()` before `load_bundle()` so its classes are registered.

`--assets embedded` explicitly selects the demo's `embed-assets` Cargo feature.
That package compiles atlases/themes on Cargo's host and embeds the photo/mesh;
only the full demo needs the PNG asset plugin. Browser `webgl` selects this feature
because this browser deployment includes its resources in the wasm module. Do not build the workspace
with all features to measure deployment size:
that unifies optional implementation features that these independent builds avoid.
Older `prebuilt` output directories are obsolete and are not refreshed by these
commands; the default executable is under `files`, as shown above.

The driver inspects the actual trusted plugin descriptors and uses nc-com's existing
planner to reject duplicate identities, missing dependencies and cycles. It writes
filenames in the resulting dependency-first order. The host loads that exact list
relative to its executable. No source-tree path, current working directory, directory
scan, hot reload or dynamic unload participates in activation. `microui-redux` is the
static typed facade; it has no native UI or renderer implementation dependency.

On Linux `$ORIGIN` locates the adjacent runtime without `LD_LIBRARY_PATH`. macOS
uses `@executable_path`; Windows uses normal adjacent-DLL lookup. The driver handles
native filenames and linking; SDL 2.26+ and the selected graphics API's system
libraries remain installed system dependencies. Packaging does not redistribute
those system libraries. CI includes macOS and Windows package builds, while actual
GPU evidence collected in this workspace is Linux and browser evidence.

The workspace's specialized Linux size profile applies to every shipped binary:

```sh
cargo +nightly com-build --demo full --backend vulkan --min-size
./target/com-native/min-size/vulkan/files/full/microui-redux-demo-full
```

This invokes the existing `build-min-size` alias for runtime, plugins and executable
using its x86_64 Linux custom target and rebuilt standard library. Install nightly's
`rust-src` component first. Count every shipped library and asset along with the
executable; moving implementation code into a plugin does not eliminate its size.
The `min-size` profile inherits release. Use the same backend and nightly compiler
on both sides of a size comparison. COM_PERFORMANCE.md records
the complete deployment, loaded memory, startup and CPU/frame measurements;
[COM_MIGRATION.md](COM_MIGRATION.md) records the completed migration sequence.

## Native verification

A finite ordinary demo run renders the requested number of successful frames and
then releases its application session and host:

```sh
cd /tmp
MICROUI_DEMO_FRAMES=8 /absolute/path/to/target/com-native/debug/glow/files/full/microui-redux-demo-full
```

Malformed or zero limits fail at startup. Omission runs until OS close or the
application's `AppState::frame` returns false. Frame errors propagate to the native
entry point. The host closes the UI logically before releasing its capabilities;
retained widget handles then report `UI_E_CLOSED`.

```sh
scripts/test-demo.sh glow
scripts/test-demo.sh vulkan
scripts/test-demo.sh wgpu
```

These tests build plugins independently, stage a companion test executable, and run
outside the repository. They exercise all four actual themes, image command submission, custom
geometry/mesh rendering, spacing changes, combo selection, Unicode log
submission and retained-handle closure. Win 3.11 is additionally selected through
the real nested menu and event callback. These application tests do not read back
every backend's pixels; actual-window image checks supplement them. Test executables are staged separately under
`target/com-tests` so they cannot inflate the installed application size. Set
`MICROUI_TEST_BUILD_ONLY=1` to stage tests for a separate display-enabled invocation.

The smaller boundary smoke remains available:

```sh
cargo run -p microui-redux-tools --bin com-build -- --demo smoke --backend glow
./target/com-native/debug/glow/files/smoke/microui-demo-com --smoke
```

It covers typed input/events, inherited and composed controls, compact menus,
native-path file-picker acceptance, invalidation, resize and explicit teardown.
Platform resource tests create real windows and remain explicitly opt-in:

```sh
cargo test -p microui-plugin-platform --lib --no-default-features --features glow -- --include-ignored
```

On a desktop use its ordinary display. A previous Mesa 25.2.8 offscreen EGL setup
crashed inside `driCreateNewScreen3` when accessible hardware was combined with
`LIBGL_ALWAYS_SOFTWARE=1`, before renderer construction. The ordinary native display
passed; CI uses Xvfb with its available software GPU drivers.

The optional independent extension adds a real derived counter and a custom inset
container, with no dependency on the UI or renderer implementation:

```sh
scripts/test-extension.sh glow
scripts/test-extension.sh vulkan
scripts/test-extension.sh wgpu
```

The driver separately builds the extension, host, UI and platform, and runs from
`/tmp`. It verifies common interface identity, shared activation delivery, callback
measurement, unchanged measurement caches, setter/constraint/resize invalidation,
exclusive attachment, topology changes and retained-handle closure. Ordinary demo
packages omit this assembly. See [its source and contracts](../plugins/extension-example).

## Optional construction workflows

The default file package exposes both construction interfaces. The smoke executable
can derive a larger atlas or load a JSON theme through the same asset plugin:

```sh
cargo com-build --assets files
./target/com-native/debug/glow/files/smoke/microui-demo-com --smoke --rebuild-assets
./target/com-native/debug/glow/files/smoke/microui-demo-com --smoke --theme themes/windows-95/theme.json
```

Relative theme paths resolve against the executable directory. Use
`cargo com-build --demo full --assets embedded` for an explicit embedded deployment;
its asset plugin exposes PNG decoding and live style edits without runtime atlas
building or JSON loading. See
[COM_ASSETS.md](COM_ASSETS.md) for immutable catalogs, bounded transfers, upload
atomicity and decoder ownership.

## Browser

```sh
scripts/web-demo.sh serve
scripts/test-browser-input.sh input
scripts/test-browser-input.sh full
```

The browser statically registers the same UI, platform and optional PNG descriptors
with one nc-com runtime in the wasm module. DOM listeners, IME, canvas and WebGL2
ownership stay in the platform assembly. The host owns only application state and
one `requestAnimationFrame` scheduling adapter. Every platform window appends its
own canvas; page CSS controls its logical dimensions, and input/frame coordinates
use physical pixels. Do not create an extra placeholder canvas in the page.

`web-demo.sh` honors `CARGO_TARGET_DIR`, `MICROUI_WASM_BINDGEN` and
`MICROUI_WEB_PROFILE=debug|release` (release by default). It installs the matching
binding CLI when needed. A `?frames=8` query runs eight frames and stops, using the
same finite-run policy as native. Normal stop and asynchronous errors release the
RAF closure, application session, canvas, input textarea and listeners. GPU
capabilities retained outside that application still own their device until their
last release.

The input suite checks actual DOM composition/paste, pointer chords, wheel scaling,
static registration, inherited editing, IME placement and WebGL resize. The full
suite exercises theme/image/action behavior and verifies both normal and failed RAF
cleanup in Chromium. Set `CHROME_BIN` and `MICROUI_BROWSER_TEST_PORT` where needed.

A lost WebGL context reports a renderer error. Applications must recreate the
platform window and its GPU resources; old image/scene capabilities cannot migrate
to a replacement device. Browser file browsing is disabled because this demo's
native-path picker requires an OS filesystem. The same text editing, layout, menus,
style controls and custom rendering remain available.
