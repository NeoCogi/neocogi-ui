//! Independent provider roots bind and deliver real node events through the production engine.

use neocogi_com::{ComInterface, ComPtr, Runtime, guid_from_qualified_name, NC_E_NOINTERFACE};
use neocogi_ui::{KeyCode, KeyStroke, Rect, Size, Ui, UiInput, UI_E_OWNER};
use neocogi_ui_core_contracts::UiPluginFactoryAbi;
use neocogi_ui_nodes_contracts::{NodesPluginFactoryAbi, UiFactoryAbi};

/// Sends real copied key input rather than manufacturing a semantic event or replacing routing.
fn key(ui: &Ui, key: KeyCode) {
    ui.input(
        UiInput::Key(KeyStroke {
            key,
            value: 0,
            pressed: true,
            repeat: false,
            modifiers: 0,
        }),
        "",
    )
    .unwrap();
}

/// Core and nodes activate separately, authenticate binding, and preserve typed event delivery.
#[test]
fn independently_activated_nodes_bind_only_to_their_core_host_and_deliver_events() {
    let registry = Runtime::global();
    let core_description = neocogi_ui_runtime::factory_plugin_description();
    let nodes_description = neocogi_ui_nodes::factory_plugin_description();
    assert_eq!(core_description.root_factory, UiPluginFactoryAbi::INTERFACE_ID);
    assert_eq!(nodes_description.root_factory, NodesPluginFactoryAbi::INTERFACE_ID);
    assert_eq!(nodes_description.dependency_count, 1);
    // SAFETY: Both descriptions and their code/data remain in this linked test executable.
    unsafe {
        registry.register_factory_plugin_description(core_description).unwrap();
        registry.register_factory_plugin_description(nodes_description).unwrap();
    }
    let core = registry
        .activate_plugin_factory::<UiPluginFactoryAbi>(guid_from_qualified_name("neocogi.ui.runtime"))
        .unwrap();
    let nodes = registry
        .activate_plugin_factory::<NodesPluginFactoryAbi>(guid_from_qualified_name("neocogi.ui.nodes.runtime"))
        .unwrap();
    let source =
        neocogi_ui_support::test_support::assets::export_bundle(&neocogi_ui_support::SkinBundle::from_atlas(neocogi_ui_support::test_support::test_atlas()))
            .unwrap();
    let gfx = neocogi_ui_support::test_support::gfx_factory();
    let runtime = core.create_runtime(source.as_com_ref(), gfx.as_com_ref()).unwrap();
    // The core object never grows concrete node factory capabilities as a side effect of binding.
    assert_eq!(runtime.query::<UiFactoryAbi>().unwrap_err(), NC_E_NOINTERFACE);
    let factory: ComPtr<UiFactoryAbi> = nodes.bind(runtime.as_com_ref()).unwrap();
    let other_runtime = core.create_runtime(source.as_com_ref(), gfx.as_com_ref()).unwrap();
    assert!(matches!(Ui::from_interfaces(other_runtime, factory.clone()), Err(UI_E_OWNER)));
    let ui = Ui::from_interfaces(runtime, factory).unwrap();
    let button = ui.widgets().button("nodes-owned activation").unwrap();
    ui.window("provider boundary", Rect { x: 10, y: 10, width: 300, height: 150 }, &button).unwrap();
    let (events_tx, events) = std::sync::mpsc::channel();
    let mut labels = Vec::new();
    neocogi_ui::events::map(button.activated(), events_tx.clone(), |event| event.label).unwrap();
    key(&ui, KeyCode::Tab);
    key(&ui, KeyCode::Space);
    ui.update_with_events(Size { width: 640, height: 400 }, &events, |message| {
        labels.push(message);
        Ok(())
    })
    .unwrap();
    assert_eq!(&labels, &["nodes-owned activation"]);
}
