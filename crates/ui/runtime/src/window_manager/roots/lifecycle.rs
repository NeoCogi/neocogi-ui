//! Validated ownership, registration, visibility and surface membership changes.
use super::*;

impl WindowManager {
    /// Completes a fresh window's constructor-selected delivery before its
    /// creating transaction returns. Registration invokes no producer code;
    /// the caller holds exclusive manager access and has not exposed the handle.
    /// This is private factory wiring, not a runtime receiver replacement API.
    pub(crate) fn connect_window_events(&mut self, window: &WindowHandle, events: neocogi_ui::abi::core::EventPort) {
        self.surfaces
            .root_node_mut(window.id())
            .expect("new window remains registered")
            .root_mut()
            .expect("window has root policy")
            .events = events;
    }

    /// Completes an application popup's delivery in its creating transaction,
    /// after ordinary COM allocation supplies its weak identity. No popup input
    /// or application call can occur before the factory exposes its capability.
    pub(crate) fn connect_popup_events(&mut self, popup: &PopupHandle, events: neocogi_ui::abi::core::EventPort) {
        let node = self.surfaces.popup_node_mut(popup.id()).expect("new popup remains registered");
        match &mut node.kind {
            SurfaceKind::Popup(PopupState::Application { events: destination, .. }) => *destination = events,
            _ => unreachable!("public popup creation selects application popup policy"),
        }
    }

    /// Validates complete runtime shutdown before changing any lifetime state.
    pub(crate) fn ensure_widgets_idle(&self) -> Result<(), neocogi_com::NcStatus> {
        for surface in &self.surfaces.nodes {
            if let SurfaceBody::Widgets { root, .. } = &surface.surface.body {
                root.data.ensure_idle()?;
            }
        }
        Ok(())
    }

    /// Derives private delivery order from real ancestry, including hidden surfaces.
    /// Child adjacency follows global chronology; reversed siblings run newest first.
    pub(crate) fn collect_dispatch_widgets(
        &self,
        drivers: &mut Vec<neocogi_com::WeakComBox<neocogi_ui::abi::core::IWidgetDispatcherAbi>>,
    ) -> Result<(), neocogi_com::NcStatus> {
        for node in self.surfaces.nodes.iter().rev().filter(|node| node.parent.is_none()) {
            self.surfaces.collect_dispatch_tree(node.key, drivers)?;
        }
        Ok(())
    }

    /// Borrows one mounted menu item's concrete public state through its stable typed capability.
    pub(crate) fn menu_item(&self, handle: &crate::MenuItemHandle) -> Result<&crate::MenuItemParameters, crate::MenuItemAccessError> {
        // Search authoritative bar and popup records by their process-unique item IDs. Forest
        // membership rejects unmounted and foreign handles without consulting event allocations or
        // maintaining an application-visible lookup table.
        for node in &self.surfaces.nodes {
            if let Some(item) = node.root().and_then(|root| root.menu_bar.as_ref()).and_then(|menu| menu.item(handle)) {
                return Ok(&item.parameters);
            }
            if let Some(item) = node.surface.body.menu().and_then(|menu| menu.item(handle)) {
                return Ok(&item.parameters);
            }
        }
        Err(crate::MenuItemAccessError::UnknownItem)
    }

    /// Mutably borrows one mounted menu item's concrete public state through its stable capability.
    pub(crate) fn menu_item_mut(&mut self, handle: &crate::MenuItemHandle) -> Result<&mut crate::MenuItemParameters, crate::MenuItemAccessError> {
        // Validate before changing transaction state so an unmounted or foreign handle is a true
        // no-op. The second linear scan is intentional; menu collections remain very small.
        self.menu_item(handle)?;
        self.invalidate_ui_commit();
        for node in &mut self.surfaces.nodes {
            match &mut node.kind {
                SurfaceKind::Root(root) => {
                    if let Some(item) = root.menu_bar.as_mut().and_then(|menu| menu.item_mut(handle)) {
                        return Ok(&mut item.parameters);
                    }
                }
                SurfaceKind::Popup(_) => {
                    if let Some(item) = node.surface.body.menu_mut().and_then(|menu| menu.item_mut(handle)) {
                        return Ok(&mut item.parameters);
                    }
                }
            }
        }
        unreachable!("a validated menu item must remain mounted during one exclusive manager borrow")
    }

    /// Separates a bar's compact heading surface and uniquely owned popup declarations once for both creation paths.
    pub(super) fn prepare_menu_bar(menu_bar: Option<crate::MenuBar>) -> (Option<MenuSurface>, Vec<Vec<MenuEntry>>) {
        // Move top-level labels into the bar while retaining only their row vectors until the root
        // exists. This flat transfer avoids constructing a recursive hierarchy beside the forest.
        let mut menu_popups = Vec::new();
        let menu_bar = menu_bar.map(|bar| {
            let menus = bar.into_menus();
            let mut headings = Vec::with_capacity(menus.len());
            menu_popups.reserve(menus.len());
            for menu in menus {
                let (label, entries) = menu.into_parts();
                headings.push(MenuSlot::Branch { label });
                menu_popups.push(entries);
            }
            MenuSurface::new(headings, false)
        });

        (menu_bar, menu_popups)
    }

    /// Checks whether a live window already owns its intrinsic bar before COM declaration transfer.
    pub(crate) fn has_menu_bar(&self, window: &WindowHandle) -> Result<bool, SurfaceMutationError> {
        let root = self.window_id(window)?;
        Ok(self
            .surfaces
            .root_node(root)
            .expect("authenticated root remains present")
            .root()
            .expect("window resolves root policy")
            .menu_bar
            .is_some())
    }

