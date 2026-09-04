# WebGL canvas demo

The browser demo compiles `microui-redux-demo-full` directly for Rust's
`wasm32-unknown-unknown` target. `web-sys` supplies DOM input, canvas sizing, and
`requestAnimationFrame`; `microui-redux-renderer-webgl-canvas` renders through the canvas's WebGL 2
context. SDL remains a native-only dependency.

## Prerequisites

Install the current stable Rust toolchain. The build script installs the Rust WebAssembly target
and a matching `wasm-bindgen` CLI under `target/web-tools` when needed. No non-Rust SDK is required.

## Build and run locally

From the workspace root, build the static site and start its local server:

```bash
./scripts/web-demo.sh serve
```

Open `http://127.0.0.1:8000`. To choose another port:

```bash
./scripts/web-demo.sh serve 8080
```

For a build without the server, run:

```bash
./scripts/web-demo.sh build
```

The output directory is `target/web-demo`. Serve that directory over HTTP rather than opening
`index.html` directly, because browsers load the generated WebAssembly file with `fetch`.

The package contains:

- `index.html`, which owns the full-page canvas and reports startup failures;
- the small JavaScript/WebAssembly bindings generated for the WebGL and DOM calls;
- the WebAssembly module, with the prebuilt UI atlas, Suzanne mesh, and `FACEPALM.png` embedded.

The browser host requests WebGL 2 from the canvas, scales its backing buffer for the device pixel
ratio, translates pointer/wheel/keyboard input into microui events, and schedules rendering with
`requestAnimationFrame`. The full demo reserves a 2-MiB WebAssembly stack for its retained graph.
The browser build has no virtual filesystem: the file-dialog entry is disabled, while all bundled
themes and demonstration assets are embedded and remain available.

## Input contract

The host captures one primary pointer until release. It reconciles the full button mask, including
mouse chords, and cancels the core gesture on `pointercancel`, unexpected `lostpointercapture`,
or browser/DOM focus loss. Touch and pen supply primary-pointer mouse emulation; multiple contacts,
pressure, and gestures are outside this host's contract. Canvas touch scrolling is disabled while
the UI owns the pointer.

Wheel deltas retain the browser's positive-right/down direction. Pixel deltas are scaled from CSS
to framebuffer pixels, lines mean 30 CSS pixels, and pages mean one canvas extent per axis.
Fractional pixels accumulate across events so slow trackpad movement is retained.

Focused text editors use a host-owned transparent textarea positioned within the editor's visible
area. Printable keys retain their DOM default; navigation/editing commands go to the core. Native
`input` commits, including paste, are drained from the textarea. Composition stays there until
`compositionend`; draining and clearing its final value prevents a subsequent final `input` event
from duplicating the commit. This follows the DOM update boundary described in
[UI Events](https://www.w3.org/TR/uievents/#events-composition-input-events).

This bridge accepts committed Unicode; it does not expose retained selection, copy/cut, rich text,
surrounding text, or inline preedit rendering to the browser. Its anchor is the editor area rather
than the exact caret. Focus is synchronized after the retained update, so mobile browsers that
require text-input focus within a trusted gesture may need a host-specific immediate focus policy
to show their software keyboard. App/host writers own those extensions, font coverage, shaping,
and accessibility. Browser callbacks are unregistered and the textarea removed when the host drops.

## GitHub Pages

The `WebGL demo` workflow builds the site on every push to `master` and supports manual dispatch.
It requests GitHub Pages enablement and configures Actions as the publishing source. If repository
or organization policy prevents automatic enablement, select **GitHub Actions** in
**Settings > Pages** once. A successful deployment publishes the demo at:

<https://neocogi.github.io/microui-redux/>

The workflow deploys only generated files from `target/web-demo`; generated bindings and binaries do
not need to be committed.
