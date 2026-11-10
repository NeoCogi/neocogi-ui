# Component and plugin boundaries

UI uses the same three-package release model as Platform. Package boundaries
express contracts versus implementation versus the final dynamic link root.
Runtime COM capabilities retain explicit ownership within that implementation.

| Responsibility | Canonical owner | Implementation |
| --- | --- | --- |
| UI, node, GFX, Skin and document interfaces | `neocogi_ui_contracts::abi` and domain modules | `neocogi-ui` modules |
| Portable asset tables, image/palette values, event owners and typed clients | `neocogi-ui-contracts` | Shared directly; no duplicate representations |
| Design metadata and bounded settings values | `neocogi_ui_contracts::design` | Document reconstruction in `neocogi_ui::document` |
| Packaged production schemas and generation helpers | `neocogi_ui_contracts::build` | Host-side `build-support` feature |
| Retained forest, layout, routing, menus, windows and commit | Public UI contracts | `neocogi_ui::runtime` |
| Built-in widgets, containers and dialogs | Public node contracts | `neocogi_ui::nodes` |
| Presentation algorithms, resource imports and geometry/text helpers | Public values | `neocogi_ui::support` |
| Publication, PNG/font construction and composition | GFX contracts | `neocogi_ui::gfx` |
| Appearance derivation and external theme services | Skin contracts | `neocogi_ui::skin`, strict syntax/compiler in `authoring` |
| Sole UI descriptor/root | `abi::library::UiLibraryFactory` | `neocogi_ui::plugin_descriptor()`, thin `neocogi-ui-dylib` export |
| Platform, renderer, GPU resources and shader compiler | `neocogi-platform-contracts` | Matching static Platform or thin Platform dylib |

The release has one canonical Rust definition and one authored NCIDL graph for
each contract. The static SDK reexports contracts for convenience. Independent
extensions should depend on contracts directly, generating/importing their own
closed interface sets as appropriate. Contracts has no dependency on the static
SDK, native renderer, PNG/font construction or filesystem theme loader.

## Activation and lifetime

The sole UI plugin is `neocogi.ui`. Its typed `UiLibraryFactory` exposes `ui()`,
`nodes()`, `gfx()` and `skin()` getters. Every result is an independently owned
factory; dropping the library root cannot revoke a returned service. Disabled
features return `NC_E_NOT_FOUND`. The root has no hard implementation-plugin
dependencies. The SDK's `services` helpers acquire this root and call the typed
getter; there is no untyped service registry or downcast path.

Former UI runtime, node, GFX and Skin plugin identities and separate wrappers are
removed. There is one UI DLL (`nc_ui`), one Platform DLL (`nc_platform`) and, when
selected, an independent extension DLL. Monolith/browser builds register the
same descriptors from normal rlibs. Final dylib packages alone export discovery.
The executable owns `neocogi-com-host` and passes process runtime access through
bootstrap; no shared COM runtime library is deployed.

Nodes still binds through public runtime capabilities, including
`IConstructionHost`, `IChoiceMenus`, windows and resource access. The returned
`UiFactory` is bound to that runtime's canonical `IWidgetHost` identity.
`Ui::from_interfaces` rejects mismatched hosts. Combining crate ownership does
not merge object identities or let a node obtain a private forest pointer.
External widget/container implementations use the same public contracts.

Retained traversal caches required projections once, including the inherited
physical `IWidget` prefix. It does not perform a provider lookup per widget or
draw. Layout/paint/dispatch order, exclusive widget lifetime, mutation guards and
replace-on-success commit boundaries are unchanged by this packaging change.
See [exclusive ownership](../contracts/EXCLUSIVE_WIDGET_OWNERSHIP.md),
[event dispatch](../contracts/WIDGET_EVENT_DISPATCH.md) and
[runtime shutdown](../contracts/UI_RUNTIME_SHUTDOWN.md).

## Graphics and appearance

Portable `AssetData`/`AssetTables` live in contracts. Validation and immutable
publication have one implementation. GFX construction, PNG decoding, font
rasterization and atlas packing live in the static SDK behind explicit features.
Skin receives a typed GFX publisher/loader; it has no second atlas algorithm.
Offline compilation invokes the same capabilities directly. Strict JSON syntax
and authored theme resolution remain separate from live appearance objects.

The compositor emits authenticated canonical Platform GPU blocks. The selected
Platform renderer owns native resource types, driver identity, frame streams,
shader compilation and backend execution. UI consumes Platform's published
geometry/image/GPU APIs rather than maintaining a parallel renderer contract.
See [asset contracts](../contracts/COM_ASSETS.md) and the
[Platform migration record](../project/PLATFORM_0_2_MIGRATION_2026-11-09.md).

Feature selection controls linked code; a package merge makes no size or speed
claim. Dynamic assembly currently enables the GFX builder and selected Skin JSON
support even with embedded resources. Test each minimal configuration as well
as the all-feature graph. The exact feature map is in [FEATURES.md](../guides/FEATURES.md).

## Authoring and validation

Contracts transports finite design metadata and bounded settings. Document
syntax, registry resolution, generated fields, construction and live form
lifetime remain separate responsibilities in the SDK's document module.
Custom catalogs are ordinary typed COM contracts. An independent example
extension retains its own source/schema and DLL identity; package reduction does
not fold extension implementation into UI.

Qualification executes real typed factories, cross-library object calls, host
identity checks, allocation regressions, generated consumers, extracted archives,
native renderer tests and browser input. Historical dated reports retain their
original measurements; they describe their stated revision rather than current
package ownership. Current acceptance is recorded in the three-crate migration
report and [release guide](../project/RELEASING.md).
