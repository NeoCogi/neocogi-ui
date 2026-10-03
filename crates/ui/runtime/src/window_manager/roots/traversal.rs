//! Committed layout, eligible updates, clipping and painter traversal.
use super::*;

impl WindowManager {
    /// Begins one cycle and lays out every visible tree before either callbacks or input routing.
    pub(crate) fn begin_update_cycle(&mut self, dimensions: Dimensioni) -> Result<(), neocogi_com::NcStatus> {
        self.ui_commit = None;
        for node in &mut self.surfaces.nodes {
            // Hidden surfaces retain coherent staged state without receiving ordinary updates.
            node.surface.body.begin_update();
        }
        self.layout_update_cycle(dimensions)
    }

    /// Applies one queued input, or exactly one initial eventless update when no raw event is pending.
    /// Returning releases all traversal borrows before the caller dispatches application events.
    pub(crate) fn update_cycle_step(&mut self, first: bool) -> Result<bool, neocogi_com::NcStatus> {
        let atlas = self.bundle.atlas().clone();
        if let Some(event) = self.input.pop_event() {
            let input = self.input.snapshot();
            self.update_for_event(&atlas, &event, input)?;
            Ok(true)
        } else if first {
            let input = self.input.snapshot();
            self.update_eligible_widget_trees(&atlas, input)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Reuses the current bundle after callbacks, so a resource replacement cannot leave an old atlas paired with new skin.
    pub(crate) fn layout_update_cycle(&mut self, dimensions: Dimensioni) -> Result<(), neocogi_com::NcStatus> {
        let atlas = self.bundle.atlas().clone();
        self.layout(Recti::new(0, 0, dimensions.width, dimensions.height), &atlas)
    }

    /// Publishes a renderable commit only after every input/callback/layout boundary has completed successfully.
    pub(crate) fn finish_update_cycle(&mut self, dimensions: Dimensioni) {
        self.ui_commit = Some(dimensions);
    }

    /// Runs one ordinary update with no routed event across every currently eligible widget tree.
    pub(super) fn update_eligible_widget_trees(&mut self, atlas: &crate::AtlasHandle, input: crate::input::InputSnapshot) -> Result<(), neocogi_com::NcStatus> {
        // Copy the resolved skin once, matching event-driven traversal, then use the forest's
        // shared visible order so modal scope and popup ownership have one eligibility policy.
        let style = self.bundle.skin().clone();
        let modal = self.surfaces.active_modal_root();
        for index in 0..self.surfaces.visible_order.len() {
            let key = self.surfaces.visible_order[index];
            if self.surface_is_eligible(key, modal) {
                self.surfaces
                    .surface_mut(key)
                    .expect("visible update surface must remain retained")
                    .body
                    .update(&style, atlas.clone(), input)?;
            }
        }
        self.reconcile_active_surface();
        Ok(())
    }

    /// Resolves the effective screen-space clip for one visible surface during parent-first layout.
    pub(super) fn resolved_surface_clip(&self, key: SurfaceKey, viewport: Recti) -> Recti {
        match key {
            SurfaceKey::Root(root) => {
                let node = self.surfaces.root_node(root).expect("visible root must remain retained");
                let state = node.root().expect("root key must retain root policy");
                if state.mode != RootMode::Child {
                    return viewport;
                }

                // A child inherits the complete boundary already committed by its direct parent.
                // Content clipping narrows that inherited rectangle once at this edge; deeper
                // children naturally accumulate every clipping ancestor through the same rule.
                let parent = match node.parent {
                    Some(SurfaceKey::Root(parent)) => parent,
                    Some(SurfaceKey::Popup(_)) | None => unreachable!("a child window must retain one root parent"),
                };
                let parent = self.surfaces.root_node(parent).expect("visible child parent must remain retained");
                let inherited = parent.surface.clip;
                match parent.root().expect("child parent must retain root policy").child_window_clip {
                    ChildWindowClip::None => inherited,
                    ChildWindowClip::Content => inherited.positive_intersection(parent.surface.geometry.body).unwrap_or_else(|| {
                        Recti::new(
                            inherited.x.max(parent.surface.geometry.body.x),
                            inherited.y.max(parent.surface.geometry.body.y),
                            0,
                            0,
                        )
                    }),
                }
            }
            SurfaceKey::Popup(_) => {
                // Popups inherit their owning window's surface boundary rather than its child-body
                // boundary. A parent's own menu can therefore overlay its children, while a popup
                // owned by a clipped child remains confined by that child's ancestors.
                let owner = self.surfaces.owning_root(key).expect("visible popup must retain a root owner");
                self.surfaces.root_node(owner).expect("visible popup owner must remain retained").surface.clip
            }
        }
    }

    /// Lays out the reusable parent-first visible traversal and commits inherited surface clips.
    pub(super) fn layout(&mut self, viewport: Recti, atlas: &crate::AtlasHandle) -> Result<(), neocogi_com::NcStatus> {
        self.sync_menu_presentation();
        let style = self.bundle.skin().clone();
        for index in 0..self.surfaces.nodes.len() {
            let key = self.surfaces.nodes[index].key;
            if !self.surfaces.is_displayed(key) {
                self.surfaces.nodes[index].clear_transient_targets();
            }
        }

        // Keys are copied one at a time so mutating geometry never requires cloning the traversal.
        for index in 0..self.surfaces.visible_order.len() {
            let key = self.surfaces.visible_order[index];
            if matches!(key, SurfaceKey::Popup(_)) {
                let rect = self
                    .resolved_popup_rect(key)
                    .expect("active popup anchor node must remain in its retained parent surface");
                self.surfaces.surface_mut(key).expect("visible popup must remain retained").rect = rect;
            }
            let clip = self.resolved_surface_clip(key, viewport);
            let node = self.surfaces.node_mut(key).expect("visible surface must remain retained");
            node.layout(&style, atlas, clip)?;
            if node.take_popup_focus_request() {
                // The popup is now mounted with committed geometry, so its first eligible Tab stop
                // can become the runtime's focused node without a speculative rectangle.
                let _ = node.surface.body.advance_focus(false);
            }
        }
        Ok(())
    }

    /// Routes and applies one normalized event across every eligible surface.
    pub(super) fn update_for_event(&mut self, atlas: &crate::AtlasHandle, event: &UiInputEvent, input: crate::input::InputSnapshot) -> Result<(), neocogi_com::NcStatus> {
        if matches!(event, UiInputEvent::Cancel) {
            // Cancellation has no recipient and must never become a synthetic mouse release.
            // Editors still receive the ordinary update so they can finish their blur policy.
            self.finish_keyboard_menu(true);
            self.pending_menu_alt = false;
            self.window_cycle_key_down = false;
            self.discard_pointer_capture_tail = false;
            for node in &mut self.surfaces.nodes {
                node.clear_transient_targets();
            }
            self.update_eligible_widget_trees(atlas, input)?;
            return Ok(());
        }
        // Resolve event-wide dismissal and menu-toggle context before selecting a recipient. An
        // outside press may change the visible forest and must do so before hit testing below.
        let style = self.bundle.skin().clone();
        let delegated_keyboard = self.delegates_keyboard();
        let (popup_keyboard_handled, menu_keyboard_handled, window_keyboard_handled) = match delegated_keyboard {
            true => {
                // Focus can enter a nested scope between a manager-owned press and
                // its release (for example Ctrl+F6 activates a remembered pane).
                // The inner owner receives later keys, so revoke outer chord tails
                // now rather than swallowing the first command after focus returns.
                self.pending_menu_alt = false;
                self.window_cycle_key_down = false;
                (false, false, false)
            }
            false => {
                let popup = self.route_application_popup_keyboard(event);
                let menu = !popup && self.route_menu_keyboard(event);
                let window = !popup && !menu && self.route_window_keyboard(event);
                (popup, menu, window)
            }
        };
        let discard_pointer = self.discard_revoked_capture_event(event);
        let menu_press = matches!(event, UiInputEvent::MouseDown { button, .. } if button.intersects(MouseButton::LEFT));
        let previous_top = menu_press.then(|| self.surfaces.active_top_popup()).flatten();
        if matches!(event, UiInputEvent::MouseDown { .. }) {
            // Dismiss before target resolution so the same outside press reaches the revealed surface.
            self.dismiss_outside_popup(input.mouse_pos);
            // Any capture revoked by outside dismissal belonged to an older gesture. This fresh
            // press is intentionally routed to the revealed surface and owns its own later tail.
            self.discard_pointer_capture_tail = false;
        }

        // Select and activate the topmost eligible pointer surface from the newly visible order.
        let hover = (event.is_pointer() && !discard_pointer)
            .then(|| self.input_surface_at(input.mouse_pos))
            .flatten();
        // Disabled surfaces remain in hit testing so they occlude lower windows, but they are not
        // routable targets. Keeping both facts prevents click-through without a second stacking
        // model or a special disabled-window event path.
        let interactive_hover = hover.filter(|surface| self.surface_accepts_input(*surface));
        let active_window_before_press = self
            .active_menu_root()
            .or_else(|| self.keyboard_input_surface().and_then(|surface| self.surfaces.owning_root(surface)));
        let pointer_owner_was_active = interactive_hover
            .and_then(|surface| self.surfaces.owning_root(surface))
            .is_some_and(|owner| active_window_before_press == Some(owner));
        if event.is_pointer() {
            // Hover is exclusive paint state even when no button is pressed or the selected chrome
            // region ultimately lets the event fall through to application content. Persistent menu
            // bars need an explicit reset because their root may instead route this event to a widget.
            self.clear_menu_pointer_hover();
            self.update_chrome_hover(interactive_hover, input.mouse_pos);
        }
        if matches!(event, UiInputEvent::MouseDown { .. })
            && self.active_menu_surface().is_some()
            && !hover.is_some_and(|surface| self.menu_contains(surface, input.mouse_pos))
        {
            // The outside press already truncated the popup path above. Release only menu keyboard
            // selection here so the same click can continue into application content.
            self.finish_keyboard_menu(false);
        }
        if matches!(event, UiInputEvent::MouseDown { .. })
            && let Some(surface) = interactive_hover
        {
            self.activate_pointer_surface(surface);
            let owner = self.surfaces.owning_root(surface).expect("visible surface must retain a root ancestor");
            self.bring_window_to_front_id(owner).expect("pointer target owner must remain registered");
            // A fresh gesture preempts every competing pointer capture but never any scope's
            // remembered keyboard focus. The selected focusable body target may replace its own
            // focus during routing below.
            self.clear_other_pointer_captures(surface);
        }

        let drag = (!discard_pointer).then(|| self.drag_input_surface()).flatten();
        let pointer = match event {
            UiInputEvent::MouseDrag { .. } => drag,
            _ => interactive_hover,
        };
        let keyboard = self.keyboard_input_surface();
        let tab_navigation = matches!(
            event,
            UiInputEvent::Key { event }
                if event.key == crate::Key::Tab
                    && match delegated_keyboard {
                        false => !event.modifiers.intersects(crate::Modifiers::ALT | crate::Modifiers::CTRL | crate::Modifiers::SUPER),
                        true => event.modifiers.intersects(crate::Modifiers::CTRL)
                            && !event.modifiers.intersects(crate::Modifiers::ALT | crate::Modifiers::SUPER),
                    }
        );
        let focus_traversal = tab_navigation
            .then(|| match event {
                UiInputEvent::Key { event } if event.is_pressed() => Some(event.modifiers.intersects(crate::Modifiers::SHIFT)),
                _ => None,
            })
            .flatten();
        let modal = self.surfaces.active_modal_root();

        // Stage pointer eligibility in every participating widget runtime before routing one target.
        for index in 0..self.surfaces.visible_order.len() {
            let key = self.surfaces.visible_order[index];
            if self.surface_is_eligible(key, modal) {
                let widget_pointer = self.surface_accepts_input(key) && pointer == Some(key) && !self.menu_contains(key, input.mouse_pos);
                self.surfaces
                    .surface_mut(key)
                    .expect("visible input surface must remain retained")
                    .body
                    .begin_input_event(widget_pointer, event);
            }
        }

        // Captured gestures take priority; otherwise route exactly one pointer hit or keyboard
        // target. Menu policy is staged until its concrete surface borrow has been released.
        let mut staged_menu_action = None;
        if event.is_pointer() && !discard_pointer {
            let captured = matches!(event, UiInputEvent::MouseDrag { .. } | UiInputEvent::MouseUp { .. })
                .then(|| self.captured_input_surface())
                .flatten();
            let mut handled = false;
            if let Some(surface) = captured {
                if self.menu_surface(surface).is_some_and(MenuSurface::has_capture) {
                    let route = self.route_menu_pointer(surface, event).expect("captured menu surface must remain retained");
                    staged_menu_action = route.action.map(|action| (surface, action));
                    handled = route.handled;
                } else {
                    handled = match surface {
                        SurfaceKey::Root(root) => {
                            // Chrome receives continuation only when it owns the window's capture. A
                            // captured application widget must keep drag and release even over chrome.
                            let chrome_captured = self
                                .root_node(root)
                                .ok()
                                .and_then(SurfaceNode::root)
                                .is_some_and(|state| state.interaction != RootInteraction::None);
                            if chrome_captured {
                                // Continuation belongs to an already-visible caption or another
                                // chrome capture, so activation-time first-press filtering no longer applies.
                                self.route_chrome_event(root, event, true, style.window_chrome().minimized_content_height)
                            } else {
                                self.surfaces
                                    .surface_mut(surface)
                                    .expect("captured window must remain retained")
                                    .body
                                    .route_captured_pointer(&style, input.mouse_buttons, event)
                                    .is_some()
                            }
                        }
                        SurfaceKey::Popup(_) => self
                            .surfaces
                            .surface_mut(surface)
                            .expect("captured popup must remain retained")
                            .body
                            .route_captured_pointer(&style, input.mouse_buttons, event)
                            .is_some(),
                    };
                }
            }
            if !handled && let Some(surface) = pointer {
                handled = match surface {
                    SurfaceKey::Root(root) => {
                        let caption_controls_visible = pointer_owner_was_active || style.window_chrome().show_without_activation == 1;
                        self.route_chrome_event(root, event, caption_controls_visible, style.window_chrome().minimized_content_height)
                    }
                    SurfaceKey::Popup(_) => false,
                };
                if !handled && self.menu_contains(surface, input.mouse_pos) {
                    let route = self.route_menu_pointer(surface, event).expect("hit menu surface must remain retained");
                    staged_menu_action = route.action.map(|action| (surface, action));
                    handled = route.handled;
                }
                if !handled {
                    let body = &mut self.surfaces.surface_mut(surface).expect("pointer surface must remain retained").body;
                    if body.accepts_pointer_input() {
                        let _ = body.route_pointer(&style, event, input.mouse_buttons);
                    }
                }
            }
        } else if popup_keyboard_handled || menu_keyboard_handled || window_keyboard_handled {
            // Manager keyboard routing mutated only scope and compact-surface state. The ordinary
            // full-tree update below still lets suspended application widgets observe stable focus.
        } else if tab_navigation {
            // Both transitions belong to the scope command, so neither Tab press nor release leaks
            // into a newly focused widget. Only the press advances; a held key may still advance
            // through ordinary repeated press events supplied by the platform.
            if let (Some(surface), Some(reverse)) = (keyboard, focus_traversal) {
                self.surfaces
                    .surface_mut(surface)
                    .expect("keyboard surface must remain retained")
                    .body
                    .advance_focus(reverse);
            }
        } else if event.is_focus_input()
            && let Some(surface) = keyboard
        {
            self.surfaces
                .surface_mut(surface)
                .expect("keyboard surface must remain retained")
                .body
                .route_focus(&style, event);
        }

        // Reconcile transient state for surfaces excluded by the routed event, then update every
        // eligible body in the shared visible order using the same immutable input snapshot.
        let modal = self.surfaces.active_modal_root();
        for index in 0..self.surfaces.nodes.len() {
            let key = self.surfaces.nodes[index].key;
            let visible = self.surfaces.is_displayed(key);
            if !visible {
                self.surfaces.nodes[index].clear_transient_targets();
            } else if !self.surface_is_eligible(key, modal) || !self.surface_accepts_input(key) {
                // A modal scope suspends underlying keyboard focus rather than destroying it. A
                // disabled surface follows the same retained-focus policy. Pointer state cannot
                // survive either exclusion and is revoked independently.
                self.surfaces.nodes[index].clear_pointer_targets();
            }
        }
        self.update_eligible_widget_trees(atlas, input)?;
        if let Some((surface, action)) = staged_menu_action {
            // Direct routing has ended its concrete surface borrow before forest visibility changes.
            self.apply_menu_action(surface, action, previous_top);
        }
        Ok(())
    }

    /// Records one ordinary root family with each parent's application content below its children.
    pub(super) fn paint_root_tree(
        &mut self,
        root: RootId,
        style: &Skin,
        atlas: &crate::AtlasHandle,
        focus_surface: Option<SurfaceKey>,
        active_window: Option<RootId>,
    ) -> Result<(), neocogi_com::NcStatus> {
        {
            // A short node borrow records the base before recursion. Releasing it here lets direct
            // child calls borrow arbitrary later forest entries without unsafe aliasing or mirrors.
            let window_enabled = self.surfaces.root_is_effectively_enabled(root);
            let node_index = self.surfaces.node_index(SurfaceKey::Root(root)).expect("visible root must remain retained");
            let node = &mut self.surfaces.nodes[node_index];
            let window_active = window_enabled && active_window == Some(root);
            let frame_kind = if node.root().is_some_and(|state| state.mode == RootMode::Modal) {
                RootFrameKind::Dialog
            } else {
                RootFrameKind::Window
            };
            record_root_background(&mut self.display_list, node.surface.clip, node.surface.rect, style, frame_kind, window_enabled)?;
            node.surface.body.paint(
                &mut self.display_list,
                style,
                atlas,
                window_enabled && focus_surface == Some(SurfaceKey::Root(root)),
                window_active,
                window_enabled,
            )?;
        }

        // Derived adjacency retains sibling chronology while visiting only this family's edges.
        let key = SurfaceKey::Root(root);
        let count = self.surfaces.index[&key].children.len();
        for index in 0..count {
            let child = self.surfaces.index[&key].children[index];
            if let SurfaceKey::Root(child_root) = child
                && self
                    .surfaces
                    .node(child)
                    .is_some_and(|node| node.root().is_some_and(|state| state.mode == RootMode::Child && state.visible))
            {
                self.paint_root_tree(child_root, style, atlas, focus_surface, active_window)?;
            }
        }

        {
            // Parent-owned presentation is deliberately delayed until every child body and child
            // overlay has recorded. The inverse input recursion below uses the same relationship.
            let window_enabled = self.surfaces.root_is_effectively_enabled(root);
            let node_index = self.surfaces.node_index(SurfaceKey::Root(root)).expect("visible root must remain retained");
            let node = &mut self.surfaces.nodes[node_index];
            if let Some(root_state) = node.root_mut()
                && let Some(bar) = root_state.menu_bar.as_mut()
            {
                bar.paint(
                    &mut self.display_list,
                    style,
                    atlas,
                    window_enabled && active_window == Some(root),
                    window_enabled,
                )?;
            }
            let root_state = node.root().expect("root overlay must retain root policy");
            let frame_kind = if root_state.mode == RootMode::Modal {
                RootFrameKind::Dialog
            } else {
                RootFrameKind::Window
            };
            let visual = root_state.chrome_visual_state();
            record_root_overlay(
                &mut self.display_list,
                node.surface.clip,
                node.surface.rect,
                node.surface.options,
                &node.surface.name,
                node.surface.geometry,
                style,
                atlas,
                frame_kind,
                window_enabled && active_window == Some(root),
                window_enabled,
                visual,
            )?;
        }
        Ok(())
    }

    /// Records the sole active popup path in parent-to-child order at its inherited transient tier.
    pub(super) fn paint_active_popup_path(&mut self, style: &Skin, atlas: &crate::AtlasHandle, focus_surface: Option<SurfaceKey>) -> Result<(), neocogi_com::NcStatus> {
        // `visible_order` materializes only the active popup ancestry. Filtering it reuses that
        // allocation and preserves the existing parent-first popup paint contract.
        for index in 0..self.surfaces.visible_order.len() {
            let key = self.surfaces.visible_order[index];
            if !matches!(key, SurfaceKey::Popup(_)) {
                continue;
            }
            let popup_enabled = self.surface_accepts_input(key);
            let node_index = self.surfaces.node_index(key).expect("active popup must remain retained");
            let node = &mut self.surfaces.nodes[node_index];
            let widget_popup = matches!(&node.surface.body, SurfaceBody::Widgets { .. });
            // Transient widget popups and compact menu popups share the semantic popup panel
            // center. Compact menus paint their own complete panel inside the body; application
            // popups receive the matching border overlay after their retained content below.
            record_root_background(
                &mut self.display_list,
                node.surface.clip,
                node.surface.rect,
                style,
                RootFrameKind::Popup,
                popup_enabled,
            )?;
            node.surface.body.paint(
                &mut self.display_list,
                style,
                atlas,
                popup_enabled && focus_surface == Some(key),
                true,
                popup_enabled,
            )?;
            if widget_popup {
                // Record border cells last so application content cannot cover the popup's black
                // outline. Popup options exclude title and resizing, making the shared recorder a
                // single semantic frame pass with no hidden window behavior.
                record_root_overlay(
                    &mut self.display_list,
                    node.surface.clip,
                    node.surface.rect,
                    node.surface.options,
                    &node.surface.name,
                    node.surface.geometry,
                    style,
                    atlas,
                    RootFrameKind::Popup,
                    false,
                    popup_enabled,
                    RootChromeVisualState::idle(),
                )?;
            }
        }
        Ok(())
    }

    /// Paints fixed window families, their transient tier, and then the manager-owned modal tier.
    pub(crate) fn paint(&mut self, _dimensions: Dimensioni) -> Result<(), neocogi_com::NcStatus> {
        // Frame validation has already matched `_dimensions` to the latest UI commit. Painting uses
        // the committed per-surface clips from that transaction so no traversal can accidentally
        // widen a structurally clipped child back to the full drawable viewport.
        // Resolve both projections from one bundle before mutating display-list and surface state.
        let atlas = self.bundle.atlas().clone();
        self.display_list.clear();
        let style = self.bundle.skin().clone();
        let active_mode = self.active_popup_owner().and_then(|owner| self.surfaces.stacking_mode(owner));
        // A menu owns keyboard presentation without discarding application focus. Otherwise the
        // same surface selected by routing is the only runtime allowed to paint remembered focus.
        let menu_root = self.active_menu_root();
        let focus_surface = if menu_root.is_some() { None } else { self.keyboard_input_surface() };
        // Window activation follows the current modal/menu/keyboard scope and is independent from
        // fixed-layer stacking. Popup focus projects to its sole owning window for chrome paint.
        let active_window = menu_root.or_else(|| focus_surface.and_then(|surface| self.surfaces.owning_root(surface)));

        // Only parentless Normal roots enter global layer scans. Each recursive call paints one
        // structurally atomic family while still sandwiching children between parent content and
        // parent-owned overlays.
        for layer in MIN_LAYER..=MAX_LAYER {
            let node_count = self.surfaces.nodes.len();
            for index in 0..node_count {
                let root = {
                    let node = &self.surfaces.nodes[index];
                    match (node.key, node.root()) {
                        (SurfaceKey::Root(root), Some(state))
                            if state.mode == (RootMode::Normal { layer }) && self.surfaces.root_is_effectively_visible(root) =>
                        {
                            Some(root)
                        }
                        _ => None,
                    }
                };
                if let Some(root) = root {
                    self.paint_root_tree(root, &style, &atlas, focus_surface, active_window)?;
                }
            }
            if active_mode == Some(RootMode::Normal { layer }) {
                self.paint_active_popup_path(&style, &atlas, focus_surface)?;
            }
        }

        let node_count = self.surfaces.nodes.len();
        for index in 0..node_count {
            let modal = {
                let node = &self.surfaces.nodes[index];
                match (node.key, node.root()) {
                    (SurfaceKey::Root(root), Some(state)) if state.mode == RootMode::Modal && self.surfaces.root_is_effectively_visible(root) => Some(root),
                    _ => None,
                }
            };
            if let Some(modal) = modal {
                self.paint_root_tree(modal, &style, &atlas, focus_surface, active_window)?;
            }
        }
        if active_mode == Some(RootMode::Modal) {
            self.paint_active_popup_path(&style, &atlas, focus_surface)?;
        }
        Ok(())
    }

    /// Truncates an active popup branch according to one outside press.
    pub(super) fn dismiss_outside_popup(&mut self, mouse: Vec2i) {
        if self.surfaces.active_popup.is_none() {
            return;
        }
        // Use the same front-surface resolution as pointer routing. Raw popup rectangles may be
        // occluded by a window in a higher layer and therefore cannot alone keep a popup open.
        let keep = match self.input_surface_at(mouse) {
            Some(SurfaceKey::Popup(popup)) if self.surfaces.popup_is_active(popup) => {
                self.surfaces.popup_depth(popup).map_or(0, |depth| depth.saturating_add(1))
            }
            _ => 0,
        };
        self.truncate_active_popup_path(keep);
    }

    /// Records the exact keyboard surface selected by one pointer press.
    pub(super) fn activate_pointer_surface(&mut self, surface: SurfaceKey) {
        if self.surfaces.node(surface).is_some() && self.surface_accepts_input(surface) {
            // A popup is not collapsed to its owner: its independent widget runtime must receive
            // subsequent Tab, key, and text transitions until focus leaves that surface.
            self.active_surface = Some(surface);
        }
    }

    /// Returns whether one retained window is visible.
    pub(super) fn root_is_visible(&self, root: RootId) -> bool {
        // Keyboard activation and owner eligibility require the complete structural ancestry, not
        // merely the local visibility bit retained for restoration after a parent is shown again.
        self.surfaces.root_is_effectively_visible(root)
    }

    /// Tests the active popup path front-to-back and returns its first clipped surface hit.
    pub(super) fn input_popup_at(&self, point: Vec2i) -> Option<SurfaceKey> {
        // Popup path order is parent-first for paint, so reverse traversal tests the deepest submenu
        // first. The sole active branch makes any additional ownership filtering unnecessary.
        self.surfaces
            .visible_order
            .iter()
            .rev()
            .copied()
            .filter(|key| matches!(key, SurfaceKey::Popup(_)))
            .find(|key| self.surfaces.surface(*key).is_some_and(|surface| surface.contains(point)))
    }

    /// Resolves one ordinary root family using the inverse of recursive paint order.
    pub(super) fn input_root_tree_at(&self, root: RootId, point: Vec2i) -> Option<SurfaceKey> {
        let node = self.surfaces.root_node(root)?;
        if node.overlay_contains(point) {
            // Parent menus and actionable chrome paint after the complete descendant family and
            // therefore receive uncaptured input before any overlapping child surface.
            return Some(SurfaceKey::Root(root));
        }

        // Later sibling records paint in front. Recurse before testing the parent base so children
        // sit above application content while remaining below parent-owned overlays.
        for &key in self.surfaces.index[&SurfaceKey::Root(root)].children.iter().rev() {
            let child_node = self.surfaces.node(key).expect("indexed child must remain retained");
            if let (SurfaceKey::Root(child), Some(state)) = (key, child_node.root())
                && state.mode == RootMode::Child
                && state.visible
                && let Some(target) = self.input_root_tree_at(child, point)
            {
                return Some(target);
            }
        }

        node.surface.contains(point).then_some(SurfaceKey::Root(root))
    }

    /// Returns the front eligible surface containing one pointer point.
    pub(super) fn input_surface_at(&self, point: Vec2i) -> Option<SurfaceKey> {
        if let Some(modal) = self.surfaces.active_modal_root() {
            // The active modal and its popup path remain the sole eligible input group. Popup
            // surfaces occupy the transient tier above the modal root itself.
            if self.active_popup_owner() == Some(modal)
                && let Some(target) = self.input_popup_at(point)
            {
                return Some(target);
            }
            return self.input_root_tree_at(modal, point);
        }

        let active_owner = self.active_popup_owner();
        let active_mode = active_owner.and_then(|owner| self.surfaces.stacking_mode(owner));
        for layer in (MIN_LAYER..=MAX_LAYER).rev() {
            // A fixed band's popup path remains above every ordinary family in that same band but
            // below all higher fixed bands, preserving the established transient-tier policy.
            if active_mode == Some(RootMode::Normal { layer })
                && let Some(target) = self.input_popup_at(point)
            {
                return Some(target);
            }
            for node in self.surfaces.nodes.iter().rev() {
                let (SurfaceKey::Root(root), Some(state)) = (node.key, node.root()) else {
                    continue;
                };
                if state.mode == (RootMode::Normal { layer })
                    && self.surfaces.root_is_effectively_visible(root)
                    && let Some(target) = self.input_root_tree_at(root, point)
                {
                    return Some(target);
                }
            }
        }
        None
    }

    /// Returns the front visible surface in the current modal group.
    pub(super) fn front_input_surface(&self) -> Option<SurfaceKey> {
        let modal = self.surfaces.active_modal_root();
        self.surfaces
            .visible_order
            .iter()
            .rev()
            .copied()
            .find(|key| self.surface_is_eligible(*key, modal) && self.surface_accepts_input(*key))
    }

    /// Returns the eligible surface that currently owns pointer capture.
    pub(super) fn captured_input_surface(&self) -> Option<SurfaceKey> {
        let modal = self.surfaces.active_modal_root();
        self.surfaces.visible_order.iter().rev().copied().find(|key| {
            self.surface_is_eligible(*key, modal) && self.surface_accepts_input(*key) && self.surfaces.node(*key).is_some_and(SurfaceNode::owns_pointer_gesture)
        })
    }

    /// Returns the surface receiving pointer-drag continuation.
    pub(super) fn drag_input_surface(&self) -> Option<SurfaceKey> {
        self.captured_input_surface().or_else(|| self.front_input_surface())
    }

    /// Returns the sole surface receiving keyboard and text input.
    pub(super) fn keyboard_input_surface(&self) -> Option<SurfaceKey> {
        let modal = self.surfaces.active_modal_root();
        self.active_surface
            .filter(|surface| self.surfaces.is_displayed(*surface) && self.surface_is_eligible(*surface, modal) && self.surface_accepts_input(*surface))
            .or_else(|| self.captured_input_surface())
            .or_else(|| modal.map(SurfaceKey::Root).filter(|surface| self.surface_accepts_input(*surface)))
            .or_else(|| self.front_input_surface())
    }

    /// Resolves native text input from committed editable focus, excluding active menu commands.
    pub(crate) fn text_input_area(&self) -> Option<Recti> {
        self.ui_commit?;
        if self.active_menu_root().is_some() {
            return None;
        }
        let surface = self.surfaces.surface(self.keyboard_input_surface()?)?;
        match &surface.body {
            SurfaceBody::Widgets { root, runtime } => runtime.text_input_area(root, self.bundle.skin()),
            SurfaceBody::Menu(_) => None,
        }
    }

    /// Delegates scope commands only when no outer menu or modal owns the focused surface.
    pub(super) fn delegates_keyboard(&self) -> bool {
        if self.active_menu_root().is_some() {
            return false;
        }
        let Some(surface) = self.keyboard_input_surface().and_then(|key| self.surfaces.surface(key)) else {
            return false;
        };
        match &surface.body {
            SurfaceBody::Widgets { root, runtime } => runtime.delegates_keyboard(root),
            SurfaceBody::Menu(_) => false,
        }
    }

    /// Returns the popup options installed for new definitions.
    pub(super) const fn default_popup_options() -> WindowOption {
        WindowOption::FRAME
            .union(WindowOption::AUTO_SIZE)
            .union(WindowOption::NO_RESIZE)
            .union(WindowOption::NO_TITLE)
    }
}
