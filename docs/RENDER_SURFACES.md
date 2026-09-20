# Independent UI render surfaces

`IRenderSurfaceFactory` creates a two-dimensional offscreen session on the
factory's existing GPU device. Query the returned `IRenderSurface` for
`IRenderer` and pass that projection to the preview UI's install/render calls.
Each session retains its own uploaded atlas; drawing another session must not
replace it or repeatedly upload unchanged pixels.

The surface size is in physical pixels. Negative or oversized values fail;
a zero component suspends frame submission. Resize changes the next requested
extent and does not modify completed images. Rendering requires an exact,
positive size and opaque clear color. This initial capability accepts atlas
and texture batches; application-specific render items are explicitly rejected.

`completed_image` returns `NC_E_NOT_FOUND` before the first successful frame.
After success it returns a normal immutable `ITexture`, with the same top-left
UV convention and nearest filtering as uploaded UI textures. Draw it using the
existing painter texture operation. It is accepted only by its owning device.
Every retained output remains unchanged after later frames, resize, or surface
release. A failed frame leaves the previous completed image available.

Surface rendering never presents, acquires a native swapchain image, or clears
the native backbuffer. A window frame separately presents the editor and draws
the completed preview texture. GPU allocations retire through the existing
device ownership path after all public images and active submissions release
them. Stream reads and resource authentication finish before GPU execution.

Glow and WebGL implement the capability through framebuffer-backed textures.
They upload each session's atlas independently and use a fresh output allocation
for each frame, so published snapshots cannot be overwritten. WGPU uses the same
independent-session ownership with RGBA8 render attachments and a UI-only pipeline
without depth. WGPU texture passes never acquire/reconfigure a native surface;
window passes retain their own format and application depth attachment. Error
scopes complete before output publication or native presentation. WGPU retains
in-flight native resources through its command submission lifetime.

Vulkan uses a private RGBA8 render pass, framebuffer, pipeline, command pool,
vertex upload, and completion fence. It shares the device's authenticated images
and sampled descriptors. The render pass transitions completed pixels to
shader-readable layout; the completion fence signals before publication.
This first implementation waits for that one submission on the CPU. It makes
ownership and failure recovery explicit, with a measurable synchronization cost.
No native acquisition/presentation semaphores or frame slots participate.
A submission/wait failure prevents unsafe command-storage reuse. A frame dropped
before submission is recoverable. Descriptors follow the existing pool-generation
rebinding when the native swapchain is rebuilt.

Native acceptance can be run with:

```sh
SDL_VIDEODRIVER=offscreen cargo test -p neocogi-render-plugin --features neocogi-com/static-runtime --lib -- --ignored --test-threads=1
```

The surface test checks asymmetric source orientation, old snapshot pixels after
later frames, unchanged window atlas identity, native allocation failure,
suspended/resized output, surface release, and final device retirement. WebGL
compilation is checked separately; compilation does not establish browser runtime
coverage.

An embedded preview leaf declares keyboard bits `focus | Tab stop | embedded
scope` (`1 | 2 | 512`). While focused, ordinary Tab and menu/window navigation
keys reach the leaf instead of the outer manager. Ctrl+Tab, with optional Shift,
leaves the scope through the editor's normal focus traversal. Outer menus and
modal eligibility still take priority. `WidgetProperties` is now version 2;
hosts and UI plugins must be rebuilt together.

The shared application host supplies `AppState::after_update` after all editor
event callbacks return and before rendering. Preview compilation, resource
installation, input delivery, and offscreen rendering belong at that boundary.
`AppState::text_input_area` selects the single native IME placement after both
UIs have updated; an embedded host translates preview coordinates into its
pane's committed screen rectangle. Neither operation belongs inside painting.

WGPU GPU checks require a native adapter (software Vulkan is sufficient) and a
working SDL native display for the surface test:

```sh
cargo test -p neocogi-render-wgpu -- --ignored --test-threads=1
cargo run -p neocogi-theme-editor --no-default-features --features monolith,embed-assets,wgpu
```

The WGPU test reads actual rendered pixels, verifies top-left source orientation
and retained-output immutability across different-size frames and allocation
failure, then submits an ordinary native frame. Unsupported SDL dummy/offscreen
video drivers return a construction diagnostic instead of reaching SDL's
panicking raw-window-handle adapter.

Vulkan checks exercise actual readback and recovery with a native display:

```sh
VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation cargo test -p neocogi-render-vulkan -- --include-ignored --test-threads=1
cargo run -p neocogi-theme-editor --no-default-features --features monolith,embed-assets,vulkan
```

The Khronos validation layer is optional and must be installed to enable it.
The tests cover private-atlas orientation, immutable retained outputs, changed
output extents, rejected allocation, pre-submission abandonment, and subsequent
ordinary window rendering. All three native backends now expose surfaces;
WebGL execution is validated separately from native Glow.
