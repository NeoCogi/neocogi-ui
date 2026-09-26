//! Concrete external window and popup projections reuse the manager's authoritative forest.
use super::*;
use crate::{
    PopupHandle, WindowHandle, Window,
    window_manager::{WindowManager, SurfaceMutationError, WindowOption, LayerBinding, ChildWindowClip},
};
use neocogi_com::{INcStringAbi, NcString};
use std::rc::Weak;

/// Preserves the distinction between window and popup identities inside the implementation.
#[derive(Clone)]
pub(super) enum NativeSurface {
    /// Ordinary windows and modal dialogs use window-only manager operations.
    Window(WindowHandle),
    /// Popup placement/lifetime remains its own capability family.
    Popup(PopupHandle),
}

impl NativeSurface {
    /// Resolves lifetime from the forest rather than retaining a shadow widget owner.
    pub(super) fn is_alive(&self, manager: &WindowManager) -> bool {
        match self {
            Self::Window(window) => manager.window_visible(window).is_ok(),
            Self::Popup(popup) => manager.popup_visible(popup).is_ok(),
        }
    }

    /// Idempotently removes the native subtree; an ancestor may already have removed it.
    pub(super) fn close(&self, manager: &mut WindowManager) -> Result<(), NcStatus> {
        let result = match self {
            Self::Window(window) => manager.destroy_window(window),
            Self::Popup(popup) => manager.destroy_popup(popup),
        };
        match result {
            Ok(()) | Err(SurfaceMutationError::UnknownWindow | SurfaceMutationError::UnknownPopup) => Ok(()),
            Err(error) => Err(status(error)),
        }
    }
}

/// Converts concrete policy errors at the ABI boundary without conflating closure and policy.
pub(super) fn status(error: SurfaceMutationError) -> NcStatus {
    match error {
        SurfaceMutationError::UnknownWindow | SurfaceMutationError::UnknownPopup => UI_E_CLOSED,
        SurfaceMutationError::InvalidLayer(_) => NC_E_INVALID_ARG,
        _ => UI_E_POLICY,
    }
}

/// Checks a title before either UTF-8 scanning or copying it into retained chrome state.
fn checked_title(title: NcCopyFromBufferOffset<'_, u8>) -> Result<String, NcStatus> {
    if title.offset() != 0 {
        return Err(NC_E_INVALID_ARG);
    }
    if title.as_slice().len() > MAX_WINDOW_TITLE_BYTES {
        return Err(neocogi_com::NC_E_OUT_OF_RANGE);
    }
    Ok(text(title)?.to_owned())
}

/// Concrete creation policy; the parent capability is authenticated before this value is built.
enum Placement {
    /// Independent window in an application-selectable fixed layer.
    Root,
    /// Visible structural child of an ordinary window.
    Child(WindowHandle),
    /// Hidden modal dialog owned by an ordinary window.
    Dialog(WindowHandle),
    /// Hidden auto-sized popup owned by a live ordinary window or dialog.
    Popup(WindowHandle),
}

impl RuntimeStorage {
    /// Resolves an actual manager-owned COM identity without trusting caller-provided numeric IDs.
    pub(super) fn parent(&self, parent: ComRef<'_, IWindowAbi>) -> Result<WindowHandle, NcStatus> {
        let identity: ComPtr<IUnknown> = parent.add_ref().query()?;
        self.mounts
            .borrow()
            .iter()
            .find_map(|mount| {
                if mount.identity.as_ptr() != identity.as_ptr() {
                    return None;
                }
                match &mount.surface {
                    NativeSurface::Window(window) => Some(window.clone()),
                    NativeSurface::Popup(_) => None,
                }
            })
            .ok_or(UI_E_OWNER)
    }

