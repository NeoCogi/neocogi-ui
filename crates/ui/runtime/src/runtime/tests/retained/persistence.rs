//! A saved component builds real controls and exposes checked children for direct stream binding.
//! The fixture is an example persistence provider, not a complete UI file schema.

use super::*;
use neocogi_com::{
    binary, com_object, guid_from_qualified_name, ComDeserialize, ComSerialize, ComRef, INcBytesAbi, IUnknown, NcBytes, NcCopyFromBufferOffset, NcGuid,
    NC_E_ABI_VERSION, NC_E_INVALID_ARG, NC_E_OUT_OF_RANGE,
};
use neocogi_com::persistence::{snapshot, FactoryRegistry, ObjectRecord};
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
#[com_object(interfaces(neocogi_com::persistence::IPersistable, neocogi_idl_example_contracts::ICompositionRoot))]
struct SettingsView {
    /// Mountable composition root.
    root: LinearWidget,
    /// Retained only by the implementation for snapshot export.
    button: ButtonWidget,
    /// Retained only by the implementation for reading edited state.
    checkbox: CheckboxWidget,
    /// Authored public output names.
    names: (String, String),
}
impl ICompositionRoot for SettingsView {
    /// The host receives an opaque mountable widget, not the child handles.
    fn root(&self) -> Result<ComPtr<IUnknown>, NcStatus> {
        self.root.node_interface()?.query()
    }
    /// Resolves the actual authored child; event attachment uses its typed stream.
    fn widget(&self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<IUnknown>, NcStatus> {
        if name.offset() != 0 {
            return Err(NC_E_INVALID_ARG);
        }
        match std::str::from_utf8(name.as_slice()).map_err(|_| NC_E_INVALID_ARG)? {
            value if value == self.names.0 => self.button.as_interface().add_ref().query(),
            value if value == self.names.1 => self.checkbox.as_interface().add_ref().query(),
            _ => Err(neocogi_com::NC_E_NOT_FOUND),
        }
    }
}
impl neocogi_com::persistence::IPersistable for SettingsView {
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
#[com_object(interfaces(neocogi_com::persistence::IPersistenceFactory))]
struct SettingsFactory {
    /// Runtime authority and resources are supplied by the host, never the file.
    widgets: WidgetFactory,
    /// Host-owned queue is live construction context, never persisted data.
    sender: std::sync::mpsc::Sender<AppEvent>,
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
impl neocogi_com::persistence::IPersistenceFactory for SettingsFactory {
    /// Preflight validates the exact initialization and export-name contract.
    fn validate(&self, version: u32, state: NcCopyFromBufferOffset<'_, u8>, children: u32) -> Result<(), NcStatus> {
        settings(version, state, children).map(|_| ())
    }
    /// Returns a complete, unmounted composition; the application chooses its window.
    fn restore(
        &self,
        version: u32,
        state: NcCopyFromBufferOffset<'_, u8>,
        children: ComRef<'_, neocogi_com::persistence::IObjectChildrenAbi>,
    ) -> Result<ComPtr<IUnknown>, NcStatus> {
        let state = settings(version, state, children.count()?)?;
        let button = self.widgets.button_with_queue(
            &state.save_label,
            Some(neocogi_ui::test_event_queue!(button, AppEvent, self.sender.clone(), |_queue, _origin, _| AppEvent::Save)?.as_com_ref()),
        )?;
        let checkbox = self.widgets.checkbox_with_queue(
            &state.checkbox_label,
            state.checked,
            Some(
                neocogi_ui::test_event_queue!(
                    checkbox,
                    AppEvent,
                    self.sender.clone(),
                    |_queue, _origin, neocogi_ui::checkbox::Event::Changed(value)| AppEvent::Enabled(value.checked)
                )?
                .as_com_ref(),
            ),
        )?;
        let mut root = self.widgets.linear(FlowDirection::TopToBottom, CrossSize::Stretch)?;
        let result = (|| {
            root.insert(
                0,
                &button,
                LinearEdge {
                    main: LayoutTrack::Fixed { extent: 32 },
                    cross: ChildCrossSize::Stretch,
                },
            )?
            .finish()?;
            root.insert(
                1,
                &checkbox,
                LinearEdge {
                    main: LayoutTrack::Fixed { extent: 32 },
                    cross: ChildCrossSize::Stretch,
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
    let (sender, messages) = std::sync::mpsc::channel();
    let mut registry = FactoryRegistry::new().unwrap();
    registry
        .register(
            VIEW_KIND,
            SettingsFactory::from_value(SettingsFactory {
                widgets: fixture.ui.widgets().clone(),
                sender,
            })
            .query()
            .unwrap(),
        )
        .unwrap();
    let component = registry.restore(&binary::to_vec(&record).unwrap()).unwrap();
    assert_eq!(snapshot(component.as_com_ref()).unwrap(), binary::to_vec(&record).unwrap());
    let outputs = component.query::<ICompositionRootAbi>().unwrap();
    assert!(
        outputs
            .widget(NcCopyFromBufferOffset::new(0, b"save_requested"))
            .unwrap()
            .query::<CheckboxAbi>()
            .is_err()
    );
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
