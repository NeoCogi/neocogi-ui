//! Native window-authoring fixture used by retained semantic tests.
//! Deployed applications use the facade Ui and actual COM window interfaces.
use crate::{Recti, Dimensioni, Menu, Node};
use crate::window_manager::{WindowManager, LayerBinding, PopupHandle, SurfaceCreationError, SurfaceMutationError, Window, WindowHandle, WindowOption};
/// Short-lived access to retained UI state owned by a [`crate::Context`].
///
/// Ordinary application code obtains this façade through [`crate::Context::ui`]. Context-aware event
/// handlers receive the same concrete type after retained widget traversal has released every
/// widget borrow. Keeping surface mutation on this non-generic façade prevents the renderer-owning
/// [`crate::Context`] and event dispatch from maintaining duplicate APIs.
///
/// `Ui` owns no surfaces, application state, or renderer resources. Its lifetime is tied to the
/// exclusive `Context` or dispatch borrow that created it, so it cannot escape a transaction or
/// re-enter an active retained traversal.
pub struct Ui<'a> {
    /// Exclusive access to the context-owned surface, input, and style transaction domain.
    window_manager: &'a mut WindowManager,
}

impl<'a> Ui<'a> {
    /// Lends one already-exclusive window manager to application-facing UI operations.
    pub(crate) fn new(window_manager: &'a mut WindowManager) -> Self {
        // Construction stays private so every public capability originates at a Context-controlled
        // borrow boundary, including the post-traversal event-dispatch boundary.
        Self { window_manager }
    }

    /// Creates an open retained window from one complete body and optional menu-bar definition.
    ///
    /// The returned handle is weak; the lending [`crate::Context`] remains the sole window owner.
    pub fn create_window(&mut self, window: Window) -> WindowHandle {
        // Transfer the complete definition to the same owner used by ordinary Context creation.
        self.window_manager.create_window(window)
    }

    /// Creates an open structural child window below `parent`'s menu and chrome.
    ///
    /// The child keeps screen-space geometry and its own retained application tree, menu, chrome,
    /// visibility, focus, and pointer capture. It inherits the fixed layer of its top-level family
    /// root and participates in chronological z-order only among windows with the same direct
    /// parent. The parent's [`crate::ChildWindowClip`] policy determines whether the complete child
    /// surface is additionally clipped to the parent application body.
    ///
    /// Destroying the parent destroys the child and its descendants. Hiding the parent excludes the
    /// complete family until the parent is shown again. Modal dialogs cannot own child windows.
    ///
    /// # Errors
    ///
    /// Returns an owner-preserving [`SurfaceCreationError`] for a stale, foreign, or modal parent.
    /// [`SurfaceCreationError::reason`] distinguishes
    /// [`SurfaceMutationError::UnknownWindow`] from
    /// [`SurfaceMutationError::InvalidChildWindowParent`], and
    /// [`SurfaceCreationError::into_input`] recovers the unchanged `window` for retry or reuse.
    #[allow(clippy::result_large_err)] // Failure deliberately returns the complete unique Window without allocation or erasure.
    pub fn create_child_window(&mut self, parent: &WindowHandle, window: Window) -> Result<WindowHandle, SurfaceCreationError<Window>> {
        // WindowManager validates the complete parent contract before transferring the Window into
        // its forest, so every failure returns the exact retained tree supplied by the caller.
        self.window_manager.create_child_window(parent, window)
    }

    /// Creates a hidden retained dialog owned by `parent`.
    ///
    /// `parent` must be a live independent or structural child window from this Context. Show the
    /// returned dialog with [`Self::set_window_visible`]; it becomes the front modal group before
    /// the layout immediately following this UI transaction.
    ///
    /// # Errors
    ///
    /// Returns an owner-preserving [`SurfaceCreationError`] when `parent` is stale, foreign, or
    /// modal. Inspect [`SurfaceCreationError::reason`] and reclaim the original `window` with
    /// [`SurfaceCreationError::into_input`].
    #[allow(clippy::result_large_err)] // Failure deliberately returns the complete unique Window without allocation or erasure.
    pub fn create_dialog(&mut self, parent: &WindowHandle, window: Window) -> Result<WindowHandle, SurfaceCreationError<Window>> {
        // Authenticate and classify the parent before the manager consumes the dialog definition.
        self.window_manager.create_dialog(parent, window)
    }

