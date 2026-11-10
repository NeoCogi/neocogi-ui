# Changelog

## 0.2.0

- Promote the coordinated contracts, static UI and UI dylib release to stable
  `0.2.0`, retaining the public APIs, runtime behavior and schemas of
  `0.2.0-alpha.2`.
- Use exact `=0.2.0` requirements for internal UI dependencies and package
  consumers. Platform remains at `0.2.0`, COM/IDL at `0.1.0` and rs-math3d
  at `0.21.0`.

## 0.2.0-alpha.2

- Update the contracts, static implementation and dynamic Platform packages to
  the published `0.2.0` release together. Their Rust and NCIDL sources are
  unchanged from `0.2.0-alpha.2`; COM/IDL and rs-math3d remain pinned to `0.1.0`
  and `0.21.0` respectively.

## 0.2.0-alpha.1

- Reduce the reusable UI release from nineteen crates to contracts, static UI and
  one thin UI dylib, matching Platform's ownership model.
- Consolidate activation under `neocogi.ui` with typed runtime, node, GFX and Skin
  factory getters; disabled services return `NC_E_NOT_FOUND`.
- Move canonical UI/GFX/Skin/design values, typed clients and packaged schema/build
  support into `neocogi-ui-contracts`; feature-select concrete implementation
  modules in `neocogi-ui` without compatibility wrapper crates.
- Update examples, independent extensions, native/browser composition, archive
  qualification, tests and documentation for the three-package release.
- Retain Platform `0.2.0-alpha.2`, rs-math3d `0.21.0` and COM/IDL `0.1.0` APIs.

## Dependency migration: Platform 0.2 alpha.2 and rs-math3d 0.21

- Consume the three published Platform 0.2.0-alpha.2 packages and rs-math3d 0.21.0.
- Use canonical renderer-owned shader compilation, typed keyboard identities/actions,
  surface descriptors, binding sizes and blend equations. Remove the compiler DLL.
- Stage native artifacts and resources through the contracts build-support API.
- Use shared checked vector normalization, cross products, bounds and column packing.
- Author mesh vertices, indices and streamed camera uniforms through typed GPU buffers.


## 0.1.0-alpha.2

- Advance all workspace packages, exact internal requirements and provider
  descriptors to `0.1.0-alpha.2`.
- Restore wheel zoom routing for Suzanne and the grid, with proportional pixel
  deltas and checked camera limits.
- Reduce rendering overhead with checked 2D orientation, cached ear geometry,
  bulk vertex packing and exact glyph-coordinate caching.
- Use published Platform `0.1.0-alpha.2`, COM/IDL `0.1.0` and rs-math3d `0.20.1`.
  Retain streamed UI vertex/frame buffers and Suzanne's camera buffer across
  frames, upload synchronously and bind explicit uniform ranges. Keep immutable
  mesh/atlas data static; preserve draw snapshots and propagate bounded-slot Busy
  without allocating replacement buffers. Migrate native rendering and external
  generated-contract tests to the same published dependency set.

## 0.1.0-alpha.1

- Coordinate all workspace package versions at `0.1.0-alpha.1` and use exact
  internal registry requirements. Enable the reusable libraries, providers,
  provider DLLs and sample contracts for crates.io; keep application compositions
  and test/tool executables private.
- Package the canonical UI, GFX and Skin IDL sources in `neocogi-ui-build`.
  Consumers generate bindings from owned Cargo output directories without a
  sibling repository or a shared build script. Ship package-owned test inputs
  and license notices with explicit freshness checks.
- Qualify the actual normalized archives outside the checkout, including all
  optional features, generated COM calls, retained event ownership, rendering
  and independently loaded provider DLLs. See [releasing](RELEASING.md).

## Earlier development (0.0.0, unreleased)

- Consume all NeoCogi Platform packages from crates.io at exactly `0.1.0-alpha.1`.
  Use the `nc_platform` assembly and packaged build-time IDL sources. Bind UI/GFX
  groups to explicit surfaces with color/resource/image permissions and no depth.
  Preserve original rendering, binding and upload failures in owned operation
  reports through the UI and GFX plugins. Rebuild all pre-release peers together.

- Bound Combo dropdowns to a positive configurable visible-item count (default
  ten), further constrained by frame space, with vertical scrolling and one
  text line of preferred edge clearance. Share fitting, scrolling and scrollbar
  mapping with compact command/context menus and cascading submenus. Preserve
  complete models and wide content coordinates through viewport translation.
  Add explicit anchored general-popup placement, keeping authored ScrollArea
  bodies and fixed headers/footers under application control. The full demo's
  fruit selector demonstrates sixteen choices through an eight-row viewport.

- Use the published crates.io COM SDK, host and IDL compiler at exactly
  `0.1.0-alpha.1`, including standalone generated-binding fixtures. Record registry
  checksums in the lockfile, remove the SSH fetching requirement, and retain
  historical Git linkage validation alongside current registry selections.
  See [dependency updates](../guides/COM_DEPENDENCY.md).

- Link historical UI revisions to their exact companion COM revisions in Cargo
  declarations and commit trailers. Retain a complete UI/COM revision ledger and
  a read-only audit command. Use the UI identity throughout the repository and
  its history. See [UI and COM history](HISTORY.md).

- Establish the standalone `NeoCogi/neocogi-ui` library and UI examples with 1,411
  history commits. Keep renderer, resource, skin and runtime
  document dependencies; move runnable demos and shared example providers under
  `examples/`. Use the COM Git SDK at `4d8d7228efbcc400f63a016f9d04ee81f6f5baf8`
  and commit the resolved lockfile. See [UI and COM history](HISTORY.md).

- Replace the in-tree COM SDK/compiler/runtime and inspector with the extracted
  `NeoCogi/neocogi-com` Git source at `245701e39ad5cede0876d05c4da67c5ada986612`.
  Hosts now explicitly link `neocogi-com-host`; generated bindings, independent
  test consumers and plugins use the same revision. Keep UI regression schemas
  local and SDK qualification upstream. See [dependency updates](../guides/COM_DEPENDENCY.md).

