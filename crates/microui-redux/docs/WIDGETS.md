# Built-in widgets and containers

Application code normally imports these types from `microui_redux::prelude`. This guide is a map
of the built-in choices; the generated API reference remains the authority for every parameter and
method.

## Construction and ownership

Most built-ins accept a one-shot `*Parameters` value and return
`(TypedWidgetHandle<ConcreteType>, Node)`. Move the unique `Node` into a window or container and keep
the weak typed handle only when later reads, mutations, or event subscriptions are needed. Removing
the node expires its handles instead of keeping an invisible widget alive.

Standalone controls also implement `From<TheirParameters>` when an unmounted concrete value is
useful: `let button = Button::from(ButtonParameters::new("Save"));`. Mount that value with
`Node::typed_widget(button)` or `Node::widget(button)`. Parameters are ordinary structs, and
construction needs no marker trait or separate stateless builder type. Composed controls such as
`TextArea` use `create()` to install their complete internal tree and weak coordination handles.

```rust
# use microui_redux::prelude::*;
# struct Model { checked: bool }
# impl Model {
#     fn checkbox_changed(&mut self, event: &CheckboxChanged) {
#         self.checked = event.checked;
#     }
# }
# fn install<B: RendererBackend>(
#     context: &mut Context<B, Model>,
# ) -> Result<WindowHandle, SubscribeError> {
let (checkbox, checkbox_node) =
    Checkbox::create(CheckboxParameters::new("Enable preview", false));
context.subscribe(checkbox.changed(), Model::checkbox_changed)?;

let (_, content) = Linear::create(LinearParameters::vertical([
    LinearItem::content(checkbox_node),
]));
let window = context
    .ui()
    .create_window(Window::new("settings", rect(40, 40, 260, 120), content));
# Ok(window)
# }
```

`Custom` is the stateless exception: `Custom::create` returns its runtime directly. Mount it with
`Node::widget`, or pair it with a registered backend callback through `Node::custom_render`.

## Controls

| Type | Purpose | Native event ports |
| --- | --- | --- |
| `Button` | Labeled or visual action | `ButtonSubmitted` |
| `Checkbox` | Boolean choice | `CheckboxChanged` |
| `Combo` | Selected-item and popup-header state | `ComboChanged`, `ComboSubmitted` |
| `ListBox` | Activatable label with an optional image | `ListBoxSubmitted` |
| `ListItem` | Activatable row with an optional semantic icon | `ListItemSubmitted` |
| `Slider` | Bounded scalar input | `SliderChanged` |
| `Number` | Draggable and editable numeric input | `NumberChanged` |
| `Textbox` | Single-line UTF-8 editor | `TextboxChanged`, `TextboxSubmitted` |
| `TextArea` | Multiline UTF-8 editor with retained scrolling | `TextAreaChanged`, `TextAreaSubmitted` |
| `Scrollbar` | Standalone horizontal or vertical range control | `ScrollbarChanged` |
| `TextBlock` | Read-only, optionally wrapped text | None |
| `ColorSwatch` | Color preview with an optional label | None |
| `Custom` | Stateless input/measurement surface for custom painting | None |

Events contain owned or copied semantic snapshots, so handlers do not need to borrow the emitting
widget. The [events guide](EVENTS.md) covers subscription forms, ordering, and context-aware
handlers. Combo popup content remains application-owned: handle `ComboSubmitted`, present the
choices, then call `select` on the combo handle.

## Containers

| Type | Use it for |
| --- | --- |
| `Linear` | One row or column with content-sized, fixed, or weighted child tracks |
| `Grid` | Explicit rows, columns, and spanning cells |
| `Disclosure` | A header whose retained body participates only while expanded |
| `ScrollArea` | A framed or unframed one-child viewport with retained scrollbars |

Containers exclusively own their child nodes. Topology mutations consume new nodes and return an
unaccepted value unchanged when insertion cannot occur; removed nodes are dropped rather than
detached for reparenting.