    /// Creates a hidden auto-sized popup owned directly by `parent`.
    ///
    /// The popup definition and retained content tree become a concrete child in the same forest.
    /// The distinct handle prevents popup identity from entering window-only operations.
    ///
    /// # Errors
    ///
    /// Returns [`SurfaceCreationError<Node>`] when `parent` is stale or belongs to another
    /// Context. [`SurfaceCreationError::into_input`] recovers the unchanged content node.
    #[allow(clippy::result_large_err)] // Failure deliberately returns the complete unique Node without allocation or erasure.
    pub fn create_popup(&mut self, parent: &WindowHandle, name: &str, content: Node) -> Result<PopupHandle, SurfaceCreationError<Node>> {
        // Authenticate the owner capability before transferring the popup content into the forest.
        self.window_manager.create_popup(parent, name, content)
    }

    /// Creates a hidden popup menu owned directly by `parent`.
    ///
    /// The supplied [`Menu`] is the same concrete declaration used below a menu-bar heading. Its
    /// label names the retained popup for diagnostics; its items, separators, and submenus are
    /// presented by the normal compact menu surface when the returned handle is shown.
    ///
    /// # Errors
    ///
    /// Returns [`SurfaceCreationError<Menu>`] when `parent` is stale or belongs to another
    /// Context. [`SurfaceCreationError::into_input`] recovers the unchanged menu declaration.
    #[allow(clippy::result_large_err)] // Failure deliberately returns the complete unique Menu without allocation or erasure.
    pub fn create_menu_popup(&mut self, parent: &WindowHandle, menu: Menu) -> Result<PopupHandle, SurfaceCreationError<Menu>> {
        // Authenticate the owner capability before transferring the menu into the surface forest.
        self.window_manager.create_menu_popup(parent, menu)
    }

    /// Replaces a retained window or dialog rectangle before the next layout commit.
    pub fn set_window_rect(&mut self, window: &WindowHandle, rect: Recti) -> Result<(), SurfaceMutationError> {
        // The manager validates context ownership before changing authoritative chrome geometry.
        self.window_manager.set_window_rect(window, rect)
    }

    /// Replaces a retained window or dialog size without changing its origin.
    pub fn set_window_size(&mut self, window: &WindowHandle, size: Dimensioni) -> Result<(), SurfaceMutationError> {
        // Size remains authoritative manager state rather than an application-side deferred value.
        self.window_manager.set_window_size(window, size)
    }

    /// Replaces the presentation and interaction options for a retained window or dialog.
    pub fn set_window_options(&mut self, window: &WindowHandle, options: WindowOption) -> Result<(), SurfaceMutationError> {
        // Apply option-dependent capture cleanup and enabled-tree propagation in the concrete
        // WindowManager implementation rather than mirroring either policy in this façade.
        self.window_manager.set_window_options(window, options)
    }

    /// Replaces the presentation options for one window-owned popup definition.
    ///
    /// Popup identity remains typed so this operation cannot accidentally mutate window chrome.
    pub fn set_popup_options(&mut self, popup: &PopupHandle, options: WindowOption) -> Result<(), SurfaceMutationError> {
        // Apply the option change to the retained popup definition before the next layout commit.
        self.window_manager.set_popup_options(popup, options)
    }

    /// Assigns an independent window to one of the sixteen fixed application layers.
    ///
    /// Structural children inherit their family root's layer and return
    /// [`SurfaceMutationError::ManagedLayer`] if passed here.
    pub fn set_window_layer(&mut self, window: &WindowHandle, layer: u8) -> Result<(), SurfaceMutationError> {
        // Layer validation and modal restrictions remain one WindowManager transaction at the
        // event-safe mutation boundary; popups derive the layer from their owning window.
        self.window_manager.set_window_layer(window, layer)
    }

    /// Returns a registered window's effective fixed or modal layer policy.
    ///
    /// A structural child reports its top-level family root's current fixed layer.
    pub fn window_layer(&self, window: &WindowHandle) -> Result<LayerBinding, SurfaceMutationError> {
        // Authenticate the handle rather than exposing manager storage or a forged default band.
        self.window_manager.window_layer(window)
    }

