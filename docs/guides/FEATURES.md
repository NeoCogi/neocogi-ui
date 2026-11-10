# Cargo features

NeoCogi UI publishes exactly three crates at the same version. `neocogi-ui-contracts`
owns canonical UI/GFX/Skin/document contracts, typed application clients, immutable
resource values, appearance values, design metadata and shared IDL sources.
`neocogi-ui` reexports those definitions and owns all concrete implementations.
`neocogi-ui-dylib` forwards selected implementation features and exports only the
`nc_get_plugin_descriptor_v1` entrypoint as `nc_ui`.

Both libraries have empty default features. Merely importing contracts or clients
creates no retained tree, renderer, theme parser or process runtime.

| Crate | Feature | Effect |
| --- | --- | --- |
| `neocogi-ui-contracts` | `build-support` | Packaged UI and matching Platform schemas, IDL source selection and Cargo generation helpers. |
| `neocogi-ui-contracts` | `compiled-assets` | Expand bounded zlib pixels from generated portable tables. |
| `neocogi-ui-contracts` | `test-support` | Client-side event test helpers. |
| Both libraries | `runtime` | Rich COM runtime clients; the executable still supplies the host. |
| `neocogi-ui` | `ui` | Retained runtime, built-in nodes, presentation algorithms and resource helpers. |
| `neocogi-ui` | `publisher` | Immutable asset publication without font rasterization or filesystem loading. |
| `neocogi-ui` | `gfx` | Graphics service, PNG loader and renderer-neutral compositor; includes `publisher`. |
| `neocogi-ui` | `builder` | Font rasterization and atlas construction; includes `gfx`. |
| `neocogi-ui` | `skin` | Appearance service and typed palette/metric derivation using an explicit publisher. |
| `neocogi-ui` | `authoring` | Strict JSON theme validation and compilation using explicitly supplied graphics capabilities. |
| `neocogi-ui` | `theme-json` | Skin JSON-loader service; includes `skin` and `authoring`. |
| `neocogi-ui` | `document` | Authored document model, reconstruction, persistence and generated typed bindings. |
| `neocogi-ui` | `save-to-rust` | Atomic source export from validated tables; includes `gfx`. |
| `neocogi-ui` | `compiled-assets` | Forwards contracts' compressed-table support. |
| `neocogi-ui` | `test-support` | Real CPU render-device and retained-engine fixtures; includes `ui`, `gfx`, `skin`, `document` and Platform test support. Implementation unit suites require this fixture feature; service-feature tests do not. |
| `neocogi-ui-dylib` | defaults: `ui,gfx` | One loadable UI service root. Additional `builder`, `skin`, `theme-json`, `document` features forward to the static SDK. |

Register the static SDK's `plugin_descriptor()` or load the single UI dylib. Both
publish `neocogi.ui` with a `UiLibraryFactory` root. Use
`neocogi_ui_contracts::services::{ui,nodes,gfx,skin}(runtime)` or acquire
`services::library(runtime)` and call its typed getters. Each call returns an
independently owned factory. An omitted service returns `NC_E_NOT_FOUND`;
`publisher` alone exposes direct publication helpers but does not enable the GFX
service. Builder and JSON-loader operations retain their existing capability
errors when their optional implementation is absent.

The root has no hard implementation-plugin dependencies. UI and nodes preserve
their separate COM factory contracts: binding nodes authenticates the retained
runtime's public construction capabilities and canonical widget-host identity.
Skin receives its GFX publisher/loader explicitly. Consolidating crates does not
make private object state cross these boundaries. Former UI runtime, nodes, GFX
and Skin plugin identities and their separate dylibs are retired.

The executable links `neocogi-com-host`, initializes it through `#[com_main]`, and
passes `Runtime` to implementations. There is no COM runtime DSO or implicit
initializer. Platform uses its matching `neocogi-platform-contracts`,
`neocogi-platform` and `neocogi-platform-dylib` packages; backend selection remains
under Platform's features.

Native demos default to `dynamic,glow`. Choose exactly one linkage mode and one
renderer, using `--no-default-features` when replacing defaults:

```sh
cargo run -p neocogi-demo-full
cargo run -p neocogi-demo-full --no-default-features --features dynamic,vulkan
cargo run -p neocogi-demo-full --no-default-features --features monolith,vulkan,embed-assets
```

Dynamic composition requests UI and Platform DLLs as Cargo artifact dependencies;
an independent extension adds its own DLL. Monoliths register the same static
service roots. Browser `webgl` uses static registration and embedded resources.
The full browser demo enables the live Skin compiler for its appearance editor.
Optional native theme parsing is selected with `monolith-skin` or the dynamic
composition's `skin` feature. Do not enable both native linkage modes.

Cargo builds providers in the application's target profile. The thin dylibs alone
own discovery exports; ordinary Rust implementation dependencies remain rlibs.
`scripts/package-native.py` delegates exact distribution layout to Platform's
build API. Development profile directories can retain stale files; distributions
contain only the selected application, UI/Platform providers, optional extension
and resources. See [native builds](COM_NATIVE.md) and [examples](EXAMPLES.md).

`embed-assets` compiles fonts, PNGs and themes under `OUT_DIR`; native defaults read
adjacent files. Both routes use the same Skin recipe and GFX loader/publisher.
Build tools call `neocogi_ui::gfx::export::write_rust` with `save-to-rust`; generated
consumers select `neocogi-ui-contracts/compiled-assets`. All paths enforce the
shared 64 MiB RGBA bound and exact storage/dimensions. See [asset contracts](../contracts/COM_ASSETS.md).

CI checks minimal feature selections separately from the all-feature unit suite,
then qualifies extracted archives, independent contract consumers, real shared
libraries, native backends and browser DOM/input behavior.
