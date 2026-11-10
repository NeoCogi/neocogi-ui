//! Cross-surface focus, popup/menu selection, capture and ordered input policy.
use super::*;

impl WindowManager {
    /// Returns whether one pointer event belongs to a capture revoked with a dismissed popup.
    pub(super) fn discard_revoked_capture_event(&mut self, event: &UiInputEvent) -> bool {
        // A fresh press always starts a new gesture and cancels any stale suppression. Drag remains
        // swallowed while armed; the terminal release is swallowed once and clears the boundary.
        match event {
            UiInputEvent::MouseDown { .. } => {
                self.discard_pointer_capture_tail = false;
                false
            }
            UiInputEvent::MouseDrag { .. } => self.discard_pointer_capture_tail,
            UiInputEvent::MouseUp { .. } if self.discard_pointer_capture_tail => {
                self.discard_pointer_capture_tail = false;
                true
            }
            _ => false,
        }
    }

    /// Borrows the concrete menu presentation attached to one root or menu-popup key.
    pub(super) fn menu_surface(&self, key: SurfaceKey) -> Option<&MenuSurface> {
        match key {
            SurfaceKey::Root(root) => self.surfaces.root_node(root)?.root()?.menu_bar.as_deref(),
            SurfaceKey::Popup(popup) => self.surfaces.popup_node(popup)?.surface.body.menu(),
        }
    }

    /// Mutably borrows the concrete menu presentation attached to one forest key.
    pub(super) fn menu_surface_mut(&mut self, key: SurfaceKey) -> Option<&mut MenuSurface> {
        match key {
            SurfaceKey::Root(root) => self.surfaces.root_node_mut(root)?.root_mut()?.menu_bar.as_deref_mut(),
            SurfaceKey::Popup(popup) => self.surfaces.popup_node_mut(popup)?.surface.body.menu_mut(),
        }
    }

    /// Returns the exact active surface when it denotes a concrete menu container.
    pub(super) fn active_menu_surface(&self) -> Option<SurfaceKey> {
        let surface = self.active_surface?;
        if !self.surface_can_receive_input(surface) {
            return None;
        }
        match surface {
            SurfaceKey::Root(_) => {
                // The root key is shared with its intrinsic bar, so a selected direct child marks
                // that concrete container as the active route.
                self.menu_surface(surface)?.keyboard_slot().map(|_| surface)
            }
            SurfaceKey::Popup(popup) if self.surfaces.popup_node(popup).and_then(|node| node.surface.body.menu()).is_some() => {
                // Placement policy does not define presentation. Both standalone and relational
                // menu popups retain their own keyboard scope even when they contain no rows.
                Some(surface)
            }
            SurfaceKey::Popup(_) => None,
        }
    }

    /// Returns the root derived from the active concrete menu surface.
    pub(super) fn active_menu_root(&self) -> Option<RootId> {
        self.active_menu_surface().and_then(|surface| self.surfaces.owning_root(surface))
    }

    /// Returns whether a traversal surface belongs to the current input group.
    pub(super) fn surface_is_eligible(&self, key: SurfaceKey, modal: Option<RootId>) -> bool {
        let Some(owner) = self.surfaces.owning_root(key) else {
            return false;
        };
        match modal {
            Some(modal) => owner == modal,
            None => self
                .surfaces
                .root_node(owner)
                .and_then(SurfaceNode::root)
                .is_some_and(|root| matches!(root.mode, RootMode::Normal { .. } | RootMode::Child)),
        }
    }

    /// Returns whether one retained surface and its structural enabled ancestry accept input.
    pub(super) fn surface_accepts_input(&self, key: SurfaceKey) -> bool {
        let Some(owner) = self.surfaces.owning_root(key) else {
            return false;
        };
        // A popup may be disabled independently, while every surface also inherits the effective
        // enabled state of its owning root. Root options are therefore checked exactly once by the
        // forest helper and popup options only at their own concrete node.
        self.surfaces.root_is_effectively_enabled(owner)
            && self
                .surfaces
                .node(key)
                .is_some_and(|node| !node.surface.options.intersects(WindowOption::DISABLED))
    }

    /// Returns current input eligibility without retaining a second focus projection.
    /// Display, modal membership and enabled ancestry are one current-input policy.
    pub(super) fn surface_can_receive_input(&self, surface: SurfaceKey) -> bool {
        self.surfaces.is_displayed(surface) && self.surface_is_eligible(surface, self.surfaces.active_modal_root()) && self.surface_accepts_input(surface)
    }

    /// Repairs active-surface identity after visibility or ownership changes.
    pub(super) fn reconcile_active_surface(&mut self) {
        let modal = self.surfaces.active_modal_root();
        let retained = self.active_surface.is_some_and(|surface| self.surface_can_receive_input(surface));
        if !retained {
            // A modal root is the only mandatory fallback. Ordinary roots continue using the front
            // visible surface lazily until a pointer or window-cycle command selects one exactly.
            self.active_surface = modal.map(SurfaceKey::Root).filter(|surface| self.surface_can_receive_input(*surface));
        }
    }