    /// Shares all attachment validation and ownership transfer across concrete creation methods.
    fn create(
        self: &Rc<Self>,
        placement: Placement,
        title: NcCopyFromBufferOffset<'_, u8>,
        rect: Rect,
        root: neocogi_com::NcMessage<'_, Mount>,
    ) -> Result<ComPtr<IUnknown>, NcStatus> {
        self.factory.host.check_mutation()?;
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let title = checked_title(title)?;
        let mut context = self.context()?;
        self.prune_mounts(&context.window_manager);
        match &placement {
            Placement::Root => {}
            Placement::Child(parent) | Placement::Dialog(parent) => {
                if context.window_manager.window_layer(parent).map_err(status)? == LayerBinding::Modal {
                    return Err(UI_E_POLICY);
                }
            }
            Placement::Popup(parent) => {
                context.window_manager.window_visible(parent).map_err(status)?;
            }
        }
        // Root windows and child collections use the same checked mount boundary.
        let node = self.factory.mount_node(root, None)?;
        let manager = &mut context.window_manager;
        let surface = match placement {
            Placement::Root => NativeSurface::Window(manager.create_window(Window::new(title, local_rect(rect), node))),
            Placement::Child(parent) => NativeSurface::Window(
                manager
                    .create_child_window(&parent, Window::new(title, local_rect(rect), node))
                    .expect("authenticated ordinary parent cannot change during an exclusive transaction"),
            ),
            Placement::Dialog(parent) => NativeSurface::Window(
                manager
                    .create_dialog(&parent, Window::new(title, local_rect(rect), node))
                    .expect("authenticated ordinary owner cannot change during an exclusive transaction"),
            ),
            Placement::Popup(parent) => NativeSurface::Popup(
                manager
                    .create_popup(&parent, &title, node)
                    .expect("authenticated popup owner cannot disappear during an exclusive transaction"),
            ),
        };
        let identity = match &surface {
            NativeSurface::Window(window) => LogicalWindow::from_value(LogicalWindow {
                runtime: Rc::downgrade(self),
                window: window.clone(),
                closed: Cell::new(false),
            }),
            NativeSurface::Popup(popup) => LogicalPopup::from_value(LogicalPopup {
                runtime: Rc::downgrade(self),
                popup: popup.clone(),
                closed: false,
            }),
        };
        self.mounts.borrow_mut().push(SurfaceMount { surface, identity: identity.clone() });
        self.pending.set(true);
        Ok(identity)
    }
}

impl IWindowManager for Runtime {
    /// Creates an independent root through the shared transactional attachment path.
    fn create_window(
        &self,
        title: NcCopyFromBufferOffset<'_, u8>,
        rect: Rect,
        root: neocogi_com::NcMessage<'_, Mount>,
    ) -> Result<ComPtr<IWindowAbi>, NcStatus> {
        self.storage.create(Placement::Root, title, rect, root)?.query()
    }
    /// Authenticates the parent's COM identity before validating ordinary-child policy.
    fn create_child(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        title: NcCopyFromBufferOffset<'_, u8>,
        rect: Rect,
        root: neocogi_com::NcMessage<'_, Mount>,
    ) -> Result<ComPtr<IWindowAbi>, NcStatus> {
        self.storage.create(Placement::Child(self.storage.parent(parent)?), title, rect, root)?.query()
    }
    /// Creates a hidden dialog while preserving the manager's existing modal-owner rules.
    fn create_dialog(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        title: NcCopyFromBufferOffset<'_, u8>,
        rect: Rect,
        root: neocogi_com::NcMessage<'_, Mount>,
    ) -> Result<ComPtr<IWindowAbi>, NcStatus> {
        self.storage.create(Placement::Dialog(self.storage.parent(parent)?), title, rect, root)?.query()
    }
    /// Creates an independently typed popup with one authoritative parent edge in the forest.
    fn create_popup(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        title: NcCopyFromBufferOffset<'_, u8>,
        root: neocogi_com::NcMessage<'_, Mount>,
    ) -> Result<ComPtr<IPopupAbi>, NcStatus> {
        self.storage
            .create(Placement::Popup(self.storage.parent(parent)?), title, Rect::default(), root)?
            .query()
    }
}

/// Explicit data for a window edit; no closure hides the data or operation being applied.
enum WindowEdit {
    /// Replaces screen allocation and exits manager-owned minimize/maximize modes.
    Rect(Rect),
    /// Replaces UTF-8 title text.
    Title(String),
    /// Changes local visibility intent.
    Visibility(bool),
    /// Changes concrete presentation bits.
    Options(WindowOption),
    /// Selects an independent fixed layer.
    Layer(u8),
    /// Changes chronology within the existing structural stacking band.
    Raise,
    /// Changes clipping of complete structural child surfaces.
    ChildClip(ChildWindowClip),
}

#[com_object(interfaces(IWindow, IWindowEvents))]
/// One retained COM window capability; all semantic state remains in the manager's forest.
struct LogicalWindow {
    /// Weak owner access avoids a manager/window reference cycle.
    pub(super) runtime: Weak<RuntimeStorage>,
    /// Native identity never crosses the ABI or becomes an application property.
    window: WindowHandle,
    /// Explicit close is idempotent independently of parent destruction.
    pub(super) closed: Cell<bool>,
}

