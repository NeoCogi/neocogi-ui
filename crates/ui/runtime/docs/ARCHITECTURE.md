# Architecture

Applications link `neocogi-ui`, canonical `neocogi-ui-contracts`, and neocogi-com support.
The facade contains concrete typed clients and application-local generic adapters.
The UI implementation, retained window manager, built-in widgets and layout policies
are packaged in the UI plugin. Native platform and GPU objects share the selected
renderer plugin. PNG decoding and optional font/theme construction belong to the
asset plugin. Several COM classes can share one assembly; a class is not a DLL.

`Button: IWidget + ILeafLayout` is a callable class contract. One COM allocation
contains its private state, constructor, concrete editing methods, and shared
protocol projections. Hosts construct the registered class from a typed argument
frame; they do not first invent or discover an `IButton` companion interface.
Generic Rust values stay local; every cross-plugin value has a declared ABI type
and checked identity.

Every built-in follows the same allocation rule. The registered Rust type is the
type that performs its input, layout, and paint algorithms; there is no parallel
`ButtonObject`, `WidgetCore<Button>`, native trait object, or `Any` payload.
`WidgetRuntime` is only embedded common lifecycle/resource machinery and cannot
hold a semantic widget. Fixed compositions may own genuine child COM objects for
different structural roles, such as a `TextArea` root and its `TextEditor` child.

`UiFactory` is a separately registered, dynamically implemented class retaining
only `IWidgetHost`. `UiRuntime::factory()` returns that shorthand for ordinary
application ergonomics. It is not an `IUiFactory` projection on the runtime, and
hosts remain free to construct any registered widget class from its typed frame.

## Source ownership

`neocogi-ui-contracts` declares cross-binary interfaces and callable class contracts;
it contains no UI implementation state. In `neocogi-ui-runtime`, each built-in under
`ui_node/widgets` or `ui_node/containers` owns its `com_object`, constructor,
callable-class implementation, shared protocol projections, semantic fields, and
layout/paint algorithm in one family module. A change to `Button`, `Grid`, or
`ScrollArea` therefore has one implementation source rather than a native type plus
a second COM-side representation.

`neocogi-ui-runtime` has no generic `src/com` implementation tree. Shared machinery follows
its actual owner: node mounting, child transactions, input conversion, widget host
state, scoped layout, and scoped painting live under `ui_node`; external frame
submission lives under `render`; event projections live beside their semantic event
types; and `Runtime` plus `UiFactory` live in their top-level implementation modules.
The plugin registry imports those concrete object types directly from the crate root.

The runtime, retained nodes, built-in leaves and containers, surfaces, and window
manager intentionally remain one Rust crate and one deployed plugin. Their source
modules separate responsibilities without introducing internal component calls or
crate boundaries into recursive traversal. This keeps ordinary Rust calls and
compiler-visible concrete code on the hot path while preserving COM only where an
independently built object actually crosses the platform boundary.

The small private `boundary` module contains only representation-free UTF-8
validation and geometry copies reused by multiple domains. It owns no objects,
runtime state, registration catalog, or alternate implementation representation.

## External nodes and cached dispatch

External leaves and containers use exactly the same retained-node representation
as built-ins. The class registry participates only when an application asks the
COM runtime to construct a class. When a widget is attached, `NodeFactory` verifies
its `IWidgetHost`, queries `ILeafLayout` and `IContainerLayout` once, rejects an
object that exposes both or neither layout role, authenticates a container's child
view, copies its routing properties, and then commits the mount.

The resulting `ComNode` owns `ComPtr<IWidgetAbi>` plus one concrete cached layout
variant: either `ComPtr<ILeafLayoutAbi>` or `ComPtr<IContainerLayoutAbi>` with its
authenticated child store. Update calls the cached widget pointer, measurement and
placement call the cached layout pointer, and paint calls the cached widget pointer
through the scoped painter. None of those phases consults the class registry,
switches on a class ID, downcasts an erased value, or treats built-in classes as a
special dispatch case. Consequently, an independently compiled user widget or
container pays the same per-phase interface call cost as a built-in COM node.

Capability discovery is deliberately not repeated after attachment. Mutable widget
state stays in the implementation object behind typed methods; only copied routing
properties and sticky invalidation are refreshed when the widget reports a real
change. Adding a new external leaf or container therefore requires a typed class
contract and the existing common capabilities, not a runtime source change or a
registration-table entry in the UI plugin.

## Retained objects and frame phases