    /// Installs a bar on an already authenticated bar-free window without duplicating menu topology.
    pub(crate) fn attach_menu_bar(&mut self, window: &WindowHandle, bar: crate::MenuBar) -> Result<(), SurfaceMutationError> {
        let root = self.window_id(window)?;
        debug_assert!(!self.has_menu_bar(window)?);
        let (bar, popups) = Self::prepare_menu_bar(Some(bar));
        self.root_node_mut(root)?.root_mut().expect("window resolves root policy").menu_bar = bar;
        for (slot, entries) in popups.into_iter().enumerate() {
            self.register_menu_popup(SurfaceKey::Root(root), slot, entries);
        }
        self.invalidate_ui_commit();
        Ok(())
    }

    /// Registers one concrete root after its optional structural owner has been validated.
    pub(super) fn register_window(
        &mut self,
        mode: RootMode,
        parent: Option<RootId>,
        window: Window,
        options: WindowOption,
        visible: bool,
        queue: Option<neocogi_ui::abi::core::EventPort>,
    ) -> WindowHandle {
        // Public child/dialog entry points authenticate and classify the owner before moving the
        // Window here. Keep registration infallible after that transfer: this is the boundary that
        // guarantees every returned creation error can still contain the untouched Window.
        debug_assert!(
            matches!((mode, parent), (RootMode::Normal { .. }, None) | (RootMode::Child | RootMode::Modal, Some(_))),
            "root mode and structural owner must agree before registration"
        );

        let (name, rect, content, menu_bar, child_window_clip) = window.into_parts();
        let (menu_bar, menu_popups) = Self::prepare_menu_bar(menu_bar);

        // The handle copies only identity. The forest owns the optional destination directly.
        let id = RootId::allocate();
        let events = queue.unwrap_or_default();
        self.surfaces.insert_root(SurfaceNode {
            key: SurfaceKey::Root(id),
            parent: parent.map(SurfaceKey::Root),
            surface: Surface::new(name, options, rect, content),
            kind: SurfaceKind::Root(RootState {
                mode,
                visible,
                interaction: RootInteraction::None,
                hovered_chrome: None,
                restore_rect: None,
                unminimized_height: None,
                events,
                menu_bar,
                child_window_clip,
            }),
        });

        // Each row vector becomes a direct child edge; recursion consumes only one parent's pending
        // children at a time, so no second menu tree coexists with the authoritative forest.
        for (trigger_slot, entries) in menu_popups.into_iter().enumerate() {
            self.register_menu_popup(SurfaceKey::Root(id), trigger_slot, entries);
        }
        self.invalidate_ui_commit();
        WindowHandle::new(id)
    }

    /// Creates an open independent window from one complete retained definition.
    pub fn create_window(&mut self, window: Window, queue: Option<neocogi_ui::abi::core::EventPort>) -> WindowHandle {
        // Independent construction has no fallible owner edge.
        self.register_window(RootMode::Normal { layer: DEFAULT_LAYER }, None, window, WindowOption::FRAME, true, queue)
    }

