# Cargo features

Features belong to the crate that implements them. The `microui-redux` application
facade has no renderer, font-builder or JSON feature. Its defaults are empty:

| Crate | Feature | Effect |
| --- | --- | --- |
| `microui-redux` | `runtime` | Link the shared nc-com runtime used by native hosts. |
| `microui-redux` | `static-runtime` | Link one ordinary runtime implementation, including browser deployments. |
| `microui-redux` | `compiled-assets` | Expand bounded zlib pixels from generated asset tables. |
| `microui-ui` | `builder` | Font rasterization and PNG atlas construction inside the implementation/tool binary. |
| `microui-ui` | `png_source` | Decode PNG image sources, rejecting animation and invalid RGBA storage. |
| `microui-ui` | `theme-json` | Strict JSON theme compilation; includes `builder` and `png_source`. |
| `microui-ui` | `save-to-rust` | Atomically export compiled resource source, including compressed COM bundles. |
| `microui-plugin-assets` | none | Optional PNG decoder provider, without font or JSON compilation. |
| `microui-plugin-assets` | `builder` / `theme-json` | Add the corresponding asset construction classes to that provider. |
| `microui-plugin-platform` | `glow` / `vulkan` / `wgpu` | Select exactly one native implementation. |
| Full/basic demos | `embed-assets` | Explicitly embed resource bytes; native defaults read adjacent files. |
| Full/basic demos | `webgl` | Select browser integration and explicitly enable `embed-assets`. |

The implementation library also has empty defaults. `test-support` on the facade
exposes native event-queue fixtures only for implementation tests; applications and
plugins use the ordinary typed `EventSource` and `EventSession` API.

Build native applications through the package driver so each plugin gets its own
feature selection and the executable receives its adjacent runtime/manifest:

```sh
cargo com-build --demo full --backend glow
cargo +nightly com-build --demo full --backend glow --min-size
```

`glow`, `vulkan`, `wgpu` and `webgl` on demo packages select host behavior. They are
separate from implementation-library features. A direct demo build does not stage
its dynamic dependencies. See [native packaging](COM_NATIVE.md) and [WebGL](WEB.md).

Native demos load executable-relative font/icon, theme, photo and mesh files by
default. The asset plugin owns decoding and compilation; the host retains typed
COM bundles and resource capabilities. `com-build` stages that plugin with
`theme-json`, which includes its font/atlas builder.

Use `--assets embedded` to explicitly select `embed-assets`. Cargo's host build
then processes fonts, PNGs and JSON into generated source under `OUT_DIR`, tracking
the recipe inputs. The target expands bounded compressed RGBA and uses compiled
metadata; it also embeds the photo and mesh. Browser `webgl` selects this mode.
Neither native resource choice links a UI implementation or theme parser into the
application's normal dependency graph.

`microui-redux-demo-assets/builder` exposes the shared recipe only to tools and
provider tests. `full-demo` adds theme choices, photo and mesh access. Both storage
modes use the same recipe and retain the original catalog when switching themes.

For another application, call `SkinBundle::to_com_rust_files` from a build tool and
enable facade `compiled-assets` in the consumer. `load_bundle()` returns a fresh
immutable catalog; `bundle_data()?` provides tables for a compatible revision.
The exporter is also available as a workspace command:

```sh
cargo run -p microui-redux-tools --bin atlas-export -- --output bundle.rs
cargo test -p microui-redux-demo-assets --features 'builder full-demo embed-assets'
```

Asset consumers enforce the shared 64 MiB RGBA limit and exact dimensions/storage.
The implementation's `ImageError`, `AtlasError` and `BuilderError` retain concrete
native error sources; across COM, statuses and owned diagnostic interfaces carry
failures. [COM_ASSETS.md](COM_ASSETS.md) describes validation and publication.

CI tests implementation features independently: no features, `builder`, `png_source`,
`theme-json` and `save-to-rust`. It checks native backends separately and runs browser
DOM/input regressions. A workspace-wide feature union can hide a broken minimal
configuration; `--all-features` also does not replace the three native backend runs.