A UI owns a retained forest. Nodes own actual widget capabilities; callers can
retain additional editing references without acquiring tree ownership. Attachment
is single-use and authenticates the widget's host. Removing a subtree logically
closes it, while retained handles keep the allocation memory-safe and return CLOSED
for later editing. Final COM release frees the remaining allocation and resources.

The runtime synchronizes pending geometry, then handles each queued input event in
order. It routes the event, updates eligible widgets, releases traversal state,
dispatches concrete application handlers, and commits their geometry changes before
routing the next event. An empty input queue still performs an eventless update.
There is no production native `Context<State>` or second callback dispatcher.

```rust,no_run
use neocogi_ui::{EventSession, IRendererAbi, Size, Ui};
use neocogi_com::{ComRef, NcStatus};
fn frame<State: 'static>(ui: &Ui, events: &EventSession<State>, size: Size,
    renderer: ComRef<'_, IRendererAbi>) -> Result<(), NcStatus>
{
    ui.update_with_events(size, events)?;
    ui.render(size, renderer)
}
```

Measurement caches include constraints and resource revisions. Changed setters,
truthful CHANGED update results, topology edits, child changes and resize invalidate
geometry independently. Paint is observational and rejects a missing or stale
commit before acquiring a GPU frame. Animation and external-state scheduling remain
explicit author responsibilities, including widgets skipped by hidden/clipped-tree
update policy. See [COM_AUTHORS.md](../../../../docs/COM_AUTHORS.md).

## Layout, painting and resource ownership

A container owns the host's `IChildrenEditor`; callers receive its distinct read-only
view. Concrete edge metadata belongs to the container. Child edits commit topology
before returning a separate old-child teardown status, so metadata must follow the
committed topology even when teardown reports an error.

Layout uses bounded indexed `ILayoutService` calls under an invocation token.
Painting uses an expiring `IPainter` scope with copied geometry. Neither interface
lends native tree, frame or application-state borrows across a dynamic call. Nested
calls restore the parent scope; expired tokens cannot become valid again.

Built-in text, frames and polygons share the UI's recording/tessellation code.
Ordered atlas, immutable-image and custom-render-item batches preserve their display
list positions. The renderer validates complete buffers and resource ownership
before acquiring a frame. Owned images and scenes retain their device/window as
needed; backend types remain local to their assembly.

An immutable asset bundle supplies fonts, icons, styles and pixels. Installation
validates the catalog, prepares CPU resources and uploads the GPU candidate before
publishing the new revision. Failure preserves the prior working UI/resources.
[COM_ASSETS.md](../../../../docs/COM_ASSETS.md) specifies catalog identity and format limits.

## Surface forest

### Concrete surface forest and parent edges

`Context` owns one private `SurfaceForest`. Its single `Vec<SurfaceNode>` retains independent and
child windows, dialogs, application popups, and menu popups. The key index and child adjacency
described below are derived access data and never own additional surfaces. Each node combines common screen-space geometry with a
concrete role:

- a window or dialog owns one application `Node` and its `UiRuntime`, window policy, the unified
  `WindowEvent` port, and an optional concrete menu-bar `MenuSurface`;
- an application-addressable popup owns its anchor and `PopupEvent` port, plus either one
  application `Node` with its `UiRuntime` or one compact `MenuSurface`;
- a private relational menu popup owns its compact `MenuSurface` and parent trigger-slot index.

The surface layer uses concrete enums for those bodies and roles. It does not store `Any`, erased
surface payloads, or a manager-side menu controller. `Node` retains only the shared COM protocol
projections needed for traversal; it does not store a second native widget implementation.

Every forest node has at most one parent edge. An independent window has none; a structural child
or dialog points to its ordinary owner; a top-level popup points to a window or dialog; and a
submenu menu surface points to its direct popup parent. Following edges derives both the top-level
family and popup owner, whose derived lookup entries are rebuilt when policy changes. The restricted constructors admit only
ordinary child-window nesting and directly owned modal dialogs: there is no reparenting, nested
dialog ownership, or public subpopup-registration API. Destroying a window recursively destroys
every child window, dialog, and popup below it. Hiding a parent removes its complete family from
effective visibility while retaining each child's local visibility intent; showing the parent
restores those locally visible children, but does not resurrect modal dialogs closed while hiding.

A popup has no independent layer, visibility flag, or destruction operation. `SurfaceForest`
stores only the deepest active popup key and derives the visible parent-to-child chain by following
parent edges. Opening another top-level popup replaces the branch, opening a submenu extends or
replaces its suffix, and outside presses or owner changes truncate it. Each removed application
popup emits exactly one `PopupEvent::Dismissed`; private menu popups need no public lifecycle port.
The visible path is derived from that active key; there is no separately authored popup visibility state.

