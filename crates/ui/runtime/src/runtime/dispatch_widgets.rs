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

    /// Verifies that dispatch widget snapshots are cached across frames and that
    /// dispatch_order_changed transitions to false after update, remaining false
    /// when topology does not change, and becoming true when nodes are mounted or unmounted.
    #[test]
    fn dispatch_order_cached_until_topology_changes() {
        use neocogi_ui_core_contracts::{IWindowManagerAbi, IWindowEventBinding, Rect, Size, UiRuntimeAbi};
        use neocogi_com::{ComPtr, NcCopyFromBufferOffset, NcValueRef};

        let bundle = crate::test_support::assets::export_bundle(&crate::SkinBundle::from_atlas(crate::test_support::test_atlas())).unwrap();
        let assets = crate::assets::ImportedAssets::read(bundle.as_com_ref(), None).unwrap();
        let gfx = neocogi_ui_support::test_support::gfx_factory();
        let compositor = gfx.create_compositor(bundle.as_com_ref()).unwrap();
        let storage = crate::runtime::create_storage(assets, compositor);
        let runtime = crate::runtime::Runtime { storage: storage.clone() };
        let runtime_com: ComPtr<UiRuntimeAbi> = crate::runtime::Runtime::from_value(runtime).query().unwrap();

        // 1. Initially, dispatch_order_changed starts true (empty cache needs initial build).
        assert!(storage.factory.state.dispatch_order_changed.get());

        // 2. Mount a button into a window.
        let ui = nc_ui_nodes::bind_builtin_nodes(runtime_com.clone()).unwrap();
        let button = ui.widgets().button_unobserved("test button").unwrap();
        let manager: ComPtr<IWindowManagerAbi> = runtime_com.query().unwrap();
        let window = manager.create_window(
            NcCopyFromBufferOffset::new(0, b"Window"),
            Rect { x: 0, y: 0, width: 300, height: 200 },
            button.into_interface().query().unwrap(),
            NcValueRef::new(&IWindowEventBinding::Unobserved).unwrap(),
        ).unwrap();

        // Mounting the node set dispatch_order_changed to true.
        assert!(storage.factory.state.dispatch_order_changed.get());

        // 3. First update cycle rebuilds the dispatch widget snapshot and resets the flag.
        runtime_com.update(Size { width: 640, height: 400 }).unwrap();
        assert!(!storage.factory.state.dispatch_order_changed.get(), "dispatch_order_changed must be false after update");

        // 4. Second update cycle without topology changes keeps the cached snapshot!
        runtime_com.update(Size { width: 640, height: 400 }).unwrap();
        assert!(!storage.factory.state.dispatch_order_changed.get(), "dispatch_order_changed must remain false across idle frames");

        // 5. Closing the window unmounts the node, which must mark dispatch_order_changed true!
        window.close().unwrap();
        assert!(storage.factory.state.dispatch_order_changed.get(), "closing window must mark dispatch_order_changed true");

        // 6. Next update rebuilds and resets the flag again.
        runtime_com.update(Size { width: 640, height: 400 }).unwrap();
        assert!(!storage.factory.state.dispatch_order_changed.get());
    }
}