- Consolidate Rust emission and implementation-macro expansion in
  `neocogi_idl::codegen`. Remove the standalone backend package; keep the thin
  procedural macro crate dependent on IDL's ordinary library. Generated COM
  contracts, ABI v1 and the host-owned global registry retain their semantics.

- Initialize the single COM registry in the host through `#[com_main]`; supply
  immutable service access to plugin acquisition and application libraries.
  Remove the COM runtime DSO/native-link bridge and global/default discovery,
  retaining the runtime as a host rlib. Core/runtime/plugin remain ABI v1.

- Consolidate the COM object/runtime/compatibility specifications into one
  ABI-v1 guide. Core, runtime and plugin
  discovery share the pre-release v1 baseline; IDL fingerprints and UI schemas
  retain separate version authorities. Rebuild every peer from the same checkout.

- Move event interfaces and transport from COM into UI. Replace IDL `#[events]`
  with Rust `ui_event_sources!`, backed by generic method metadata and UI-owned
  catalog validation. Advance module encoding to v13; preserve the single global
  shared COM runtime registry. Keep standalone COM examples independent of UI.

- Simplify event production to mutable `post(event)` with direct native update
  errors. Remove observation guards and the update context's saved error bridge;
  unobserved events are constructed and immediately discarded. Preserve committed
  geometry before fallible event work and stop further emissions on failure.
- Make `IActivatedEvent` version 2 an empty occurrence marker; the host's weak
  source envelope identifies commands without copying display labels. Add common
  immutable `IErrorEvent` status/message reports through that same queue and
  origin stack. Reporting is explicit and delivery failures still return statuses.
  The single shared COM runtime continues to own the global plugin registry.

- Complete the COM-only event migration: remove native widget output enums,
  conversion traits, generic constructor associations, product helper aliases and
  generated form dispatch scaffolding. Producers construct exclusive snapshots;
  handlers authenticate origins and read concrete event getters directly.
- Add explicit `#[events(...)]` IDL declarations and inherited getter metadata.
  Advance canonical IDL encoding to v9 and owned design descriptors to v12.
  Optional generated forms expose only checked fields; the shared document receiver
  preserves COM ownership and checks reload generation plus widget membership.
  These changes supersede the optional native decoding/form handler paths below.

- Replace per-widget event receiver interfaces with one `IEventReceiver` carrying
  exclusive `ComBox<IEvent>` snapshots and weak widget/service origins. Add typed
  immutable event getters, optional native payload decoding, and a common channel.
  Migrate producers, composites, forms, tools and plugin examples. Failed posts
  return the original event owner; missing event interfaces return `None` while
  ABI failures remain errors. Form generation and membership checks remain intact.
  This supersedes the concrete generic queue migration recorded below.
- Support recursive `WeakComBox` IDL value fields with checked exclusive ownership,
  canonical weak projections, and design descriptor version 11.

- Add `LocalCom::downgrade` and `WeakLocalCom<T>::upgrade` for construction-proven
  weak access to shared Rust implementations. Promotion preserves the existing
  allocation, threading restrictions and COM borrow gate; final strong release
  expires observers regardless of retained weak clones. UI runtime ownership is
  unchanged pending a separate evaluation.

- Support `WeakComPtr<I>` in IDL results and recursively borrowed record/enum
  fields, preserving interface identity, threading, expiration and weak ownership.
  Add checked weak result lowering and metadata without promoting producers.
  Design catalog schema 10 distinguishes weak fields from strong object fields.

- Rename application message behavior from `EventModel` to `MessageProcessor`
  and its inbox owner from `Controller` to `Dispatcher`. Typed local access uses
  `dispatcher.processor`; delivery remains synchronous and exclusively borrowed.
  `apply_pending_changes` retains its explicit `ApplyOutcome` contract.

- Share COM lifetime operations through one allocation control header. Remove
  per-class weak operation tables and generated counting forwarders. Weak handles
  are one pointer, strong handles remain two; typed destruction and deallocation
  stay in the allocating provider. The migration introduced provisional
  construction publication and rollback without exposing private Arc layout;
  ordinary initialized construction subsequently removed that protocol.

- Complete the concrete `IEventQueue<Object, Event>` migration across applications,
  composites, designer previews and test fixtures. Remove generated callback
  receiver classes, `mapped_queue`, per-event queue builders and sender/mapper
  constructors. Applications own one concrete sender-bearing COM queue per
  behavioral inbox; mutable command selection stays in ordinary model state.
  Reject retained old controls through current membership and preview generation.
  Cover module scaffolding/generation/build and real factory delivery through
  cargo-neocogi, including allocation-free contract builds. Earlier observer and
  callback entries below record superseded designs, not supported compatibility APIs.

