# Changelog

## 0.9.0-alpha.0 (unreleased)

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
wrappers. [COM_DESIGN.md](COM_DESIGN.md) defines the accepted architecture, and
[COM_AUTHORS.md](COM_AUTHORS.md) defines its current author-facing contracts.

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
  in COM_PERFORMANCE.md.

See [author contracts](COM_AUTHORS.md) for explicit external-state, skipped-update,
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
