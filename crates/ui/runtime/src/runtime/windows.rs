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
        SurfaceMutationError::Busy => UI_E_BUSY,
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

/// Ordinary window placement is data so all three policies share one COM initializer.
/// The enclosing transaction authenticates any parent before mounting content.
enum WindowPlacement {
    /// Independent visible window in an application-selectable fixed layer.
    Root,
    /// Visible structural child of an ordinary window.
    Child(WindowHandle),
    /// Initially hidden modal dialog owned by an ordinary window.
    Dialog(WindowHandle),
}

/// Construction distinguishes window and popup contracts without erasing their queues.
enum Placement {
    /// Ordinary placement and the one constructor-selected window receiver.
    Window(WindowPlacement, Option<ComPtr<neocogi_ui_core_contracts::IEventReceiverAbi>>),
    /// Hidden auto-sized popup owned by a live ordinary window or dialog.
    Popup(WindowHandle, Option<ComPtr<neocogi_ui_core_contracts::IEventReceiverAbi>>),
}

impl RuntimeStorage {
    /// Resolves an actual manager-owned COM identity without trusting caller-provided numeric IDs.
    pub(super) fn parent(&self, parent: ComRef<'_, IWindowAbi>) -> Result<WindowHandle, NcStatus> {
        let identity: ComPtr<IUnknown> = parent.try_add_ref()?.query()?;
        self.mounts
            .borrow()
            .iter()
            .find_map(|mount| {
                if mount.identity.as_raw().object != identity.as_raw().object {
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
        root: neocogi_com::ComBox<neocogi_ui_core_contracts::IWidgetAbi>,
    ) -> Result<ComPtr<IUnknown>, (NcStatus, neocogi_com::ComBox<neocogi_ui_core_contracts::IWidgetAbi>)> {
        let prepared = (|| {
            self.factory.host.check_mutation()?;
            if rect.width < 0 || rect.height < 0 {
                return Err(NC_E_INVALID_ARG);
            }
            let title = checked_title(title)?;
            let context = self.context()?;
            self.prune_mounts(&context.window_manager);
            match &placement {
                Placement::Window(WindowPlacement::Root, _) => {}
                Placement::Window(WindowPlacement::Child(parent) | WindowPlacement::Dialog(parent), _) => {
                    if context.window_manager.window_layer(parent).map_err(status)? == LayerBinding::Modal {
                        return Err(UI_E_POLICY);
                    }
                }
                Placement::Popup(parent, _) => {
                    context.window_manager.window_visible(parent).map_err(status)?;
                }
            }
            Ok((title, context))
        })();
        let (title, mut context) = match prepared {
            Ok(prepared) => prepared,
            Err(status) => return Err((status, root)),
        };
        // Root windows and child collections use the same checked mount boundary.
        let node = self.factory.mount_node(root, None)?;
        let manager = &mut context.window_manager;
        let (identity, surface) = match placement {
            Placement::Window(placement, queue) => {
                LogicalWindow::create(Rc::downgrade(self), manager, placement, Window::new(title, local_rect(rect), node), queue)
            }
            Placement::Popup(parent, queue) => {
                let popup = manager
                    .create_popup(&parent, &title, node, None)
                    .expect("authenticated popup owner cannot disappear during an exclusive transaction");
                LogicalPopup::create(Rc::downgrade(self), manager, popup, queue)
            }
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
        root: neocogi_com::ComBox<neocogi_ui_core_contracts::IWidgetAbi>,
        events: neocogi_com::NcValueRef<'_, neocogi_ui_core_contracts::EventBinding>,
    ) -> Result<ComPtr<IWindowAbi>, (NcStatus, neocogi_com::ComBox<neocogi_ui_core_contracts::IWidgetAbi>)> {
        let prepared = (|| {
            let queue = events.to_owned()?.into_queue();
            Ok(Placement::Window(WindowPlacement::Root, queue))
        })();
        let placement = match prepared {
            Ok(placement) => placement,
            Err(status) => return Err((status, root)),
        };
        let surface = self.storage.create(placement, title, rect, root)?;
        // Construction chose this exact concrete class; publication cannot fail.
        Ok(surface.query::<IWindowAbi>().expect("constructed surface declares its public interface"))
    }
    /// Authenticates the parent's COM identity before validating ordinary-child policy.
    fn create_child(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        title: NcCopyFromBufferOffset<'_, u8>,
        rect: Rect,
        root: neocogi_com::ComBox<neocogi_ui_core_contracts::IWidgetAbi>,
        events: neocogi_com::NcValueRef<'_, neocogi_ui_core_contracts::EventBinding>,
    ) -> Result<ComPtr<IWindowAbi>, (NcStatus, neocogi_com::ComBox<neocogi_ui_core_contracts::IWidgetAbi>)> {
        let prepared = (|| {
            let queue = events.to_owned()?.into_queue();
            Ok(Placement::Window(WindowPlacement::Child(self.storage.parent(parent)?), queue))
        })();
        let placement = match prepared {
            Ok(placement) => placement,
            Err(status) => return Err((status, root)),
        };
        let surface = self.storage.create(placement, title, rect, root)?;
        // Construction chose this exact concrete class; publication cannot fail.
        Ok(surface.query::<IWindowAbi>().expect("constructed surface declares its public interface"))
    }
    /// Creates a hidden dialog while preserving the manager's existing modal-owner rules.
    fn create_dialog(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        title: NcCopyFromBufferOffset<'_, u8>,
        rect: Rect,
        root: neocogi_com::ComBox<neocogi_ui_core_contracts::IWidgetAbi>,
        events: neocogi_com::NcValueRef<'_, neocogi_ui_core_contracts::EventBinding>,
    ) -> Result<ComPtr<IWindowAbi>, (NcStatus, neocogi_com::ComBox<neocogi_ui_core_contracts::IWidgetAbi>)> {
        let prepared = (|| {
            let queue = events.to_owned()?.into_queue();
            Ok(Placement::Window(WindowPlacement::Dialog(self.storage.parent(parent)?), queue))
        })();
        let placement = match prepared {
            Ok(placement) => placement,
            Err(status) => return Err((status, root)),
        };
        let surface = self.storage.create(placement, title, rect, root)?;
        // Construction chose this exact concrete class; publication cannot fail.
        Ok(surface.query::<IWindowAbi>().expect("constructed surface declares its public interface"))
    }
    /// Creates an independently typed popup with one authoritative parent edge in the forest.
    fn create_popup(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        title: NcCopyFromBufferOffset<'_, u8>,
        root: neocogi_com::ComBox<neocogi_ui_core_contracts::IWidgetAbi>,
        events: neocogi_com::NcValueRef<'_, neocogi_ui_core_contracts::EventBinding>,
    ) -> Result<ComPtr<IPopupAbi>, (NcStatus, neocogi_com::ComBox<neocogi_ui_core_contracts::IWidgetAbi>)> {
        let prepared = (|| {
            let queue = events.to_owned()?.into_queue();
            Ok(Placement::Popup(self.storage.parent(parent)?, queue))
        })();
        let placement = match prepared {
            Ok(placement) => placement,
            Err(status) => return Err((status, root)),
        };
        let surface = self.storage.create(placement, title, Rect::default(), root)?;
        // Construction chose this exact concrete class; publication cannot fail.
        Ok(surface.query::<IPopupAbi>().expect("constructed surface declares its public interface"))
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

#[com_object(interfaces(IWindow))]
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
    /// Publishes a forest record and its COM origin in one infallible transaction
    /// after the caller has validated parent, content and placement policy.
    fn create(
        runtime: Weak<RuntimeStorage>,
        manager: &mut WindowManager,
        placement: WindowPlacement,
        window: Window,
        queue: Option<ComPtr<neocogi_ui_core_contracts::IEventReceiverAbi>>,
    ) -> (ComPtr<IUnknown>, NativeSurface) {
        let window = match placement {
            WindowPlacement::Root => manager.create_window(window, None),
            WindowPlacement::Child(parent) => manager
                .create_child_window(&parent, window, None)
                .expect("authenticated ordinary parent cannot change during an exclusive transaction"),
            WindowPlacement::Dialog(parent) => manager
                .create_dialog(&parent, window, None)
                .expect("authenticated ordinary owner cannot change during an exclusive transaction"),
        };
        let local = Self::into_local(Self {
            runtime,
            window: window.clone(),
            closed: Cell::new(false),
        });
        let origin = local
            .query::<IWindowAbi>()
            .expect("LogicalWindow declares IWindow")
            .downgrade()
            .expect("LogicalWindow has shared ownership");
        // Registration emits no events. The exclusive manager transaction keeps
        // this record private until its initialized COM identity is connected.
        manager.connect_window_events(&window, neocogi_ui_core_contracts::EventPort::with_queue(queue, origin));
        (local.query().expect("every generated object declares IUnknown"), NativeSurface::Window(window))
    }

    /// Secures a live owner while allowing observational reads in layout and paint.
    fn owner(&self) -> Result<Rc<RuntimeStorage>, NcStatus> {
        if self.closed.get() {
            return Err(UI_E_CLOSED);
        }
        let runtime = self.runtime.upgrade().ok_or(neocogi_com::NC_E_DROPPED)?;
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
    /// Executes the parent's removal decision and synchronously drops widget owners.
    /// This is not a close request: active descendants reject it before mutation.
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

#[com_object(interfaces(IPopup))]
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
    /// Completes a freshly registered popup's COM identity and delivery while
    /// its creating transaction still owns the manager. The caller must not
    /// expose the native handle or invoke popup operations before this returns.
    pub(super) fn create(
        runtime: Weak<RuntimeStorage>,
        manager: &mut WindowManager,
        popup: PopupHandle,
        queue: Option<ComPtr<neocogi_ui_core_contracts::IEventReceiverAbi>>,
    ) -> (ComPtr<IUnknown>, NativeSurface) {
        let local = Self::into_local(Self {
            runtime,
            popup: popup.clone(),
            closed: false,
        });
        let origin = local
            .query::<IPopupAbi>()
            .expect("LogicalPopup declares IPopup")
            .downgrade()
            .expect("LogicalPopup has shared ownership");
        manager.connect_popup_events(&popup, neocogi_ui_core_contracts::EventPort::with_queue(queue, origin));
        (local.query().expect("every generated object declares IUnknown"), NativeSurface::Popup(popup))
    }

    /// Retains the live owner for one synchronous checked call.
    fn owner(&self) -> Result<Rc<RuntimeStorage>, NcStatus> {
        if self.closed {
            return Err(UI_E_CLOSED);
        }
        let runtime = self.runtime.upgrade().ok_or(neocogi_com::NC_E_DROPPED)?;
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
    /// Removes the definition and expires weak widget observations after native detach.
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
        let owner = ui.window("pointer popup owner", Rect { x: 20, y: 20, width: 400, height: 200 }, body).unwrap();
        let menus = ui.menus().unwrap();
        let builder = menus.create("Pointer actions").unwrap();
        let (events_tx, events) = std::sync::mpsc::channel();
        builder
            .item_with_queue(
                "Run at pointer",
                Some(
                    neocogi_ui::test_event_queue!(neocogi_ui::IMenuItemAbi, String, events_tx.clone(), |_queue, _origin, event| {
                        event.query_ref::<neocogi_ui::IActivatedEventAbi>()?;
                        "Run at pointer".to_owned()
                    })
                    .unwrap()
                    .as_com_ref(),
                ),
            )
            .unwrap();
        let mut popup = menus.popup(owner.as_com_ref(), &builder).unwrap();
        let labels = Vec::new();
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

#[cfg(test)]
mod placement_tests {
    use neocogi_ui::{IWindowAbi, Rect, Visibility, IWindowHiddenEventAbi, IWindowDestroyedEventAbi};

    /// The shared initializer preserves each placement's visibility, public event
    /// origin and complete subtree closure through actual COM calls.
    #[test]
    fn window_placements_share_one_queue_without_sharing_origins() {
        let runtime = crate::test_support::create_runtime(crate::test_support::test_atlas());
        let ui = nc_ui_nodes::bind_builtin_nodes(runtime).unwrap();
        let (queue, events) = neocogi_ui::events::channel();
        let weak_queue = queue.downgrade().unwrap();
        let rect = Rect { x: 20, y: 20, width: 320, height: 240 };
        let root = ui
            .window_with_queue("root", rect, ui.widgets().button_unobserved("root").unwrap(), Some(queue.as_com_ref()))
            .unwrap();
        let child = ui
            .child_window_with_queue(
                root.as_com_ref(),
                "child",
                rect,
                ui.widgets().button_unobserved("child").unwrap(),
                Some(queue.as_com_ref()),
            )
            .unwrap();
        let dialog = ui
            .dialog_with_queue(
                root.as_com_ref(),
                "dialog",
                rect,
                ui.widgets().button_unobserved("dialog").unwrap(),
                Some(queue.as_com_ref()),
            )
            .unwrap();
        // All three retain the exact same application receiver. They must publish
        // their own window projection as origin, regardless of placement policy.
        drop(queue);
        assert_eq!(root.snapshot().unwrap().visibility, Visibility::Visible);
        assert_eq!(child.snapshot().unwrap().visibility, Visibility::Visible);
        assert_eq!(dialog.snapshot().unwrap().visibility, Visibility::Hidden);
        let origins = [&root, &child, &dialog].map(|window| window.downgrade().unwrap());
        // Geometry setters are intentionally silent. Visibility transitions are
        // lifecycle events; exercise each placement's receiver using a real hide.
        for (index, window) in [&root, &child, &dialog].into_iter().enumerate() {
            window.set_visibility(Visibility::Visible).unwrap();
            window.set_visibility(Visibility::Hidden).unwrap();
            let found = events.try_iter().any(|message| {
                message.origin.service::<IWindowAbi>().unwrap().unwrap().same_object(&origins[index])
                    && message.query::<IWindowHiddenEventAbi>().unwrap().is_some()
            });
            assert!(found, "placement {index} must identify its own visibility event");
            window.set_visibility(Visibility::Visible).unwrap();
        }
        root.close().unwrap();
        let closed: Vec<_> = events
            .try_iter()
            .filter_map(|message| {
                message
                    .query::<IWindowDestroyedEventAbi>()
                    .unwrap()
                    .map(|_| message.origin.service::<IWindowAbi>().unwrap().unwrap())
            })
            .collect();
        assert_eq!(closed.len(), 3);
        assert!(closed.iter().any(|origin| origin.same_object(&origins[0])));
        assert!(closed.iter().any(|origin| origin.same_object(&origins[1])));
        assert!(closed.iter().any(|origin| origin.same_object(&origins[2])));
        assert!(weak_queue.upgrade().unwrap().is_none(), "closure releases all three receiver references");
        assert!(
            origins.iter().all(|origin| origin.upgrade().unwrap().is_some()),
            "caller handles retain closed values"
        );
        drop((root, child, dialog));
        assert!(origins.iter().all(|origin| origin.upgrade().unwrap().is_none()));
    }
}
