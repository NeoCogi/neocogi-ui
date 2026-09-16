# Cargo features

Features belong to the crate that implements the selected capability. The
`neocogi-ui` application facade keeps its defaults empty. Its required GFX
contract and portable bundle values are part of the ordinary API because every
retained UI imports resources and submits composed frames. Applications opt into
`skin` only when they need theme-authoring contracts.

| Crate | Feature | Effect |
| --- | --- | --- |
| Native demos | `monolith` | Link the runtime plus only the UI, renderer, GFX, and Skin implementations selected by that assembly. |
| `neocogi-ui` | `runtime` | Link the shared neocogi-com runtime used by native hosts. |
| `neocogi-ui` | `static-runtime` | Link one ordinary runtime implementation, including browser deployments. |
| `neocogi-gfx-support` | `compiled-assets` | Expand bounded zlib pixels from generated asset tables. |
| `neocogi-ui` | `skin` | Re-export the optional Skin plugin's typed authoring contracts. |
| `neocogi-ui-impl` | none | Non-plugin implementation support; exports no loader entrypoint. |
| `neocogi-ui-runtime` | `dynamic-plugin` | Export the core runtime provider entrypoint in an assembled shared object. |
| `neocogi-ui-nodes` | `dynamic-plugin` | Export the built-in nodes provider entrypoint in a separate assembled shared object. |
| `neocogi-gfx` | none | PNG decoding, resource publication, and renderer-neutral frame composition. |
| `neocogi-gfx` | `builder` | Font rasterization and PNG atlas construction into portable tables. |
| `neocogi-skin` | none | Live typed metric/palette compilation through the always-available GFX publisher. |
| `neocogi-skin` | `theme-json` | Strict external JSON and appearance compilation; resource baking uses an explicit GFX loader. |
| `neocogi-gfx-impl` | `save-to-rust` | Build-tool-only atomic source export from validated portable tables. |
| `neocogi-skin-impl` | `builder` | Offline JSON-theme generation through the GFX implementation; never selected by the Skin plugin. |
| `neocogi-render-plugin` | `glow` / `vulkan` / `wgpu` / `webgl` | Build exactly one renderer plugin identity; WebGL is the browser choice. |
| `neocogi-demo-host` | `png-assets` | Expose the host PNG-upload client for applications that select the GFX plugin. |
| Full/basic demos | `embed-assets` | Explicitly embed resource bytes; native defaults read adjacent files. |
| Full/basic demos | `webgl` | Select browser integration and explicitly enable `embed-assets`. |

Shared native builds intentionally allow Cargo to unify the dynamic `runtime`
feature. The facade contains no registry state: every dependent calls the same
loaded `libnc_com_shared_runtime`. In contrast, `static-runtime` must not be
enabled independently in plugin DSOs because each image would then contain a
separate registry implementation.

The UI implementation has no optional asset features. `test-support` on the facade
exposes native event-queue fixtures only for implementation tests; applications and
plugins use the ordinary typed `EventSource` and `EventSession` API.

Build native applications through the package driver so each plugin gets its own
feature selection and the executable receives its adjacent runtime/manifest:

```sh
cargo neocogi build --assembly full --variant glow
cargo neocogi build --assembly full --variant glow --profile min-size
cargo neocogi build --assembly full --variant vulkan --profile min-size
cargo neocogi build --assembly full --variant vulkan --variant monolith --release
```

Generated ABI signatures are build-tool-computed SHA-256 values derived from
the exact module fingerprint and declaration ID. This is part of the contract
ABI, not an assembly feature or variant, and rustc performs no contract
hashing. Dynamic plugins and monolithically linked descriptions therefore
receive the same exact signature checks. The `monolith` variant stages no
Neocogi shared libraries or `plugins.txt` because the executable registers its
linked descriptors directly.

`glow`, `vulkan`, `wgpu` and `webgl` on demo packages select host behavior. They are
separate from implementation-library features. A direct demo build does not stage
its dynamic dependencies. See [native packaging](COM_NATIVE.md) and [WebGL](WEB.md).

Native demos load executable-relative font/icon, theme, photo and mesh files by
default. The GFX plugin owns decoding and resource baking; the skin plugin owns
appearance and external-theme compilation. The host retains only typed COM bundles
and resource capabilities. `cargo neocogi` stages both plugins for file-backed
assemblies and selects their implementation features independently.

Use `--variant embedded` to explicitly select `embed-assets`. Cargo's host build
then processes fonts, PNGs and JSON into generated source under `OUT_DIR`, tracking
the recipe inputs. The target expands bounded compressed RGBA and uses compiled
metadata; it also embeds the photo and mesh. Browser `webgl` selects this mode.
Neither native resource choice links a UI implementation or theme parser into the
application's normal dependency graph.

`neocogi-demo-assets/full-demo` adds theme choices, photo and mesh access.
The asset compiler is an optional host build dependency enabled by `embed-assets`. Both storage
modes execute the same typed Skin recipe through the same GFX loader and publisher implementation
and retain the original catalog when switching themes. The offline route instantiates those GFX
services statically; it does not maintain a second Skin-owned atlas algorithm.

For another application, call `neocogi_gfx_impl::export::write_rust` from a build tool and
enable `neocogi-gfx-support/compiled-assets` directly in the consumer.
`load_bundle()` returns a fresh
immutable catalog; `bundle_data()?` provides tables for a compatible revision.
The exporter is also available as a workspace command:

```sh
cargo run -p neocogi-asset-tools --bin atlas-export -- --output bundle.rs
cargo test -p neocogi-demo-assets --features 'full-demo embed-assets'
```

Asset consumers enforce the shared 64 MiB RGBA limit and exact dimensions/storage.
UI `ImageError` classifies direct raw image storage. Canonical `AssetData`/`AssetTables`
validation returns `NC_E_INVALID_ARG` for portable resource violations. Asset construction returns concrete I/O and theme errors; COM methods retain details
through owned diagnostic interfaces. [COM_ASSETS.md](COM_ASSETS.md) describes validation and publication.

CI tests the UI and each compiler feature independently: no features, `builder`,
`theme-json` and `save-to-rust`. It checks native backends separately and runs browser
DOM/input regressions. A workspace-wide feature union can hide a broken minimal
configuration; `--all-features` also does not replace the three native backend runs.
