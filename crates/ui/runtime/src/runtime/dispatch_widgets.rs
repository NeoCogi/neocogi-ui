//! Weak dispatch selection is separate from handler execution and its errors.
use crate::window_manager::WindowManager;
use neocogi_com::{NcStatus, WeakComBox};
use neocogi_ui_core_contracts::IWidgetDispatcherAbi;

/// Reusable descendant-first snapshot checked out for one exclusive UI update.
/// Entries own only weak tokens; no snapshot can prolong a removed widget's value.
#[derive(Default)]
pub(super) struct DispatchWidgets {
    /// Cached projections in tree order; capacity survives clearing between updates.
    widgets: Vec<WeakComBox<IWidgetDispatcherAbi>>,
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
    pub(super) fn iter(&self) -> impl Iterator<Item = WeakComBox<IWidgetDispatcherAbi>> + '_ {
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

    /// Concrete private behavior makes this a capability-liveness test, rather than a passive control test.
    struct Probe;
    impl neocogi_ui::LeafContent for Probe {
        fn measure(&self, _: neocogi_ui::LayoutLimits, _: neocogi_com::ComRef<'_, neocogi_ui::ITextMetricsAbi>) -> Result<neocogi_ui::Size, NcStatus> {
            Ok(neocogi_ui::Size { width: 10, height: 10 })
        }
        fn paint(&mut self, _: neocogi_ui::WidgetPaint, _: &mut neocogi_ui::Painter<'_>) -> Result<(), NcStatus> {
            Ok(())
        }
    }
    impl neocogi_ui::DispatchingLeafContent for Probe {
        fn dispatch_next(&mut self, _: &mut neocogi_ui::LeafDispatchContext<'_>) -> Result<bool, NcStatus> {
            Ok(false)
        }
    }
    /// Expiry before or during a pass is skipped without disturbing live order.
    /// Yielded entries remain weak even after the scratch buffer is cleared.
    #[test]
    fn iterator_checks_each_receiver_and_preserves_only_buffer_capacity() {
        let runtime = crate::test_support::create_runtime(crate::test_support::test_atlas());
        let ui = nc_ui_nodes::bind_builtin_nodes(runtime).unwrap();
        let before = ui.widgets().dispatching_leaf(Probe, neocogi_ui::WidgetOptions::NONE).unwrap();
        let first = ui.widgets().dispatching_leaf(Probe, neocogi_ui::WidgetOptions::NONE).unwrap();
        let during = ui.widgets().dispatching_leaf(Probe, neocogi_ui::WidgetOptions::NONE).unwrap();
        let last = ui.widgets().dispatching_leaf(Probe, neocogi_ui::WidgetOptions::NONE).unwrap();
        let mut snapshot = DispatchWidgets {
            widgets: vec![
                before.node_interface().unwrap().query::<IWidgetDispatcherAbi>().unwrap(),
                first.node_interface().unwrap().query::<IWidgetDispatcherAbi>().unwrap(),
                during.node_interface().unwrap().query::<IWidgetDispatcherAbi>().unwrap(),
                last.node_interface().unwrap().query::<IWidgetDispatcherAbi>().unwrap(),
            ],
        };
        let capacity = snapshot.widgets.capacity();
        drop(before);
        let mut iter = snapshot.iter();
        let mut selected = iter.next().unwrap();
        assert!(selected.same_object(&first.node_interface().unwrap().query::<IWidgetDispatcherAbi>().unwrap()));
        drop(during);
        assert!(
            iter.next()
                .unwrap()
                .same_object(&last.node_interface().unwrap().query::<IWidgetDispatcherAbi>().unwrap())
        );
        assert!(iter.next().is_none());
        drop(iter);
        snapshot.clear();
        assert_eq!(snapshot.widgets.capacity(), capacity);
        assert!(snapshot.iter().next().is_none());
        drop(first);
        assert_eq!(selected.dispatch_next(), Err(neocogi_com::NC_E_DROPPED));
    }
    #[test]
    fn review_callback_shutdown_does_not_restore_expired_cache_entries() {
        use neocogi_com::{ComPtr, NcCopyFromBufferOffset, NcValueRef};
        use neocogi_ui_core_contracts::{IWindowManagerAbi, Rect, Size, UiRuntimeAbi, UI_E_CLOSED};
        let bundle = crate::test_support::assets::export_bundle(&crate::SkinBundle::from_atlas(crate::test_support::test_atlas())).unwrap();
        let assets = crate::assets::ImportedAssets::read(bundle.as_com_ref(), None).unwrap();
        let gfx = neocogi_ui_support::test_support::gfx_factory(crate::test_support::host_runtime());
        let compositor = gfx.create_compositor(bundle.as_com_ref()).unwrap();
        let storage = crate::runtime::create_storage(assets, compositor);
        let runtime = crate::runtime::Runtime { storage: storage.clone() };
        let runtime: ComPtr<UiRuntimeAbi> = crate::runtime::Runtime::from_value(runtime).query().unwrap();
        let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
        let manager: ComPtr<IWindowManagerAbi> = runtime.query().unwrap();
        let button = ui.widgets().dispatching_leaf(Probe, neocogi_ui::WidgetOptions::NONE).unwrap();
        let weak = button.node_interface().unwrap();
        let _window = manager
            .create_window(
                NcCopyFromBufferOffset::new(0, b"window"),
                Rect { x: 0, y: 0, width: 300, height: 200 },
                button.into_interface().query().unwrap(),
                NcValueRef::new(&neocogi_ui_core_contracts::EventBinding::Unobserved).unwrap(),
            )
            .unwrap();
        let (send, messages) = std::sync::mpsc::channel();
        let app = neocogi_ui_support::test_support::events::TestApp::new(runtime.clone(), messages, |runtime, ()| runtime.close());
        send.send(()).unwrap();
        assert_eq!(app.update(&ui, Size { width: 640, height: 400 }), Err(UI_E_CLOSED));
        assert!(!weak.is_alive(), "widget value was destroyed synchronously");
        drop(weak);
        assert!(
            storage.dispatch_widgets.borrow().widgets.is_empty(),
            "closed runtime must not reacquire the expired scheduler weak owners"
        );
    }
}
