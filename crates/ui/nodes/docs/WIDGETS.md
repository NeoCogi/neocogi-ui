# Built-in widgets and containers

The concrete Rust classes, factory, and standard file picker live in `crates/ui/nodes` and
deploy through `neocogi.ui.nodes.runtime`. Core retains only shared node contracts and the
engine that mounts their COM capabilities; it contains no built-in implementation catalog.

Applications use `neocogi_ui::Ui::widgets()` to construct independent COM objects.
Factories return `OwnedWidget<C>`; a thin `Widget<C>` client holds a cached weak projection, with no copied semantic
state or native widget wrapper. Each callable widget class requires `IWidget` and
the one applicable shared layout protocol. The class's concrete Rust allocation
supplies common options, lifecycle, update, measurement, painting, editing, and
optional event families.

Controls that require a structural owner at construction use their dedicated `Ui`
method. In particular, `Ui::combo_box_with_queue` creates a header together with its private
selection-menu popup after receiving the popup's owning window.

## Construction and ownership

```rust,no_run
use neocogi_ui::{CrossSize, FlowDirection, IWindowAbi,
    LayoutTrack, LinearEdge, Rect, Ui};
use neocogi_com::{ComPtr, ComRef, NcStatus};
fn install(ui: &Ui, queue: ComRef<'_, neocogi_ui::CheckboxEventQueueAbi>) -> Result<ComPtr<IWindowAbi>, NcStatus> {
    let checkbox = ui.widgets().checkbox_with_queue("Enable preview", false, Some(queue))?;
    let mut column = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch)?;
    column.insert(0, checkbox, LinearEdge::new(LayoutTrack::Content)).map_err(|(status, _candidate)| status)?;
    ui.window("Settings", Rect { x: 40, y: 40, width: 260, height: 120 }, column).map_err(|(status, _candidate)| status)
}
```

Successful attachment consumes the exclusive widget owner. Rejection returns
`(status, original_owner)` for retry. These fresh-construction examples explicitly
discard a returned candidate when abandoning construction; forwarding providers
must instead return it to their caller. Attachment never returns a mounted child. Capture `owner.handle()` first
when a form needs later editing access. Nodes cannot be reparented or mounted
twice. Removing an owner rejects new calls and destroys the subtree once any
already admitted call returns. Retained weak handles report `NC_E_DROPPED`; they
cannot preserve live resources. Generic widget close no longer exists.

## Controls and standard events

| Client | Purpose | Event enum variant |
| --- | --- | --- |
| `Button` | Text/image action | `Event::Activated` |
| `Checkbox` | Boolean choice | `Event::Changed` |
| `Combo` | Optional selection and runtime-owned choice popup | `Event::Changed` |
| `ListBox` | Activatable label/image | `Event::Activated` |
| `ListItem` | Activatable text/icon row | `Event::Activated` |
| `Slider`, `Number` | Numeric controls | `Event::Changed` |
| `Textbox`, `TextArea` | UTF-8 editors | `Event::Changed`, `Event::Submitted` |
| `Scrollbar` | Horizontal/vertical range | `Event::Changed` |
| `TextBlock` | Read-only text with explicit wrapping | None |
| `ColorSwatch` | Color preview and optional label | None |

Each emitting class owns one event port initialized from its factory's
`EventBinding<Object, Event>` argument. The receiver is fixed before publication;
there is no later attachment operation.
`neocogi_ui::textbox::Event`, for example, is the actual contract message enum
covering both Changed and Submitted.
Use `*_unobserved` constructors when no events are needed.
Events own the value captured at the interaction. Ordinary programmatic setters
remain silent. The [events guide](../../runtime/docs/EVENTS.md) describes delivery; exact interfaces and
shared capabilities are documented in `neocogi-ui-core-contracts`, concrete
classes in `neocogi-ui-nodes-contracts`, and clients in the facade's API reference.

Combo boxes retain a selection-menu hierarchy but no generic application payload.
`Ui::combo_box_with_queue(parent, items, ChoiceSelection::None, queue)` is the flat convenience: it
assigns each label an ID equal to its ordinal, displays the `Select…` fallback, and
allows `set_placeholder` to replace that fallback. `Ui::combo_menu_with_queue` instead consumes
the NCIDL-generated core `ChoiceMenuEntry` preorder sequence and `MenuTextRange` values,
so submenus and stable application-defined leaf IDs cross COM without a handwritten
parallel structure. Every branch declares its direct-child count; invalid UTF-8
ranges, empty or malformed branches, duplicate IDs, excessive nesting, and unknown
selected IDs reject the complete candidate atomically. `selection()` and changed
events return that same generated `ChoiceSelection`. Domain data remains in the
application and is mapped from the committed stable ID rather than reconstructed
from a display string. File-dialog rows likewise retain exact native paths
independently of lossy UTF-8 display labels.

## Containers and child edits

| Client | Geometry policy |
| --- | --- |
| `Linear` | Row/column direction with content, fixed or weighted tracks |
| `Grid` | Explicit rows, columns and spans |
| `SplitView` | Two panes separated by a draggable, keyboard-adjustable divider |
| `Disclosure` | Actual header/body widgets with expansion-controlled participation |
| `ScrollArea` | A single replaceable content root with composed scrollbars |
| `TextArea` | An actual editor with scrolling; its editor role cannot be replaced |

Each container exclusively owns its child editor and actual child owners.
Concrete edge metadata lives with its layout policy. A topology operation returns
`Result<(), (NcStatus, Owner)>` for insertion/replacement. Failure preserves
existing topology and returns the unchanged candidate;
success publishes matching metadata and drops removed owners. Destruction has no
recoverable status, and there is no `finish()` or separate cleanup outcome.