## Retained mutation

Convenience methods on `TypedWidgetHandle<W>` return `None` after the mounted widget has been
removed. Use `try_read`, `try_update`, or `try_update_with` for behavior not exposed by a specialized
method, and finish the access closure before starting retained traversal. After every programmatic
mutation, call `Context::update_ui` or `Context::update_ui_state` before depending on its visual,
interaction, or layout result. See the [layout guide](LAYOUT.md) for the synchronization contract.

## Reporting geometry changes during update

`Context::update_ui` (or `update_ui_state`) commits measurement and layout automatically. Widget
and application authors do not call a separate measurement function. The runtime commits layout
before routing the next queued input event, so an earlier event's size or topology change affects
later hit testing in the same update.

`Widget::update` returns `Change::Changed` when it changes inputs used by measurement or child
placement. `Change::NoChange` means those inputs stayed the same. It is independent of semantic
event emission: a button can submit an event while returning `NoChange`. The event handler still
runs, and its checked widget or topology mutations still invalidate their owners. The return value
also does not suppress painting or updates of children. Static widgets can use the default update.

| Change | Who records invalidation? |
| --- | --- |
| Widget update changes text, preferred size, child participation, or child translation | Widget author returns `Changed`, including during `update(None)` |
| Programmatic mutation through a public typed handle | Handle records invalidation before granting mutable access |
| Controlled child insertion, removal, or replacement | Owning container's mutation API |
| New available size or wrapping width | Runtime measurement cache compares constraints |
| New installed skin/atlas bundle | Runtime measurement cache compares skin revision |
| Origin-only move, unchanged available size | Placement moves the widget; intrinsic measurement can be reused |
| Paint-only color, caret, animation, or private drawing cache | No geometry invalidation needed; paint observes current state |
| External/shared state changed without going through a handle | Application/widget author must synchronize it explicitly |

A widget that reads external data should copy the geometry-relevant data into its own retained state
and compare it during update. For example:

```rust
use microui_redux::{Change, Widget, WidgetOption, WidgetUpdateCtx, WidgetPaintCtx, UiInputEvent};
use std::{cell::Cell, rc::Rc};

/// A widget that observes a height requested by application-owned shared state.
struct HeightWidget {
    /// Height committed for subsequent measurement and painting.
    height: i32,
    /// Desired geometry; the runtime cannot observe writes to this cell.
    desired_height: Rc<Cell<i32>>,
}
impl Widget for HeightWidget {
    fn widget_opt(&self) -> &WidgetOption { &WidgetOption::NONE }
    fn update(&mut self, _: &mut WidgetUpdateCtx<'_>, _: Option<&UiInputEvent>) -> Change {
        let next = self.desired_height.get().max(0);
        let change = Change::from(next != self.height);
        self.height = next;
        change
    }
    fn paint(&mut self, _: &mut WidgetPaintCtx<'_>) { /* Paint committed state. */ }
}
```

The author contract has deliberate limits:

- **The widget author must report changes truthfully.** The runtime cannot inspect arbitrary fields,
  compare previous widget values, or infer dependencies from event types. Returning `NoChange` after
  changing measurement/placement inputs can leave stale geometry. Return `Changed` when uncertain;
  this costs cache reuse but preserves correctness. Cursor-only edits can return `NoChange` when
  measurement does not read the cursor. Committing numeric text on focus loss must report any
  resulting value change even though that update may receive no input event.
- **The application author owns synchronization of shared/external state.** An `Rc<Cell<_>>`,
  `RefCell`, background result, or other application model can change without touching the widget.
  The application must apply it through a typed handle before the relevant layout, or arrange an
  eligible widget update that observes it and returns `Changed`. A widget that measures directly
  from an externally mutated cell while always returning `NoChange` violates this contract.
