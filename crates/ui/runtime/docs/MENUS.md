# Application menus

Menus are compact UI-owned records presented in ordinary retained menu surfaces.
An intrinsic bar belongs to window chrome; a standalone menu uses an ordinary
`IPopup` capability. Applications construct either form with `MenuBuilder` and retain
only the actual `MenuItem` capabilities needed for editing; receivers are fixed when each item is created.
There is no native row widget, copied application menu model, or erased command data.

## Construction and events

A root builder owns one declaration until the UI transfers it into a bar or popup.
Its child builders append into that same declaration and cannot be mounted alone.
After successful transfer, further appends return CLOSED. Validation failures leave
the declaration available for correction. Item capabilities resolve the same record
before and after mounting; they never keep a removed menu surface logically open.

```rust,no_run
use neocogi_ui::{ IWindowAbi, Rect, Ui};
use neocogi_com::{ComPtr, NcStatus};
fn install(ui: &Ui, sender: &std::sync::mpsc::Sender<String>) -> Result<ComPtr<IWindowAbi>, NcStatus> {
    let body = ui.widgets().button_unobserved("Document")?;
    let window = ui.window("Editor", Rect { x: 20, y: 20, width: 640, height: 480 }, &body)?;
    let menus = ui.menus()?;
    let bar = menus.create("Editor menu")?;
    let file = bar.submenu("File")?;
    let open = file.item_observed("Open", sender.clone(), |event| event.label)?;
    open.set_shortcut_hint("Ctrl+O")?;
    let save = file.item("Save")?;
    save.set_enabled(false)?;
    file.separator()?;
    file.submenu("Recent")?.item("Document.txt")?;
    menus.attach_bar(window.as_com_ref(), &bar)?;
    Ok(window)
}
```

The same builder creates a standalone popup:

```rust,no_run
use neocogi_ui::{IWindowAbi, Rect, Ui};
use neocogi_com::{ComRef, NcStatus};
fn show(ui: &Ui, owner: ComRef<'_, IWindowAbi>, anchor: Rect) -> Result<(), NcStatus> {
    let menus = ui.menus()?;
    let menu = menus.create("Object actions")?;
    let inspect = menu.item("Inspect")?;
    let mut popup = menus.popup(owner, &menu)?;
    popup.show_at(anchor)?;
    // Applications retain item/popup capabilities when later editing or hiding is needed.
    Ok(())
}
```

Use `set_label`, `set_enabled`, `set_mark`, `set_shortcut_hint`, and the font-style
capability to edit a row. Setters are silent. Activation captures its label before
callback delivery, and the manager closes the selected popup path before the
application handler runs. Check/radio-group transitions belong to application
handlers; a shortcut hint does not register an accelerator.

A combo popup is a choice-menu specialization of the same compact surface and popup
forest. Its leaves carry choice identities rather than command-item event ports.
The owning combo model supplies the committed identity and replaces the complete
validated hierarchy after a model edit. Branches compile into the same relational
submenu forest used by command menus; no combo-specific popup tree exists. Opening
restores every branch leading to a nested committed leaf, so that exact leaf remains
the keyboard and paint fallback from the first visible frame. Routing stages a leaf
identity, closes the menu path, and commits it at the next checked composition
boundary before application changed-event delivery. Pointer hover, keyboard focus,
and committed choice are independent: hover wins only while the pointer has an
actual row target, keyboard navigation wins while active, and otherwise the
committed row remains highlighted. The trigger fixes the top-level popup width;
submenu rows retain their ordinary intrinsic menu width.

## Placement and interaction

Menu topology and placement remain manager-private. Every menu popup is a concrete forest node with
one parent edge. A bar-owned top-level popup adds a trigger-slot index and anchors below its heading;
a submenu adds an index and anchors at the right edge of its parent row. A standalone popup instead
uses the exact screen anchor retained by its public `IPopup`, while its descendants use the
same relational submenu edges as bar-owned menus. The manager resolves relational placement from
current surface geometry, so open menus follow their window and parent rows without retained anchor
widgets.

An intrinsic bar is also the overlay boundary for structural child windows. Within one window
family the manager records the parent's background and application body, then its child families,
then the parent bar and chrome. Pointer selection walks that relationship in reverse, so a root bar
both renders and handles input above children that geometrically overlap it. If the parent window
uses `ChildClip::Content`, each complete child surface is additionally clipped below the bar
to the parent's application body. The active menu-popup branch still occupies the transient tier of
the owning window's inherited fixed or modal band.

The manager stores only the deepest active popup. Following sole parent edges derives the one
visible heading-to-descendant chain:

- pressing a closed heading opens its popup;
- pressing the active heading closes the path;
- pressing another heading replaces the path;
- pressing or hovering a submenu row appends that child while retaining its ancestors;
- opening or hovering a sibling submenu replaces only the older descendant branch;
- selecting an enabled item or pressing outside the chain closes the complete path.

A disabled item cannot be invoked: it emits no `Activation` event and does not close the
menu. The popup remains one retained input surface, so disabled rows and separators still occlude
content behind it while resolving to no submission action. Hovering a bar heading while no menu is
open changes presentation only. Once a click or keyboard command opens a menu, pointer movement
hot-tracks the hierarchy: another heading immediately replaces the top-level popup, a submenu row
opens or replaces its child, and an ordinary sibling row closes an unrelated child branch. Moving
over application widgets or outside the menu entries leaves the current path open until an outside
press, explicit cancellation, or item submission ends the menu scope.

The same surfaces provide Windows-style keyboard navigation without a parallel menu model:

- `F10` or an unchorded Alt press and release enters the active window's menu bar; repeating either
  gesture leaves menu navigation.
- Left and Right traverse top-level headings; Down opens the selected heading at its first row and
  Up opens it at its last row. In a popup, Down and Up wrap through enabled items and submenus, Home
  and End select the first and last eligible rows, Right opens a selected submenu, and Left returns
  to its parent or moves to the preceding heading.
- Enter or Space opens the selected heading/submenu or invokes the selected enabled item. Escape
  closes one popup level and finally leaves the menu bar.
- Disabled items and separators remain visible and occluding but are skipped by keyboard selection.

Pointer-opened menus enter the same keyboard scope, so subsequent arrows and activation keys work
without an extra F10 press. While that scope is active, application widget key and text delivery is
suspended; closing the menu restores the exact persistent widget focus. Any non-Alt key cancels a
pending Alt tap, allowing combinations such as Alt+Down to continue to a focused combo. The menu
also consumes Ctrl+F6 window-cycle chords; window cycling resumes only after the menu scope closes.

## Skin and current scope

The complete visuals for `MenuRole::Title` and `MenuRole::Item` color menu labels, item text,
marks, and arrows in the row's exact interaction state. Checked and radio state affects only the
independent marker glyph. `MenuPopup` supplies the separator color. A theme can pair a dark selected PNG or
flat highlight with a light hovered/focused content color without changing ordinary menu text.
The `MenuBar` and `MenuPopup` visuals fill the persistent bar and popup surfaces. Menu surfaces
receive the same installed asset revision as window chrome and retained widgets and paint it directly. The
current keyboard-selected heading or row uses `MenuState::Focused`, an open title uses
`MenuState::Open`, and active window states
continue to mark the owner while menu scope suspends its application widget cue.

Keyboard navigation is intrinsic to menu surfaces, but mnemonics and shortcut dispatch remain
outside the menu component. Shortcut hints are presentation only, and applications still own
check/radio state transitions and accelerator policy.
