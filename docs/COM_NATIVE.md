# Native COM packages and browser assembly

Build every native binary independently and stage a complete executable-relative package:

```sh
cargo run -p microui-redux-tools --bin com-build -- --demo full --backend glow
./target/com-native/debug/glow/prebuilt/full/microui-redux-demo-full
```

`--demo` selects `full`, `simple`, `calculator`, `backend-frame-cube`, or `smoke`
(the default). `--backend` selects exactly one of `glow`, `vulkan`, or `wgpu`.
`--release` selects ordinary release builds. The driver honors `CARGO_TARGET_DIR`,
the selected Cargo toolchain, and `CARGO_NET_OFFLINE`; it does not run Cargo from a
build script. Native demo feature flags are markers: implementation selection
belongs to the separately built platform package.

Packages live in `target/com-native/{debug,release,min-size}/{backend}/{assets}/{demo}`.
They contain the selected executable, one nc-com runtime library, UI and platform
plugins, and a `plugins.txt` manifest. The full demo additionally ships the PNG-only
asset plugin for its compressed photograph. Its font atlas and four themes are
compiled by Cargo's host build; target-side font construction and JSON loading are
absent. Do not build the workspace with all features to measure deployment size:
that unifies optional implementation features that these independent builds avoid.

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
cargo +nightly run -p microui-redux-tools --bin com-build -- --demo full --backend glow --min-size
```

This invokes the existing `build-min-size` alias for runtime, plugins and executable
using its x86_64 Linux custom target and rebuilt standard library. Install nightly's
`rust-src` component first. Count every shipped library and asset along with the
executable; moving implementation code into a plugin does not eliminate its size.
Final measurements and remaining migration cleanup are tracked in
[COM_MIGRATION.md](COM_MIGRATION.md).

## Native verification

A finite ordinary demo run renders the requested number of successful frames and
then releases its application session and host:

```sh
cd /tmp
MICROUI_DEMO_FRAMES=8 /absolute/path/to/target/com-native/debug/glow/prebuilt/full/microui-redux-demo-full
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
outside the repository. They exercise all four actual themes, the visible compressed
image, custom geometry/mesh rendering, spacing changes, combo selection, Unicode log
submission and retained-handle closure. Test executables are staged separately under
`target/com-tests` so they cannot inflate the installed application size. Set
`MICROUI_TEST_BUILD_ONLY=1` to stage tests for a separate display-enabled invocation.

The smaller boundary smoke remains available:

```sh
cargo run -p microui-redux-tools --bin com-build -- --demo smoke --backend glow
./target/com-native/debug/glow/prebuilt/smoke/microui-demo-com --smoke
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

## Optional construction workflows

The smoke executable can exercise runtime asset construction separately from the
ordinary demos' compiled assets:

```sh
cargo run -p microui-redux-tools --bin com-build -- --assets builder
./target/com-native/debug/glow/builder/smoke/microui-demo-com --smoke --rebuild-assets
cargo run -p microui-redux-tools --bin com-build -- --assets theme-json
./target/com-native/debug/glow/theme-json/smoke/microui-demo-com --smoke --theme themes/windows-95/theme.json
```

JSON mode stages recipe-relative font/artwork sources. Relative theme paths resolve
against the executable directory. Other demos deliberately reject construction-mode
packaging rather than shipping unused loaders. See [COM_ASSETS.md](COM_ASSETS.md) for
immutable catalogs, bounded transfers, upload atomicity and decoder ownership.

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