    /// Rewrites menu trigger presentation from the deepest authoritative popup leaf.
    pub(super) fn sync_menu_presentation(&mut self) {
        // Clear every derived highlight first. This is linear in menu surfaces and avoids retaining
        // a second open-path projection beside the forest's authoritative active leaf.
        for index in 0..self.surfaces.nodes.len() {
            let node = &mut self.surfaces.nodes[index];
            if let Some(root) = node.root_mut()
                && let Some(bar) = root.menu_bar.as_deref_mut()
            {
                bar.set_open_slot(None);
            }
            if let Some(menu) = node.surface.body.menu_mut() {
                menu.set_open_slot(None);
            }
        }

        // Each active menu popup marks exactly the trigger slot in its direct parent surface.
        for index in 0..self.surfaces.visible_order.len() {
            let key = self.surfaces.visible_order[index];
            let SurfaceKey::Popup(popup) = key else { continue };
            let relation = {
                let Some(node) = self.surfaces.popup_node(popup) else { continue };
                let Some(parent) = node.parent else { continue };
                let PopupState::Menu { trigger_slot } = node.popup().expect("popup key must retain popup policy") else {
                    continue;
                };
                (parent, *trigger_slot)
            };
            let (parent, trigger_slot) = relation;
            if let Some(source) = self.menu_surface_mut(parent) {
                source.set_open_slot(Some(trigger_slot));
            }
        }
    }

    /// Resolves the original application request or the currently visible parent-row relation.
    pub(super) fn resolved_popup_placement(&self, key: SurfaceKey) -> Option<PopupPlacement> {
        let SurfaceKey::Popup(popup) = key else { return None };
        let definition = self.surfaces.popup_node(popup)?;
        match definition.popup()? {
            PopupState::Application { placement, .. } => Some(*placement),
            PopupState::Menu { trigger_slot } => {
                let parent = definition.parent?;
                let menu = self.menu_surface(parent)?;
                let trigger = menu.slot_rect(*trigger_slot)?;
                let below = matches!(parent, SurfaceKey::Root(_));
                let outer = menu.outer_rect();
                Some(PopupPlacement::Anchored {
                    anchor: if below {
                        trigger
                    } else {
                        Recti::new(outer.x, trigger.y, outer.width, trigger.height)
                    },
                    side: if below {
                        neocogi_ui::abi::core::PopupSide::Below
                    } else {
                        neocogi_ui::abi::core::PopupSide::Right
                    },
                    preferred: Dimensioni::default(),
                })
            }
        }
    }

    /// Revokes competing pointer gestures without clearing any scope's remembered keyboard focus.
    pub(super) fn clear_other_pointer_captures(&mut self, keep: SurfaceKey) {
        for node in &mut self.surfaces.nodes {
            if node.key != keep {
                node.clear_pointer_targets();
            }
        }
        // A root menu bar and its application body share one forest key, so revoke same-root widget
        // capture explicitly while leaving its focus identity intact.
        if let Some(node) = self.surfaces.node_mut(keep) {
            node.surface.body.clear_pointer_targets();
        }
    }

    /// Clears cached menu hover before one topmost surface receives the current pointer event.
    pub(super) fn clear_menu_pointer_hover(&mut self) {
        // Menu bars share their root key with application content, so ordinary widget routing cannot
        // tell the bar that the pointer left it. Clear every compact surface first; routing below
        // restores hover only on the menu that actually owns the event without revoking capture.
        for node in &mut self.surfaces.nodes {
            if let Some(root) = node.root_mut()
                && let Some(bar) = root.menu_bar.as_deref_mut()
            {
                bar.clear_pointer_hover();
            }
            if let Some(menu) = node.surface.body.menu_mut() {
                menu.clear_pointer_hover();
            }
        }
    }

    /// Commits one exclusive root-chrome hover target for stateful caption and resize painting.
    pub(super) fn update_chrome_hover(&mut self, surface: Option<SurfaceKey>, point: Vec2i) {
        // Clear every root first because input selection is topmost and exclusive. A popup or menu
        // hover therefore removes stale caption art from the owning and underlying windows alike.
        for node in &mut self.surfaces.nodes {
            if let Some(root) = node.root_mut() {
                root.hovered_chrome = None;
            }
        }
        let Some(SurfaceKey::Root(root)) = surface else {
            return;
        };
        let part = self.root_node(root).ok().and_then(|node| node.chrome_part_at(point));
        if let Ok(node) = self.root_node_mut(root) {
            node.root_mut().expect("root hover target must retain root policy").hovered_chrome = part;
        }
    }

    /// Returns whether the concrete menu attached to a visible key contains one pointer point.
    pub(super) fn menu_contains(&self, key: SurfaceKey, point: Vec2i) -> bool {
        self.menu_surface(key).is_some_and(|menu| menu.contains(point))
    }

    /// Routes a pointer event directly to a root bar or menu-popup body.
    pub(super) fn route_menu_pointer(&mut self, key: SurfaceKey, event: &UiInputEvent) -> Option<crate::runtime::menu::MenuRoute> {
        // A click or keyboard command must establish the menu scope before pointer movement can
        // navigate it. Restrict hot-tracking to the same owning root so merely crossing a closed
        // window's menu bar cannot steal another window's active menu path.
        let hot_tracking = self
            .active_menu_root()
            .zip(self.surfaces.owning_root(key))
            .is_some_and(|(active, owner)| active == owner && self.surfaces.active_popup.is_some());
        self.menu_surface_mut(key).map(|menu| menu.route_pointer(event, hot_tracking))
    }