`SplitView` owns both panes and the divider's geometry and gesture state in one
concrete container. Its implementation belongs to nodes; core supplies the existing
`IWidget`/`IContainerLayout`, child ownership, capture, focus, and clipping protocols.
There is no split-specific core routing or editor-side drag callback.

```rust,no_run
use neocogi_ui::{SplitAxis, SplitOptions, OwnedWidget, SplitViewAbi, Ui};
use neocogi_com::NcStatus;
fn split(ui: &Ui) -> Result<OwnedWidget<SplitViewAbi>, NcStatus> {
    let mut split = ui.widgets().split_view_unobserved(SplitOptions {
        axis: SplitAxis::Horizontal, // Left/right panes and a vertical handle.
        position: 320, minimum_first: 200, minimum_second: 240, handle_width: 8,
    })?;
    split.insert(0, ui.widgets().textbox_unobserved("Inspector")?).map_err(|(status, _candidate)| status)?;
    split.insert(1, ui.widgets().textbox_unobserved("Preview")?).map_err(|(status, _candidate)| status)?;
    Ok(split)
}
```

Zero panes leave an empty container; one pane fills it; two enable the divider.
Inserting a third pane rejects and returns the unchanged candidate. Replace/remove
drop the affected owner through the ordinary topology transaction.
`SplitAxis::Vertical` places the first pane above the second. Positions and minima
are pixels along the movement axis. The requested position survives temporary
allocation clamping; when both minima cannot fit, they compress proportionally.
The handle uses normal button appearance with a small grip, and `handle_width`
controls its visible and interactive thickness. Tab focuses it; the corresponding
arrow keys move it by eight pixels. Capture loss cancels dragging. `Event::Changed`
reports user changes only; `set_settings()` and window resizing are silent.

The structural children of a composed control are real, distinct COM objects, not
second representations of the parent. For example, `TextArea` is the scrolling
root and owns one `TextEditor` content child; the child alone owns the text buffer,
cursor, font, and text-event sources. `ScrollArea` similarly owns a transform
surface and two actual scrollbar children. Small typed state capabilities are shared only where
distinct roles must coordinate. A combo header and its compact choice-menu popup use one
runtime-private state allocation for shared UTF-8 text, generated preorder entries, optional stable
selection, and an explicit popup state machine. Its fields are ordinary values mutated through
exclusive interface receivers; pointer hover and keyboard candidate state remain solely in the
shared menu surfaces. The composition manager above `WindowManager` owns the association with a
generic popup handle. The surface forest contains menu values and popup ownership, never a second
combo widget, parent backpointer, or type-erased payload.

## Geometry and external state

Setters retain a sticky geometry change when measurement or placement inputs change.
`update` returns CHANGED for the same reason; painting-only changes and event emission
alone may return NO_CHANGE. Returning NO_CHANGE cannot erase a prior setter's marker.
Resize constraints, topology and installed asset revisions invalidate independently.
Call `ui.update` or `ui.update_with_events` after edits and before rendering.

Widgets skipped by hidden/clipped-tree update policy cannot report an unseen external
change. Authors must synchronize such data through an authorized setter, arrange a
later truthful update, and schedule any needed frame explicitly. Painting must not
mutate semantic state or emit coordination events. The full obligations are in
[COM_AUTHORS.md](../../../../docs/contracts/COM_AUTHORS.md#update-measurement-and-external-state).

## Custom widgets and containers

A local leaf can implement the static `LeafContent` trait and call
`ui.widgets().leaf(content, options)`. Its concrete content remains inline in one
COM object; the helper shares common lifecycle and bounded paint/input validation.
A widget with custom editing interfaces implements those declared COM interfaces
and composes `WidgetState` directly. The common `IWidget` contract exposes
`replace_flags`, `set_flags`, and `clear_flags` over current requested
`WidgetOptions`; delegate them to that shared state. Effective `properties()`
may add transient behavior flags and must not be copied back as requested flags.

The [independent extension](../../../../examples/ui-extension) demonstrates a real
constructible `Counter: IWidget + ILeafLayout`, standard activation events, and an
`IInset: IContainer` using `IChildrenEditor` and bounded `ILayoutService` calls.
The host activates the extension factory and calls its typed `counter` method
with an `IWidgetHost` constructor argument. Neither widget imports the UI implementation,
native atlas IDs, tree borrows, GPU contexts or backend callbacks.
Custom drawing uses the painter's solid geometry/image methods or an owned
`IRenderItem` implemented by the renderer assembly.

External controls use the same constructor contract: one
`EventBinding<Editor, EditorEvent>`, with all event kinds in its complete enum.
The application creates the typed queue before calling the custom factory;
producers retain that receiver and a weak public origin. No inherited interface adds another port.
Dispatch consumes local messages and never consults a class registry.

## Native text input and cancellation

Committed text travels separately from logical key strokes. After updating, the host
projects `ui.text_input_area()` to the platform's IME/composition area. Focus loss
cancels active pointer/key interactions; platform integrations own composition and
clipboard delivery. These responsibilities, physical scaling and browser limitations
are documented in [COM_AUTHORS.md](../../../../docs/contracts/COM_AUTHORS.md) and
[WEB.md](../../../../docs/guides/WEB.md).

Built-in container algorithms use `Result` for measurement and placement, including
all indexed child operations. A failed child operation stops the algorithm and
returns its original status through `IContainerLayout`; it never supplies a zero
preference as a substitute. Parent geometry is published only on success. Child
placements completed before a failure are derived state and are recomputed on a
subsequent layout attempt. Track measurement remains allocation-free, while grid
and linear placement retain their reusable buffers across attempts.
