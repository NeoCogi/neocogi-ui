# Built-in widgets and containers

Applications use `neocogi_ui::Ui::widgets()` to construct actual COM objects.
A thin `Widget<C>` client owns a checked class projection, with no copied semantic
state or native widget wrapper. Each callable widget class requires `IWidget` and
the one applicable shared layout protocol. The class's concrete Rust allocation
supplies common options, lifecycle, update, measurement, painting, editing, and
optional event families.

## Construction and ownership

```rust,no_run
use neocogi_ui::{CrossSize, EventHandler, EventSession, FlowDirection, IWindowAbi,
    LayoutTrack, LinearEdge, Rect, Ui};
use neocogi_ui::events::CheckboxChanged;
use neocogi_com::{ComPtr, NcStatus};
struct Preview;
impl EventHandler<bool, CheckboxChanged> for Preview {
    fn handle(&mut self, enabled: &mut bool, event: &CheckboxChanged) -> Result<(), NcStatus> {
        *enabled = event.checked;
        Ok(())
    }
}
fn install(ui: &Ui, events: &EventSession<bool>) -> Result<ComPtr<IWindowAbi>, NcStatus> {
    let checkbox = ui.widgets().checkbox("Enable preview", false)?;
    events.subscribe_checkbox(&checkbox, Preview)?;
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

| Client | Purpose | EventSession method |
| --- | --- | --- |
| `Button` | Text/image action | `subscribe_activation` |
| `Checkbox` | Boolean choice | `subscribe_checkbox` |
| `Combo` | Selection and popup header | `subscribe_combo_changed`, `subscribe_combo_popup` |
| `ListBox` | Activatable label/image | `subscribe_activation` |
| `ListItem` | Activatable text/icon row | `subscribe_activation` |
| `Slider`, `Number` | Numeric controls | `subscribe_numeric` |
| `Textbox`, `TextArea` | UTF-8 editors | `subscribe_text_changed`, `subscribe_text_submitted` |
| `Scrollbar` | Horizontal/vertical range | `subscribe_scrollbar` |
| `TextBlock` | Read-only text with explicit wrapping | None |
| `ColorSwatch` | Color preview and optional label | None |

Events own the value captured at the interaction. Ordinary programmatic setters
remain silent. The [events guide](EVENTS.md) describes delivery; exact interfaces and
capabilities are documented in `neocogi-ui-contracts` and the facade's API reference.

Combo lists can keep generic Rust values in the application; selection indices and
labels cross the concrete COM contract. Domain data crossing plugins requires its
own declared interface or an owner-scoped key, without reconstructing identity from
a display string. File-dialog rows retain exact native paths independently of lossy
UTF-8 display labels.

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
surface and two actual scrollbar children. Small scalar models are shared only
where those distinct roles must coordinate. They contain no widget implementation,
parent backpointer, or type-erased payload.

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
once while wiring its `EventSession`; dispatch then calls the cached event interface
and never consults the class registry during update, layout, or paint.

## Native text input and cancellation

Committed text travels separately from logical key strokes. After updating, the host
projects `ui.text_input_area()` to the platform's IME/composition area. Focus loss
cancels active pointer/key interactions; platform integrations own composition and
clipboard delivery. These responsibilities, physical scaling and browser limitations
are documented in [COM_AUTHORS.md](../../../../docs/COM_AUTHORS.md) and
[WEB.md](../../../../docs/WEB.md).
