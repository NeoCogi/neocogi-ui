# Native COM development package

The incremental migration is tracked in [COM_MIGRATION.md](COM_MIGRATION.md). The accepted final design is [COM_DESIGN.md](COM_DESIGN.md); this document describes the currently executable slice, not completion of that design.

Build and stage the runtime, UI assembly, SDL/Glow assembly and small host:

```sh
cargo run -p microui-redux-tools --bin com-build
./target/com-native/debug/microui-demo-com
```

For release artifacts, pass `-- --release` to the driver and use `target/com-native/release`. The driver uses the selected Cargo toolchain, builds each binary separately, and honors `CARGO_TARGET_DIR`. Its builds are offline, so dependencies must already be cached. It does not invoke Cargo from a build script.

The host reads the staged `plugins.txt` in order, loads both trusted libraries from beside its executable, and activates their concrete bootstrap classes. `microui-redux` contains only static typed clients and contracts. `microui-ui` contains retained implementation code. The small crates under `plugins/` own descriptors and explicitly selected exports; an ordinary implementation-library dependency never exports a plugin entrypoint accidentally.

On Linux the executable uses an `$ORIGIN` runtime search path. It can start outside the source tree without `LD_LIBRARY_PATH`. The driver includes native filename/search-path handling for macOS and Windows, but those packages have not yet passed the platform deployment gates. Native SDL/OpenGL system dependencies still need to be installed; this is not a system-library bundler.

A deterministic input/render/teardown smoke run is available:

```sh
cd /tmp
SDL_VIDEODRIVER=offscreen LIBGL_ALWAYS_SOFTWARE=1 \
  /absolute/path/to/target/com-native/debug/microui-demo-com --smoke
```

This uses a real SDL GL context and Mesa rendering. It verifies separate-binary activation, initial rendering, keyboard focus/activation into an application-owned sink, label invalidation, changed window allocation, explicit update-before-render, and logical closure. It does not claim screenshot comparison, hardware-driver coverage, or completion of every input family.

The current slice has deliberate transitions with assigned removal milestones:

- The retained engine is moved intact to `microui-ui`. Existing demos and behavior tests temporarily use that implementation crate while their public integration is converted. Item 17 removes obsolete integration APIs and documentation.
- A private leaf bridge reuses existing Node measurement caches, window layout, input routing, clipping, framing and display-list execution. Actual button label/event state lives in one COM allocation. The bridge and its weak invalidation hook are removed by the common-node ownership work in item 9.
- The initial manager mounts button-like leaves exposing widget, leaf-layout, activation and lifetime capabilities. The manager now exposes child windows, hidden modal dialogs, typed popups, visibility, layering, title/options and child clipping. Arbitrary leaves and containers are items 9–11.
- Text measurement and painter text commands currently use the body-font button appearance needed by the slice. Full resource/font/style and primitive capabilities are completed with the remaining widget/render interfaces. Finite and unbounded constraints are already explicit copied records.
- UI and platform assemblies both use the same generated demo atlas. They do not yet negotiate arbitrary asset bundles. Item 15 replaces this packaging input with the shared immutable resource-bundle contract. The public application does not depend on the demo asset builder.
- The renderer submits final atlas triangles in one bulk read using reusable storage. External textures, custom render items, atlas replacement, detailed backend diagnostics and other GPU backends are item 14; unsupported operations currently fail explicitly.
- The slice input record covers primary pointer events, Tab, primary keyboard activation and resize. Complete key/modifier/text/IME/scroll/capture behavior is part of items 9 and 16.
- The application explicitly dispatches its one typed subscription after each update. Registration-order sessions, cascade limits and all event families are item 12. The producer never owns the sink or lease.

Painter and render-stream objects own storage. Their monotonically increasing invocation identities expire on every return path and are never reused; retaining an interface is safe but retaining an old token does not extend authority. Tests also exercise reentrant update, mutation during observation, failed paint before backend submission, malformed input, foreign ownership, duplicate mounts and retained handles after closure. See [COM_AUTHORS.md](COM_AUTHORS.md) for the complete author obligations.