    /// Returns the direct menu-popup child opened by one branch slot.
    pub(super) fn menu_popup_for_slot(&self, surface: SurfaceKey, slot: usize) -> Option<PopupId> {
        // Restrict lookup to direct derived edges; the forest remains the sole menu owner.
        self.surfaces.index.get(&surface)?.children.iter().find_map(|&key| {
            let node = self.surfaces.node(key)?;
            match (key, node.popup()) {
                (SurfaceKey::Popup(popup), Some(PopupState::Menu { trigger_slot })) if *trigger_slot == slot => Some(popup),
                _ => None,
            }
        })
    }

    /// Clears every derived menu-child selection after restoring the parent surface.
    pub(super) fn finish_keyboard_menu(&mut self, dismiss_popups: bool) {
        if dismiss_popups {
            // Item submission and explicit Alt/F10 cancellation close the complete menu ancestry.
            self.dismiss_active_popups();
        }
        for node in &mut self.surfaces.nodes {
            if let Some(root) = node.root_mut()
                && let Some(bar) = root.menu_bar.as_deref_mut()
            {
                bar.clear_keyboard_target();
            }
            if let Some(menu) = node.surface.body.menu_mut() {
                menu.clear_keyboard_target();
            }
        }
        self.pending_menu_alt = false;
    }

    /// Opens one branch popup and selects an edge item for continued keyboard navigation.
    pub(super) fn open_menu_slot(&mut self, surface: SurfaceKey, slot: usize, select_last: bool) -> bool {
        let Some(target) = self.menu_popup_for_slot(surface, slot) else {
            return false;
        };
        let _ = self.menu_surface_mut(surface).is_some_and(|menu| menu.focus_slot(slot));
        if self.open_popup_id(target).is_err() {
            return false;
        }
        let child = self
            .menu_surface_mut(SurfaceKey::Popup(target))
            .expect("a menu branch must own a compact menu-popup surface");
        let selected = if select_last { child.focus_last() } else { child.focus_first() };
        // The popup forest identity and the selected local child now describe the complete menu
        // focus route; no separate manager-owned menu-root mirror is necessary.
        self.active_surface = Some(SurfaceKey::Popup(target));
        selected
    }

    /// Enters the active window's intrinsic menu bar while preserving application widget focus.
    pub(super) fn begin_keyboard_menu(&mut self) -> bool {
        let Some(surface) = self.keyboard_input_surface() else { return false };
        let Some(root) = self.surfaces.owning_root(surface) else { return false };
        if !self
            .surfaces
            .root_node(root)
            .and_then(SurfaceNode::root)
            .and_then(|state| state.menu_bar.as_deref())
            .is_some()
        {
            return false;
        }

        // An application popup and an intrinsic menu cannot share the sole transient branch.
        // Dismiss it before borrowing the bar and installing the first selection.
        self.dismiss_active_popups();
        let selected = self.menu_surface_mut(SurfaceKey::Root(root)).is_some_and(MenuSurface::focus_first);
        if selected {
            self.active_surface = Some(SurfaceKey::Root(root));
        }
        selected
    }

    /// Moves the selected bar heading and optionally replaces the open top-level popup.
    pub(super) fn move_keyboard_menu_heading(&mut self, root: RootId, forward: bool, open: bool) -> bool {
        let bar = SurfaceKey::Root(root);
        let Some(menu) = self.menu_surface_mut(bar) else {
            return false;
        };
        if !menu.move_keyboard_focus(forward) {
            return false;
        }
        let Some(slot) = menu.keyboard_slot() else { return false };
        !open || self.open_menu_slot(bar, slot, false)
    }

    /// Closes one popup level and restores selection to its parent trigger.
    pub(super) fn close_keyboard_menu_level(&mut self) -> bool {
        let Some(popup) = self.surfaces.active_popup else {
            self.finish_keyboard_menu(false);
            return true;
        };
        let Some(node) = self.surfaces.popup_node(popup) else {
            self.finish_keyboard_menu(true);
            return true;
        };
        let Some(parent) = node.parent else {
            self.finish_keyboard_menu(true);
            return true;
        };
        let PopupState::Menu { trigger_slot } = node.popup().expect("popup key must retain popup policy") else {
            self.finish_keyboard_menu(true);
            return true;
        };
        let trigger_slot = *trigger_slot;
        let depth = self.surfaces.popup_depth(popup).expect("active menu popup must retain rooted ancestry");
        self.truncate_active_popup_path(depth);
        let _ = self.menu_surface_mut(parent).is_some_and(|menu| menu.focus_slot(trigger_slot));
        true
    }