    /// Shows or hides a retained window or dialog while preserving its application tree.
    ///
    /// Hiding a window removes its complete structural family from effective visibility. Child
    /// windows retain their local visibility intent and recover when their ancestors are shown;
    /// modal descendants are explicitly closed and do not recover implicitly.
    ///
    /// Popup definitions have a distinct typed identity and cannot be passed to this generic window
    /// operation. Use [`Self::show_popup`], [`Self::show_popup_at`], or [`Self::hide_popup`] instead.
    pub fn set_window_visible(&mut self, window: &WindowHandle, visible: bool) -> Result<(), SurfaceMutationError> {
        // Mutate the window synchronously at the safe dispatch boundary so the following layout sees
        // the requested visibility without an application-owned frame flag.
        self.window_manager.set_window_visible(window, visible)
    }

    /// Shows a popup at the current pointer position and activates its keyboard scope.
    pub fn show_popup(&mut self, popup: &PopupHandle) -> Result<(), SurfaceMutationError> {
        // Placement and active-path replacement remain atomic inside the manager.
        self.window_manager.show_popup(popup)
    }

    /// Hides an active popup and every active descendant while retaining their definitions.
    ///
    /// Closing any active suffix records one [`crate::PopupEvent::Dismissed`] event per removed
    /// application popup.
    pub fn hide_popup(&mut self, popup: &PopupHandle) -> Result<(), SurfaceMutationError> {
        // Delegate path truncation to the manager so visibility has one authoritative source.
        self.window_manager.hide_popup(popup)
    }

    /// Shows and activates a popup at an exact screen-space anchor before the following layout commit.
    ///
    /// This atomic form is intended for composed controls such as combos. It applies popup-path
    /// replacement and replaces the popup rectangle, so no pointer-relative intermediate placement
    /// can be observed. Widget popups select their first eligible keyboard target after layout;
    /// popup menus enter their ordinary compact-menu keyboard scope. The typed handle prevents
    /// passing a window or dialog as the popup target.
    pub fn show_popup_at(&mut self, popup: &PopupHandle, anchor: Recti) -> Result<(), SurfaceMutationError> {
        // Delegate the complete transaction while retaining compile-time popup identity across the
        // event façade boundary.
        self.window_manager.show_popup_at(popup, anchor)
    }

    /// Raises a registered window or dialog inside its effective layer.
    ///
    /// This operation never moves an ordinary window across another numeric application layer. An
    /// independent window raises its complete family among fixed-layer peers; a child raises only
    /// among siblings with the same direct parent. A visible dialog moves its modal group in front
    /// and closes popups from the previous modal group. A stale or foreign handle returns a checked
    /// mutation error.
    pub fn bring_window_to_front(&mut self, window: &WindowHandle) -> Result<(), SurfaceMutationError> {
        // Let WindowManager raise the authenticated node inside its structural stacking band.
        self.window_manager.bring_window_to_front(window)
    }

    /// Permanently unregisters a window or dialog and its complete structural subtree.
    ///
    /// Every child window, dialog, and popup below the target is dropped, making its stable handles
    /// stale and expiring its weak widget and event endpoints.
    pub fn destroy_window(&mut self, window: &WindowHandle) -> Result<(), SurfaceMutationError> {
        // Destruction also expires every weak application widget and event handle in the subtree.
        self.window_manager.destroy_window(window)
    }

    /// Borrows the concrete state of one mounted menu item.
    pub fn menu_item(&self, handle: &crate::MenuItemHandle) -> Result<&crate::MenuItemParameters, crate::MenuItemAccessError> {
        // The handle's private stable ID selects manager-owned state; its event endpoint remains an
        // independent subscription capability.
        self.window_manager.menu_item(handle)
    }

    /// Mutably borrows the concrete state of one mounted menu item.
    pub fn menu_item_mut(&mut self, handle: &crate::MenuItemHandle) -> Result<&mut crate::MenuItemParameters, crate::MenuItemAccessError> {
        // WindowManager invalidates layout conservatively before lending any public field mutably.
        self.window_manager.menu_item_mut(handle)
    }

    /// Returns the active popup names at the event-dispatch boundary for internal tests.
    pub(crate) fn debug_active_popup_names(&self) -> Vec<String> {
        // This observation proves ordering only: retained updates and manager menu policy have both
        // completed before an application handler receives the item's typed submission.
        self.window_manager.debug_active_popup_names()
    }
}
