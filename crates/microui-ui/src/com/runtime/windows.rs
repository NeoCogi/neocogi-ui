//! Concrete window/popup COM projections reuse the manager's single authoritative forest.
use super::*;
use crate::{
    PopupHandle, WindowHandle, Window,
    window_manager::{WindowManager, SurfaceMutationError, WindowOption, LayerBinding, ChildWindowClip},
};
use nc_com::{INcStringAbi, NcString};
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
        root: ComRef<'_, IWidgetAbi>,
    ) -> Result<ComPtr<IUnknown>, NcStatus> {
        self.factory.host.check_mutation()?;
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let title = text(title)?.to_owned();
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
                closed: Cell::new(false),
            }),
        };
        self.mounts.borrow_mut().push(Mount { surface, identity: identity.clone() });
        self.pending.set(true);
        Ok(identity)
    }
}

impl IWindowManager for Runtime {
    /// Creates an independent root through the shared transactional attachment path.
    fn create_window(&self, title: NcCopyFromBufferOffset<'_, u8>, rect: Rect, root: ComRef<'_, IWidgetAbi>) -> Result<ComPtr<IWindowAbi>, NcStatus> {
        self.storage.create(Placement::Root, title, rect, root)?.query()
    }
    /// Authenticates the parent's COM identity before validating ordinary-child policy.
    fn create_child(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        title: NcCopyFromBufferOffset<'_, u8>,
        rect: Rect,
        root: ComRef<'_, IWidgetAbi>,
    ) -> Result<ComPtr<IWindowAbi>, NcStatus> {
        self.storage.create(Placement::Child(self.storage.parent(parent)?), title, rect, root)?.query()
    }
    /// Creates a hidden dialog while preserving the manager's existing modal-owner rules.
    fn create_dialog(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        title: NcCopyFromBufferOffset<'_, u8>,
        rect: Rect,
        root: ComRef<'_, IWidgetAbi>,
    ) -> Result<ComPtr<IWindowAbi>, NcStatus> {
        self.storage.create(Placement::Dialog(self.storage.parent(parent)?), title, rect, root)?.query()
    }
    /// Creates an independently typed popup with one authoritative parent edge in the forest.
    fn create_popup(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        title: NcCopyFromBufferOffset<'_, u8>,
        root: ComRef<'_, IWidgetAbi>,
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
        Ok(())
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
        self.edit(WindowEdit::Title(text(title)?.to_owned()))
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
            Visibility::VISIBLE => true,
            Visibility::HIDDEN => false,
            _ => return Err(NC_E_INVALID_ARG),
        };
        self.edit(WindowEdit::Visibility(visible))
    }
    /// Rejects unknown bits before the manager reconciles enabled descendants and captures.
    fn set_options(&self, options: WindowOptions) -> Result<(), NcStatus> {
        options.validate()?;
        self.edit(WindowEdit::Options(WindowOption::from_bits(options.bits).ok_or(NC_E_INVALID_ARG)?))
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
            ChildClip::NONE => ChildWindowClip::None,
            ChildClip::CONTENT => ChildWindowClip::Content,
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
        runtime.factory.state.error.take().map_or(Ok(()), Err)
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
    pub(super) closed: Cell<bool>,
}

/// Explicit popup mutation payloads use the same concrete dispatch pattern as window edits.
enum PopupEdit {
    /// Replace anchor and activate the transient input scope atomically.
    Show(Rect),
    /// Dismiss this active branch while retaining its definition.
    Hide,
    /// Replace popup appearance and automatic sizing.
    Options(WindowOption),
}

impl LogicalPopup {
    /// Retains the live owner for one synchronous checked call.
    fn owner(&self) -> Result<Rc<RuntimeStorage>, NcStatus> {
        if self.closed.get() {
            return Err(UI_E_CLOSED);
        }
        let runtime = self.runtime.upgrade().ok_or(UI_E_CLOSED)?;
        runtime.open()?;
        Ok(runtime)
    }
    /// Shares phase/reentry checks while keeping each mutation's data explicit.
    fn edit(&self, edit: PopupEdit) -> Result<(), NcStatus> {
        let runtime = self.owner()?;
        runtime.factory.host.check_mutation()?;
        let mut context = runtime.context()?;
        match edit {
            PopupEdit::Show(rect) => context.window_manager.show_popup_at(&self.popup, local_rect(rect)),
            PopupEdit::Hide => context.window_manager.hide_popup(&self.popup),
            PopupEdit::Options(options) => context.window_manager.set_popup_options(&self.popup, options),
        }
        .map_err(status)?;
        runtime.pending.set(true);
        Ok(())
    }
}

impl IPopup for LogicalPopup {
    /// Reads actual active-path membership, including modal and owner eligibility.
    fn visibility(&self) -> Result<Visibility, NcStatus> {
        let runtime = self.owner()?;
        let visible = runtime.context()?.window_manager.popup_visible(&self.popup).map_err(status)?;
        Ok(if visible { Visibility::VISIBLE } else { Visibility::HIDDEN })
    }
    /// Validates allocation before atomically changing anchor and input scope.
    fn show(&self, anchor: Rect) -> Result<(), NcStatus> {
        if anchor.width < 0 || anchor.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        self.edit(PopupEdit::Show(anchor))
    }
    /// Dismisses active descendants through existing manager policy.
    fn hide(&self) -> Result<(), NcStatus> {
        self.edit(PopupEdit::Hide)
    }
    /// Rejects unknown bits and preserves popup-only chrome restrictions.
    fn set_options(&self, options: WindowOptions) -> Result<(), NcStatus> {
        options.validate()?;
        self.edit(PopupEdit::Options(WindowOption::from_bits(options.bits).ok_or(NC_E_INVALID_ARG)?))
    }
    /// Removes the definition and closes retained widget handles after the native detach.
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
        NativeSurface::Popup(self.popup.clone()).close(&mut context.window_manager)?;
        self.closed.set(true);
        runtime.pending.set(true);
        runtime.prune_mounts(&context.window_manager);
        runtime.factory.state.error.take().map_or(Ok(()), Err)
    }
}

impl IWindowEvents for LogicalWindow {
    /// Uses the manager's actual native source, releasing owner checks before foreign sink retention.
    fn subscribe(&self, sink: ComRef<'_, IWindowSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.owner()?.factory.host.check_mutation()?;
        crate::event::subscribe(self.window.events(), sink)
    }
}
impl IPopupEvents for LogicalPopup {
    /// Uses the popup's existing dismissal source without creating a second lifecycle record or queue.
    fn subscribe(&self, sink: ComRef<'_, IPopupSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.owner()?.factory.host.check_mutation()?;
        crate::event::subscribe(self.popup.events(), sink)
    }
}