    /// Applies a key press inside the current bar or popup scope.
    pub(super) fn apply_keyboard_menu_key(&mut self, root: RootId, event: crate::runtime::KeyEvent) -> bool {
        if !event.is_pressed() {
            // All releases remain manager-owned while the menu scope is active.
            return true;
        }
        let Some(surface) = self.active_menu_surface() else {
            return false;
        };
        let popup = matches!(surface, SurfaceKey::Popup(_));
        // Only a relational popup attached directly to a root has sibling bar headings. A
        // standalone popup and every submenu use their own local horizontal-key behavior.
        let bar_popup = match surface {
            SurfaceKey::Popup(popup) => self
                .surfaces
                .popup_node(popup)
                .is_some_and(|node| matches!(node.popup(), Some(PopupState::Menu { .. })) && matches!(node.parent, Some(SurfaceKey::Root(_)))),
            SurfaceKey::Root(_) => false,
        };
        match event.key {
            crate::runtime::Key::Enter | crate::runtime::Key::Space if !self.menu_surface(surface).is_some_and(MenuSurface::accepts_activation) => true,
            crate::runtime::Key::ArrowDown if popup => self.menu_surface_mut(surface).is_some_and(|menu| menu.move_keyboard_focus(true)),
            crate::runtime::Key::ArrowUp if popup => self.menu_surface_mut(surface).is_some_and(|menu| menu.move_keyboard_focus(false)),
            crate::runtime::Key::PageDown if popup => self.menu_surface_mut(surface).is_some_and(|menu| menu.page_keyboard_focus(true)),
            crate::runtime::Key::PageUp if popup => self.menu_surface_mut(surface).is_some_and(|menu| menu.page_keyboard_focus(false)),
            crate::runtime::Key::Home => self.menu_surface_mut(surface).is_some_and(MenuSurface::focus_first),
            crate::runtime::Key::End => self.menu_surface_mut(surface).is_some_and(MenuSurface::focus_last),
            crate::runtime::Key::ArrowRight if !popup => self.move_keyboard_menu_heading(root, true, false),
            crate::runtime::Key::ArrowLeft if !popup => self.move_keyboard_menu_heading(root, false, false),
            crate::runtime::Key::ArrowDown if !popup => {
                let Some(slot) = self.menu_surface(surface).and_then(MenuSurface::keyboard_slot) else {
                    return true;
                };
                self.open_menu_slot(surface, slot, false)
            }
            crate::runtime::Key::Enter | crate::runtime::Key::Space if !popup && !event.repeat => {
                let Some(slot) = self.menu_surface(surface).and_then(MenuSurface::keyboard_slot) else {
                    return true;
                };
                self.open_menu_slot(surface, slot, false)
            }
            crate::runtime::Key::ArrowUp if !popup => {
                let Some(slot) = self.menu_surface(surface).and_then(MenuSurface::keyboard_slot) else {
                    return true;
                };
                self.open_menu_slot(surface, slot, true)
            }
            crate::runtime::Key::ArrowRight if popup => match self.menu_surface(surface).and_then(MenuSurface::keyboard_branch_slot) {
                Some(slot) => self.open_menu_slot(surface, slot, false),
                None if bar_popup => self.move_keyboard_menu_heading(root, true, true),
                None => true,
            },
            crate::runtime::Key::ArrowLeft if popup => {
                let SurfaceKey::Popup(popup) = surface else { unreachable!() };
                if bar_popup {
                    self.move_keyboard_menu_heading(root, false, true)
                } else if self.surfaces.popup_depth(popup) == Some(0) {
                    // A standalone popup menu has no parent row or sibling heading to visit.
                    true
                } else {
                    self.close_keyboard_menu_level()
                }
            }
            crate::runtime::Key::Enter | crate::runtime::Key::Space if popup && !event.repeat => {
                let action = self.menu_surface_mut(surface).and_then(MenuSurface::activate_keyboard_slot);
                match action {
                    Some(MenuAction::OpenSlot(slot)) => self.open_menu_slot(surface, slot, false),
                    Some(MenuAction::HoverOpenSlot(_) | MenuAction::HoverCloseChild) => true,
                    Some(MenuAction::SubmitAndClose(result)) => {
                        self.posted(result);
                        self.finish_keyboard_menu(true);
                        true
                    }
                    None => true,
                }
            }
            crate::runtime::Key::Escape => self.close_keyboard_menu_level(),
            _ => true,
        }
    }

    /// Handles Alt/F10 scope transitions and active menu navigation before widget key routing.
    pub(super) fn route_menu_keyboard(&mut self, event: &UiInputEvent) -> bool {
        let UiInputEvent::Key { event } = event else {
            // Text input belongs to neither mnemonics nor type-ahead in this MVP, but an active
            // menu still consumes it so suspended application focus cannot edit behind the menu.
            return self.active_menu_surface().is_some() && matches!(event, UiInputEvent::Text { .. });
        };

        if event.key == crate::runtime::Key::Alt {
            if event.is_pressed() {
                self.pending_menu_alt = !event.repeat;
            } else if std::mem::take(&mut self.pending_menu_alt) {
                if self.active_menu_surface().is_some() {
                    self.finish_keyboard_menu(true);
                } else {
                    let _ = self.begin_keyboard_menu();
                }
            }
            return true;
        }
        if event.is_pressed() && self.pending_menu_alt {
            // Any chorded key cancels Alt-tap activation; this event may continue to a focused
            // widget when no keyboard menu is already active.
            self.pending_menu_alt = false;
        }

        let plain_f10 = event.key == crate::runtime::Key::Function(10) && event.modifiers == crate::runtime::Modifiers::NONE;
        if plain_f10 {
            if event.is_pressed() && !event.repeat {
                if self.active_menu_surface().is_some() {
                    self.finish_keyboard_menu(true);
                } else {
                    let _ = self.begin_keyboard_menu();
                }
            }
            return true;
        }

        let Some(root) = self.active_menu_root() else {
            return false;
        };
        self.apply_keyboard_menu_key(root, *event)
    }

