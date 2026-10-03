//! Weak dispatch selection is separate from handler execution and its errors.
use crate::window_manager::WindowManager;
use neocogi_com::{NcStatus, WeakComBox};
use neocogi_ui_core_contracts::IWidgetAbi;

/// Reusable descendant-first snapshot checked out for one exclusive UI update.
/// Entries own only weak tokens; no snapshot can prolong a removed widget's value.
#[derive(Default)]
pub(super) struct DispatchWidgets {
    /// Cached projections in tree order; capacity survives clearing between updates.
    widgets: Vec<WeakComBox<IWidgetAbi>>,
}
impl DispatchWidgets {
    /// Collects the current authoritative tree without invoking widget handlers.
    pub(super) fn rebuild(&mut self, manager: &WindowManager) -> Result<(), NcStatus> {
        self.widgets.clear();
        manager.collect_dispatch_widgets(&mut self.widgets)
    }

    /// Checks liveness lazily on each next(), including after earlier callbacks.
    /// Returned tokens borrow no widget implementation and do not keep it alive.
    /// This iterator is used by the thread-local UI; generated COM entry points
    /// still perform their own lifetime and borrow admission before value access.
    pub(super) fn iter(&self) -> impl Iterator<Item = WeakComBox<IWidgetAbi>> + '_ {
        self.widgets.iter().filter(|widget| widget.is_alive()).cloned()
    }

    /// Releases every observer without discarding the reusable buffer allocation.
    pub(super) fn clear(&mut self) {
        self.widgets.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Expiry before or during a pass is skipped without disturbing live order.
    /// Yielded entries remain weak even after the scratch buffer is cleared.
    #[test]
    fn iterator_checks_each_receiver_and_preserves_only_buffer_capacity() {
        let runtime = crate::test_support::create_runtime(crate::test_support::test_atlas());
        let ui = nc_ui_nodes::bind_builtin_nodes(runtime).unwrap();
        let before = ui.widgets().button_unobserved("expired before iteration").unwrap();
        let first = ui.widgets().button_unobserved("first").unwrap();
        let during = ui.widgets().button_unobserved("expired during iteration").unwrap();
        let last = ui.widgets().button_unobserved("last").unwrap();
        let mut snapshot = DispatchWidgets {
            widgets: vec![
                before.node_interface().unwrap(),
                first.node_interface().unwrap(),
                during.node_interface().unwrap(),
                last.node_interface().unwrap(),
            ],
        };
        let capacity = snapshot.widgets.capacity();
        drop(before);
        let mut iter = snapshot.iter();
        let mut selected = iter.next().unwrap();
        assert!(selected.same_object(&first.node_interface().unwrap()));
        drop(during);
        assert!(iter.next().unwrap().same_object(&last.node_interface().unwrap()));
        assert!(iter.next().is_none());
        drop(iter);
        snapshot.clear();
        assert_eq!(snapshot.widgets.capacity(), capacity);
        assert!(snapshot.iter().next().is_none());
        drop(first);
        assert_eq!(selected.dispatch_next(), Err(neocogi_com::NC_E_DROPPED));
    }
}
