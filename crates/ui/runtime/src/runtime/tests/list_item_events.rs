//! List-item capture respects delivery policy at the real COM update boundary.
use super::*;
use neocogi_ui::IActivatedEventAbi;

/// Supplies valid local geometry and focus for either a click or keyboard activation.
fn state(clicked: bool) -> WidgetUpdate {
    let rect = Rect { x: 0, y: 0, width: 100, height: 20 };
    WidgetUpdate {
        in_hover_root: true,
        rect,
        clip: rect,
        screen: rect,
        hovered: true,
        focused: true,
        clicked,
        active: false,
        action: WidgetAction::Activate,
        mouse_buttons: 0,
        modifiers: 0,
    }
}

/// Exercises all three inputs that converge on the row's semantic activation branch.
fn activations() -> [WidgetEvent; 3] {
    let key = |key| {
        WidgetEvent::Key(KeyStroke {
            key,
            value: 0,
            pressed: true,
            repeat: false,
            modifiers: 0,
        })
    };
    [WidgetEvent::None, key(KeyCode::Enter), key(KeyCode::Space)]
}

/// Calls the generated widget ABI; eventless input represents an eligible pointer click.
fn activate(driver: &crate::runtime::testing::OriginDriver, widget: &mut WeakComBox<IWidgetAbi>, input: WidgetEvent) -> Result<GeometryChange, NcStatus> {
    let origin = neocogi_com::events::EventOrigin::Widget(widget.clone().into_unknown());
    driver.with_origin(origin, || widget.update(state(matches!(input, WidgetEvent::None)), input, NcCopyFromBufferOffset::new(0, b"")))?

}

/// Direct unobserved posting allocates one marker per occurrence, independent of label size.
#[test]
fn unobserved_list_item_activations_allocate_only_their_marker() {
    let source = crate::test_support::assets::export_bundle(&crate::SkinBundle::from_atlas(crate::test_support::test_atlas())).unwrap();
    let gfx = neocogi_ui_support::test_support::gfx_factory();
    let (runtime, driver) = crate::runtime::testing::create_runtime(source.as_com_ref(), gfx.as_com_ref()).unwrap();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime).unwrap();
    for label in ["", "a", "Unicode row: αβγ 🦀"] {
        let item = ui.widgets().list_item_unobserved(label).unwrap();
        let mut widget = item.as_interface().query_weak::<IWidgetAbi>().unwrap();
        // Resource import and each activation path are warmed outside observation.
        for input in activations() {
            activate(&driver, &mut widget, input).unwrap();
        }
        let measurement = crate::test_support::AllocationMeasurement::begin();
        for _ in 0..100 {
            for input in activations() {
                activate(&driver, &mut widget, input).unwrap();
            }
        }
        let count = measurement.finish();
        assert_eq!(count.events, 300, "one marker per activation, label {label:?}: {count:?}");
    }
}

/// Marker events retain exact weak identity independently of display edits and source destruction.
#[test]
fn observed_list_item_keeps_activation_identity_after_label_edits_and_source_drop() {
    let source = crate::test_support::assets::export_bundle(&crate::SkinBundle::from_atlas(crate::test_support::test_atlas())).unwrap();
    let gfx = neocogi_ui_support::test_support::gfx_factory();
    let (runtime, driver) = crate::runtime::testing::create_runtime(source.as_com_ref(), gfx.as_com_ref()).unwrap();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime).unwrap();
    let (queue, messages) = neocogi_com::events::channel();
    let item = ui.widgets().list_item_with_queue("initial", Some(queue.as_com_ref())).unwrap();
    let mut row = item.handle();
    let mut widget = row.as_interface().query::<IWidgetAbi>().unwrap();
    let labels = ["pointer snapshot", "Enter snapshot", "Space snapshot: α"];
    let mut captured = Vec::new();
    for (input, label) in activations().into_iter().zip(labels) {
        row.set_text(label).unwrap();
        assert!(messages.try_recv().is_err(), "programmatic text replacement stays silent");
        activate(&driver, &mut widget, input).unwrap();
        // Leave the event owned by the test, without interpreting it in the receiver.
        let message = messages.try_recv().unwrap();
        let origin = message.origin.widget::<ListItemAbi>().unwrap().unwrap();
        assert!(origin.same_object(row.as_interface()));
        row.set_text("later live label").unwrap();
        assert!(message.query::<IActivatedEventAbi>().unwrap().is_some());
        assert!(messages.try_recv().is_err(), "exactly one fact per activation");
        captured.push(message);
    }
    drop(item);
    for message in &captured {
        assert!(!message.origin.widget::<ListItemAbi>().unwrap().unwrap().is_alive());
        assert!(message.query::<IActivatedEventAbi>().unwrap().is_some());
    }
}

/// Delivery rejection reports closure while preserving the row's authoritative state.
#[test]
fn disconnected_list_item_receiver_does_not_change_the_label() {
    let source = crate::test_support::assets::export_bundle(&crate::SkinBundle::from_atlas(crate::test_support::test_atlas())).unwrap();
    let gfx = neocogi_ui_support::test_support::gfx_factory();
    let (runtime, driver) = crate::runtime::testing::create_runtime(source.as_com_ref(), gfx.as_com_ref()).unwrap();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime).unwrap();
    let (queue, messages) = neocogi_com::events::channel();
    let item = ui.widgets().list_item_with_queue("retained", Some(queue.as_com_ref())).unwrap();
    let row = item.handle();
    let mut widget = row.as_interface().query::<IWidgetAbi>().unwrap();
    drop(messages);
    for input in activations() {
        assert_eq!(activate(&driver, &mut widget, input), Err(neocogi_com::NC_E_CLOSED));
        assert_eq!(row.text().unwrap(), "retained");
        assert!(row.as_interface().is_alive());
    }
}
