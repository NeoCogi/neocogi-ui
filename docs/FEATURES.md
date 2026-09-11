# Cargo features

Features belong to the crate that implements them. The `neocogi-ui` application
facade has no renderer, font-builder or JSON feature. Its defaults are empty:

| Crate | Feature | Effect |
| --- | --- | --- |
| COM graph | `com-validation-none` | Preserve structural validation but skip contract comparison. |
| COM graph | `com-validation-sha-256` | Validate canonical contracts with 256-bit cryptographic digests. |
| COM graph | `com-validation-fnv-1a-128` | Validate canonical contracts with compact 128-bit FNV fingerprints. |
| COM graph | `com-validation-mangling` | Validate and store complete collision-free canonical contract bytes. |
| Native demos | `monolith` | Link the runtime plus UI, renderer, and asset implementations into the executable. |
| `neocogi-ui` | `runtime` | Link the shared neocogi-com runtime used by native hosts. |
| `neocogi-ui` | `static-runtime` | Link one ordinary runtime implementation, including browser deployments. |
| `neocogi-ui` | `compiled-assets` | Expand bounded zlib pixels from generated asset tables. |
| `neocogi-ui-runtime` | none | Imports resolved resources for layout and painting; contains no asset compiler. |
| `neocogi-assets` | none | PNG decoding and live metric/palette compilation, without font or JSON parsing. |
| `neocogi-assets` | `builder` | Font rasterization and PNG atlas construction into portable tables. |
| `neocogi-assets` | `theme-json` | Strict JSON compilation, including the atlas builder. |
| `neocogi-assets` | `save-to-rust` | Atomic source export from validated portable tables. |
| `neocogi-render-plugin` | `glow` / `vulkan` / `wgpu` / `webgl` | Build exactly one renderer plugin identity; WebGL is the browser choice. |
| Full/basic demos | `embed-assets` | Explicitly embed resource bytes; native defaults read adjacent files. |
| Full/basic demos | `webgl` | Select browser integration and explicitly enable `embed-assets`. |

The UI implementation has no optional asset features. `test-support` on the facade
exposes native event-queue fixtures only for implementation tests; applications and
plugins use the ordinary typed `EventSource` and `EventSession` API.

Build native applications through the package driver so each plugin gets its own
feature selection and the executable receives its adjacent runtime/manifest:

```sh
cargo neocogi build --assembly full --variant glow
cargo neocogi build --assembly full --variant glow --profile min-size
cargo neocogi build --assembly full --variant vulkan --variant sha-256 --profile min-size
cargo neocogi build --assembly full --variant vulkan --variant monolith --release
```

Every assembly declares `com-validation = "none" | "sha-256" |
"fnv-1a-128" | "mangling"`. The package driver applies the matching feature to
the executable, runtime, and every plugin after renderer/resource variants.
Those four values are also reserved built-in `--variant` names, allowing one
command to compose renderer, resource, linkage, and validation choices. Repeated
validation variants are applied in order and the last one wins.
Each dynamic plugin descriptor records the resulting mode. Assembly staging
inspects all selected libraries and rejects the plan if their reported modes
differ, so `plugins.txt` cannot describe a mixed validation graph.
The `monolith` variant is valid only with `com-validation = "none"`; it stages no
Neocogi shared libraries or `plugins.txt` because the executable registers its
linked descriptors directly.

`glow`, `vulkan`, `wgpu` and `webgl` on demo packages select host behavior. They are
separate from implementation-library features. A direct demo build does not stage
its dynamic dependencies. See [native packaging](COM_NATIVE.md) and [WebGL](WEB.md).

Native demos load executable-relative font/icon, theme, photo and mesh files by
default. The asset plugin owns decoding and compilation; the host retains typed
COM bundles and resource capabilities. `cargo neocogi` stages that plugin with
`theme-json`, which includes its font/atlas builder.

Use `--variant embedded` to explicitly select `embed-assets`. Cargo's host build
then processes fonts, PNGs and JSON into generated source under `OUT_DIR`, tracking
the recipe inputs. The target expands bounded compressed RGBA and uses compiled
metadata; it also embeds the photo and mesh. Browser `webgl` selects this mode.
Neither native resource choice links a UI implementation or theme parser into the
application's normal dependency graph.

`neocogi-demo-assets/full-demo` adds theme choices, photo and mesh access.
The asset compiler is an optional host build dependency enabled by `embed-assets`. Both storage
modes use the same recipe and retain the original catalog when switching themes.

For another application, call `neocogi_assets::export::write_rust` from a build tool and
enable facade `compiled-assets` in the consumer. `load_bundle()` returns a fresh
immutable catalog; `bundle_data()?` provides tables for a compatible revision.
The exporter is also available as a workspace command:

```sh
cargo run -p neocogi-asset-tools --bin atlas-export -- --output bundle.rs
cargo test -p neocogi-demo-assets --features 'full-demo embed-assets'
```

Asset consumers enforce the shared 64 MiB RGBA limit and exact dimensions/storage.
UI `ImageError` and `AtlasError` classify invalid raw resource storage. Asset
construction returns concrete I/O and theme errors; COM methods retain details
through owned diagnostic interfaces. [COM_ASSETS.md](COM_ASSETS.md) describes validation and publication.

CI tests the UI and each asset feature independently: no features, `builder`,
`theme-json` and `save-to-rust`. It checks native backends separately and runs browser
DOM/input regressions. A workspace-wide feature union can hide a broken minimal
configuration; `--all-features` also does not replace the three native backend runs.