    /// Dismisses an active application popup on one initial non-repeated Escape press.
    pub(super) fn route_application_popup_keyboard(&mut self, event: &UiInputEvent) -> bool {
        // Text and pointer events cannot participate in the popup's keyboard command. Keeping the
        // variant test at the boundary also leaves their normal routing entirely untouched.
        let UiInputEvent::Key { event } = event else {
            return false;
        };
        // Dismissal is deliberately a press-only command. Repeats and releases continue through
        // ordinary routing to the surface active for those later transitions, avoiding retained
        // key-tail state in WindowManager and avoiding a global reservation of Escape key-up.
        if event.key != crate::runtime::Key::Escape || !event.is_pressed() || event.repeat {
            return false;
        }

        // Only the concrete active application-popup surface may begin this command. Menu popups,
        // ordinary roots, and modal scopes retain their independent Escape policies.
        let Some(SurfaceKey::Popup(popup)) = self.active_surface else {
            return false;
        };
        let Some(node) = self.surfaces.popup_node(popup) else {
            return false;
        };
        if !self.surface_accepts_input(SurfaceKey::Popup(popup))
            || !self.surfaces.popup_is_active(popup)
            || !matches!(node.popup(), Some(PopupState::Application { .. }))
            || !matches!(node.surface.body, SurfaceBody::Widgets { .. })
        {
            return false;
        }

        // Truncation immediately restores the direct parent as the active keyboard surface. Any
        // later physical transitions are independent input and will resolve against that surface.
        let depth = self.surfaces.popup_depth(popup).expect("active application popup must retain rooted ancestry");
        self.truncate_active_popup_path(depth);
        true
    }

    /// Handles the Windows-style Ctrl+F6 window-cycle command before widget key routing.
    pub(super) fn route_window_keyboard(&mut self, event: &UiInputEvent) -> bool {
        let UiInputEvent::Key { event } = event else {
            return false;
        };
        if event.key != crate::runtime::Key::Function(6) {
            return false;
        }

        // Once a qualifying press is accepted, every later F6 transition remains manager-owned
        // until release. This closes the command boundary even if Control is released first.
        if self.window_cycle_key_down {
            if !event.is_pressed() {
                self.window_cycle_key_down = false;
            }
            return true;
        }
        let forward_modifiers = crate::runtime::Modifiers::CTRL;
        let reverse_modifiers = crate::runtime::Modifiers::CTRL | crate::runtime::Modifiers::SHIFT;
        let is_cycle_chord = event.modifiers == forward_modifiers || event.modifiers == reverse_modifiers;
        if !is_cycle_chord {
            return false;
        }
        if !event.is_pressed() {
            // Consume an isolated matching release defensively without inventing an activation.
            return true;
        }
        self.window_cycle_key_down = true;

        // Repeated presses belong to the accepted physical chord but do not race through windows.
        // An active intrinsic menu or modal dialog likewise retains its current keyboard scope.
        if event.repeat || self.active_menu_surface().is_some() || self.surfaces.active_modal_root().is_some() {
            return true;
        }

        // Resolve the source before closing an application popup because that popup may currently
        // own capture and therefore identify the active ordinary window. Each candidate runtime
        // remains mounted, preserving its remembered widget focus across the switch.
        let current = self.keyboard_input_surface().and_then(|surface| self.surfaces.owning_root(surface));
        self.dismiss_active_popups();
        let Some(target) = self.surfaces.window_cycle_target(current, event.modifiers == reverse_modifiers) else {
            return true;
        };
        self.active_surface = Some(SurfaceKey::Root(target));
        self.bring_window_to_front_id(target)
            .expect("a visible cycle target collected from the forest must remain registered");
        true
    }

