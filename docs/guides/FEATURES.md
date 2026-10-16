# Cargo features

Features belong to the crate that implements the selected capability. The
`neocogi-ui` application facade keeps its defaults empty. Its required GFX
contract and portable bundle values are part of the ordinary API because every
retained UI imports resources and submits composed frames. Applications opt into
`skin` only when they need theme-authoring contracts.

| Crate | Feature | Effect |
| --- | --- | --- |
| Native demos | `dynamic` (default) | Build and load Cargo cdylib artifacts against one shared runtime. |
| Native demos | `monolith` | Link the runtime plus only the UI, renderer, GFX, and Skin implementations selected by that assembly. |
| `neocogi-com` | defaults: `std`, `macros` | Local object/data helpers and event contracts/adapters without linking a runtime. |
| `neocogi-com` | `macros` | Procedural macros and built-in event contracts, usable without `std` or allocation; IDL is a build dependency. |
| `neocogi-com` | `std` with `macros` | Add standard-channel mappings and scoped/component dispatch adapters. |
| `neocogi-com` | `runtime` | Link shared `nc_com` runtime services, including caller-scoped persistence; implies `std`. |
| `neocogi-com` | `static-runtime` | Implies `runtime`; link its implementation inside one executable or browser module. |
| `neocogi-ui` | `runtime` | Link the shared neocogi-com runtime used by native hosts. |
| `neocogi-ui` | `static-runtime` | Link one ordinary runtime implementation, including browser deployments. |
| `neocogi-gfx-support` | `compiled-assets` | Expand bounded zlib pixels from generated asset tables. |
| `neocogi-ui` | `skin` | Re-export the optional Skin plugin's typed authoring contracts. |
| `neocogi-ui-support` | none | Non-plugin resource and presentation helpers; owns no runtime or concrete nodes. |
| `neocogi-ui-support` | `test-support` | Deterministic canonical resources and phase snapshots; never replaces runtime behavior. |
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
| `neocogi-demo-host` | `skin` | Link/register the live appearance compiler for browser or monolithic startup; does not enable JSON parsing. |
| `neocogi-demo-host` | `monolith-skin` | Add Skin to a native monolith, including its external JSON theme loader. |
| Full/basic demos | `embed-assets` | Explicitly embed resource bytes; native defaults read adjacent files. |
| Full/basic demos | `webgl` | Select browser integration and explicitly enable `embed-assets`. |

The full demo's `webgl` selection additionally enables the host's `skin` capability. Compiling
authored themes into embedded tables does not replace the live appearance compiler used by its
style editor. The browser links/registers that provider without its filesystem-based JSON loader;
the simple demo does not need it.

Shared native builds intentionally allow Cargo to unify the dynamic `runtime`
feature. The facade contains no registry state: every dependent calls the same
loaded `libnc_com`. In contrast, `static-runtime` must not be
enabled independently in plugin DSOs because each image would then contain a
separate registry implementation.

The UI implementation has no optional asset features. Applications, plugins and
native event tests install concrete receivers directly and use the same
standard `std::sync::mpsc` channels. The facade has no separate event-test feature.

The native host observes drawable size after each platform poll. When minimized,
it sends one cancellation to end held gestures and discards subsequent input
observations until a drawable size returns; no UI update runs at zero size.
The runtime also bounds input retained between updates to 4,096 transitions
and 4 MiB of committed text. On overflow it drops the incomplete backlog,
queues cancellation, then retains the newest transition. An individual text
commit above 4 MiB is rejected before copying.

Cargo owns native artifact and feature selection:

```sh
cargo run -p neocogi-demo-full
cargo run -p neocogi-demo-full --no-default-features --features dynamic,vulkan
cargo run -p neocogi-demo-full --no-default-features --features monolith,vulkan,embed-assets
```

Native defaults are `dynamic,glow`. Choose one renderer and one linkage; Cargo
adds requested features to defaults unless `--no-default-features` is supplied.
`dynamic` requests DLL artifacts through `neocogi-demo-plugins`; `monolith`
registers normal Rust dependencies. Browser `webgl` uses static registration and
embedded resources. Never enable both linkage modes in one build.

Generated interface signatures are computed by the IDL library. Rust macro
expansion does not hash contracts. Both linkage modes perform the same exact
interface negotiation against one runtime authority.

Native file-backed resources are copied beside the executable during compilation.
Runtime paths use that application directory, independent of the source checkout.
`embed-assets` instead processes fonts, PNGs and JSON into generated Rust under
`OUT_DIR`. Dynamic composition currently retains the GFX builder and selected
Skin JSON support even with embedded resources; the former assembly-specific
feature overlays are removed. This is a simpler fixed composition rather than
an automatic size optimizer. See [native Cargo builds](COM_NATIVE.md).

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
cargo run -p neocogi-asset-tools --bin neocogi-atlas-export -- --output bundle.rs
cargo test -p neocogi-demo-assets --features 'full-demo embed-assets'
```

Asset consumers enforce the shared 64 MiB RGBA limit and exact dimensions/storage.
UI `ImageError` classifies direct raw image storage. Canonical `AssetData`/`AssetTables`
validation returns `NC_E_INVALID_ARG` for portable resource violations. Asset construction returns concrete I/O and theme errors; COM methods retain details
through owned diagnostic interfaces. [COM_ASSETS.md](../contracts/COM_ASSETS.md) describes validation and publication.

CI tests the UI and each compiler feature independently: no features, `builder`,
`theme-json` and `save-to-rust`. It checks native backends separately and runs browser
DOM/input regressions. A workspace-wide feature union can hide a broken minimal
configuration; `--all-features` also does not replace the three native backend runs.
