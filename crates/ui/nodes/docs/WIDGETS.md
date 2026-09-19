# Built-in widgets and containers

The concrete Rust classes, factory, and standard file picker live in `crates/ui/nodes` and
deploy through `neocogi.ui.nodes.runtime`. Core retains only shared node contracts and the
engine that mounts their COM capabilities; it contains no built-in implementation catalog.

Applications use `neocogi_ui::Ui::widgets()` to construct independent COM objects.
A thin `Widget<C>` client owns a checked class projection, with no copied semantic
state or native widget wrapper. Each callable widget class requires `IWidget` and
the one applicable shared layout protocol. The class's concrete Rust allocation
supplies common options, lifecycle, update, measurement, painting, editing, and
optional event families.

Controls that require a structural owner at construction use their dedicated `Ui`
method. In particular, `Ui::combo_box` creates a header together with its private
selection-menu popup after receiving the popup's owning window.

## Construction and ownership

```rust,no_run
use neocogi_ui::{CrossSize, Events, FlowDirection, IWindowAbi,
    LayoutTrack, LinearEdge, Rect, Ui};
use neocogi_com::{ComPtr, NcStatus};
fn install(ui: &Ui, events: &mut Events<bool>) -> Result<ComPtr<IWindowAbi>, NcStatus> {
    let checkbox = ui.widgets().checkbox("Enable preview", false)?;
    events.map(checkbox.changed(), |event| event.checked)?;
    let mut column = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch)?;
    column.insert(0, &checkbox, LinearEdge::new(LayoutTrack::Content))?.finish()?;
    ui.window("Settings", Rect { x: 40, y: 40, width: 260, height: 120 }, &column)
}
```

Attachment consumes the widget's sole mount claim, even while callers retain owned
editing references. Nodes cannot be reparented or mounted twice. Removal logically
closes the subtree, so retained handles report CLOSED without becoming dangling
pointers. Final COM release frees the allocation separately.

## Controls and standard events

| Client | Purpose | Port accessor |
| --- | --- | --- |
| `Button` | Text/image action | `activated()` |
| `Checkbox` | Boolean choice | `changed()` |
| `Combo` | Optional selection and runtime-owned choice popup | `changed()` |
| `ListBox` | Activatable label/image | `activated()` |
| `ListItem` | Activatable text/icon row | `activated()` |
| `Slider`, `Number` | Numeric controls | `changed()` |
| `Textbox`, `TextArea` | UTF-8 editors | `changed()`, `submitted()` |
| `Scrollbar` | Horizontal/vertical range | `changed()` |
| `TextBlock` | Read-only text with explicit wrapping | None |
| `ColorSwatch` | Color preview and optional label | None |

Events own the value captured at the interaction. Ordinary programmatic setters
remain silent. The [events guide](EVENTS.md) describes delivery; exact interfaces and
shared capabilities are documented in `neocogi-ui-core-contracts`, concrete
classes in `neocogi-ui-nodes-contracts`, and clients in the facade's API reference.

Combo boxes retain a selection-menu hierarchy but no generic application payload.
`Ui::combo_box(parent, items, ChoiceSelection::None)` is the flat convenience: it
assigns each label an ID equal to its ordinal, displays the `Select…` fallback, and
allows `set_placeholder` to replace that fallback. `Ui::combo_menu` instead consumes
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
| `Disclosure` | Actual header/body widgets with expansion-controlled participation |
| `ScrollArea` | A single replaceable content root with composed scrollbars |
| `TextArea` | An actual editor with scrolling; its editor role cannot be replaced |

Each container owns one authoritative child sequence. Concrete edge metadata lives
with its layout policy. A topology operation returns `ChildEditResult`: topology can
commit even if closing a removed child fails. Call `finish()` when no additional
metadata must be reconciled, and do not retry an already committed insertion or
replacement as if it had rolled back.

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
[COM_AUTHORS.md](../../../../docs/COM_AUTHORS.md#update-measurement-and-external-state).

## Custom widgets and containers

A local leaf can implement the static `LeafContent` trait and call
`ui.widgets().leaf(content, options)`. Its concrete content remains inline in one
COM object; the helper shares common lifecycle and bounded paint/input validation.
A widget with custom editing interfaces implements those declared COM interfaces
and composes `WidgetState` directly.

The [independent extension](../../../../examples/ui-extension) demonstrates a real
constructible `Counter: IWidget + ILeafLayout`, standard activation events, and an
`IInset: IContainer` using `IChildrenEditor` and bounded `ILayoutService` calls.
The host activates `Counter` directly through the COM runtime with a typed
`IWidgetHost` constructor argument. Neither widget imports the UI implementation,
native atlas IDs, tree borrows, GPU contexts or backend callbacks.
Custom drawing uses the painter's solid geometry/image methods or an owned
`IRenderItem` implemented by the renderer assembly.

External controls may also implement standard event capabilities such as
`IActivationEvents` or `ITextEvents`. The application queries that typed capability
once while mapping its `Events` inbox; producers post through the cached typed
receiver. Dispatch consumes local messages and never consults the class registry.

## Native text input and cancellation

Committed text travels separately from logical key strokes. After updating, the host
projects `ui.text_input_area()` to the platform's IME/composition area. Focus loss
cancels active pointer/key interactions; platform integrations own composition and
clipboard delivery. These responsibilities, physical scaling and browser limitations
are documented in [COM_AUTHORS.md](../../../../docs/COM_AUTHORS.md) and
[WEB.md](../../../../docs/WEB.md).