impl LogicalWindow {
    /// Secures a live owner while allowing observational reads in layout and paint.
    fn owner(&self) -> Result<Rc<RuntimeStorage>, NcStatus> {
        if self.closed.get() {
            return Err(UI_E_CLOSED);
        }
        let runtime = self.runtime.upgrade().ok_or(UI_E_CLOSED)?;
        runtime.open()?;
        Ok(runtime)
    }
    /// Applies one fully specified edit under the common phase/reentry checks.
    fn edit(&self, edit: WindowEdit) -> Result<(), NcStatus> {
        let runtime = self.owner()?;
        runtime.factory.host.check_mutation()?;
        let mut context = runtime.context()?;
        let manager = &mut context.window_manager;
        match edit {
            WindowEdit::Rect(rect) => manager.set_window_rect(&self.window, local_rect(rect)),
            WindowEdit::Title(title) => manager.set_window_name(&self.window, title),
            WindowEdit::Visibility(visible) => manager.set_window_visible(&self.window, visible),
            WindowEdit::Options(options) => manager.set_window_options(&self.window, options),
            WindowEdit::Layer(layer) => manager.set_window_layer(&self.window, layer),
            WindowEdit::Raise => manager.bring_window_to_front(&self.window),
            WindowEdit::ChildClip(clip) => manager.set_child_window_clip(&self.window, clip),
        }
        .map_err(status)?;
        runtime.pending.set(true);
        context.window_manager.event_result()
    }
}

impl IWindow for LogicalWindow {
    /// Copies authoritative policy rather than a facade-maintained shadow.
    fn snapshot(&self) -> Result<WindowSnapshot, NcStatus> {
        let runtime = self.owner()?;
        runtime.context()?.window_manager.window_snapshot(&self.window).map_err(status)
    }
    /// Owns the returned text snapshot beyond this short manager borrow.
    fn title(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        let runtime = self.owner()?;
        let context = runtime.context()?;
        Ok(NcString::from_utf8_str(context.window_manager.window_title(&self.window).map_err(status)?))
    }
    /// Validates a complete UTF-8 replacement before entering mutation.
    fn set_title(&self, title: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.edit(WindowEdit::Title(checked_title(title)?))
    }
    /// Rejects negative allocation before changing manager geometry.
    fn set_rect(&self, rect: Rect) -> Result<(), NcStatus> {
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        self.edit(WindowEdit::Rect(rect))
    }
    /// Preserves the difference between local intent and structurally effective visibility.
    fn set_visibility(&self, visibility: Visibility) -> Result<(), NcStatus> {
        let visible = match visibility {
            Visibility::Visible => true,
            Visibility::Hidden => false,
            _ => return Err(NC_E_INVALID_ARG),
        };
        self.edit(WindowEdit::Visibility(visible))
    }
    /// Rejects unknown bits before the manager reconciles enabled descendants and captures.
    fn set_options(&self, options: WindowOptions) -> Result<(), NcStatus> {
        options.validate()?;
        self.edit(WindowEdit::Options(WindowOption::from_bits(u32::from(options.raw())).ok_or(NC_E_INVALID_ARG)?))
    }
    /// Delegates fixed-layer range and managed-layer policy to the authoritative manager.
    fn set_layer(&self, layer: u8) -> Result<(), NcStatus> {
        self.edit(WindowEdit::Layer(layer))
    }
    /// Raises only inside the current family/layer band.
    fn raise(&self) -> Result<(), NcStatus> {
        self.edit(WindowEdit::Raise)
    }
    /// Changes parent-owned child clipping without altering child-local state.
    fn set_child_clip(&self, clip: ChildClip) -> Result<(), NcStatus> {
        let clip = match clip {
            ChildClip::None => ChildWindowClip::None,
            ChildClip::Content => ChildWindowClip::Content,
            _ => return Err(NC_E_INVALID_ARG),
        };
        self.edit(WindowEdit::ChildClip(clip))
    }
    /// Removes the complete native subtree, then closes every removed COM widget capability.
    fn close(&self) -> Result<(), NcStatus> {
        if self.closed.get() {
            return Ok(());
        }
        let Some(runtime) = self.runtime.upgrade() else {
            self.closed.set(true);
            return Ok(());
        };
        if runtime.factory.state.closed.get() {
            self.closed.set(true);
            return Ok(());
        }
        runtime.factory.host.check_mutation()?;
        let mut context = runtime.context()?;
        runtime.factory.state.error.set(None);
        NativeSurface::Window(self.window.clone()).close(&mut context.window_manager)?;
        self.closed.set(true);
        runtime.pending.set(true);
        runtime.prune_mounts(&context.window_manager);
        let posted = context.window_manager.event_result();
        let phase = runtime.factory.state.error.take().map_or(Ok(()), Err);
        posted.and(phase)
    }
}

