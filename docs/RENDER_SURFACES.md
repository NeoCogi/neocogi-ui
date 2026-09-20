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
for each frame, so published snapshots cannot be overwritten. Vulkan and WGPU
currently reject surface creation explicitly while preserving their ordinary
window rendering. They remain required follow-up work for editor backend parity.

Native acceptance can be run with:

```sh
SDL_VIDEODRIVER=offscreen cargo test -p neocogi-render-plugin --features neocogi-com/static-runtime --lib -- --ignored --test-threads=1
```

The surface test checks asymmetric source orientation, old snapshot pixels after
later frames, unchanged window atlas identity, native allocation failure,
suspended/resized output, surface release, and final device retirement. WebGL
compilation is checked separately; compilation does not establish browser runtime
coverage.
