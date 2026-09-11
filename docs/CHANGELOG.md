# Changelog

## 0.9.0-alpha.0 (unreleased)

The application API now consists of a thin static facade over actual COM objects.
This replaces the 0.8 native node/weak-handle authoring API without compatibility
wrappers. The accepted migration and outstanding validation are recorded in
[COM_MIGRATION.md](COM_MIGRATION.md).

- Use `nc_com_shared_runtime` as the runtime library name in direct builds,
  native bindings, and generated LTO roots. Assemblies deploy one runtime library
  without the former duplicate filename. The Cargo package remains `neocogi-com-runtime`.
- Import and maintain neocogi-com locally: checked recursive interface identities,
  borrowed clients, callable class contracts with typed constructor frames,
  class-required shared protocols, generic owned local classes, static runtime
  binding and explicit dynamic plugin exports.
- Stop inspecting Cargo manifests during COM macro expansion. Generated code uses
  the canonical `::neocogi_com` path or an explicit facade override, removing
  `proc-macro-crate` and its TOML parser from application and plugin builds.
- Add assembly-wide COM validation selection (`none`, SHA-256, FNV-1a-128, or
  collision-free mangling) and an optional native monolith. The monolith uses
  unchecked validation, one statically linked runtime, and direct registration
  of the linked UI, renderer, and asset descriptors.
- Record the selected COM validation mode in core ABI v7 plugin descriptors and
  schema-v5 inspection artifacts. Dynamic assembly planning now rejects plugins
  built with different modes before staging `plugins.txt`.
- Reserve `#[com_class]` for callable contract traits and use
  `#[com_object(Class)]` or `#[com_object(interfaces(...))]` for concrete Rust
  implementations. Class declarations therefore describe the public boundary,
  while object declarations describe private storage and threading.
- Remove the older implementation-owned `class_id`, name, version, constructor,
  and interface-list descriptor grammar. A registered object's complete public
  boundary now has exactly one source: its callable class contract.
- Replace the redundant `IButton` and `IButtonStyle` capabilities with the
  constructible `Button` class contract. Its constructor and concrete editing,
  activation, font, icon, fill, and image operations now share one class
  projection; only the reusable `IWidget` and `ILeafLayout` protocols remain.
- Make Checkbox, Scrollbar, and Combo registered class contracts as well. Their
  concrete editing and event methods no longer require matching one-object
  interfaces, while their callback sinks remain reusable COM protocols.
- Make Textbox, TextBlock, and the fixed TextArea composition registered class
  contracts. Text, cursor, wrapping, typography, and editor subscriptions now
  live on those concrete class projections; TextArea retains only shared
  scrolling and container-layout protocols.
- Make Slider and Number registered class contracts. Their value editing,
  typography, and numeric subscriptions now live directly on the concrete class
  projections instead of three overlapping numeric interfaces.
- Make ColorSwatch, ListItem, and ListBox registered class contracts. Label,
  typography, icon, image, and activation operations now live directly on those
  class projections; the single-use icon and image interfaces are removed.
- Make Linear and Grid registered class contracts. Their constructors, topology
  edits, and relationship policies now share the concrete class projection;
  only runtime-facing `IContainerLayout` remains required.
- Make ScrollArea and Disclosure registered class contracts. Their fixed
  compositions retain shared scrolling/container protocols while content,
  expansion, header, and body access live on the concrete class projections.
- Make the always-present PngDecoder and StyleCompiler asset roots callable
  classes; diagnostics remain a reusable interface on both objects.
- Make the optional AssetLoader and ThemeLoader roots callable classes as well;
  their mutable build sessions and immutable result bundles remain interfaces.
- Replace the optional extension assembly's one-object provider interface and
  separate class ID with a callable `ExtensionProvider` class; its custom
  `IInset` result remains a reusable container capability.
- Replace the registered platform root's `IPlatform` plus separate class-ID
  declaration with a callable `Platform` class contract. Renderer diagnostics
  remain a shared capability on the same native bootstrap object.
- Replace `IUiFactory` with a separately registered `UiFactory` class. The
  runtime returns a host-bound instance for convenience, while hosts can construct
  `Button` and other registered classes directly from typed argument frames.
- Store the runtime's current imported assets as one shared value instead of a
  wrapper around nested reference-counted snapshots. Installing a replacement now
  requires a mutable `UiRuntime` receiver, making runtime-wide mutation explicit.
- Store each independently constructed widget's imported resources directly as
  one `Rc<RefCell<ImportedAssets>>`; remove its redundant revision and resource
  wrappers and borrow that sole value during semantic phases.
- Give the dynamically linked `UiFactory` the runtime's existing imported asset
  handle and pass that same handle into every product and fixed internal child.
  Factory construction and widget phases perform no repeated portable bundle
  imports; directly activated classes retain their host-refreshing cache.
- Co-package built-in widgets, containers and window-manager policy in the UI
  plugin; keep each renderer and its OS/device owners together in the platform
  plugin. An optional asset plugin and independently built extension demonstrate
  additional deployment boundaries.
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
  and factory in their top-level modules. The plugin registry imports concrete object
  types directly from the `neocogi-ui-runtime` crate root.
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