    /// Applies one pointer-produced menu action after releasing the originating surface borrow.
    pub(super) fn apply_menu_action(&mut self, surface: SurfaceKey, action: MenuAction, previous_top: Option<PopupId>) {
        match action {
            MenuAction::OpenSlot(slot) => {
                let target = self
                    .menu_popup_for_slot(surface, slot)
                    .expect("a menu branch slot must retain one direct popup child");
                // Outside dismissal already closed the old path before this bar press reached the
                // window. A repeated heading press therefore closes the keyboard scope as well.
                if matches!(surface, SurfaceKey::Root(_)) && previous_top == Some(target) {
                    self.finish_keyboard_menu(false);
                    return;
                }
                let _ = self.open_menu_slot(surface, slot, false);
            }
            MenuAction::HoverOpenSlot(slot) => {
                // Hot-tracking never toggles the current heading closed. MenuSurface emits this
                // action only when the hovered branch differs from the child already projected from
                // the active forest path, so opening it atomically replaces only the required suffix.
                let _ = self.open_menu_slot(surface, slot, false);
            }
            MenuAction::HoverCloseChild => {
                // Only popup rows can produce this action: every menu-bar slot is a branch. Retain
                // the hovered row's own popup and remove the child level plus every descendant.
                if let SurfaceKey::Popup(popup) = surface
                    && let Some(depth) = self.surfaces.popup_depth(popup)
                {
                    self.truncate_active_popup_path(depth.saturating_add(1));
                }
            }
            MenuAction::SubmitAndClose(result) => {
                self.posted(result);
                // The surface already queued the typed event. Arm tail suppression before removing
                // its popup so the physical release cannot fall through to newly exposed content.
                self.discard_pointer_capture_tail = true;
                self.finish_keyboard_menu(true);
            }
        }
    }

    /// Routes one event to window chrome and reports whether the overlay consumed it.
    pub(super) fn route_chrome_event(&mut self, root: RootId, event: &UiInputEvent, caption_controls_visible: bool, minimized_content_height: i32) -> bool {
        if self.root_node(root).is_err() || !self.surface_accepts_input(SurfaceKey::Root(root)) {
            return false;
        }
        match event {
            UiInputEvent::MouseDown { pos, button } if button.intersects(MouseButton::LEFT) => {
                match self.root_node(root).ok().and_then(|node| node.chrome_part_at(*pos)) {
                    Some(RootChromePart::Caption(button)) if caption_controls_visible => {
                        // Caption actions commit on release over the same button. Capture preserves
                        // pressed-state PNG selection and lets a drag outside cancel the action.
                        let node = self.root_node_mut(root).expect("chrome target must remain retained");
                        node.surface.body.clear_transient_targets();
                        node.root_mut().expect("chrome target must be a root").interaction = RootInteraction::Caption(button);
                        true
                    }
                    Some(RootChromePart::Resize(axis)) => {
                        let node = self.root_node_mut(root).expect("chrome target must remain retained");
                        node.surface.body.clear_transient_targets();
                        node.root_mut().expect("chrome target must be a root").interaction = RootInteraction::Resizing(axis);
                        true
                    }
                    Some(RootChromePart::Caption(_)) | Some(RootChromePart::Title) => {
                        // A base Classic Mac title does not display caption boxes. Its first
                        // press therefore treats their reserved geometry as ordinary title rather
                        // than activating an invisible close, zoom, or windowshade action.
                        let node = self.root_node_mut(root).expect("chrome target must remain retained");
                        node.surface.body.clear_transient_targets();
                        node.root_mut().expect("chrome target must be a root").interaction = RootInteraction::Moving;
                        true
                    }
                    None => false,
                }
            }
            UiInputEvent::MouseDrag { pos, delta, .. } => {
                let node = self.root_node_mut(root).expect("drag target must remain retained");
                let initial = node.surface.rect;
                match node.root().expect("drag target must be a root").interaction {
                    RootInteraction::Moving => {
                        node.surface.rect.x = initial.x.saturating_add(delta.x);
                        node.surface.rect.y = initial.y.saturating_add(delta.y);
                    }
                    RootInteraction::Resizing(axis) => {
                        let minimum = node.surface.geometry.minimum_outer;
                        if matches!(axis, RootResizeAxis::Width | RootResizeAxis::Both) {
                            node.surface.rect.width = initial.width.saturating_add(delta.x).max(minimum.width);
                        }
                        if matches!(axis, RootResizeAxis::Height | RootResizeAxis::Both) {
                            node.surface.rect.height = initial.height.saturating_add(delta.y).max(minimum.height);
                        }
                    }
                    RootInteraction::Caption(_) => return true,
                    RootInteraction::None => return node.chrome_part_at(*pos).is_some(),
                }
                if (node.surface.rect.x, node.surface.rect.y, node.surface.rect.width, node.surface.rect.height)
                    != (initial.x, initial.y, initial.width, initial.height)
                {
                    // Snapshot after geometry mutation so subscribers receive the committed value.
                    let rect = node.surface.rect;
                    let result = neocogi_ui::abi::core::events::window_geometry_changed(crate::runtime::boundary::copied_rect(rect))
                        .and_then(|event| node.emit_window_event(event));
                    self.posted(result);
                    self.invalidate_ui_commit();
                }
                true
            }
            UiInputEvent::MouseUp { pos, button }
                if button.intersects(MouseButton::LEFT)
                    && self
                        .root_node(root)
                        .ok()
                        .and_then(SurfaceNode::root)
                        .is_some_and(|state| state.interaction != RootInteraction::None) =>
            {
                let (interaction, released_part) = {
                    let node = self.root_node(root).expect("release target must remain retained");
                    (node.root().expect("release target must be a root").interaction, node.chrome_part_at(*pos))
                };
                self.root_node_mut(root)
                    .expect("release target must remain retained")
                    .root_mut()
                    .expect("release target must be a root")
                    .interaction = RootInteraction::None;
                if let RootInteraction::Caption(button) = interaction
                    && released_part == Some(RootChromePart::Caption(button))
                {
                    self.activate_caption_button(root, button, minimized_content_height);
                }
                true
            }
            _ => event
                .position()
                .is_some_and(|point| self.root_node(root).ok().and_then(|node| node.chrome_part_at(point)).is_some()),
        }
    }