#[com_object(interfaces(IPopup, IPopupEvents))]
/// Popup projection has its own placement/lifetime methods and cannot impersonate a window.
pub(super) struct LogicalPopup {
    /// Retaining the popup never retains the runtime through a strong cycle.
    pub(super) runtime: Weak<RuntimeStorage>,
    /// Private identity of the manager-owned popup definition.
    pub(super) popup: PopupHandle,
    /// Explicit close is idempotent after removal.
    pub(super) closed: bool,
}

/// Explicit popup mutation payloads use the same concrete dispatch pattern as window edits.
enum PopupEdit {
    /// Read the manager's latest consumed pointer and activate the transient input scope there.
    Show,
    /// Replace the anchor with an exact screen rectangle and activate the transient input scope atomically.
    ShowAt {
        /// Caller-provided screen-space geometry copied before entering the manager transaction.
        anchor: Rect,
    },
    /// Dismiss this active branch while retaining its definition.
    Hide,
    /// Replace popup appearance and automatic sizing.
    Options {
        /// Fully validated popup presentation flags translated into the manager's native type.
        options: WindowOption,
    },
}

impl LogicalPopup {
    /// Retains the live owner for one synchronous checked call.
    fn owner(&self) -> Result<Rc<RuntimeStorage>, NcStatus> {
        if self.closed {
            return Err(UI_E_CLOSED);
        }
        let runtime = self.runtime.upgrade().ok_or(UI_E_CLOSED)?;
        runtime.open()?;
        Ok(runtime)
    }
    /// Shares phase/reentry checks while keeping each mutation's data explicit.
    fn edit(&mut self, edit: PopupEdit) -> Result<(), NcStatus> {
        let runtime = self.owner()?;
        runtime.factory.host.check_mutation()?;
        let mut context = runtime.context()?;
        match edit {
            // Pointer-relative placement is resolved inside the same manager transaction that
            // activates the popup, so application code cannot observe an intermediate anchor.
            PopupEdit::Show => context.window_manager.show_popup(&self.popup),
            // Explicit placement keeps the caller's copied screen rectangle authoritative.
            PopupEdit::ShowAt { anchor } => context.window_manager.show_popup_at(&self.popup, local_rect(anchor)),
            // Hiding truncates the active popup branch without destroying its retained definition.
            PopupEdit::Hide => context.window_manager.hide_popup(&self.popup),
            // Option changes remain popup-specific and cannot accidentally mutate window chrome.
            PopupEdit::Options { options } => context.window_manager.set_popup_options(&self.popup, options),
        }
        .map_err(status)?;
        runtime.pending.set(true);
        context.window_manager.event_result()
    }
}

impl IPopup for LogicalPopup {
    /// Reads actual active-path membership, including modal and owner eligibility.
    fn visibility(&self) -> Result<Visibility, NcStatus> {
        let runtime = self.owner()?;
        let visible = runtime.context()?.window_manager.popup_visible(&self.popup).map_err(status)?;
        Ok(if visible { Visibility::Visible } else { Visibility::Hidden })
    }
    /// Uses the pointer snapshot committed by the input event immediately preceding this callback.
    fn show(&mut self) -> Result<(), NcStatus> {
        self.edit(PopupEdit::Show)
    }
    /// Validates allocation before atomically changing the exact anchor and input scope.
    fn show_at(&mut self, anchor: Rect) -> Result<(), NcStatus> {
        if anchor.width < 0 || anchor.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        self.edit(PopupEdit::ShowAt { anchor })
    }
    /// Dismisses active descendants through existing manager policy.
    fn hide(&mut self) -> Result<(), NcStatus> {
        self.edit(PopupEdit::Hide)
    }
    /// Rejects unknown bits and preserves popup-only chrome restrictions.
    fn set_options(&mut self, options: WindowOptions) -> Result<(), NcStatus> {
        options.validate()?;
        self.edit(PopupEdit::Options {
            options: WindowOption::from_bits(u32::from(options.raw())).ok_or(NC_E_INVALID_ARG)?,
        })
    }
    /// Removes the definition and closes retained widget handles after the native detach.
    fn close(&mut self) -> Result<(), NcStatus> {
        if self.closed {
            return Ok(());
        }
        let Some(runtime) = self.runtime.upgrade() else {
            self.closed = true;
            return Ok(());
        };
        if runtime.factory.state.closed.get() {
            self.closed = true;
            return Ok(());
        }
        runtime.factory.host.check_mutation()?;
        let mut context = runtime.context()?;
        runtime.factory.state.error.set(None);
        NativeSurface::Popup(self.popup.clone()).close(&mut context.window_manager)?;
        self.closed = true;
        runtime.pending.set(true);
        runtime.prune_mounts(&context.window_manager);
        let posted = context.window_manager.event_result();
        let phase = runtime.factory.state.error.take().map_or(Ok(()), Err);
        posted.and(phase)
    }
}

