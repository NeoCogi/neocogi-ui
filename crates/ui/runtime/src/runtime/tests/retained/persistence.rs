//! A saved opaque component builds real controls and binds only named outputs.
//! The fixture is an example persistence provider, not a complete UI file schema.

use super::*;
use neocogi_ui_nodes_contracts::CheckboxChanged;
use neocogi_com::{
    binary, com_object, guid_from_qualified_name, ComDeserialize, ComSerialize, ComRef, INcBytesAbi, IUnknown, NcBytes, NcCopyFromBufferOffset, NcGuid,
    NC_E_ABI_VERSION, NC_E_INVALID_ARG, NC_E_OUT_OF_RANGE,
};
use neocogi_com_events::{bind_output, NamedOutputs};
use neocogi_com_events::{INamedOutputs, INamedOutputsAbi};
use neocogi_com_persistence::{snapshot, FactoryRegistry, ObjectRecord};
use neocogi_idl_example_contracts::{ICompositionRoot, ICompositionRootAbi};

/// Persistent recipe identity remains independent from runtime/widget identities.
const VIEW_KIND: NcGuid = guid_from_qualified_name("example.persisted-settings-view");

/// Full authored state for this small example composition.
#[derive(Clone, Debug, PartialEq, ComSerialize, ComDeserialize)]
struct Settings {
    /// Save-control text; not used as its event identity.
    save_label: String,
    /// Boolean-control text.
    checkbox_label: String,
    /// Persisted initial checked state; saving reads the live committed value.
    checked: bool,
    /// Stable application-facing activation output.
    save_output: String,
    /// Stable application-facing boolean output.
    checked_output: String,
}

/// Private control handles never cross the application's integration boundary.
#[com_object(interfaces(
    neocogi_com_persistence::IPersistable,
    neocogi_com_events_contracts::INamedOutputs,
    neocogi_idl_example_contracts::ICompositionRoot
))]
struct SettingsView {
    /// Mountable composition root.
    root: LinearWidget,
    /// Retained only by the implementation for snapshot export.
    button: ButtonWidget,
    /// Retained only by the implementation for reading edited state.
    checkbox: CheckboxWidget,
    /// Setup-only table; events use direct concrete receiver calls.
    outputs: NamedOutputs,
    /// Authored public output names.
    names: (String, String),
}
impl ICompositionRoot for SettingsView {
    /// The host receives an opaque mountable widget, not the child handles.
    fn root(&self) -> Result<ComPtr<IUnknown>, NcStatus> {
        self.root.node_interface()?.query()
    }
}
impl INamedOutputs for SettingsView {
    /// Delegates named setup through ordinary mutable state.
    fn bind_output(&mut self, name: NcCopyFromBufferOffset<'_, u8>, receiver: ComRef<'_, IUnknown>) -> Result<(), NcStatus> {
        self.outputs.bind_com(name, receiver)
    }
}
impl neocogi_com_persistence::IPersistable for SettingsView {
    /// Selects the provider explicitly registered by the application.
    fn type_id(&self) -> Result<NcGuid, NcStatus> {
        Ok(VIEW_KIND)
    }
    /// This composition owns revision one of Settings.
    fn schema_version(&self) -> Result<u32, NcStatus> {
        Ok(1)
    }
    /// Saves current semantic values instead of replaying the original bytes.
    fn save_state(&self) -> Result<ComPtr<INcBytesAbi>, NcStatus> {
        let state = Settings {
            save_label: self.button.label()?,
            checkbox_label: self.checkbox.label()?,
            checked: self.checkbox.checked()?,
            save_output: self.names.0.clone(),
            checked_output: self.names.1.clone(),
        };
        Ok(NcBytes::from_vec(binary::to_vec(&state).map_err(|_| NC_E_INVALID_ARG)?))
    }
    /// Internal controls belong to this composition's initialization recipe.
    fn child_count(&self) -> Result<u32, NcStatus> {
        Ok(0)
    }
    /// This recipe has no separately persisted component children.
    fn child(&self, _: u32) -> Result<ComPtr<IUnknown>, NcStatus> {
        Err(NC_E_OUT_OF_RANGE)
    }
}

