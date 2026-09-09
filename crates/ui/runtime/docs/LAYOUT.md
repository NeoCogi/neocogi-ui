# Retained layout

## Preferred sizing, tracks, and retained layout

Layout traverses the retained node tree in three stages:

```text
constraints flow down -> desired sizes flow up -> exact rectangles flow down
```

A parent is the immediate container holding a child. During measurement, the parent sends each
child `Constraints`: a width and height that are independently bounded or unbounded. These are
questions about available space, not assigned sizes. Each child answers with its desired size, and
containers combine those answers into desired sizes reported toward the root. Once the root has an
actual rectangle, every parent walks downward again and assigns each child an exact
`Recti { x, y, width, height }`.

A **track** is the one-dimensional unit a parent sizes while turning those measurements into
rectangles. It starts as a rule—`Content`, `Fixed`, or `Flex`—and resolves to one non-negative pixel
extent:

```text
horizontal Linear inside 600 px

| Content: 80 | gap: 10 | Flex(1): 400 | gap: 10 | Fixed: 100 |
|   child 0   |         |    child 1   |         |   child 2  |
```

For a horizontal `Linear`, one track supplies each child's width; for a vertical `Linear`, one
track supplies each child's height. The container combines that main-axis extent with its
cross-axis rule to produce the child's rectangle.

“Slot” is close to “track” for Linear, but it would obscure why Linear and Grid use the same sizing
vocabulary and scalar resolver. They remain separate container algorithms: each measures its own
children and places its own rectangles, and Grid does not construct, contain, or delegate to a
Linear widget. Grid independently passes its row and column track data to the common resolver in
`layout.rs`. Several children can use the same row or column track, and one child can span several
tracks. A child's slot is the resulting two-dimensional area; a track is only one row, column,
width, or height used to form that area. Tracks are parent-owned because the container stores and
interprets these rules—the child reports desired content without knowing whether its parent will
place it in a content, fixed, or flexible track.

Each COM leaf implements intrinsic measurement; a container implements scoped indexed
measurement and placement. The ABI uses explicit finite/unbounded `LayoutLimits`,
including a real bounded zero. Built-in Rust algorithms translate those values to
private Constraints and share the same scalar track resolver.

`IWidget::update` returning CHANGED invalidates measurement and placement along the
ancestor path, including eventless updates. NO_CHANGE preserves caches unless another
input invalidated them. Changed setters, topology edits, new resize constraints and
resource revisions invalidate independently. A same-value setter is silent and preserves
geometry. Changes such as caret blinking may affect only paint.

The runtime owns one actual COM node per attachment. Containers own child identities
through IChildrenEditor and keep only parent-owned edge metadata such as tracks/spans.
Attachments are exclusive and removal closes the subtree; an application COM reference
can survive logical closure but cannot revive or reparent that widget.

Parent-local allocations, child offsets and clips remain in private NodeRuntime state.
Scoped child services expose only indexed geometry; neither native Nodes nor borrowed
child collections reach an extension. Expired tokens and forbidden reentry fail explicitly.

Every Linear direction uses the same content/fixed/flex resolver as Grid. Fixed tracks
stay exact, Content uses intrinsic size, and Flex shares finite remaining space or uses
intrinsic size when unbounded. LinearCrossSize selects content, stretch or fixed cross
extent. Grid resolves columns before wrapped row heights. Negative preferences normalize
to zero; empty containers do not acquire a theme-defined fallback cell.

## Programmatic mutation and commit validity

Use typed facade setters between phases, then call `Ui::update` before relying on derived
layout or input results. The runtime synchronizes geometry after each queued input and
its typed event handlers. Paint observes the resulting commit and rejects stale geometry
or different drawable dimensions before GPU submission.

Widget authors must report geometry changes truthfully and latch out-of-band semantic
changes with WidgetState. Apps must deliver worker/shared-model updates through those
setters or explicit change recording. The runtime cannot observe arbitrary interior
mutation, and hidden/clipped widgets are not guaranteed an update. Synchronize state
before it must affect layout. Resize invalidation does not substitute for reporting
unrelated semantic changes. See the full [author contract](../../../../docs/COM_AUTHORS.md).

Key transitions carry logical identity, pressed/released state, modifiers and repeat.
Composed UTF-8 text is a separate authoritative input channel. WidgetUpdate copies held
button/modifier state after applying the current event; events remain FIFO.

Keyboard focus persists after a pointer release and is independent of pointer capture. A widget
runtime owns one focused node ID; nested containers do not maintain competing mutable focus
pointers. Routing walks the retained tree to that identity and rejects it when an ancestor is no
longer eligible. Within the active surface, `Tab` and `Shift+Tab` replace the ID with the next
enabled, visible `TAB_STOP` surface in retained sibling order and wrap at the ends.
Hidden, clipped, disabled, and pointer-focus-only surfaces are skipped. A modal dialog suspends
ordinary-window traversal. A menu uses the common active surface identity plus its concrete
container's selected direct-child slot; it suspends widget key/text delivery without discarding the
focused node ID that will resume when the menu closes. Custom widgets opt into focus, traversal, and
the shared Windows-style action mapping through `WidgetProperties::keyboard`; their default behavior
is keyboard-inert.

An application-addressable popup is a concrete keyboard surface. A widget popup selects its first
eligible target after layout and wraps Tab inside that tree; a popup menu uses the compact menu's
direct-row navigation instead. Escape or focus transfer outside either popup restores the direct
parent's remembered focused ID. When the popup accepts an initial non-repeated Escape press, only
that press is manager-owned. Later repeats and the physical release use ordinary routing against the
restored parent surface, keeping popup dismissal stateless. Without an active popup, every Escape
transition likewise remains available to the focused application widget.

At the window level, `Ctrl+F6` and `Ctrl+Shift+F6` cycle forward and backward through visible
ordinary roots. Each root's runtime keeps its focused ID while not selected; modal dialogs and open
intrinsic menus retain their narrower keyboard scopes instead of participating in the cycle.

Paint exposes focus only for the manager-selected keyboard surface even though every window runtime
retains its own target. Focused controls, disclosure rows, and menus resolve the focused state of
their own semantic role; there is no widget-independent outline pass or hidden frame geometry.
Active titles and framed windows likewise select their explicit active chrome state. Deactivated windows
retain their remembered widget focus and
enabled client presentation while their frame and title select `ChromeState::Base`. Only an
explicit `WindowOptions::DISABLED` resolves the chrome, intrinsic menu, client backgrounds, child
appearances, text, and icons through the theme's `disabled` state.

## Phase and regression validation

Update and paint visit eligible parents before children and siblings in retained order.
A successful edit of a later widget is visible when traversal reaches it; editing an
already visited widget does not rerun it. End-of-event geometry synchronization makes
all committed changes authoritative before the next input. Errors are observable;
there is no automatic rollback of already committed semantic/topology edits.

Layout and paint are observational. Do not emit application events or mutate authored
state through captured COM interfaces in either phase. Use private caches only, release
interior borrows before calling another component, and return BUSY for recursive entry.
The shared scoped services and WidgetState enforce authority; authors still own the
truthfulness of changes and external-state synchronization.

Native algorithm fixtures test layout arithmetic, retained cache reuse, focus, routing
and clipping. The independently built extension additionally verifies real COM identity,
exclusive attachment, changed/no-change caching, resize, child replacement and exactly-once
closure. Its execution commands are in [COM_NATIVE.md](../../../../docs/COM_NATIVE.md).