- Make IDL collection ownership explicit with `Vec<T>`, `Slice<T>` / `[T]`, and
  `Array<T, N>` / `[T; N]`. Support recursively copied arrays in methods and
  buffers through checked wire codecs. Move `Option<T>` into an ordinary bundled
  generic enum, remove special optional lowering, and require borrowing the whole
  enum for owned payloads. Expose enum deserialization and conditional library
  decoder policies. COM designer catalog schema 5 uses named enum definitions
  for optional fields. See [the generic value contract](https://github.com/NeoCogi/neocogi-com/blob/4d8d7228efbcc400f63a016f9d04ee81f6f5baf8/docs/contracts/IDL_GENERIC_VALUES.md) for syntax,
  ownership, migration rules, and the remaining explicit limits.

- Reset all workspace packages and plugin release metadata to **0.0.0** for
  NeoCogi UI. The project was formerly **microui-redux** and has grown
  to include COM components and IDL tooling, shared libraries and plugins, a
  theme editor, and an experimental UI designer. Package versions remain
  independent of ABI, contract, and file-format version numbers.

- Make Go to Definition reach generated IDL Rust through `.include.rs` module
  entries, and preserve COM macro spans for derived types. Migrate contract
  includes and new-module templates; retain explicit IDL provenance in `.rs.map`.

- Emit readable `.rs.map` provenance beside Cargo-generated IDL bindings, and add
  IDL source annotations to generated declarations/members. Map formatted ranges,
  message helpers and observers to their authored origins without changing ABI
  identity. Expose typed lookup APIs and report the map through `module generate`.
  See [IDL source maps](../guides/IDL_SOURCE_MAPS.md) for scope and coordinate conventions.

- Generate one concrete observer and local event enum from an NCIDL `#[observer]`
  producer. Built-in controls, choice menus and file dialogs retain that observer
  during construction. Rust factory shims accept an mpsc sender and typed mapper;
  text changes and submissions share one allocation. Add explicit unobserved and
  prebuilt-observer constructors, checked nullable COM inputs, and generated named
  export forwarding for documents. Migrate demos, editors and composite children.
  Existing controller scheduling/`LocalCom` and window/menu/popup binding remain.
  This observer design was later replaced by [typed event queues](../contracts/CLASS_EVENT_STREAMS.md);
  the original implementation and measurements remain in Git history.

- Rename typed output wiring to `events::bind`. Use one persistent owned
  `Controller<Model>` and `IEventOwner` contract for application and composite
  behavior. Mount components with an explicit typed `Mount` view/owner pair;
  remove optional provider discovery and the borrowed scoped-dispatch bridge.
- Add construction-backed `LocalCom<T>` access through the existing COM borrow
  gate. Models use ordinary fields and `&mut self`; the file dialog no longer
  needs shared mutable request/model wrappers or a forwarding widget. Retire
  removed owners after active handlers return and preserve pending cleanup when
  a local guard temporarily blocks entry. Cache descendant-first delivery order.
- Accept explicitly borrowed marked messages in ordinary IDL parameters. Rebuild
  changed UI contracts and their consumers together. See the
  the historical owned-event implementation for dynamic and monolithic
  min-size comparisons, lifecycle tests and the idle-allocation regression.

- Consolidate nine COM production crates into five: move runtime link declarations
  into the facade, combine IDL parsing/generation/Cargo integration in
  `neocogi-idl`, and expose built-in event contracts and channel adapters through
  `neocogi_com::events`. Remove the runtime-sys, events, events-contracts, and
  idl-build packages. Runtime/shared-library ownership and C ABI signatures remain
  unchanged; `nc_com` remains the sole deployed COM runtime library.
- Add explicit crate-rooted Rust contract paths to source graph schema 5 so a
  logical IDL module can live inside a facade without changing its identity or
  fingerprints. Build scripts use `neocogi_idl::cargo::Module`; event consumers
  depend directly on `neocogi-com` with `macros` (plus `std` for channel adapters).

- Name the workspace **NeoCogi UI**, hosted at
  [`NeoCogi/ui`](https://github.com/NeoCogi/neocogi-ui), retaining
  descriptive `neocogi-*` Cargo packages and the `cargo neocogi` command.
  Deploy short library names (`libnc_com.so`, `libnc_ui.so`, `libnc_ui_nodes.so`,
  `libnc_render.so`, `libnc_gfx.so`, and `libnc_skin.so` on Linux). Generated
  component roots preserve authored Cargo library target names for both local
  and registry plugins. Direct Rust implementation imports use those short
  targets; public facades and ABI identities remain unchanged. Standardize
  demo, example, benchmark, and asset-tool executables under `neocogi-*` names.
  See the [library mapping](../guides/COM_NATIVE.md#profiles-and-output) and
  [executable mapping](../guides/EXAMPLES.md). Rebuild and restage native assemblies;
  old runtime filenames and plugin manifests belong to the previous build.

- Consolidate COM object persistence into core contracts, the shared runtime, and
  `neocogi_com::persistence`; remove the separate persistence implementation and
  contract crates. Keep recipe registries caller-owned, with mutable registration,
  shared restoration, and deterministic factory cleanup. `FactoryRegistry::new()`
  is now fallible. Preserve interface identities and NCB-v1 bytes, and use the
  same versioned persistence table in shared, monolithic, and browser builds.
  Rebuild hosts and runtime together for the added pre-alpha table getter.

- Add the concrete nodes `SplitView` container and typed facade for two panes,
  draggable/keyboard-adjustable dividers, minimum sizes, and user change events.
  Use it between the theme editor inspector and live preview. Core's existing
  layout and input protocols own routing; no editor drag state is required.
  `UiFactory` is now version 2; rebuild shared callers and providers together.

- Add the normal-theme UI theme editor with an independent live preview, complete
  typed parameter inspector, original-texture nine-slice canvas with four red
  guides/sliders, interactive control/layout/typography gallery, and canonical
  state sheet. Support inheritance, visual/whole-role copy, bounded gesture-based
  undo/redo, recoverable invalid drafts, per-resource pickers, remembered native
  folders, Open/Save/Save As dialogs, and portable asset collection. Commit native
  JSON after immutable resource copies; browser import/export supports complete
  project ZIPs as well as file/folder import. Ship shared native,
  monolithic native, and WebAssembly through the same application implementation.
- Version `IFileDialog` to 2 with Open/Save presentation and exact native filename
  suggestions. Filename Enter accepts a selection; applications own overwrite
  confirmation. Rebuild shared hosts and nodes providers together.
- Size editor confirmation/export prompts to content and align their action
  buttons with the file picker. Add `TextWrapping::WordOrCharacter` in version 2
  of the wrapping vocabulary so long overwrite paths remain visible without
  changing word-only behavior. Rebuild shared hosts and UI providers together.
- Extract `neocogi-skin-authoring` so editor, runtime loader, and offline builds
  share one document/compiler. Read theme schema versions 1 and 2 and write version
  2, retaining authored absence and resource paths, per-state insets, center
  visibility, nine flat colors, and independent image slicing/tint.
- Add renderer-owned immutable offscreen surfaces to Glow/WebGL, Vulkan, and WGPU.
  Keep independent atlas bindings and publish output only after successful frames.
  Embedded keyboard scopes retain Tab/menu navigation with Ctrl+Tab exit; preview
  routing cancels on outer modal exclusion, hiding, and native focus loss, and
  translates one native IME area through committed pane geometry.
- Version `IAssetBundle` to 4 with an explicit independent `image()` capability.
  Appearance-only revisions retain pixel identity and avoid full-image copying and
  atlas uploads. Resource rebuilds publish new images; metadata revisions retain
  no older bundle chain. Rebuild native hosts/plugins together for this contract.
- Replace Debug-derived editor labels with explicit UI names so native `min-size`
  builds retain section/palette/state/geometry/role labels with `-Zfmt-debug=none`.
  Add staged native, real browser, image-lifetime, routing, persistence, and
  documented software-rendering performance regressions.

- Give each UI runtime a private compositor created through the shareable GFX
  factory. Creating a second UI or installing its resources no longer replaces
  the first UI's renderer atlas while leaving its retained resource snapshot
  unchanged. `UiPluginFactory` version 3 takes `(assets, gfx)`; callers no longer
  allocate and pass a mutable compositor. Preserve the same contract in shared
  libraries, native monoliths, and WebAssembly, with no new production interior
  mutability or implementation dependency. Add production-path regressions for
  independent revisions, catalogs, failed uploads, closure, and frame state.

- Replace custom event queues and posting wrappers with `std::sync::mpsc`.
  Applications and composites own ordinary receivers; typed COM mappings hold
  senders and translate each concrete event into a local Rust message. Receiver
  destruction discards later delivery; mapping still runs before sending fails.
- Dispatch exactly one message per COM visit and discover nested component drivers
  automatically. The runtime owns descendant-first scheduling, frame boundaries,
  retirement checks and one finite update budget. Reaching the exact ceiling stops
  before another dequeue; unattempted work remains for the next update.
- Declare published outputs as concrete `#[message]` data with named
  `name: Message;` bindings. Producers own one optional typed receiver, installed
  by an ordinary mutable setter. Remove output projections, subscriptions, leases,
  generations and support-version negotiation. Both deployment ABIs remain v1.

- Simplify contracts to compile-time Rust bindings and transitive signatures;
  remove native contract libraries/archives, module getters/registries, and
  intrinsic-event seeding. Plugins remain dynamically loadable or statically linked.
- Unify callable declarations as interfaces and factories. Keep typed
  factory construction, required sibling projections, exact signatures, and
  mutable call gating; remove duplicate object descriptions and ordinary unit
  objects' generated zero-argument constructors.
- Keep plugin descriptor and runtime table ABIs at v1 during pre-alpha, with
  matching exported symbols and Rust types; use inspection schema 9.
  Reject incompatible factory signatures before bootstrap. Add an ordinary-Cargo
  sample exercising both dynamic deployment and a self-contained monolith.
- Remove an unused runtime import and the demo's warning-only build script so
  valid workspace checks do not report deployment guidance as a build warning.


The application API now consists of a thin static facade over actual COM objects.
This replaces the 0.8 native node/weak-handle authoring API without compatibility
wrappers. [COM_DESIGN.md](../architecture/COM_DESIGN.md) defines the accepted architecture, and
[COM_AUTHORS.md](../contracts/COM_AUTHORS.md) defines its current author-facing contracts.

- Replace the caller-coordinated combo header with a runtime-owned composite. Combo construction
  now registers a compact choice menu through the generic popup API under an explicit window
  owner, while a separate composition manager keeps combo semantics outside the root forest. It
  represents selection with the single NCIDL-generated `ChoiceSelection`, supplies the visible
  `Select…` fallback by default, rejects unknown stable leaf IDs instead of clamping, and atomically
  replaces its generated preorder hierarchy with an explicit optional selection. The same choice
  menu supports relational submenus without `IComboBoxes`, a combo-only popup widget, or a second
  handwritten contract structure; opening restores the branch path containing a nested committed
  leaf.
  Popup hover, keyboard candidate, and committed selection are separate typed states, so opening
  starts on the committed item and pointer departure restores it without a blank frame. The popup
  uses the header's theme-derived outer border box as its fixed horizontal extent, aligning both
  edges without counting either control's frame twice.
- Deploy concrete built-in nodes and standard compositions through their own
  `neocogi.ui.nodes.runtime` plugin. Core runtime activation now publishes only
  `neocogi.ui.core`; the separately activated `NodesPluginFactory` binds to a `UiRuntime` through
  generated public capabilities and returns the nodes `UiFactory`. `Ui::from_interfaces` validates
  the canonical host identity, file dialogs dispatch through mounted component event providers,
  and Combo delegates hierarchy, popup policy, selection, and its generated changed event to the
  core `IChoiceMenu`.
- Complete the physical UI source split: move concrete widgets, containers, factory, file picker,
  and node phase adapters into `crates/ui/nodes`. Keep retained topology, scoped service
  implementations, traversal, and GFX submission in `crates/ui/runtime`; both implementation
  crates now own their actual plugin roots. Remove the shared `neocogi-ui-impl` arrangement and
  redundant `runtime-plugin` wrapper. `neocogi-ui-support` contains only binary-local canonical
  resource imports, pure geometry/text algorithms, and phase-local recording helpers, not either
  provider's implementation. Neither provider has a production Rust dependency on the other.
  Move useful node/picker and pure-helper tests to their production owners, eliminate private
  core storage from picker fixtures, and verify independent activation, host authentication, and
  real event delivery. Replace the scoped layout client's first-error `Cell` with an ordinary
  field mutated through `&mut self`, and reuse NCIDL `LayoutParticipation` instead of its parallel
  native enum.
- Repair full-demo browser startup by explicitly linking/registering the live Skin compiler that
  its style editor requires, without adding the native JSON loader. Preserve native monolithic
  JSON support as an explicit feature. Report WASM startup errors to the browser console, retain
  the page's failure status when initialization publishes no canvas, and remove ripgrep from the
  web build/test scripts. Browser full-demo coverage now inspects the actual rendered framebuffer
  as well as production theme/image/action and teardown behavior.
- Retain the `AssetTables` collection of generated NCIDL records as the UI runtime's canonical immutable resource
  snapshot. `AtlasHandle` now adds only catalog-scoped identities and sorted integer lookup slots;
  `Skin` borrows generated metrics and chrome policy and selects generated appearance fields
  directly. Remove the parallel font/glyph/icon records, native chrome and appearance catalogs,
  UI-only atlas validator, conversion layers, and test export/reimport machinery.
- Resolve external JSON themes into one typed `ThemeRecipe` and execute every resource through
  the GFX `AssetLoader` and `AssetPublisher`. Offline generation statically instantiates those same
  services instead of using a second Skin-owned native builder loop. Preserve bounded one-at-a-time
  reads, injectable sources, canonical ordering, and owned path/status/GFX diagnostics; verify exact
  offline and plugin-facing output parity.
- Use `nc_com_shared_runtime` as the runtime library name in direct builds,
  native bindings, and generated LTO roots. Assemblies deploy one runtime library
  without the former duplicate filename. The Cargo package remains `neocogi-com-runtime`.
- Document dynamic runtime feature unification as the mechanism that routes
  every facade to that shared object's single registry, while retaining
  `static-runtime` only for one monolithic image. Reconnect linked-runtime build
  checks to the current factory registration and dynamic plugin tests.
- Import and maintain neocogi-com locally: checked recursive interface identities,
  borrowed clients, generated IDL class/factory descriptions, required shared
  protocols, typed factory construction, static runtime binding, and explicit
  dynamic factory-plugin exports.
- Stop inspecting Cargo manifests during COM macro expansion. Generated code uses
  the canonical `::neocogi_com` path or an explicit facade override, removing
  `proc-macro-crate` and its TOML parser from application and plugin builds.
- Make SHA-256 ABI signatures unconditional and add an optional native
  monolith. The monolith uses one statically linked runtime and directly
  registers the linked UI, renderer, and asset descriptors with the same exact
  validation as a dynamic assembly.
- Compute module fingerprints once in the IDL build tool, derive distinct
  declaration signatures, and embed their final 32 bytes in generated Rust.
  Remove compiler-side signature serialization and const SHA-256 so rustc only
  compiles typed metadata and compares precomputed identities.
- Move the reusable IDL compiler, Cargo build integration, and compile-checked
  fixtures from runtime-oriented `crates/idl` into `tools/idl` beside the
  assembly and inspection tools.
- Add one-Cargo-crate-per-module Rust-like IDL generation. Dynamic assemblies
  stage fingerprint-qualified contract libraries and factory-only plugins;
  loading and inspection reject conflicting revisions before publication.
- Generate standalone descriptions as self-contained `no_std` Rust instead of
  C. Hosts and plugins retain only typed bindings and exact getter imports, so
  each fingerprinted description library is mapped once per process.
- Keep native contract descriptions metadata-only. Contract libraries and
  schema-v8 inspection output retain identities, fingerprints, declarations,
  and dependencies without embedding or exposing the original `.ncidl` source.
- Split optional asset authoring into the `neocogi.ui.assets` contract module.
  Its generated factories reuse `Size`, `AssetMetrics`, `FlatPalette`, native
  paths, bundles, and texture interfaces from `neocogi.ui.core`, producing a real
  fingerprinted native dependency instead of duplicate cross-module types.
- Split renderer-neutral geometry, image, texture, diagnostics, and bulk frame
  declarations into `neocogi.renderer`. Asset bundles expose only the derived
  `IRgbaImage` capability to renderer implementations, and the UI caches that
  projection so frame submission neither leaks skin metadata nor repeats interface
  discovery.
- Split normalized input, native-window polling, and IME placement into
  `neocogi.platform`. Platform construction now consumes `IRgbaImage`, allowing
  platform/renderer plugins to omit the UI core contract and its complete widget,
  skin, and asset descriptor surface.
- Route the browser DOM input adapter through `neocogi.platform` directly as
  well, so WebGL builds enforce the same no-UI-core renderer dependency checked
  by native backend builds.
- Split the demo mesh protocol into `neocogi.demo.render`; the renderer plugin's
  normal dependency graph no longer reimports UI core through the unrelated
  widget-extension demonstration.
- Replace the mixed `neocogi.ui.assets` service surface with deployable
  `neocogi.gfx` and `neocogi.skin` plugins. GFX owns PNG/resource construction;
  skin owns typed appearance and external-theme compilation. Optional facade
  features prevent either contract from entering unrelated UI/plugin graphs.
- Move portable bundles and their font, icon, metrics, and appearance records
  from UI core into `neocogi.gfx`, and move immutable native paths into
  `neocogi.platform`. UI and Skin now consume lower-level resource contracts;
  GFX no longer imports the retained UI descriptor merely to build an atlas.
- Extract portable bundle publication, validation, embedded expansion, and
  concrete appearance-state helpers into non-plugin `neocogi-gfx-support`.
  The GFX implementation no longer imports the UI Rust facade; UI retains only
  compatibility re-exports for application source stability.
- Select optional plugins per assembly: simple, calculator, and backend-frame
  packages no longer stage or monolithically link Skin, while full and smoke
  packages retain it for external-theme coverage. Demo asset facade features
  likewise include the Skin contract only for the full theme-selection surface,
  and assemblies without Skin no longer copy unused theme directories.
- Keep contract traversal state changes outside debug-only assertions, allowing
  optimized assembly inspection to accept the shared Renderer dependency reached
  through UI, GFX, and Platform without falsely diagnosing a cycle.
- Deduplicate ABI-identical native contract artifacts emitted under the host and
  compatible custom `min-size` Cargo target labels. Embedded builds can use host
  build dependencies without turning one descriptor library into a false conflict.
- Give Skin explicit typed GFX `AssetLoader` and `AssetPublisher` capabilities and delegate
  bounded font, PNG, and atlas operations to it. Skin no longer statically compiles
  a second copy of GFX's font rasterizer, decoder, packer, bundle reader,
  validator, or publisher. Both JSON themes and live style edits submit complete
  appearance tables through a generic GFX revision transaction. Live style
  compilation retains only the always-available publisher, so embedded GFX omits
  native font and atlas construction while resource work remains a coarse loading
  operation outside every frame.
- Split the former shared asset implementation package into independent GFX and
  Skin implementation crates plus a small pure appearance compiler. Cargo no
  longer unifies decoder/builder and JSON/schema features into one compilation
  unit or relinks both fat-LTO plugins after an unrelated implementation edit.
- Move atlas lookup, glyph and icon expansion, final rectangle/triangle clipping,
  batch construction, scoped render-stream ownership, and the single renderer
  submission from the deployable UI implementation into a typed GFX compositor.
  UI retains semantic display-list construction and crosses the boundary once per
  draw operation, never once per glyph or vertex. Generic native executor fixtures
  remain test-only and no longer enter the UI runtime shared object. Exact custom-target
  `cargo bloat` measurements show UI `.text` falling by 10.6 KiB, GFX `.text` growing
  by 13.9 KiB, and the complete executable/shared-object set growing by 4,920 bytes;
  the architectural split is deliberately not presented as a total-size reduction.
- Replace the compositor's raw atlas-rectangle call with one closed `NinePatchDraw`.
  UI now retains and replays one semantic operation while GFX alone resolves image
  sources, partitions the destination and source grids, filters empty cells, clips
  final triangles, and merges their atlas batch. UI tests inspect semantic patches;
  compositor-stream tests verify the physical geometry at its owning boundary.
- Compile generated Rust contract descriptions for the underlying platform ABI
  when the `min-size` build uses its fingerprint-distinct custom target.
- Move portable appearance roles and semantic font/icon roles into
  `neocogi/gfx/resources.ncidl`, giving their names, representations, and table
  order one generated source of truth. Concrete runtime states remain Rust-only
  GFX support values and never become wire discriminators.
- Remove implementation-only widget state and window-chrome models from the UI
  contract crate. Widget state belongs to UI implementation support, runtime
  chrome state belongs to the runtime, and JSON chrome documents belong to the
  asset loader; only `AssetChrome` crosses their contract boundary.
- Use generated IDL `class` descriptions with `#[com_object(Class)]` and IDL
  `factory` descriptions with `#[com_factory(Factory)]`. Descriptions own public
  identity/threading/projections; implementations own private state and bodies.
- Remove runtime class tables, class-ID construction, constructor pointers and
  erased argument frames. Typed factory methods are the only public construction
  path.
- Replace the redundant `IButton` and `IButtonStyle` capabilities with the
  factory-created `Button` IDL class description. Its concrete editing,
  activation, font, icon, fill, and image operations now share one class
  projection; only the reusable `IWidget` and `ILeafLayout` protocols remain.
- Make Checkbox, Scrollbar, and Combo IDL class descriptions as well. Their
  concrete editing and event methods no longer require matching one-object
  interfaces, while their callback sinks remain reusable COM protocols.
- Make Textbox, TextBlock, and the fixed TextArea composition IDL class
  contracts. Text, cursor, wrapping, typography, and editor subscriptions now
  live on those concrete class projections; TextArea retains only shared
  scrolling and container-layout protocols.
- Make Slider and Number IDL class descriptions. Their value editing,
  typography, and numeric subscriptions now live directly on the concrete class
  projections instead of three overlapping numeric interfaces.
- Make ColorSwatch, ListItem, and ListBox IDL class descriptions. Label,
  typography, icon, image, and activation operations now live directly on those
  class projections; the single-use icon and image interfaces are removed.
- Make Linear and Grid IDL class descriptions. Their factory methods, topology
  edits, and relationship policies now share the concrete class projection;
  only runtime-facing `IContainerLayout` remains required.
- Make ScrollArea and Disclosure IDL class descriptions. Their fixed
  compositions retain shared scrolling/container protocols while content,
  expansion, header, and body access live on the concrete class projections.
- Make the always-present PngDecoder and StyleCompiler asset roots IDL classes;
  diagnostics remain a reusable interface on both objects.
- Make the optional AssetLoader and ThemeLoader roots IDL classes as well;
  their mutable build sessions and immutable result bundles remain interfaces.
- Replace the optional extension assembly's one-object provider interface and
  separate class ID with an `ExtensionProvider` IDL class; its custom
  `IInset` result remains a reusable container capability.
- Replace the registered platform root's `IPlatform` plus separate class-ID
  declaration with a `Platform` IDL class. Renderer diagnostics
  remain a shared capability on the same native bootstrap object.
- Replace `IUiFactory` with an IDL `UiFactory`. The nodes plugin's generated root binds a
  host-scoped instance to an independently created core runtime; its typed methods construct
  `Button` and other classes.
- Store the runtime's current imported assets as one shared value instead of a
  wrapper around nested reference-counted snapshots. Installing a replacement now
  requires a mutable `UiRuntime` receiver, making runtime-wide mutation explicit.
- Store each independently constructed widget's imported resources directly as
  one `Rc<RefCell<ImportedAssets>>`; remove its redundant revision and resource
  wrappers and borrow that sole value during semantic phases.
- Give the dynamically linked `UiFactory` one shared imported-asset cache sourced from its bound
  runtime host and pass it into every product and fixed internal child. Factory construction and
  stable widget phases perform no repeated portable-bundle imports; revision changes refresh that
  provider-local cache through the public host capability.
- Separate built-in widgets, containers, Combo presentation, and standard file dialogs from the
  core retained runtime and window-manager provider. Keep each renderer and its OS/device owners
  together in the platform plugin; the independently built extension demonstrates a third UI
  provider using the same public node protocols.
- Keep common lifecycle/options/invalidation in WidgetState, authoritative child
  ownership behind scoped layout services, and semantic widget state concrete.
  Native authoring/closure-renderer support remains only in behavior fixtures.
- Make every registered built-in the concrete Rust widget allocation itself.
  Remove the temporary `*Object` wrappers and generic `WidgetCore`; the remaining
  `WidgetRuntime` contains only common lifecycle, resource, and rendering support.
  Composed controls retain real structural child objects rather than duplicate
  parent implementations.
- Remove the UI implementation's generic `src/com` tree. Retained-node services now
  live under `ui_node`, external renderer submission under `render`, event sink
  projections beside their semantic event types, and the dynamically linked runtime
  and factory in their owning core/nodes crates. Provider-neutral resource/presentation helpers
  live in `neocogi-ui-support`, without concrete object types or retained runtime machinery.
- Share one typed EventSource transport between built-ins and independent widgets;
  EventSession owns app state and concrete handlers outside the producer graph.
- Preserve menus, native-path file dialogs, immutable resources, full/basic demos,
  browser input/IME and ordered typed image/mesh/custom drawing.
- Split `IPopup` placement into `show()` at the latest consumed pointer and
  `show_at(anchor)` for exact screen-space placement. The full demo's Popup
  button again opens its standalone menu at the triggering pointer position.
- Render enabled hovered and pressed Mac OS 9 item controls with white content
  over the Platinum theme's black selection face. Combo/list choices and tree
  disclosures now retain readable selection contrast before receiving focus.
- Load native demo assets from adjacent files through the asset plugin by default.
  `embed-assets` explicitly selects build-host compilation and embedded resource
  bytes; browser `webgl` enables it. Generated COM bundles retain bounded pixel expansion.
- Add the installable `cargo-neocogi` command for configured debug, release, and
  nightly min-size assemblies. It resolves local/registry component sources through
  Cargo, discovers artifacts from JSON, validates plugin load order, and preserves
  the last assembly on failed updates. Package/workspace metadata and standalone
  TOML support named applications, renderer variants, resources, and lock policies.
- Resolve the asset generator's host facade only for `embed-assets`, and keep demo
  runtime default features consistent between assembly and application builds.
- Build compatible plugins and applications in one Cargo feature graph, with an
  explicit isolation option. Generated pure cdylib roots receive profile LTO while
  reusing Rust implementations. Preserve application lock selections and workspace
  source overrides, and report actual compilation versus cache hits.
- Remove the native 8 ms frame delay, permit transactional theme installation from
  event callbacks, and flush preceding atlas geometry before image draws.
- Give `IPainter` exclusive mutable receivers and keep its invocation counter,
  active scope, and optional frame context as ordinary fields in the concrete
  recorder. Built-in semantic frame, icon, text, and image operations now append
  to the runtime's single display list without per-widget lists or renderers.
- Skip discarded fixed-grid measurements and retain paint/text/custom-geometry
  buffers. Remaining native throughput and deployment-size overhead are recorded
  at the named historical source revision.

See [author contracts](../contracts/COM_AUTHORS.md) for explicit external-state, skipped-update,
painting, lifecycle and callback responsibilities. Resize and retained setter
invalidation remain independent of a widget's ordinary update return value.

## 0.8.0

Version 0.8 is a deliberate retained-API redesign. It unifies application authoring, input,
events, surfaces, menus, and rendering around uniquely owned retained values. Superseded alpha APIs
and compatibility layers were removed.

### Retained authoring and events

- `Node` is the sole mountable owner of a widget or container. Built-in constructors return a typed
  weak handle beside that node for later state access.
- Concrete widgets now own their semantic state, typed event ports, measurement, update, and paint
  behavior. `LeafWidget` and `ContainerWidget` define the two retained layout roles.
- `Context` owns input ordering, surface lifetime, rendering, and the application event dispatcher.
  `update_ui` and `update_ui_state` are the synchronization boundary for input and programmatic
  mutations.
- `WidgetEventPortHandle<E>` connects through one typed `subscribe(port, callback)` API; callbacks
  receive application state, `Ui`, and the concrete event after retained widget borrows end.
- `Widget::update` reports `Change::Changed` or `NoChange` for geometry inputs. Context updates
  automatically commit layout; shared state, update eligibility/order, and observational painting
  remain explicit widget/application author responsibilities.
- Custom containers receive an opaque weak `ChildrenHandle` through `bind_children`. Checked edits
  preserve rejected owners and automatically invalidate geometry. Standalone widgets implement
  `From<Parameters>`; redundant builder marker traits and stateless builders were removed.
- `FileDialog` is an application-owned component with native-path requests/results and typed
  completion. Directory reads are transactional and fallible; hidden or destroyed dialog surfaces
  settle pending requests through the same event dispatcher.

### Surfaces, menus, and input

- One `SurfaceForest` owns windows, structural child windows, dialogs, application popups, and menu
  popups. Parent edges define lifetime and one active popup path defines transient visibility.
- Stable `WindowHandle`, `PopupHandle`, and `MenuItemHandle` capabilities reject destroyed, stale,
  or foreign targets. Failed owned-surface creation returns its unchanged input for retry.
- Structural child windows inherit their family layer and may clip their complete output to the
  parent's application content.
- Declarative menu bars and standalone menu popups compile directly into manager-owned menu
  surfaces. Checked item access supports live labels, enabled state, shortcut hints, and check/radio
  marks.
- Logical key events, persistent focus, wrapping Tab traversal, Ctrl+F6 window cycling, popup Escape
  handling, and F10/Alt menu navigation now share one backend-neutral keyboard contract.
- Forest key lookup, child adjacency, inherited policy, and visibility are derived from its single
  authoritative node vector. Normal geometry phases reuse that projection instead of rescanning
  ancestry and all potential children for each surface.
- Native/browser hosts forward explicit cancellation and committed text. Browser wheel units,
  button chords, pointer capture, and composition use one owned input bridge with documented limits.

### Rendering, themes, and widgets

- `Painter` records backend-neutral work into a display list. `RendererBackend::Frame<'a>` provides
  one exclusive submission frame, and typed custom-render callbacks receive that selected frame
  directly.
- `RendererFrame::finish` explicitly returns recording/submission/presentation errors; dropping a
  frame abandons pending work. Native surface backends retain their SDL window owner.
- Glow drawing shares UI state restoration and upload preparation. WGPU validates device texture
  limits and resolves error scopes before replacing retained resources. Vulkan uses Ash's native
  constructors directly, and unused renderer code no longer hides behind blanket warnings.
- Skin installation requires all application catalog IDs in the candidate atlas; compatible
  derivation preserves IDs across allocation changes. Slider setters stay silent and do not snap
  programmatic values during later eventless updates.
- `ContextFrame::render_ui` is paint-only and rejects missing, stale, or dimension-mismatched UI
  commits before acquiring a backend frame.
- `Skin` provides typed role/state visuals, stable font and icon references, configurable window
  chrome, and atomic skin/atlas replacement. Strict JSON themes compile before atlas construction.
- Original Windows 3.11, Windows 95, and Mac OS 9 themes are bundled and selectable in the full demo.
- A WebGL 2 canvas renderer, browser-native animation loop, local build/server script, and GitHub Pages
  workflow make the complete demo available in the browser.
- Public retained scrolling now includes `Scrollbar`, `ScrollArea`, and a scrollable `TextArea`.
- Demo atlases use one recipe for runtime construction, normal Cargo host generation, and export.
  The stale external-atlas metadata path and nested Cargo build were removed.
- The examples and guides were updated for the 0.8 contracts. Standalone guides now participate in
  doctesting, with complete Rust examples compiled during documentation tests.

## 0.7.0 — 2026-05-25

Version 0.7 completed the first retained migration by moving root lifetime, interaction identity,
and frame traversal into `Context` instead of requiring applications to resubmit every root each
frame.

- Applications registered windows, dialogs, and popups in `Context`, then controlled visibility,
  options, and explicit destruction through retained handles.
- Private runtime identity and typed widget events replaced public interaction lookup.
- Widget execution was split into explicit measure, update, and paint phases.
- Retained layout gained weighted sibling sizing, fractional sizing, scroll areas, and corrected root
  chrome, popup, dialog, and resize behavior.
- Rendering was unified behind `Painter`, `DisplayList`, `Renderer`, and `RendererBackend`; external
  textures and atlas inputs gained stricter validation.
- Glow, Vulkan, and WGPU examples adopted the same retained-root lifecycle.

## 0.6.x

Version 0.6 introduced retained `WidgetTree` authoring on top of the earlier per-frame root loop.

- Retained trees replaced immediate closure-based widget authoring.
- Widgets gained persistent state, measurement, typed events, and custom painting.
- Custom-render leaves received geometry and input for application rendering.
- Atlas construction and generated/external data gained multi-font support.
- Version 0.6.1 made runtime atlas construction the demo default and prebuilt embedding opt-in.

## 0.5

- Widget identity moved to pointer-based IDs and explicit stateful widget instances.
- Layout was split into a shared engine and specialized row/stack flows, including directional
  stacks and weighted or fractional sizing.
- Text editing, scrollbar behavior, file-dialog layout, and PNG decoding were corrected.
- Container styling used shared `Rc<Style>` values and window chrome state moved into `Window`.
- WGPU example rendering and custom graphics demos were added.

## 0.4

- Stateful built-in controls replaced the legacy immediate widget calls.
- Widget measurement and input were routed through a shared execution context.
- Persistent widget identity tracked focus and hover.
- Dialogs, file-dialog interaction, atlas handling, and scrollbar tests were expanded.
- Styles became shared across containers and panels.

## 0.3

- Standard-library collections and parsing replaced the earlier custom equivalents.
- Container layout, clip stacks, and command recording were redesigned.
- Atlas building, generated Rust data, external PNG data, images, and multiple fonts were added.
- Custom drawing and 3D pass-through rendering gained mouse, keyboard, text, and drag input.
- Dialogs and the file dialog were introduced.
- The calculator, simple example, and full rendering demo established the example suite.

- Unify IDL records/enums as recursively checked values and remove `#[message]`.
  Separate application storage, raw wire fields, and typed producer guards;
  check bool bytes and selected enum payloads in copied slots, borrowed values,
  and buffers. Reject inline recursion, allow explicit recursive sequences, and
  share byte/work/depth budgets through nested conversion. Preserve zero-copy
  numeric buffers only through an explicit layout/content proof. Advance core
  ABI to 8 and canonical IDL encoding to 3; rebuild all components together.
- Transport complete structured value definitions, named recursion, and enum tags
  through COM designer catalog schema 4. Keep one stream/one lifetime consumer
  and explicit application handlers; values no longer manufacture receivers.
  See [the recursive value contract](https://github.com/NeoCogi/neocogi-com/blob/4d8d7228efbcc400f63a016f9d04ee81f6f5baf8/docs/contracts/RECURSIVE_VALUE_CONTRACT.md) for the supported positions and validation evidence.