/// Context-bound construction reuses ordinary UI factory methods.
#[com_object(interfaces(neocogi_com_persistence::IPersistenceFactory))]
struct SettingsFactory {
    /// Runtime authority and resources are supplied by the host, never the file.
    widgets: WidgetFactory,
}
/// Parses and validates before any widget allocation or topology change.
fn settings(version: u32, bytes: NcCopyFromBufferOffset<'_, u8>, children: u32) -> Result<Settings, NcStatus> {
    if version != 1 {
        return Err(NC_E_ABI_VERSION);
    }
    if bytes.offset() != 0 || children != 0 {
        return Err(NC_E_INVALID_ARG);
    }
    let value: Settings = binary::from_slice(bytes.as_slice()).map_err(|_| NC_E_INVALID_ARG)?;
    if value.save_output.is_empty() || value.checked_output.is_empty() || value.save_output == value.checked_output {
        return Err(NC_E_INVALID_ARG);
    }
    Ok(value)
}
impl neocogi_com_persistence::IPersistenceFactory for SettingsFactory {
    /// Preflight validates the exact initialization and export-name contract.
    fn validate(&self, version: u32, state: NcCopyFromBufferOffset<'_, u8>, children: u32) -> Result<(), NcStatus> {
        settings(version, state, children).map(|_| ())
    }
    /// Returns a complete, unmounted composition; the application chooses its window.
    fn restore(
        &self,
        version: u32,
        state: NcCopyFromBufferOffset<'_, u8>,
        children: ComRef<'_, neocogi_com_persistence::IObjectChildrenAbi>,
    ) -> Result<ComPtr<IUnknown>, NcStatus> {
        let state = settings(version, state, children.count()?)?;
        let button = self.widgets.button(&state.save_label)?;
        let checkbox = self.widgets.checkbox(&state.checkbox_label, state.checked)?;
        let mut outputs = NamedOutputs::new();
        outputs.export(&state.save_output, button.as_interface().add_ref(), "activated")?;
        outputs.export(&state.checked_output, checkbox.as_interface().add_ref(), "changed")?;
        let mut root = self.widgets.linear(FlowDirection::TopToBottom, CrossSize::Stretch)?;
        let result = (|| {
            root.insert(
                0,
                &button,
                LinearEdge {
                    main: LayoutTrack::Fixed { extent: 32 }.into(),
                    cross: ChildCrossSize::Stretch.into(),
                },
            )?
            .finish()?;
            root.insert(
                1,
                &checkbox,
                LinearEdge {
                    main: LayoutTrack::Fixed { extent: 32 }.into(),
                    cross: ChildCrossSize::Stretch.into(),
                },
            )?
            .finish()
        })();
        if let Err(error) = result {
            // Retire any topology already committed before returning construction failure.
            let _ = root.node_interface()?.close();
            return Err(error);
        }
        Ok(SettingsView::from_value(SettingsView {
            root,
            button,
            checkbox,
            outputs,
            names: (state.save_output, state.checked_output),
        }))
    }
}

/// Application messages have no dependency on internal widget classes.
#[derive(Debug, PartialEq)]
enum AppEvent {
    /// User requested a save action.
    Save,
    /// User changed the boolean setting.
    Enabled(bool),
}

/// Loads, mounts, edits, saves, reloads, and rebinds through opaque capabilities.
#[test]
fn persisted_component_builds_real_ui_and_binds_without_child_handles() {
    let mut fixture = Headless::standard();
    let state = Settings {
        save_label: "Save".into(),
        checkbox_label: "Enabled".into(),
        checked: false,
        save_output: "save_requested".into(),
        checked_output: "enabled_changed".into(),
    };
    let record = ObjectRecord {
        type_id: VIEW_KIND.to_uuid_bytes(),
        schema_version: 1,
        state: binary::to_vec(&state).unwrap(),
        children: vec![],
    };
    let mut registry = FactoryRegistry::new();
    registry
        .register(
            VIEW_KIND,
            SettingsFactory::from_value(SettingsFactory { widgets: fixture.ui.widgets().clone() })
                .query()
                .unwrap(),
        )
        .unwrap();
    let component = registry.restore(&binary::to_vec(&record).unwrap()).unwrap();
    assert_eq!(snapshot(component.as_com_ref()).unwrap(), binary::to_vec(&record).unwrap());
    let outputs = component.query::<INamedOutputsAbi>().unwrap();
    let (sender, messages) = std::sync::mpsc::channel();
    bind_output(outputs.as_com_ref(), "save_requested", sender.clone(), |_: Activation| AppEvent::Save).unwrap();
    bind_output(outputs.as_com_ref(), "enabled_changed", sender.clone(), |value: CheckboxChanged| {
        AppEvent::Enabled(value.checked)
    })
    .unwrap();
    assert!(bind_output(outputs.as_com_ref(), "save_requested", sender.clone(), |_: CheckboxChanged| AppEvent::Save).is_err());
    let mountable = component.query::<ICompositionRootAbi>().unwrap().root().unwrap().query::<IWidgetAbi>().unwrap();
    let owner = window(
        &fixture.ui,
        "loaded",
        Rect { x: 10, y: 10, width: 240, height: 80 },
        &Widget::from_interface(mountable),
    );
    fixture.frame(VIEW).unwrap();
    for point in [Point { x: 50, y: 26 }, Point { x: 50, y: 62 }] {
        pointer(&fixture.ui, point, true);
        pointer(&fixture.ui, point, false);
        fixture.frame(VIEW).unwrap();
    }
    assert_eq!(messages.try_recv().unwrap(), AppEvent::Save);
    assert_eq!(messages.try_recv().unwrap(), AppEvent::Enabled(true));
    let saved = snapshot(component.as_com_ref()).unwrap();
    let saved_record: ObjectRecord = binary::from_slice(&saved).unwrap();
    let saved_state: Settings = binary::from_slice(&saved_record.state).unwrap();
    assert!(saved_state.checked, "snapshot must capture the committed user edit");
    owner.close().unwrap();
    drop(outputs);
    drop(component);
    let restored = registry.restore(&saved).unwrap();
    assert_eq!(snapshot(restored.as_com_ref()).unwrap(), saved);
    let outputs = restored.query::<INamedOutputsAbi>().unwrap();
    bind_output(outputs.as_com_ref(), "save_requested", sender, |_: Activation| AppEvent::Save).unwrap();
    let mountable = restored.query::<ICompositionRootAbi>().unwrap().root().unwrap().query::<IWidgetAbi>().unwrap();
    let _owner = window(
        &fixture.ui,
        "reloaded",
        Rect { x: 10, y: 10, width: 240, height: 80 },
        &Widget::from_interface(mountable),
    );
    fixture.frame(VIEW).unwrap();
    pointer(&fixture.ui, Point { x: 50, y: 26 }, true);
    pointer(&fixture.ui, Point { x: 50, y: 26 }, false);
    fixture.frame(VIEW).unwrap();
    assert_eq!(messages.try_recv().unwrap(), AppEvent::Save);
}
