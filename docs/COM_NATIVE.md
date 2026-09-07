# Native COM packages and browser assembly

Install the command from this checkout and build a runnable assembly:

```sh
cargo install --path tools/cargo-nc-com
cargo nc-com build --assembly full
cargo nc-com run --assembly full
```

`cargo nc-com` builds the runtime, selected plugins, and application in order,
then stages the executable, native libraries, plugin manifest, and assets. It
reads the assembly definitions in `[workspace.metadata.nc-com]` in the root
[Cargo.toml](../Cargo.toml). Users of a published tool can instead install with
`cargo install cargo-nc-com --locked`; publishing the tool and its component
prerequisites is a separate release operation.

`--assembly` selects `full` (the default), `simple`, `calculator`,
`backend-frame-cube`, or `smoke`. Glow is the default backend. Renderer variants
select matching application and platform features explicitly:

```sh
cargo nc-com run --assembly full --variant vulkan --release
cargo nc-com run --assembly calculator --variant wgpu
cargo nc-com run --assembly smoke --variant extension -- --smoke
```

These commands build selected packages rather than the entire workspace, so a
Glow build does not select the Vulkan/WGPU renderer packages. `--workspace` on
ordinary Cargo still selects every renderer root, including their mandatory
Ash/WGPU dependencies; `--no-default-features` does not deselect those packages.

For contributors, the equivalent command without installation is
`cargo run -p cargo-nc-com -- build --assembly full`. The installed command needs
no workspace alias. Run `cargo nc-com plan --assembly full --variant vulkan` to
review the selected typed configuration without compiling or inspecting plugins.
The [tool reference](../tools/cargo-nc-com/README.md) describes package metadata,
standalone `--config` files, per-component features, registry version requirements,
resource paths, and lockfile behavior.

## Profiles and output

```sh
cargo nc-com build --assembly full                       # Debug
cargo nc-com build --assembly full --release
cargo nc-com build --assembly full --toolchain nightly --release
rustup component add rust-src --toolchain nightly
cargo nc-com build --assembly full --profile min-size
```

The `min-size` preset selects nightly automatically and includes its x86_64 Linux
GNU target JSON, size-oriented compiler settings, and rebuilt standard library.
Consumers do not need our `build-min-size` alias or a custom profile definition.
Use `--toolchain nightly-YYYY-MM-DD` to select an installed dated toolchain. Custom
nightly targets can change with the compiler, so pin and test the toolchain used
for a release. Ordinary custom profiles remain defined by the application's
workspace. Compatible plugins and the application build in one Cargo feature
graph after preparing the runtime. Generated pure cdylib roots re-export Rust
implementations, allowing the shared libraries to receive the profile's LTO.
Existing cdylib-only components build directly. `isolated = true` on a plugin
retains an independent feature graph where required. Identical settings share
Cargo's cache; genuinely different graphs can still require different std artifacts.

The command prints the completed output directory. Its layout is:

```text
TARGET/nc-com/APPLICATION/ASSEMBLY/VARIANTS/dist/COMPILER/TARGET/PROFILE/
```

`TARGET` at the start is Cargo's configured build directory, or `--target-dir`.
`COMPILER` identifies the selected compiler; the second `TARGET` identifies the
compilation platform. `dev` outputs use the `debug` directory name. Keep every
staged runtime/plugin library and resource with the executable. `run` launches
the staged binary and forwards arguments after `--`, preserving its exit code.

The command discovers artifacts from Cargo's JSON output and snapshots the runtime
and explicitly isolated plugins before later builds can replace their files.
Grouped plugins are collected from the application's Cargo invocation. The phase
summary distinguishes compiled targets from cache hits. Runtime linker aliases handle hashed registry
artifacts; originals are retained for platform loader identities. A child process
inspects trusted plugin descriptors and uses nc-com's planner to reject missing
dependencies, duplicate identities, and cycles. It exits before staging directories
are renamed, releasing Windows library handles. Failed compilation, inspection,
or resource copying preserves the previous completed assembly.

On Linux `$ORIGIN` locates the adjacent runtime without `LD_LIBRARY_PATH`. macOS
uses `@loader_path`; Windows uses adjacent DLLs and the appropriate import
library. The tool preserves environment rustflags and adds these loader settings.
SDL 2.26+ and the selected graphics API's system libraries remain system
prerequisites; they are not automatically redistributed. Foreign targets are
rejected because native descriptor inspection cannot execute their libraries.
Linux is locally verified; CI exercises macOS and Windows independently.

## Resources and component boundaries

The default assemblies include the asset plugin, `assets`, and `themes`.
Full-demo packaging also places its photograph at `assets/FACEPALM.png`. Inputs
resolve relative to the owning manifest/configuration file; staged paths resolve
relative to the application executable. Theme parsing, image decoding, and font
rasterization run in the asset plugin. Call
`microui_redux_demo_host::initialize()` before `load_bundle()`.

```sh
cargo nc-com run --assembly full --variant vulkan --variant embedded
cargo nc-com run --assembly smoke -- --smoke --theme themes/windows-95/theme.json
```

Place `--variant embedded` after the renderer variant. It adds the application's
`embed-assets` feature and removes external resources. The full demo retains its
PNG asset plugin; other embedded demos omit the asset plugin. The smoke assembly's
`extension` variant adds the independent example plugin. Browser `webgl` selects
embedded resources and retains its separate static registration workflow.

Native demos still declare shared runtime/plugin crates as ordinary dependencies.
Their native code uses the runtime C ABI and loads plugin files; it does not call
the implementation crates through Rust. An ordinary demo `cargo build` can emit
those libraries but still needs a runtime linker search path and does not produce
a runnable assembly. The new command supplies `NC_COM_RUNTIME_LINK_DIR`, builds
explicit component features, and stages the descriptor-planned manifest.
`microui-redux` remains the static typed facade without native UI/renderer
implementation dependencies. See the [backend source map](../crates/microui-ui/docs/BACKENDS.md#backend-sources).

Package metadata is our tool's configuration, not a Cargo execution hook. Ordinary
`cargo build` and `cargo install` do not automatically run the assembly command.
The generic tool supports versioned registry components using generated Cargo
projects, but this checkout's pre-alpha plugins are not thereby published.
Generated component and assembly lockfiles live under the target directory and
are initially seeded from the application's lockfile. Retain their `sources`
directories when using `--locked`, since `cargo clean` removes them. Changes to
the application lock require an unlocked build to refresh the generated locks.

## Existing regression driver

`cargo com-build` remains available for the existing regression scripts and their
established `target/com-native/PROFILE/BACKEND/ASSETS/DEMO` paths. Its options remain
`--demo`, `--backend`, `--assets files|embedded`, `--release`, and `--min-size`:

```sh
cargo com-build --demo full --backend glow
./target/com-native/debug/glow/files/full/microui-redux-demo-full
```

That workspace-local driver requires `cargo +nightly com-build ... --min-size`
for its older min-size alias. New applications should use `cargo nc-com` and
configuration instead of adding more demo-specific branches to that driver.
The regression examples below retain the driver's established paths.
COM_PERFORMANCE.md records whole-deployment measurements;
[COM_MIGRATION.md](COM_MIGRATION.md) records the component migration history.

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
