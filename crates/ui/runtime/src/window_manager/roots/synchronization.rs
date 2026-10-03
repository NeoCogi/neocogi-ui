//! Forest geometry synchronization and public copied window-policy projection.
use super::*;

impl WindowManager {
    /// Collects sticky widget changes even in hidden surfaces before a cache can be reused.
    /// Changes visit every tree; failures stop immediately and preserve retry invalidation.
    pub(crate) fn collect_widget_changes(&mut self) -> Result<bool, neocogi_com::NcStatus> {
        let mut changed = false;
        for node in &mut self.surfaces.nodes {
            if let SurfaceBody::Widgets { root, .. } = &mut node.surface.body {
                changed |= root.synchronize_measurement_invalidation()?;
            }
        }
        Ok(changed)
    }

    /// Copies current window policy into the canonical COM snapshot without a second state store.
    pub(crate) fn window_snapshot(&self, window: &WindowHandle) -> Result<neocogi_ui_core_contracts::WindowSnapshot, SurfaceMutationError> {
        use neocogi_ui_core_contracts::{ChildClip, LayerKind, Rect, Visibility, WindowLayer, WindowOptions, WindowSnapshot};
        let id = self.window_id(window)?;
        let node = self.root_node(id)?;
        let root = node.root().expect("validated window has root policy");
        let rect = node.surface.rect;
        let layer = match self.window_layer(window)? {
            super::LayerBinding::Fixed(number) => WindowLayer { kind: LayerKind::Fixed, number },
            super::LayerBinding::Modal => WindowLayer { kind: LayerKind::Modal, number: 0 },
        };
        Ok(WindowSnapshot {
            rect: Rect {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
            },
            options: WindowOptions::from_raw(node.surface.options.bits() as u16),
            layer,
            visibility: if self.root_is_visible(id) { Visibility::Visible } else { Visibility::Hidden },
            child_clip: match root.child_window_clip {
                super::ChildWindowClip::None => ChildClip::None,
                super::ChildWindowClip::Content => ChildClip::Content,
            },
        })
    }

    /// Borrows the title only within the implementation's exclusive context transaction.
    pub(crate) fn window_title(&self, window: &WindowHandle) -> Result<&str, SurfaceMutationError> {
        let id = self.window_id(window)?;
        Ok(&self.root_node(id)?.surface.name)
    }

    /// Changes one parent-owned child clip and invalidates the existing retained layout commit.
    pub(crate) fn set_child_window_clip(&mut self, window: &WindowHandle, clip: super::ChildWindowClip) -> Result<(), SurfaceMutationError> {
        let id = self.window_id(window)?;
        self.root_node_mut(id)?.root_mut().expect("validated window has root policy").child_window_clip = clip;
        self.invalidate_ui_commit();
        Ok(())
    }

    /// Queries active popup policy after authenticating a live popup definition.
    pub(crate) fn popup_visible(&self, popup: &PopupHandle) -> Result<bool, SurfaceMutationError> {
        self.popup_id(popup).map(|id| self.surfaces.popup_is_active(id))
    }

    /// Permanently removes one application popup and its descendant popup definitions.
    pub(crate) fn destroy_popup(&mut self, popup: &PopupHandle) -> Result<(), SurfaceMutationError> {
        let id = self.popup_id(popup)?;
        self.surfaces.ensure_subtree_idle(SurfaceKey::Popup(id))?;
        self.hide_popup(popup)?;
        let mut removed = vec![SurfaceKey::Popup(id)];
        let mut index = 0;
        while index < removed.len() {
            removed.extend_from_slice(&self.surfaces.index[&removed[index]].children);
            index += 1;
        }
        let removed: HashSet<_> = removed.into_iter().collect();
        self.surfaces.nodes.retain(|node| !removed.contains(&node.key));
        self.surfaces.rebuild_index();
        self.surfaces.rebuild_visible_order();
        self.reconcile_active_surface();
        self.invalidate_ui_commit();
        Ok(())
    }
}