impl IWindowEventsStream for LogicalWindow {
    /// Resolves the existing owner and installs delivery directly in its record.
    fn attach(&mut self, destination: ComRef<'_, IWindowEventsSinkAbi>) -> Result<(), NcStatus> {
        let runtime = self.owner()?;
        runtime.factory.host.check_mutation()?;
        runtime.context()?.window_manager.map_window_events(&self.window, destination.add_ref())
    }
}
impl IPopupEventsStream for LogicalPopup {
    /// Resolves the existing owner and installs delivery directly in its record.
    fn attach(&mut self, destination: ComRef<'_, IPopupEventsSinkAbi>) -> Result<(), NcStatus> {
        let runtime = self.owner()?;
        runtime.factory.host.check_mutation()?;
        runtime.context()?.window_manager.map_popup_events(&self.popup, destination.add_ref())
    }
}

#[cfg(test)]
mod tests {
    use neocogi_ui_support::test_support::events::TestApp;
    use neocogi_ui::{Point, Rect, Size, UiInput, Visibility};

    /// Pointer-relative public popup activation uses the latest input position consumed by the runtime.
    #[test]
    fn popup_show_places_a_standalone_menu_at_the_current_pointer() {
        // Construct the production COM runtime and public clients so the regression crosses the
        // same popup capability boundary used by the full demo.
        let runtime = crate::test_support::create_runtime(crate::test_support::test_atlas());
        let ui = nc_ui_nodes::bind_builtin_nodes(runtime).unwrap();
        let body = ui.widgets().button_unobserved("body").unwrap();
        let owner = ui.window("pointer popup owner", Rect { x: 20, y: 20, width: 400, height: 200 }, &body).unwrap();
        let menus = ui.menus().unwrap();
        let builder = menus.create("Pointer actions").unwrap();
        let item = builder.item("Run at pointer").unwrap();
        let mut popup = menus.popup(owner.as_com_ref(), &builder).unwrap();
        let (events_tx, events) = std::sync::mpsc::channel();
        let labels = Vec::new();
        neocogi_ui::events::bind(item.activated().unwrap(), events_tx.clone(), |event| event.label).unwrap();
        let size = Size { width: 640, height: 400 };

        // Consume a pointer transition before opening. The popup request must use this committed
        // screen position rather than the popup definition's default rectangle or its owner origin.
        let pointer = Point { x: 300, y: 250 };
        ui.input(UiInput::Pointer(pointer), "").unwrap();
        let app = TestApp::new(labels, events, |state, message| {
            state.push(message);
            Ok(())
        });
        ui.update_with_events(size, app.as_interface()).unwrap();
        popup.show().unwrap();
        ui.update_with_events(size, app.as_interface()).unwrap();

        // Press just inside the expected popup frame. Reaching the first row proves the coordinates
        // crossed IPopup and were used by the manager's subsequent hit testing.
        let first_row = Point { x: pointer.x + 2, y: pointer.y + 2 };
        ui.input(UiInput::Down { position: first_row, buttons: 1 }, "").unwrap();
        ui.update_with_events(size, app.as_interface()).unwrap();
        assert_eq!(&*app.state(), &["Run at pointer"]);
        assert_eq!(popup.visibility(), Ok(Visibility::Hidden));

        // Explicit closure verifies the newly exercised operation did not alter retained lifetime
        // ownership for either the popup definition or its window.
        popup.close().unwrap();
        owner.close().unwrap();
    }
}

/// Marker projection shares the same generic stream and object identity.
impl IWindowEvents for LogicalWindow {}

/// Marker projection shares the same generic stream and object identity.
impl IPopupEvents for LogicalPopup {}
