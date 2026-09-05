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

The implementation library also has empty defaults. `test-support` on the facade
exposes native event-queue fixtures only for implementation tests; applications and
plugins use the ordinary typed `EventSource` and `EventSession` API.

Build native applications through the package driver so each plugin gets its own
feature selection and the executable receives its adjacent runtime/manifest:

```sh
cargo run -p microui-redux-tools --bin com-build -- --demo full --backend glow
cargo +nightly run -p microui-redux-tools --bin com-build -- --demo full --backend glow --min-size
```

`glow`, `vulkan`, `wgpu` and `webgl` on demo packages select host behavior. They are
separate from implementation-library features. A direct demo build does not stage
its dynamic dependencies. See [native packaging](COM_NATIVE.md) and [WebGL](WEB.md).

All demos consume compiled COM bundles. Cargo's host build dependency processes
fonts, PNGs and JSON and exports generated source into `OUT_DIR`, tracking the
recipe's input files. The application target expands only the compressed RGBA
stream and uses compiled metadata; it has no UI implementation or theme parser
dependency. `microui-redux-demo-assets/builder` exposes the shared recipe to tools
and provider tests; it does not select an alternative application loader.
`full-demo` adds the three authored themes and sample photo.

For another application, call `SkinBundle::to_com_rust_files` from a build tool and
enable facade `compiled-assets` in the consumer. `load_bundle()` returns a fresh
immutable catalog; `bundle_data()?` provides tables for a compatible revision.
The exporter is also available as a workspace command:

```sh
cargo run -p microui-redux-tools --bin atlas-export -- --output bundle.rs
cargo test -p microui-redux-demo-assets --features 'builder full-demo'
```

Asset consumers enforce the shared 64 MiB RGBA limit and exact dimensions/storage.
The implementation's `ImageError`, `AtlasError` and `BuilderError` retain concrete
native error sources; across COM, statuses and owned diagnostic interfaces carry
failures. [COM_ASSETS.md](COM_ASSETS.md) describes validation and publication.

CI tests implementation features independently: no features, `builder`, `png_source`,
`theme-json` and `save-to-rust`. It checks native backends separately and runs browser
DOM/input regressions. A workspace-wide feature union can hide a broken minimal
configuration; `--all-features` also does not replace the three native backend runs.