    /// Applies one caption action after its capture has ended on the originating button.
    pub(super) fn activate_caption_button(&mut self, root: RootId, button: RootCaptionButton, minimized_content_height: i32) {
        match button {
            RootCaptionButton::Close => {
                // A caption click is intent. The behavioral parent may decline,
                // hide or remove the window after handling its queued event.
                // Keep visibility and ownership unchanged, even if posting fails.
                let result = neocogi_ui::abi::core::events::window_close_requested().and_then(|event| {
                    self.root_node_mut(root)
                        .expect("caption target must remain registered")
                        .emit_window_event(event)
                });
                self.posted(result);
            }
            RootCaptionButton::Minimize => {
                let node = self.root_node_mut(root).expect("caption target must remain registered");
                let restore_height = node.root_mut().expect("caption target must be a root").unminimized_height.take();
                let event: fn(neocogi_ui::abi::core::Rect) -> Result<neocogi_com::ComBox<neocogi_ui::abi::core::IEventAbi>, neocogi_com::NcStatus> =
                    if let Some(height) = restore_height {
                        // Maximized windows restore against their live viewport; ordinary windows use
                        // the exact height saved by the preceding minimize action.
                        let maximized = node.root().expect("caption target must be a root").restore_rect.is_some();
                        node.surface.rect.height = if maximized { node.surface.clip.height } else { height };
                        neocogi_ui::abi::core::events::window_restored as fn(_) -> _
                    } else {
                        let expanded_height = node.surface.rect.height;
                        node.root_mut().expect("caption target must be a root").unminimized_height = Some(expanded_height);
                        node.surface.rect.height = node
                            .surface
                            .rect
                            .height
                            .saturating_sub(node.surface.geometry.body.height)
                            .saturating_add(minimized_content_height.max(0));
                        neocogi_ui::abi::core::events::window_minimized as fn(_) -> _
                    };
                let rect = crate::runtime::boundary::copied_rect(node.surface.rect);
                let result = event(rect).and_then(|event| node.emit_window_event(event));
                self.posted(result);
                self.invalidate_ui_commit();
            }
            RootCaptionButton::Maximize => {
                let node = self.root_node_mut(root).expect("caption target must remain registered");
                let state = node.root_mut().expect("caption target must be a root");
                let restore = state.restore_rect.take();
                let unminimized_height = state.unminimized_height.take();
                if let Some(height) = unminimized_height {
                    // Maximizing a compact window starts from its expanded normal geometry.
                    node.surface.rect.height = height;
                }
                let event: fn(neocogi_ui::abi::core::Rect) -> Result<neocogi_com::ComBox<neocogi_ui::abi::core::IEventAbi>, neocogi_com::NcStatus> =
                    if let Some(rect) = restore {
                        // Restore exactly the normal placement captured by the preceding maximize.
                        node.surface.rect = rect;
                        neocogi_ui::abi::core::events::window_restored as fn(_) -> _
                    } else {
                        // The committed clip is the complete available viewport for top-level, child,
                        // and modal roots. Save normal placement before replacing it.
                        let normal = node.surface.rect;
                        let rect = node.surface.clip;
                        node.root_mut().expect("caption target must be a root").restore_rect = Some(normal);
                        node.surface.rect = rect;
                        neocogi_ui::abi::core::events::window_maximized as fn(_) -> _
                    };
                let rect = crate::runtime::boundary::copied_rect(node.surface.rect);
                let result = event(rect).and_then(|event| node.emit_window_event(event));
                self.posted(result);
                self.invalidate_ui_commit();
            }
        }
    }
}