### Chronological order and visible traversal

The relative order of window and dialog nodes in the forest vector is the authoritative activation
chronology. Creating a visible window appends it. Showing or explicitly fronting a window or dialog
moves that complete `SurfaceNode` to the vector tail; its stable handle, event allocation, parent
edges, and owned widget allocations remain valid. For independent windows this changes order among
fixed-layer peers. For a child it changes order only among siblings with the same direct parent.
Changing an independent window's fixed layer mutates only that node's mode, so moving a complete
family between layers does not manufacture a new activation.

The forest keeps a disposable key index beside its authoritative vector. Each entry records the
current vector position, direct child keys in sibling chronology, derived root policy, and visible
membership. Appending indexes the new node; moving or removing nodes rebuilds positions and edges.
Parent-first policy derivation visits each node once after an ordering, layer, visibility, enabled,
or popup-path change. A new hidden popup derives only its own inherited policy. These projections
never own surfaces or accept independent application edits.

The forest rebuilds one allocation-reusing `visible_order` after those policy changes. It scans independent roots once for each fixed layer from `MIN_LAYER` to
`MAX_LAYER`, recursively inserts each locally visible child family in parent-first order, and then
scans modal nodes. The active popup chain is inserted at its owning fixed or modal transient tier.
Layout, retained updates, and diagnostics consume that materialized traversal directly. Layout
and paint reuse it without rebuilding policy. Stable-key lookup and visible membership are expected
O(1); policy derivation and a complete child traversal are O(n). The sixteen layer scans have a
fixed factor. Ownership edits still move Vec elements and may rebuild indexes in O(n); recursive
traversal uses stack space proportional to nesting depth.

Painting and hit testing follow the same derived direct-child edges. One family
paints each parent background and application body, then its child families in sibling chronology,
then the parent's intrinsic menu bar and manager chrome. Hit testing performs the inverse: parent
menu/chrome, child families in reverse sibling chronology, then the parent body. This gives the
required sandwich without a general overlay graph or render-command reordering. The popup branch
keeps its existing transient tier above the ordinary families in its inherited fixed band. Stable
sorting, z-index counters, and separate fixed/modal order arrays remain unnecessary.

### Window bands, transients, and activation

Independent windows occupy one of sixteen fixed application layers. Layer `0` is the bottom, layer
`15` is the top and the default, and `Ui::set_window_layer(&window, layer)` changes an independent
window's `LayerBinding::Fixed(u8)`. Structural children have no separately mutable layer: they
inherit the fixed band of their top-level family, report that effective `LayerBinding::Fixed(u8)`
through `Ui::window_layer`, and reject `set_window_layer` with `ManagedLayer`. Numeric validation
uses `MIN_LAYER`, `MAX_LAYER`, and `DEFAULT_LAYER`. Dialogs report `LayerBinding::Modal`; popups
expose no layer binding and use their owner's fixed or modal band. Applications author only the independent root's band; child bindings are derived and committed before the mutation returns.

Within a fixed layer, independent windows retain chronological order. Children retain a separate
chronological order at each direct parent. `Ui::bring_window_to_front(&window)` raises an
independent family among fixed-layer peers or a child family among its siblings; it cannot escape
that structural scope or cross a higher fixed layer. Pointer hit testing is the inverse of the
recursive family paint order, so overlap selects the surface whose pixels are visually in front.

A composed control creates a generic popup once in its stable window owner, then calls
`show_popup(&popup)` at the current pointer or `show_popup_at(&popup, anchor)` at an exact
screen-space rectangle. Generic popups keep that screen anchor until explicitly shown elsewhere;
they do not follow later owner movement. `PopupHandle` parameters keep popup operations separate
from window operations at compile time.

An intrinsic `MenuBar` is consumed with its `Window` into one compact concrete `MenuSurface` for the
bar and one directly owned `MenuSurface` body for each private popup definition. These are not
retained widget leaves and do not own `UiRuntime` values. Logical entries remain values inside the
menu surfaces rather than becoming retained row widgets. Top-level menus retain a
below-heading-slot relation; submenus retain a right-of-row-slot relation to their direct parent
popup. Each surface reuses storage for the local rectangles produced by its authoritative
measurement, and the manager translates those slots after layout, so open menus follow window
movement and ancestor menu geometry. `MenuItemHandle` capabilities remain directly
addressable through `Ui` and project a separate `submitted()` event endpoint; no menu coordinator,
public submenu handle, per-row anchor node,
generic surface payload, temporary recursive menu tree, or second visibility model is involved.

