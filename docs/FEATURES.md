# Cargo features

- `builder` *(enabled by default through `theme-json`)* – enables the runtime atlas builder and PNG decoding helpers used by the examples.
- `png_source` – accepts static PNG-compressed serialized atlases and `ImageSource::Png { .. }`; pixels are decoded to RGBA and pass the same strict atlas validation as raw sources before a handle is returned. Animated PNGs are rejected.
- `theme-json` *(default)* – enables strict versioned JSON theme loading and relative per-state
  image patches. It includes `builder` and `png_source`; repeated references to one resolved image
  path share one region in the completed theme atlas and use the ordinary atlas draw batch.
- `save-to-rust` *(default)* – enables `AtlasHandle::to_rust_files` to emit the current atlas as Rust code for embedding.

Disabling default features leaves only the raw RGBA upload path (`ImageSource::Raw { .. }`):
`cargo build -p microui-redux --no-default-features`

Serialized atlases are loaded with `AtlasHandle::try_from(&source)`. Construction returns a
concrete `AtlasError` for malformed pixels or metadata; there is no lossy or infallible atlas-load
path. Every font must include an underscore fallback, and all atlas rectangles must fit inside the
declared texture. Public image decoding, serialized atlas construction, builder atlas allocation,
and builder icon decoding share a 64-MiB limit for each decoded RGBA or normalized color buffer
(`MAX_DECODED_RGBA_BYTES`, exactly 4,096 × 4,096 four-byte pixels).

`load_image_bytes` returns the concrete `ImageError`. Its variants distinguish invalid signed raw
dimensions, checked storage failures, exact RGBA-length mismatches, dimension mismatches, decoder
failures, and unsupported animation. `ImageStorageError` retains the failed dimensions and
allocation limit. Higher layers preserve that structure: `AtlasError::Image` and
`BuilderError::Image` expose `ImageError` as their standard error source instead of translating it
through `std::io::Error` or matching diagnostic text.

The demo packages have their own feature namespace. `glow`, `vulkan`, and `wgpu` select one of the
native renderer crates through `demos/demo-host`. `webgl` selects the WebGL 2 canvas renderer and
requires the `wasm32-unknown-unknown` target. The demos build their atlas at runtime unless you opt
into the forwarded `prebuilt-atlas` feature from `demos/demo-assets`:
`cargo run -p microui-redux-demo-full --no-default-features --features vulkan`

To generate and embed the atlas at build time instead, add `prebuilt-atlas` explicitly. This
changes the demo's runtime asset-loading path; `demo-full` still enables the main crate's
`theme-json` feature through its manifest:
`cargo run -p microui-redux-demo-full --no-default-features --features "vulkan prebuilt-atlas"`

Use [`scripts/web-demo.sh`](../scripts/web-demo.sh) for the browser build. It selects `webgl`,
packages the generated JavaScript, WebAssembly, embedded assets, and HTML into `target/web-demo`,
and can start a local HTTP server. See the [WebGL guide](WEB.md).

Runtime and prebuilt demo atlases use the same recipe in `demos/demo-assets/src/recipe.rs`.
For `prebuilt-atlas`, a normal Cargo host build dependency calls the core builder and exporter
and writes the PNG plus Rust metadata together into `OUT_DIR`. Asset change tracking is derived
from that recipe. Cross-compilation therefore runs generation on the host without launching a
nested Cargo process or maintaining a separate target directory. When both demo asset features
are enabled, `prebuilt-atlas` selects the embedded runtime loader; `builder` still exposes the
shared recipe to tools. CI decodes the generated atlas and compares its pixels and resource
metadata with runtime generation using `cargo test -p microui-redux-demo-assets
--no-default-features --features "builder prebuilt-atlas"`.

To export an atlas as Rust, enable `save-to-rust` (and `png_source` when serializing PNG-backed atlas data) and call `AtlasHandle::to_rust_files`. The workspace helper enables its required library features in its own manifest:
`cargo run -p microui-redux-tools --bin atlas-export -- --output path/to/atlas.rs`


## Feature validation

CI tests the core package independently with defaults, no features, and each of `builder`,
`png_source`, `theme-json`, and `save-to-rust` enabled alone. Each job runs unit, public API, and
documentation tests; feature-gated documentation examples compile when their API is available.
This isolation matters because testing the whole workspace enables a union of dependencies'
features and can hide a broken minimal configuration.

Native demos are checked separately with `--no-default-features --features glow`, `vulkan`, and
`wgpu`. WebGL compilation and DOM regressions run on pull requests independently of Pages
deployment. `--all-features` is not a substitute for these backend selections: conditional backend
aliases choose only one branch when several renderer features are enabled together.