/// Fits one requested popup in finite permitted bounds. All edge arithmetic precedes narrowing.
/// The line clearance is soft only when it would prevent the smallest usable body.
pub(super) fn fit_popup_rect(
    request: PopupPlacement,
    clip: Recti,
    desired: Dimensioni,
    gap: i32,
    minimum_height: i32,
) -> (Recti, Option<neocogi_ui::abi::core::PopupSide>) {
    use neocogi_ui::abi::core::PopupSide;
    use crate::runtime::math::clamp_i64_to_i32;
    let left = i64::from(clip.x);
    let top = i64::from(clip.y);
    let right = left + i64::from(clip.width.max(0));
    let bottom = top + i64::from(clip.height.max(0));
    let gap = i64::from(gap.max(0)).min(bottom - top);
    let minimum = i64::from(minimum_height.max(0)).min(i64::from(desired.height.max(0)));
    let (mut low, mut high) = (top + gap, bottom - gap);
    if high - low < minimum {
        low = top;
        high = bottom;
    }
    let mut width = i64::from(desired.width.max(0)).min(right - left);
    let mut height = i64::from(desired.height.max(0));
    let (x, y, side) = match request {
        PopupPlacement::At(rect) => (i64::from(rect.x), i64::from(rect.y), None),
        PopupPlacement::Anchored { anchor, side, .. } => {
            let ax = i64::from(anchor.x);
            let ay = i64::from(anchor.y);
            let ar = ax + i64::from(anchor.width.max(0));
            let ab = ay + i64::from(anchor.height.max(0));
            match side {
                PopupSide::Below | PopupSide::Above => {
                    let mut below = (high - ab).max(0);
                    let mut above = (ay - low).max(0);
                    // Clearances must not make a usable one-row side disappear.
                    if below.max(above) < minimum {
                        low = top;
                        high = bottom;
                        below = (high - ab).max(0);
                        above = (ay - low).max(0);
                    }
                    let prefer_below = side == PopupSide::Below;
                    let preferred = if prefer_below { below } else { above };
                    let opposite = if prefer_below { above } else { below };
                    let choose_below = if height <= preferred || preferred >= opposite {
                        prefer_below
                    } else {
                        !prefer_below
                    };
                    // If the trigger covers the tiny frame, an overlapping clipped body
                    // is preferable to making every row unreachable until resize.
                    let available = if below.max(above) == 0 {
                        (high - low).max(0)
                    } else if choose_below {
                        below
                    } else {
                        above
                    };
                    height = height.min(available);
                    (
                        ax,
                        if choose_below { ab } else { ay - height },
                        Some(if choose_below { PopupSide::Below } else { PopupSide::Above }),
                    )
                }
                PopupSide::Right | PopupSide::Left => {
                    let space_right = (right - ar).max(0);
                    let space_left = (ax - left).max(0);
                    let prefer_right = side == PopupSide::Right;
                    let preferred = if prefer_right { space_right } else { space_left };
                    let opposite = if prefer_right { space_left } else { space_right };
                    let choose_right = if width <= preferred || preferred >= opposite {
                        prefer_right
                    } else {
                        !prefer_right
                    };
                    width = width.min(if choose_right { space_right } else { space_left });
                    (
                        if choose_right { ar } else { ax - width },
                        ay,
                        Some(if choose_right { PopupSide::Right } else { PopupSide::Left }),
                    )
                }
                _ => unreachable!("popup side is validated before request publication"),
            }
        }
    };
    height = height.min((high - low).max(0));
    let x = x.clamp(left, (right - width).max(left));
    let y = y.clamp(low, (high - height).max(low));
    (Recti::new(clamp_i64_to_i32(x), clamp_i64_to_i32(y), width as i32, height as i32), side)
}

#[cfg(all(test, feature = "test-support"))]
mod popup_fitting_tests {
    use super::*;
    use neocogi_ui::abi::core::PopupSide;

    /// Creates a complete trigger snapshot with the same preferred dimensions for each side.
    fn anchored(y: i32, side: PopupSide) -> PopupPlacement {
        PopupPlacement::Anchored {
            anchor: Recti::new(100, y, 120, 24),
            side,
            preferred: Dimensioni::default(),
        }
    }

    /// Requested side, complete rows and line clearance match the numerical examples.
    #[test]
    fn below_above_and_bounded_height_use_the_original_trigger() {
        let desired = Dimensioni::new(120, 242);
        for (frame_height, y, expected_y, expected_height, expected_side) in [
            (600, 100, 124, 242, PopupSide::Below),
            (600, 400, 158, 242, PopupSide::Above),
            (240, 100, 124, 100, PopupSide::Below),
        ] {
            let (rect, side) = fit_popup_rect(anchored(y, PopupSide::Below), Recti::new(0, 0, 600, frame_height), desired, 16, 26);
            assert_eq!((rect.x, rect.y, rect.width, rect.height), (100, expected_y, 120, expected_height));
            assert_eq!(side, Some(expected_side));
        }
    }

    /// Finite zero stays zero, negative origins stay bounded, and side fitting can flip left.
    #[test]
    fn finite_clips_and_submenu_sides_do_not_wrap_or_become_unbounded() {
        let trigger = PopupPlacement::Anchored {
            anchor: Recti::new(560, 200, 30, 24),
            side: PopupSide::Right,
            preferred: Dimensioni::default(),
        };
        let (rect, side) = fit_popup_rect(trigger, Recti::new(0, 0, 600, 300), Dimensioni::new(120, 200), 16, 12);
        assert_eq!(side, Some(PopupSide::Left));
        assert_eq!(rect.x, 440);
        assert!(rect.y + rect.height <= 284);
        for clip in [Recti::new(-50, -30, 40, 20), Recti::new(0, 0, 0, 0)] {
            let (rect, _) = fit_popup_rect(
                PopupPlacement::At(Recti::new(i32::MAX, i32::MIN, 200, 200)),
                clip,
                Dimensioni::new(200, 200),
                16,
                20,
            );
            assert!(rect.width <= clip.width && rect.height <= clip.height);
            assert!(rect.x >= clip.x && rect.y >= clip.y);
            assert!(i64::from(rect.x) + i64::from(rect.width) <= i64::from(clip.x) + i64::from(clip.width));
            assert!(i64::from(rect.y) + i64::from(rect.height) <= i64::from(clip.y) + i64::from(clip.height));
        }
    }
}