- **An update callback is not a universal watcher.** Hidden, clipped, or otherwise ineligible widgets
  are not guaranteed to run. Changes that must affect layout while a widget is ineligible should
  go through its public typed handle. Similarly, a widget watching another widget that updates
  later in traversal sees that change only on a subsequent update; synchronize such dependencies
  in application code or a typed event handler when same-transaction geometry is required.
- **Painting must remain observational.** Do not change measurement inputs, topology, or application
  coordination state from paint, including through captured handles. Private rendering caches are
  allowed only when they do not change the current layout or emit coordination events.

`Changed` is intentionally conservative and local. The runtime combines it with existing dirty
markers and propagates invalidation to dependent ancestors. A parent's `NoChange` cannot hide a
child's `Changed`, and a later `NoChange` cannot undo a prior handle mutation. No public dirty-bit
API, global dependency registry, or automatic shared-state observer is required.

Slider construction and programmatic `set_value` clamp finite values to the configured range but
preserve values between steps. They never emit a user-change event, including during later updates.
For example, setting `4.4` on a step-1 slider retains `4.4`. A subsequent user adjustment snaps
relative to the lower bound and clamps to the range. Keyboard, wheel, pointer, and changed numeric
text commits share this policy; a text commit on focus loss can emit `SliderChanged` and return
`Changed` even without a routed event. An unchanged result emits nothing. Non-finite setter inputs
or invalid numeric text preserve the previous finite value.
## Typed list-row data

List rows can carry explicit application data independently of display text:

```rust
use microui_redux::prelude::*;
use std::path::PathBuf;

let path = PathBuf::from("report.txt");
let (row, node) = ListItem::create(ListItemParameters::new("Report").with_value(path));
let submitted: WidgetEventPortHandle<ListItemSubmitted<PathBuf>> = row.submitted();
// Subscribe to `submitted` and use event.value as the native path; event.label is presentation.
# let _ = (node, submitted);
```

The payload remains concrete through the widget and event source. It must be `Clone + 'static`
because each submission owns a clone. Ordinary rows use the default `()` payload. Prefer a concrete
enum for several row actions; neither strings-as-keys nor type erasure are needed.

## Native text input and host cancellation

An editor returns `KeyboardBehavior::TEXT_INPUT` together with its focus/traversal flags while it
accepts text. `Textbox` and `TextArea` do so continuously; `Number` and `Slider` do so only in their
text editing mode. A custom editor must make the same choice itself. After `update_ui` or
`update_ui_state`, `Context::text_input_area()` returns the focused editor's clipped content area
in framebuffer coordinates, or `None`. Queued input invalidates that query until the next update.

Hosts must deliver committed strings with `Context::text` independently of `Context::key`. A
logical character key is useful for commands but is not a text insertion: one composed character
can require several keys, and paste or speech input can insert text without any character key.
Custom widgets apply their own normalization, editing, and measurement-change policy to those
strings. Built-in editors share their existing single-line or multiline normalization.

Call `Context::cancel_input()` when the host loses focus, receives pointer cancellation, or
unexpectedly loses pointer capture. This boundary is processed in input order. It clears held
buttons/modifiers, focus, hover, capture, and active popup/menu gestures. Widgets receive an update
with no routed event and cleared held state, so editors apply their normal blur/commit policy.
Cancellation never synthesizes a mouse release or rolls back an application value. Custom drag
widgets must clear their private gesture state when capture/held state disappears; they cannot
depend on always receiving `MouseUp`.

The area query is deliberately an editor box, **not an exact caret rectangle**. Hosts requiring
caret-accurate candidate positioning need an explicit editor-specific extension. The core stores
committed text only: native hosts own preedit presentation, candidate selection, and composition
cancellation. Selection synchronization, copy/cut of retained text, rich-text paste, surrounding
text/reconversion, accessibility, and mobile virtual keyboard policy are application/host work;
the textarea bridge in the demo does not supply those features. Atlas glyph coverage and shaping
remain independent of accepting a Unicode string. See `TYPOGRAPHY.md` for rendering limitations.