    /// Creates a structural child, returning the unchanged Window if its parent is rejected.
    #[allow(
        clippy::result_large_err,
        reason = "Rejected creation returns the caller's unique input inline without another allocation."
    )]
    pub fn create_child_window(
        &mut self,
        parent: &WindowHandle,
        window: Window,
        queue: Option<neocogi_ui::abi::core::EventPort>,
    ) -> Result<WindowHandle, SurfaceCreationError<Window>> {
        // Authenticate and classify the parent while `window` remains local. On either validation
        // failure, move that exact value into the concrete error instead of dropping its unique
        // retained tree. Registration then adds one backward edge and cannot fail.
        let parent = match self.creation_owner_id(parent, SurfaceMutationError::InvalidChildWindowParent) {
            Ok(parent) => parent,
            Err(reason) => return Err(SurfaceCreationError::new(reason, window)),
        };
        Ok(self.register_window(RootMode::Child, Some(parent), window, WindowOption::FRAME, true, queue))
    }

    /// Creates a hidden dialog, returning the unchanged Window if its owner is rejected.
    #[allow(
        clippy::result_large_err,
        reason = "Rejected creation returns the caller's unique input inline without another allocation."
    )]
    pub fn create_dialog(
        &mut self,
        owner: &WindowHandle,
        window: Window,
        queue: Option<neocogi_ui::abi::core::EventPort>,
    ) -> Result<WindowHandle, SurfaceCreationError<Window>> {
        // A dialog accepts the same ordinary owner roles as a structural child but reports its own
        // policy reason. Keep the Window untouched until both capability and role checks pass.
        let owner = match self.creation_owner_id(owner, SurfaceMutationError::InvalidDialogOwner) {
            Ok(owner) => owner,
            Err(reason) => return Err(SurfaceCreationError::new(reason, window)),
        };
        Ok(self.register_window(RootMode::Modal, Some(owner), window, WindowOption::FRAME, false, queue))
    }

    /// Creates an application popup, returning the unchanged Node if its owner is rejected.
    #[allow(
        clippy::result_large_err,
        reason = "Rejected creation returns the caller's unique input inline without another allocation."
    )]
    pub fn create_popup(
        &mut self,
        owner: &WindowHandle,
        name: &str,
        content: Node,
        queue: Option<neocogi_ui::abi::core::EventPort>,
    ) -> Result<PopupHandle, SurfaceCreationError<Node>> {
        // Resolve the authenticated window capability while the Node is still owned locally. Once
        // validation succeeds, application-popup registration is an infallible ownership transfer.
        let owner = match self.window_id(owner) {
            Ok(owner) => owner,
            Err(reason) => return Err(SurfaceCreationError::new(reason, content)),
        };
        // A top-level popup's sole parent edge points directly to its owning root.
        let surface = Surface::new(name.to_owned(), Self::default_popup_options(), Recti::default(), content);
        Ok(self.register_application_popup(SurfaceKey::Root(owner), surface, queue))
    }

    pub fn create_menu_popup(
        &mut self,
        owner: &WindowHandle,
        menu: crate::Menu,
        queue: Option<neocogi_ui::abi::core::EventPort>,
    ) -> Result<PopupHandle, SurfaceCreationError<crate::Menu>> {
        // Authenticate while the complete recursive declaration remains recoverable by the caller.
        let owner = match self.window_id(owner) {
            Ok(owner) => owner,
            Err(reason) => return Err(SurfaceCreationError::new(reason, menu)),
        };
        let (name, entries) = menu.into_parts();
        let (rows, children) = Self::consume_menu_entries(entries);
        let handle = self.register_application_popup(SurfaceKey::Root(owner), Surface::menu(name, MenuSurface::new(rows, true)), queue);

        // Nested declarations retain the same relational children used beneath a menu-bar heading.
        // Only the root popup has an application handle and exact screen anchor.
        let parent = SurfaceKey::Popup(handle.id());
        for (trigger_slot, entries) in children {
            self.register_menu_popup(parent, trigger_slot, entries);
        }
        Ok(handle)
    }

    /// Creates a hidden selection menu using the same popup forest and compact menu surface.
    pub(crate) fn create_choice_popup(
        &mut self,
        owner: &WindowHandle,
        menu: crate::Menu,
        committed: Option<u64>,
    ) -> Result<PopupHandle, SurfaceCreationError<crate::Menu>> {
        let owner = match self.window_id(owner) {
            Ok(owner) => owner,
            Err(reason) => return Err(SurfaceCreationError::new(reason, menu)),
        };
        let (name, entries) = menu.into_parts();
        let (rows, children) = Self::consume_menu_entries(entries);
        let mut surface = Surface::menu(name, MenuSurface::choices(rows, committed));
        // The trigger determines horizontal extent; only menu height is intrinsic. The menu
        // surface draws its own outline, so a second window-frame or padding would misalign it.
        surface.options = WindowOption::NO_PADDING | WindowOption::NO_TITLE | WindowOption::NO_RESIZE | WindowOption::AUTO_HEIGHT;
        let handle = self.register_application_popup(SurfaceKey::Root(owner), surface, None);
        for (trigger_slot, entries) in children {
            self.register_choice_menu_popup(SurfaceKey::Popup(handle.id()), trigger_slot, entries, committed);
        }
        Ok(handle)
    }

    /// Replaces one choice menu's complete hierarchy after its owner changed model revision.
    pub(crate) fn replace_choice_popup(&mut self, popup: &PopupHandle, menu: crate::Menu, committed: Option<u64>) -> Result<(), SurfaceMutationError> {
        let id = self.popup_id(popup)?;
        let active = self.surfaces.popup_is_active(id);
        if active {
            // Retain the application popup while dismissing only its old relational suffix. This
            // prevents programmatic selection or label edits from closing an already open combo.
            let keep = self.surfaces.popup_depth(id).ok_or(SurfaceMutationError::UnknownPopup)? + 1;
            self.truncate_active_popup_path(keep);
        }
        let (_, entries) = menu.into_parts();
        let (rows, children) = Self::consume_menu_entries(entries);
        let surface = self
            .surfaces
            .popup_node_mut(id)
            .and_then(|node| node.surface.body.menu_mut())
            .ok_or(SurfaceMutationError::UnknownPopup)?;
        if !surface.replace_choices(rows, committed) {
            return Err(SurfaceMutationError::UnknownPopup);
        }
        self.remove_popup_descendants(id);
        for (trigger_slot, entries) in children {
            self.register_choice_menu_popup(SurfaceKey::Popup(id), trigger_slot, entries, committed);
        }
        if active {
            let path = self.prepare_choice_path(id)?;
            self.active_surface = Some(SurfaceKey::Popup(id));
            self.activate_choice_path(path)?;
        }
        self.invalidate_ui_commit();
        Ok(())
    }

    /// Opens a choice menu and restores the complete branch containing its committed leaf.
    pub(crate) fn show_choice_popup_at(&mut self, popup: &PopupHandle, anchor: Recti) -> Result<(), SurfaceMutationError> {
        let root = self.popup_id(popup)?;
        let path = self.prepare_choice_path(root)?;
        self.show_popup_at(popup, anchor)?;
        self.activate_choice_path(path)?;
        self.invalidate_ui_commit();
        Ok(())
    }

    /// Resets transient choice state and resolves the relational path to the committed leaf.
    pub(super) fn prepare_choice_path(&mut self, root: PopupId) -> Result<Vec<PopupId>, SurfaceMutationError> {
        let keys = self.popup_subtree(root);
        let mut prepared = false;
        let mut selected = None;
        for key in keys {
            if let SurfaceKey::Popup(candidate) = key
                && let Some(menu) = self.surfaces.popup_node_mut(candidate).and_then(|node| node.surface.body.menu_mut())
            {
                prepared |= menu.prepare_choices();
                if menu.committed_choice_slot().is_some() {
                    selected = Some(candidate);
                }
            }
        }
        if !prepared {
            return Err(SurfaceMutationError::UnknownPopup);
        }

        // Resolve the selected ancestry before changing the active branch. Validation guarantees
        // at most one matching stable leaf, while following retained parent edges avoids a second
        // menu tree or application-visible submenu handle.
        let mut path = Vec::new();
        let mut current = selected;
        while let Some(candidate) = current.filter(|candidate| *candidate != root) {
            path.push(candidate);
            current = match self.surfaces.popup_node(candidate).and_then(|node| node.parent) {
                Some(SurfaceKey::Popup(parent)) => Some(parent),
                Some(SurfaceKey::Root(_)) | None => return Err(SurfaceMutationError::InvalidPopupParent),
            };
        }
        path.reverse();
        Ok(path)
    }

    /// Opens one already validated choice path and focuses every branch leading to its leaf.
    pub(super) fn activate_choice_path(&mut self, path: Vec<PopupId>) -> Result<(), SurfaceMutationError> {
        for child in path {
            let (parent, trigger_slot) = {
                let node = self.surfaces.popup_node(child).ok_or(SurfaceMutationError::UnknownPopup)?;
                let parent = node.parent.ok_or(SurfaceMutationError::InvalidPopupParent)?;
                let PopupState::Menu { trigger_slot } = node.popup().ok_or(SurfaceMutationError::UnknownPopup)? else {
                    return Err(SurfaceMutationError::InvalidPopupParent);
                };
                (parent, *trigger_slot)
            };
            if !self.menu_surface_mut(parent).is_some_and(|menu| menu.focus_slot(trigger_slot)) {
                return Err(SurfaceMutationError::InvalidPopupParent);
            }
            self.open_popup_id(child)?;
            self.active_surface = Some(SurfaceKey::Popup(child));
        }
        Ok(())
    }

    /// Takes an activated leaf identity after menu input and visibility changes have completed.
    pub(crate) fn take_choice_submission(&mut self, popup: &PopupHandle) -> Result<Option<u64>, SurfaceMutationError> {
        let id = self.popup_id(popup)?;
        for key in self.popup_subtree(id) {
            if let SurfaceKey::Popup(candidate) = key
                && let Some(submitted) = self
                    .surfaces
                    .popup_node_mut(candidate)
                    .and_then(|node| node.surface.body.menu_mut())
                    .and_then(MenuSurface::take_choice_submission)
            {
                return Ok(Some(submitted));
            }
        }
        Ok(None)
    }

    /// Registers one application-addressable popup below an already validated root.
    pub(super) fn register_application_popup(&mut self, parent: SurfaceKey, surface: Surface, queue: Option<neocogi_ui::abi::core::EventPort>) -> PopupHandle {
        let id = PopupId::allocate();
        // The forest record owns delivery; the handle needs only its existing lookup identity.
        let events = queue.unwrap_or_default();
        self.surfaces.insert_popup(SurfaceNode {
            key: SurfaceKey::Popup(id),
            parent: Some(parent),
            surface,
            kind: SurfaceKind::Popup(PopupState::Application {
                anchor: Recti::default(),
                events,
                focus_on_layout: false,
            }),
        });
        self.invalidate_ui_commit();
        PopupHandle::new(id)
    }

    /// Separates direct menu rows from recursive children without creating a parallel menu tree.
    pub(super) fn consume_menu_entries(entries: Vec<MenuEntry>) -> (Vec<MenuSlot>, Vec<(usize, Vec<MenuEntry>)>) {
        let mut rows = Vec::with_capacity(entries.len());
        let mut children = Vec::new();
        for (slot, entry) in entries.into_iter().enumerate() {
            match entry {
                MenuEntry::Item(item) => rows.push(MenuSlot::Item(item.record)),
                MenuEntry::Choice { id, label } => rows.push(MenuSlot::Choice { id, label, enabled: true }),
                MenuEntry::Separator => rows.push(MenuSlot::Separator),
                MenuEntry::Submenu(menu) => {
                    // A branch label stays in this parent surface; only its owned row vector waits
                    // until the parent forest node exists and can become the child's sole edge.
                    let (label, entries) = menu.into_parts();
                    rows.push(MenuSlot::Branch { label });
                    children.push((slot, entries));
                }
            }
        }
        (rows, children)
    }

    /// Consumes one declaration row vector directly into a relational menu-popup forest node.
    pub(super) fn register_menu_popup(&mut self, parent: SurfaceKey, trigger_slot: usize, entries: Vec<MenuEntry>) {
        let (rows, children) = Self::consume_menu_entries(entries);
        let id = PopupId::allocate();
        self.surfaces.insert_popup(SurfaceNode {
            key: SurfaceKey::Popup(id),
            parent: Some(parent),
            surface: Surface::menu(String::new(), MenuSurface::new(rows, true)),
            kind: SurfaceKind::Popup(PopupState::Menu { trigger_slot }),
        });
        // The parent is retained before recursion, satisfying the forest's sole structural invariant.
        for (trigger_slot, entries) in children {
            self.register_menu_popup(SurfaceKey::Popup(id), trigger_slot, entries);
        }
    }

    /// Consumes one selection-menu branch using the same relational submenu forest as commands.
    pub(super) fn register_choice_menu_popup(&mut self, parent: SurfaceKey, trigger_slot: usize, entries: Vec<MenuEntry>, committed: Option<u64>) {
        let (rows, children) = Self::consume_menu_entries(entries);
        let id = PopupId::allocate();
        self.surfaces.insert_popup(SurfaceNode {
            key: SurfaceKey::Popup(id),
            parent: Some(parent),
            surface: Surface::menu(String::new(), MenuSurface::choices(rows, committed)),
            kind: SurfaceKind::Popup(PopupState::Menu { trigger_slot }),
        });
        for (trigger_slot, entries) in children {
            self.register_choice_menu_popup(SurfaceKey::Popup(id), trigger_slot, entries, committed);
        }
    }

    /// Collects one application popup and every relational menu descendant in parent-first order.
    pub(super) fn popup_subtree(&self, popup: PopupId) -> Vec<SurfaceKey> {
        let mut keys = vec![SurfaceKey::Popup(popup)];
        let mut index = 0;
        while index < keys.len() {
            keys.extend_from_slice(&self.surfaces.index[&keys[index]].children);
            index += 1;
        }
        keys
    }

    /// Removes only relational descendants while retaining the application popup's stable handle.
    pub(super) fn remove_popup_descendants(&mut self, popup: PopupId) {
        let mut removed = self.popup_subtree(popup);
        removed.remove(0);
        let removed: HashSet<_> = removed.into_iter().collect();
        self.surfaces.nodes.retain(|node| !removed.contains(&node.key));
        // Rebuilding adjacency also replaces every cached policy entry. Restore the derived
        // projection before a replacement child is inserted because incremental insertion reads
        // its retained parent's policy rather than walking ancestry for each new descendant.
        self.surfaces.rebuild_index();
        self.surfaces.rebuild_visible_order();
    }

    /// Replaces a retained window title after authenticating its application capability.
    pub fn set_window_name(&mut self, window: &WindowHandle, name: String) -> Result<(), SurfaceMutationError> {
        let root = self.window_id(window)?;
        self.root_node_mut(root)?.surface.name = name;
        self.invalidate_ui_commit();
        Ok(())
    }

    /// Replaces a window outer rectangle silently.
    pub fn set_window_rect(&mut self, window: &WindowHandle, rect: Recti) -> Result<(), SurfaceMutationError> {
        let root = self.window_id(window)?;
        let node = self.root_node_mut(root)?;
        // An explicit application rectangle becomes the new normal placement and exits both
        // manager-owned size modes so no caption can restore stale geometry over this request.
        let state = node.root_mut().expect("window identity must resolve root policy");
        state.restore_rect = None;
        state.unminimized_height = None;
        node.surface.rect = rect;
        self.invalidate_ui_commit();
        Ok(())
    }

    /// Replaces window presentation and interaction options and reconciles capture immediately.
    pub fn set_window_options(&mut self, window: &WindowHandle, options: WindowOption) -> Result<(), SurfaceMutationError> {
        let root = self.window_id(window)?;
        // Snapshot transient ownership before applying DISABLED because disabled menu surfaces are
        // intentionally no longer discoverable through the ordinary active-menu query.
        let active_menu_root = self.active_menu_root();
        let active_popup_owner = self.active_popup_owner();
        self.root_node_mut(root)?.set_root_options(options);
        self.surfaces.rebuild_visible_order();

        // A disabled structural root disables every child window in that family. Revoke their
        // pointer targets immediately rather than waiting for another input event to discover that
        // a former capture or hover can no longer participate; retained keyboard focus survives.
        for index in 0..self.surfaces.nodes.len() {
            let key = self.surfaces.nodes[index].key;
            if !self.surface_accepts_input(key) {
                self.surfaces.nodes[index].clear_pointer_targets();
            }
        }
        // Transient branches cannot remain open above an owner that no longer accepts input.
        if active_menu_root.is_some_and(|owner| !self.surfaces.root_is_effectively_enabled(owner)) {
            self.finish_keyboard_menu(true);
        }
        if active_popup_owner.is_some_and(|owner| !self.surfaces.root_is_effectively_enabled(owner)) {
            self.dismiss_active_popups();
        }
        self.reconcile_active_surface();
        self.invalidate_ui_commit();
        Ok(())
    }

    /// Replaces popup frame, padding, and automatic-size policy through its typed handle.
    pub fn set_popup_options(&mut self, popup: &PopupHandle, options: WindowOption) -> Result<(), SurfaceMutationError> {
        let popup = self.popup_id(popup)?;
        let disabled = options.intersects(WindowOption::DISABLED);
        {
            let node = self.surfaces.popup_node_mut(popup).expect("authenticated popup must remain retained");
            // Popups never acquire manager chrome; enforce that invariant regardless of caller
            // flags. Disabling additionally clears any local widget capture immediately.
            node.surface.options = options | WindowOption::NO_TITLE | WindowOption::NO_RESIZE;
            if disabled {
                node.clear_transient_targets();
            }
        }
        if disabled && self.surfaces.popup_is_active(popup) {
            let depth = self.surfaces.popup_depth(popup).expect("active popup must retain rooted ancestry");
            self.truncate_active_popup_path(depth);
        }
        self.reconcile_active_surface();
        self.invalidate_ui_commit();
        Ok(())
    }

    /// Assigns an independent window to one fixed application stacking layer.
    pub fn set_window_layer(&mut self, window: &WindowHandle, layer: u8) -> Result<(), SurfaceMutationError> {
        // Validate the public numeric domain before resolving structural policy, preserving the
        // established error precedence for every kind of authenticated window handle.
        if layer > MAX_LAYER {
            return Err(SurfaceMutationError::InvalidLayer(layer));
        }
        let root = self.window_id(window)?;
        let state = self.root_node(root)?.root().expect("root lookup must return root policy");
        // Child bands follow their family root and dialogs occupy the modal band, so neither may
        // acquire a conflicting independent layer value.
        if !matches!(state.mode, RootMode::Normal { .. }) {
            return Err(SurfaceMutationError::ManagedLayer);
        }
        self.surfaces.set_root_layer(root, layer);
        self.invalidate_ui_commit();
        Ok(())
    }

    /// Returns the effective fixed or modal layer policy of one window.
    pub fn window_layer(&self, window: &WindowHandle) -> Result<LayerBinding, SurfaceMutationError> {
        let root = self.window_id(window)?;
        // Mutations commit inherited policy before returning, so this indexed read immediately
        // reflects a family layer change without repeating the ancestry walk.
        let mode = self.surfaces.stacking_mode(root).ok_or(SurfaceMutationError::UnknownWindow)?;
        Ok(match mode {
            RootMode::Normal { layer } => LayerBinding::Fixed(layer),
            RootMode::Modal => LayerBinding::Modal,
            RootMode::Child => unreachable!("stacking_mode resolves every child to an ancestor band"),
        })
    }

    /// Shows or hides a window while retaining its application and popup definitions.
    pub fn set_window_visible(&mut self, window: &WindowHandle, visible: bool) -> Result<(), SurfaceMutationError> {
        let root = self.window_id(window)?;
        self.set_window_visible_id(root, visible)
    }

    /// Applies visibility policy to one already authenticated internal window identity.
    pub(super) fn set_window_visible_id(&mut self, root: RootId, visible: bool) -> Result<(), SurfaceMutationError> {
        let mode = self.root_node(root)?.root().expect("root lookup must return root policy").mode;
        if visible {
            // Dialogs additionally require an effectively visible ordinary owner. Ordinary windows
            // may retain a true local bit beneath a hidden parent and recover with that parent.
            if mode == RootMode::Modal {
                let owner = match self.root_node(root)?.parent {
                    Some(SurfaceKey::Root(owner)) => owner,
                    _ => return Err(SurfaceMutationError::InvalidDialogOwner),
                };
                if !self.root_is_visible(owner) {
                    return Err(SurfaceMutationError::InvalidDialogOwner);
                }
                if self.active_menu_root().is_some_and(|menu_root| menu_root != root) {
                    // A newly active modal owns the only keyboard scope; an underlying menu cannot
                    // remain highlighted or consume keys through that boundary.
                    self.finish_keyboard_menu(true);
                }
                if self.surfaces.active_modal_root() != Some(root) {
                    self.dismiss_active_popups();
                }
            }
            let result = self.root_node_mut(root)?.set_root_visible(true);
            self.posted(result);
            self.surfaces.move_root_to_front(root);
            if mode == RootMode::Modal {
                // A newly visible dialog is the concrete keyboard surface for its modal group.
                self.active_surface = Some(SurfaceKey::Root(root));
            }
        } else {
            // Descendant children retain their own visibility intent, but every modal descendant is
            // explicitly closed so showing the family again cannot resurrect a dismissed dialog.
            let affected = self.owned_root_ids(root);
            if self.active_menu_root().is_some_and(|menu_root| affected.contains(&menu_root)) {
                self.finish_keyboard_menu(true);
            }
            if self.active_popup_owner().is_some_and(|owner| affected.contains(&owner)) {
                self.dismiss_active_popups();
            }
            let result = self.root_node_mut(root)?.set_root_visible(false);
            self.posted(result);
            for affected_root in affected.iter().copied().filter(|affected| *affected != root) {
                let node = self.root_node_mut(affected_root).expect("collected root must remain retained");
                if node.root().is_some_and(|state| state.mode == RootMode::Modal) {
                    let result = node.set_root_visible(false);
                    self.posted(result);
                } else {
                    // Hidden structural descendants are absent from traversal, so revoke their
                    // transient identities without overwriting the visibility they should recover.
                    node.clear_transient_targets();
                }
            }
            if self
                .active_surface
                .and_then(|active| self.surfaces.owning_root(active))
                .is_some_and(|active| affected.contains(&active))
            {
                // Structural children and dialogs return to their direct owning root. Independent
                // windows have no parent and leave front-surface fallback to choose the next root.
                self.active_surface = self.surfaces.root_node(root).and_then(|node| node.parent);
            }
            self.surfaces.rebuild_visible_order();
            self.reconcile_active_surface();
        }
        self.invalidate_ui_commit();
        Ok(())
    }

    /// Shows one popup at the current pointer position.
    pub fn show_popup(&mut self, popup: &PopupHandle) -> Result<(), SurfaceMutationError> {
        // Input applies each queued transition before the matching application dispatch boundary,
        // making this snapshot the exact click position for pointer-triggered popup requests.
        let mouse = self.input.snapshot().mouse_pos;
        // A one-pixel anchor preserves the pointer origin while leaving automatic popup sizing to
        // the retained menu or widget body during the following layout commit.
        self.show_popup_at(popup, rect(mouse.x, mouse.y, 1, 1))
    }

    /// Shows one popup at an exact screen-space anchor and updates the sole active path.
    pub fn show_popup_at(&mut self, popup: &PopupHandle, anchor: Recti) -> Result<(), SurfaceMutationError> {
        // Resolve the stable ID before opening; foreign and destroyed handles have no matching
        // application-popup node in this forest.
        let popup = self.popup_id(popup)?;
        // Open first so an invalid child path cannot partially replace its retained anchor.
        self.open_popup_id(popup)?;
        if self.active_menu_surface().is_some() {
            // Successful application-popup replacement already dismissed the menu ancestry. Clear
            // only its keyboard selection so the newly opened popup remains active.
            self.finish_keyboard_menu(false);
        }
        let node = self.surfaces.popup_node_mut(popup).expect("authenticated popup must remain retained");
        let focus_widgets = matches!(node.surface.body, SurfaceBody::Widgets { .. });
        let PopupState::Application { anchor: current, focus_on_layout, .. } = node.popup_mut().expect("popup lookup must return popup policy") else {
            return Err(SurfaceMutationError::UnknownPopup);
        };
        *current = anchor;
        // Widget geometry is committed later in this transaction, so defer first-focus selection
        // until that tree has valid allocations and clipping. Menu bodies own row selection and do
        // not manufacture a meaningless widget-focus request.
        *focus_on_layout = focus_widgets;
        node.surface.rect = anchor;
        self.active_surface = Some(SurfaceKey::Popup(popup));
        self.invalidate_ui_commit();
        Ok(())
    }

    /// Opens one retained popup identity without replacing its configured placement relation.
    pub(super) fn open_popup_id(&mut self, popup: PopupId) -> Result<(), SurfaceMutationError> {
        let node = self.surfaces.popup_node(popup).ok_or(SurfaceMutationError::UnknownPopup)?;
        let parent = node.parent.expect("every popup must retain one parent edge");
        if node.surface.options.intersects(WindowOption::DISABLED) {
            return Err(SurfaceMutationError::InvalidPopupParent);
        }
        let owner = self.surfaces.owning_root(SurfaceKey::Popup(popup)).ok_or(SurfaceMutationError::UnknownPopup)?;
        if !self.popup_owner_is_eligible(owner) {
            return Err(SurfaceMutationError::InvalidPopupParent);
        }

        // A top-level popup may replace another branch. A child is legal only while its sole parent
        // edge is in the active ancestry; no separately stored owner/path pair can disagree.
        let (keep, retained) = match parent {
            SurfaceKey::Root(_) => {
                let retained = self.surfaces.popup_is_active(popup);
                (usize::from(retained), retained)
            }
            SurfaceKey::Popup(parent) => {
                if !self.surfaces.popup_is_active(parent) {
                    return Err(SurfaceMutationError::InvalidPopupParent);
                }
                let parent_depth = self.surfaces.popup_depth(parent).expect("active popup must have a rooted ancestry");
                let retained = self.surfaces.popup_is_active(popup);
                (parent_depth + 1 + usize::from(retained), retained)
            }
        };

        self.truncate_active_popup_path(keep);
        if !retained {
            self.surfaces.set_active_popup(Some(popup));
        }
        self.invalidate_ui_commit();
        Ok(())
    }

    /// Hides one active popup and all of its active descendants.
    pub fn hide_popup(&mut self, popup: &PopupHandle) -> Result<(), SurfaceMutationError> {
        let popup = self.popup_id(popup)?;
        if self.surfaces.popup_is_active(popup) {
            let depth = self.surfaces.popup_depth(popup).expect("active popup must have rooted ancestry");
            self.truncate_active_popup_path(depth);
        }
        Ok(())
    }

    /// Raises one authenticated window inside its structural layer.
    pub fn bring_window_to_front(&mut self, window: &WindowHandle) -> Result<(), SurfaceMutationError> {
        let root = self.window_id(window)?;
        self.bring_window_to_front_id(root)
    }

    /// Raises one already authenticated internal window identity inside its structural layer.
    pub(super) fn bring_window_to_front_id(&mut self, root: RootId) -> Result<(), SurfaceMutationError> {
        let state = self.root_node(root)?.root().expect("root lookup must return root policy");
        let mode = state.mode;
        let visible = state.visible;
        if mode == RootMode::Modal && visible && self.surfaces.active_modal_root() != Some(root) {
            if self.active_menu_root().is_some_and(|menu_root| menu_root != root) {
                self.finish_keyboard_menu(true);
            }
            self.dismiss_active_popups();
        }
        self.surfaces.move_root_to_front(root);
        if mode == RootMode::Modal && visible {
            // Fronting a visible dialog selects that concrete root as the modal keyboard surface.
            self.active_surface = Some(SurfaceKey::Root(root));
        }
        self.invalidate_ui_commit();
        Ok(())
    }

    /// Destroys one window together with every child window, dialog, and popup below it.
    pub fn destroy_window(&mut self, window: &WindowHandle) -> Result<(), SurfaceMutationError> {
        let root = self.window_id(window)?;
        self.surfaces.ensure_subtree_idle(SurfaceKey::Root(root))?;
        // Materialize the root subtree before mutating storage so popup dismissal and active-window
        // cleanup can use one stable membership set.
        let removed = self.owned_root_ids(root);
        if self.active_menu_root().is_some_and(|menu_root| removed.contains(&menu_root)) {
            self.finish_keyboard_menu(true);
        }
        if self.active_popup_owner().is_some_and(|owner| removed.contains(&owner)) {
            self.dismiss_active_popups();
        }
        if self
            .active_surface
            .and_then(|active| self.surfaces.owning_root(active))
            .is_some_and(|active| removed.contains(&active))
        {
            self.active_surface = self.surfaces.root_node(root).and_then(|node| node.parent);
        }
        for &id in &removed {
            // Commit the final fact before removing the record. Consumer queues own it
            // independently, so components can settle requests after native teardown.
            let node = self.root_node_mut(id)?;
            let result = neocogi_ui::abi::core::events::window_destroyed().and_then(|event| node.emit_window_event(event));
            self.posted(result);
        }
        self.surfaces.remove_roots(&removed);
        self.reconcile_active_surface();
        self.invalidate_ui_commit();
        Ok(())
    }

    /// Resolves an already authenticated internal root identity to its concrete forest node.
    pub(super) fn root_node(&self, root: RootId) -> Result<&SurfaceNode, SurfaceMutationError> {
        self.surfaces.root_node(root).ok_or(SurfaceMutationError::UnknownWindow)
    }

    /// Mutably resolves an already authenticated root identity to its concrete forest node.
    pub(super) fn root_node_mut(&mut self, root: RootId) -> Result<&mut SurfaceNode, SurfaceMutationError> {
        self.surfaces.root_node_mut(root).ok_or(SurfaceMutationError::UnknownWindow)
    }

    /// Resolves one application-facing window handle to its process-unique concrete identity.
    pub(super) fn window_id(&self, window: &WindowHandle) -> Result<RootId, SurfaceMutationError> {
        // Process-wide uniqueness makes membership in this forest sufficient authentication. A
        // stale or foreign handle cannot share its ID with any current node.
        self.surfaces
            .root_node(window.id())
            .and_then(SurfaceNode::root)
            .map(|_| window.id())
            .ok_or(SurfaceMutationError::UnknownWindow)
    }

    /// Queries effective visibility after authenticating the window against this forest.
    pub(crate) fn window_visible(&self, window: &WindowHandle) -> Result<bool, SurfaceMutationError> {
        // Read the policy already derived from authoritative parent edges at the mutation boundary.
        self.window_id(window).map(|id| self.root_is_visible(id))
    }

    /// Authenticates a creation owner and requires an ordinary window role.
    pub(super) fn creation_owner_id(&self, owner: &WindowHandle, invalid_role: SurfaceMutationError) -> Result<RootId, SurfaceMutationError> {
        // Authenticate first so stale and foreign handles retain the precise UnknownWindow reason.
        // Both child windows and dialogs accept independent or structural-child owners, while a
        // modal root is rejected with the operation-specific reason supplied by its caller.
        let owner = self.window_id(owner)?;
        let state = self.root_node(owner)?.root().expect("root lookup must return root policy");
        if matches!(state.mode, RootMode::Normal { .. } | RootMode::Child) {
            Ok(owner)
        } else {
            Err(invalid_role)
        }
    }

    /// Resolves one application-facing popup handle to its process-unique concrete identity.
    pub(super) fn popup_id(&self, popup: &PopupHandle) -> Result<PopupId, SurfaceMutationError> {
        // A menu popup can share the `PopupId` wrapper but never the same process-wide value. The
        // role check additionally preserves the public API boundary around application popups.
        self.surfaces
            .popup_node(popup.id())
            .and_then(SurfaceNode::popup)
            .filter(|popup| matches!(popup, PopupState::Application { .. }))
            .map(|_| popup.id())
            .ok_or(SurfaceMutationError::UnknownPopup)
    }

    /// Collects one root and every structurally owned child window or modal dialog below it.
    pub(super) fn owned_root_ids(&self, root: RootId) -> Vec<RootId> {
        let mut ids = vec![root];
        // Every node has one parent, so direct adjacency visits each descendant once without
        // duplicate detection or repeated global scans. Popup subtrees cannot own root windows.
        let mut index = 0;
        while index < ids.len() {
            for &key in &self.surfaces.index[&SurfaceKey::Root(ids[index])].children {
                if let SurfaceKey::Root(child) = key {
                    ids.push(child);
                }
            }
            index += 1;
        }
        ids
    }

    /// Returns whether a popup owner may participate under current visibility and modal policy.
    pub(super) fn popup_owner_is_eligible(&self, owner: RootId) -> bool {
        let Some(state) = self.surfaces.root_node(owner).and_then(SurfaceNode::root) else {
            return false;
        };
        if !self.surfaces.root_is_effectively_visible(owner) || !self.surfaces.root_is_effectively_enabled(owner) {
            return false;
        }
        match self.surfaces.active_modal_root() {
            Some(modal) => owner == modal,
            None => matches!(state.mode, RootMode::Normal { .. } | RootMode::Child),
        }
    }

    /// Returns the owning root derived from the deepest active popup's ancestry.
    pub(super) fn active_popup_owner(&self) -> Option<RootId> {
        self.surfaces.active_popup.and_then(|popup| self.surfaces.owning_root(SurfaceKey::Popup(popup)))
    }

    /// Removes the active path suffix after `keep`, notifying deepest popups first.
    pub(super) fn truncate_active_popup_path(&mut self, keep: usize) {
        let Some(mut current) = self.surfaces.active_popup else { return };
        let restore_owner = match self.active_surface {
            Some(SurfaceKey::Popup(active)) if self.surfaces.popup_is_active(active)
                && self.surfaces.popup_depth(active).is_some_and(|depth| depth >= keep) => {
                // Save only the owning root. A direct parent may be removed by this same suffix.
                self.surfaces.owning_root(SurfaceKey::Popup(active))
            }
            _ => None,
        };
        let mut revoked_capture = false;
        let mut retained_leaf = Some(current);
        while self.surfaces.popup_depth(current).is_some_and(|depth| depth >= keep) {
            let parent = match self.surfaces.popup_node(current).and_then(|node| node.parent) {
                Some(SurfaceKey::Popup(parent)) => Some(parent),
                _ => None,
            };
            let popup = self.surfaces.popup_node_mut(current).expect("active popup must remain retained");
            // Once the popup leaves the active ancestry only the manager can consume the remainder
            // of an application capture gesture, so remember capture before clearing the runtime.
            revoked_capture |= popup.surface.body.owns_pointer_gesture();
            let result = popup.dismiss_popup();
            self.posted(result);
            retained_leaf = parent;
            let Some(parent) = parent else { break };
            current = parent;
        }
        self.discard_pointer_capture_tail |= revoked_capture;
        self.surfaces.set_active_popup(retained_leaf);
        if let Some(owner) = restore_owner {
            // Visibility has been committed. Derive focus from the surviving path, skipping
            // ineligible retained nodes; never replace an unrelated window's active surface.
            let mut destination = retained_leaf.map(SurfaceKey::Popup).or(Some(SurfaceKey::Root(owner)));
            self.active_surface = loop {
                match destination {
                    Some(surface) if self.surface_can_receive_input(surface) => break Some(surface),
                    Some(SurfaceKey::Popup(popup)) => destination = self.surfaces.popup_node(popup).and_then(|node| node.parent),
                    _ => break None,
                }
            };
        }
        self.reconcile_active_surface();
        self.invalidate_ui_commit();
    }

    /// Dismisses the complete active popup branch.
    pub(super) fn dismiss_active_popups(&mut self) {
        self.truncate_active_popup_path(0);
    }
}