Dialogs occupy a dedicated modal band above all sixteen numeric layers. The frontmost visible
dialog is the only input-eligible window, and its active popup path uses the transient tier above
that dialog. Other windows remain visible, laid out, and painted but cannot interact until no
dialog remains visible. Bringing a dialog forward closes transients from the previous modal owner;
hiding it reveals the next visible dialog in z-order. Raising an ordinary owner does not reorder
its dialogs because they occupy the separate modal band.

Visual order is deliberately separate from keyboard activation. A pointer press records its exact
root or popup `SurfaceKey` without moving the owning family to a different fixed layer, and
keyboard/text routing returns to that concrete surface. Each widget runtime stores one persistent
focused `RuntimeNodeId` independently from pointer capture. Validation and key routing walk the
retained tree to that identity while enforcing every ancestor's participation and clip gates.
`Tab` and `Shift+Tab` replace the ID with the next eligible `KeyboardBehavior::TAB_STOP` surface in
retained sibling order, wrap at the ends, and skip hidden, clipped, disabled, or
pointer-focus-only surfaces.

Showing an application-addressable popup selects its concrete surface immediately. A widget popup
focuses its first Tab stop after geometry is committed and then wraps Tab traversal inside its own
tree. A popup menu instead uses the same direct-row keyboard navigation as an intrinsic menu.
Escape or an outside press dismisses either body and restores its direct parent surface, whose
independently retained focused widget ID was never discarded. An Escape dismissal retains no
key-tail state: only the initial non-repeated press is the dismissal command, while later repeats
and release transitions route normally against the restored parent surface.

`Ctrl+F6` selects the next visible independent or child window in activation chronology and
`Ctrl+Shift+F6` selects the previous one, wrapping at both ends. Selection reuses ordinary
within-scope raising and changes only the manager's active surface, so every window runtime preserves
its focused widget. An application popup closes before the switch; hidden roots are skipped, an
intrinsic menu retains its narrower keyboard scope, and an active modal dialog cannot be escaped.

`WindowOption::DISABLED` is independent from activation and visibility. A disabled root remains in
layout, update, paint, and hit-test order so retained state stays current and its pixels still
occlude lower windows, but it is excluded from pointer, keyboard, popup, menu, and chrome routing.
Painting passes one explicit enabled fact through base window chrome, the intrinsic menu, and
the root widget runtime; it does not synthesize disabled state merely because a different window is
active. Structural children inherit their parent's disabled policy. Modal dialogs own their policy
independently, allowing an enabled dialog to remain usable above a disabled owner.

The frontmost dialog replaces the ordinary active window as the keyboard scope while modal. An
intrinsic or standalone menu uses the same active `SurfaceKey` as windows and widget popups. The
root key denotes an intrinsic bar; each popup key denotes one concrete popup container. A
`MenuSurface` retains only its selected direct-child slot, so the active surface plus that slot is
the menu's complete focus route and no parallel `keyboard_menu_root` exists. Menu navigation
consumes key/text delivery but preserves the widget runtime's path so closing the menu resumes it
exactly. Pointer drags remain with their captured surface, while wheel input has no capture lifecycle
and goes to the topmost eligible surface under the pointer. Hiding or destroying an active surface
or structural ancestor repairs that same identity from the retained forest.

The manager projects that same routing decision into paint. Only the current keyboard surface
receives a visible focused state; nonselected runtimes preserve their target without drawing duplicate
carets or fills. Each widget paints the focused state of its semantic role during the ordinary tree
pass, while the active owner window selects the active state of its title and frame roles. During menu navigation
the menu selection replaces the suspended widget cue while the owning window remains visibly active.


An application desktop is an ordinary window with explicit layer, chrome and child
clip policy. Its application updates the rectangle when the platform drawable
changes. The full demo shows this composition through actual window capabilities.

## Extension and test boundaries

The [extension example](../../../plugins/extension-example) is independently built
and dynamically loaded. Its counter and inset container exercise inheritance,
shared event delivery, scoped layout, measurement invalidation and logical closure
without linking the UI implementation. [COM_NATIVE.md](../../../../docs/COM_NATIVE.md)
documents native and static-browser assembly.

Unit fixtures under `src/test_support` also exercise individual native algorithms.
Their borrowed-state callback driver uses the facade's test-only queue access;
it is absent from normal application/plugin builds. Production event ownership and
cross-assembly behavior are tested through COM sessions and staged deployments.
