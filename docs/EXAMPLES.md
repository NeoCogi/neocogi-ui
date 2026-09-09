# Examples

| Example | Demonstrates |
| --- | --- |
| [simple](../demos/basic/src/bin/simple.rs) | A small facade-only application with one framed button. |
| [calculator](../demos/basic/src/bin/calculator.rs) | Concrete typed actions, text updates and weighted layout. |
| [backend-frame-cube](../demos/basic/src/bin/backend-frame-cube.rs) | A local COM leaf emitting solid triangles through the shared painter. |
| [full](../demos/demo-full/src/app.rs) | Menus, dialogs, editing, images, themes, containers and custom graphics. |
| [smoke](../demos/com-slice/src/main.rs) | Independent dynamic loading and explicit runtime/renderer boundary checks. |

After installing the [assembly tool](../tools/cargo-neocogi), build and run a complete native package:

```sh
cargo neocogi run --assembly calculator
```

Choose `full`, `simple`, `calculator`, `backend-frame-cube` or `smoke` with
`--assembly`. Glow is the default; `--variant vulkan` and `--variant wgpu` select
the other renderers. A direct `cargo run` on a native demo does not assemble
its required adjacent plugins. The [package guide](COM_NATIVE.md) is the canonical
reference for release/min-size builds, system dependencies, optional assets,
finite runs, independent test packaging and browser workflows.

The full demo preserves the fullscreen layer-zero X-Y grid beneath content-clipped
floating children. Left-drag the grid to orbit, use its wheel to zoom, or choose
Grid > Reset View. Grid > Minor Lines controls density. Closing the main Demo
Window hides it; Grid > Show Demo Window restores the same object. Minimize,
maximize and restore use the window manager's geometry policy.

File/View/Help menus demonstrate ordinary, disabled, checked and radio commands,
cascading theme/spacing menus and a separate Popup button. Menu shortcuts are
presentation hints; they do not automatically register keyboard accelerators.
View > Show Textures scrolls to the sampler's images. The sample includes native
RGBA images, a compressed photograph and a deterministic noise texture. Images and
the Suzanne mesh are external by default, resolved beside the native executable.
The explicit `embed-assets` feature includes their bytes; browser `webgl` enables it.

The palette editor exposes 18 RGBA roles and five layout metrics. Default Skin
permits palette edits; image themes make those color controls inert and retain
their authored artwork. Scalar metrics remain editable. All four themes are
available on native and browser targets. Native file packages use the asset plugin's
JSON loader; embedded builds compile them on Cargo's build host.
Accepted edits publish one immutable revision; animation frames do not repeatedly
reinstall unchanged styles. The log uses the actual TextBlock as its stored text,
and Enter/Submit share the concrete submission operation.

The [painter](../demos/demo-full/src/painter_demo.rs),
[falloff editor](../demos/demo-full/src/falloff.rs),
[grid](../demos/demo-full/src/grid.rs) and
[Suzanne viewport](../demos/demo-full/src/mesh.rs) implement `LeafContent` directly.
There is one COM widget allocation for each. Solid drawing uses the shared UI
painter; Suzanne edits one renderer-owned `IMeshScene: IRenderItem` object, whose
backend-specific GPU implementation stays in the platform plugin.

## Keyboard and text

SDL and browser adapters forward copied logical keys, modifiers and repeat state
separately from committed UTF-8. The shared UI provides:

- Tab/Shift+Tab navigation among eligible controls in the active window.
- Ctrl+F6/Ctrl+Shift+F6 navigation among ordinary visible windows, retaining each
  window's remembered focus; modal and menu scopes keep their own input authority.
- Enter/Space activation, Space checkbox toggling, arrow-key numeric adjustment,
  disclosure expansion and combo selection.
- An initial Escape press to dismiss an active popup. Repeats and releases continue
  through ordinary input routing after that popup is gone.
- F10 or an unchorded Alt tap to enter menus; arrows, Home/End, Enter/Space and Escape
  navigate their actual compact rows.

The grid and falloff editor accept pointer focus without adding inert Tab stops.
Suzanne also accepts arrow-key orbiting and committed W/S zoom. The browser demo
retains the same controls and all themes, while its native-filesystem picker is
inert. [WEB.md](WEB.md) documents IME and browser input limits; component/app writers
own the synchronization obligations in [COM_AUTHORS.md](COM_AUTHORS.md).
