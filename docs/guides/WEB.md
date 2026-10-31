# WebGL canvas demo

The browser demo compiles `neocogi-demo-full` directly for Rust's
`wasm32-unknown-unknown` target. `neocogi-render-plugin` owns DOM input, canvas sizing and WebGL2, while the concrete demo
host owns `requestAnimationFrame`. UI, platform and PNG descriptors register in one static neocogi-com
runtime in the same wasm module. SDL remains a native-only dependency.
The demo's `webgl` feature explicitly selects `embed-assets`; fonts, themes, photo
and mesh are included in the wasm deployment. Native demos default to adjacent files.

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

- `index.html`, which styles the platform-created canvas and reports startup failures;
- the small JavaScript/WebAssembly bindings generated for the WebGL and DOM calls;
- the WebAssembly module, with the prebuilt UI atlas, Suzanne mesh, and `FACEPALM.png` embedded.

The platform creates an independent WebGL2 canvas for each window, scales its backing buffer for
the device pixel ratio, and queues copied pointer/wheel/keyboard input. The host polls those
transitions through `IInputSink`, updates its typed event session, synchronizes IME and renders.
Its sole scheduling adapter uses `requestAnimationFrame`. The full demo reserves a 2-MiB WebAssembly stack for its retained graph.
The browser build has no virtual filesystem: the file-dialog entry is disabled, while all bundled
themes and demonstration assets are embedded and remain available.

`CARGO_TARGET_DIR` selects the shared artifact, browser-test site and installed tool
root for both scripts. Relative roots resolve from the workspace; spaces are
supported. `NEOCOGI_WASM_BINDGEN` takes precedence and selects an already installed
matching CLI (relative paths also resolve from the workspace). The builder reports
an invalid explicit path/version without installing a different default executable.
Automatic installation applies to the derived default tool path.
`NEOCOGI_WEB_PROFILE=debug` enables a faster development build.
The `frames` query parameter, such as `?frames=8`, renders a finite number of frames and then
releases the session and host. Normal stop and asynchronous frame errors both break the RAF
ownership cycle. A lost WebGL context requires recreating the window and its GPU resources;
there is no automatic restoration behind retained image or mesh identities. See the
[package guide](COM_NATIVE.md#browser) for the shared native/browser lifecycle.

## Input contract

The platform captures one primary pointer until release. It reconciles the full button mask, including
mouse chords, and cancels the core gesture on `pointercancel`, unexpected `lostpointercapture`,
or browser/DOM focus loss. Touch and pen supply primary-pointer mouse emulation; multiple contacts,
pressure, and gestures are outside this host's contract. Canvas touch scrolling is disabled while
the UI owns the pointer.

Wheel deltas retain the browser's positive-right/down direction. Pixel deltas are scaled from CSS
to framebuffer pixels, lines mean 30 CSS pixels, and pages mean one canvas extent per axis.
Fractional pixels accumulate across events so slow trackpad movement is retained.

Focused text editors use a platform-owned transparent textarea positioned within the editor's visible
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
and accessibility. Browser callbacks are unregistered and the textarea removed when the platform window is finally released.

## Browser regression

After the web build installs the matching bindings tool, run `./scripts/test-browser-input.sh input`
and `./scripts/test-browser-input.sh full`. The full suite renders every compiled theme, exercises
the photograph and typed application actions, and checks both successful and failed animation-loop
teardown, including final application-state and canvas/textarea release.
The script builds the wasm library test export, starts a temporary localhost server, and checks its
result in headless Chromium. Set `CHROME_BIN` to a compatible executable or
`NEOCOGI_BROWSER_TEST_PORT` to change the default port 8118. Generated bindings and result HTML
remain under the selected target root's `browser-input-tests` and `browser-full-tests`
directories (default `target`). Before launching Chromium, the runner waits up to ten
seconds for its own HTTP child to publish a private signal after binding the port.
An occupied port, early child exit or startup timeout fails with the server log;
another listener's response cannot qualify the suite. Exit and handled termination
signals reap the child and remove its private startup files. Separate simultaneous
runs should use distinct ports and target roots to avoid sharing generated sites.
The fixture covers DOM composition/paste, keyboard,
wheel conversion, button chords, cancellation, and teardown. It stubs capture acquisition because
synthetic pointer events cannot acquire native capture; trusted pointer/OS IME/mobile behavior
still needs interactive platform testing.

The fast producer/consumer path and install-policy matrix runs with
`python3 -m unittest discover -s scripts -p test_web_paths.py`. It uses actual
script copies and subprocess canaries, so it does not replace the real wasm/browser
suites. The browser CI job runs both real suites under a custom target root with
spaces, while the Pages job retains its deployment artifact path.

`python3 -m unittest discover -s scripts -p test_browser_server.py` exercises the
unchanged runner with real loopback servers, a fetching browser canary, and stubbed
build tools. It checks both selected suites, an unrelated PASS listener, early
exit, failed readiness publication, bounded timeout, termination and cleanup after
a failing browser result. It needs loopback access and complements the actual
WebAssembly/Chromium suites; it does not execute their rendering or DOM tests.

## GitHub Pages

The `WebGL demo` workflow builds the site and runs DOM input regressions on pull requests, pushes
to `master`, and manual dispatch. Pull-request jobs have read-only repository permissions and never
configure or deploy Pages. Before the first deployment, a repository administrator
must select **Settings > Pages > Build and deployment > Source > GitHub Actions**.
The workflow's `GITHUB_TOKEN` can configure and deploy an existing site; it cannot
create the repository's Pages site. Adding `pages: write` does not grant that
administration capability. After enabling Pages, rerun the failed deployment job.
See the [GitHub publishing-source instructions](https://docs.github.com/en/pages/getting-started-with-github-pages/configuring-a-publishing-source-for-your-github-pages-site#publishing-with-a-custom-github-actions-workflow).
A successful deployment publishes the demo at:

<https://neocogi.github.io/neocogi-ui/>

The workflow deploys only generated files from `target/web-demo`; generated bindings and binaries do
not need to be committed.
