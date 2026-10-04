//! Generated modules are compiled as ordinary Rust, not merely parsed or string-checked.
use neocogi_ui_document::Document;

#[rustfmt::skip]
#[allow(dead_code)]
#[path = "fixtures/full_bindings.rs"]
mod full;
#[rustfmt::skip]
#[allow(dead_code)]
#[path = "fixtures/empty_bindings.rs"]
mod empty;
#[rustfmt::skip]
#[allow(dead_code)]
#[path = "fixtures/custom_bindings.rs"]
mod custom;

/// Refresh fixtures only when explicitly requested; normal runs compare the
/// freshly generated source with the source compiled into this test binary.
fn fixture(actual: String, filename: &str, compiled: &str) {
    if std::env::var_os("NC_UPDATE_BINDINGS_FIXTURES").is_some() {
        std::fs::write(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(filename), actual).unwrap();
    } else {
        assert_eq!(actual, compiled);
    }
}

/// Exact fixture equality keeps the compiled artifact synchronized with the generator's output.
#[test]
fn generated_bindings_compile_for_every_public_contract_and_escaped_name() {
    let document = Document::from_json(include_bytes!("fixtures/bindings.ui.json")).unwrap();
    fixture(document.rust_bindings().unwrap(), "full_bindings.rs", include_str!("fixtures/full_bindings.rs"));
}

/// Empty documents still produce valid optional fields and a callable binding function.
#[test]
fn empty_public_contract_generates_a_valid_module() {
    let document = Document::from_json(include_bytes!("fixtures/empty.ui.json")).unwrap();
    fixture(
        document.rust_bindings().unwrap(),
        "empty_bindings.rs",
        include_str!("fixtures/empty_bindings.rs"),
    );
}

/// Generated custom fields and handlers read their event contract from the COM interface.
#[test]
fn custom_widget_contract_generates_checked_field_and_direct_event_binding() {
    use neocogi_com::{NcStatus, NC_E_INVALID_ARG};
    use neocogi_ui::{IWidgetAbi, Ui, OwnedWidget};
    use neocogi_ui_document::{Child, CustomWidgetFactory, CustomWidgetRegistry, Edge, Node, Track, WidgetSpec};
    use std::rc::Rc;

    /// Test provider names the IDL-generated COM description, not a duplicate event schema.
    struct CounterFactory {
        /// Designer-visible constructor defaults.
        defaults: Vec<u8>,
    }
    impl CustomWidgetFactory for CounterFactory {
        /// Copies the generated projection and typed constructor defaults.
        fn manifest(&self) -> Result<neocogi_ui_document::design::DesignWidgetManifest, String> {
            neocogi_ui_document::design::DesignWidgetManifest::from_idl(
                "example.button",
                self.defaults.clone(),
                &neocogi_demo_contracts::CounterAbi::IDL_INTERFACE,
            )
        }
        /// Binding generation never constructs a live widget.
        fn create(&self, _: &Ui, _: &[u8], _: Option<neocogi_com::ComPtr<neocogi_com::events::IEventReceiverAbi>>) -> Result<OwnedWidget<IWidgetAbi>, NcStatus> {
            Err(NC_E_INVALID_ARG)
        }
    }
    let mut registry = CustomWidgetRegistry::new();
    registry
        .register(Rc::new(CounterFactory {
            defaults: neocogi_ui_document::design::encode_settings("Custom").unwrap(),
        }))
        .unwrap();
    let mut document = Document::new("custom");
    document.root.children.push(Child {
        edge: Edge::Linear { main: Track::Content, cross: None },
        node: Node::new("extension", registry.default_spec("example.button").unwrap()),
    });
    fixture(
        document.rust_bindings_with(&registry).unwrap(),
        "custom_bindings.rs",
        include_str!("fixtures/custom_bindings.rs"),
    );
    assert_eq!(Document::from_json(&document.to_json().unwrap()).unwrap(), document);
    assert_eq!(Document::from_binary(&document.to_binary().unwrap()).unwrap(), document);
    let events = neocogi_demo_contracts::CounterAbi::IDL_INTERFACE.events;
    assert_eq!(events.iter().map(|event| event.qualified_name).collect::<Vec<_>>(), ["neocogi.ui.core.IActivatedEvent", "neocogi.demo.ICounterChangedEvent"]);
    assert_eq!(events[1].fields[0].name, "value");
    assert_eq!(events[1].fields[0].ty, neocogi_com::design::IdlValueType::Scalar("u32"));
    assert!(document.rust_bindings().unwrap_err().contains("not registered"));
    let WidgetSpec::Custom { settings, .. } = &mut document.root.children.last_mut().unwrap().node.widget else {
        unreachable!()
    };
    settings.resize(neocogi_ui_document::design::MAX_SETTINGS_BYTES + 1, 0);
    assert!(registry.validate_document(&document).unwrap_err().contains("byte limit"));
}

/// Registry registration rejects an advertised getter missing from its value graph.
#[test]
fn custom_factory_cannot_omit_its_event_getter_value_graph() {
    use neocogi_com::{NcStatus, NC_E_INVALID_ARG, design::IdlInterface};
    use neocogi_ui::{IWidgetAbi, Ui, OwnedWidget};
    use neocogi_ui_document::{CustomWidgetFactory, CustomWidgetRegistry};
    use std::rc::Rc;

    /// Fabricated metadata omits the reachable definitions for its real typed receiver.
    static HIDDEN: IdlInterface = IdlInterface {
        abi: &neocogi_demo_contracts::CounterAbi::INTERFACE_DESCRIPTOR,
        chain: &neocogi_demo_contracts::CounterAbi::INTERFACE_CHAIN,
        rust_abi: "neocogi_demo_contracts::CounterAbi",
        events: &[neocogi_com::design::IdlEvent {
            qualified_name: "example.IMissingEvent",
            abi: &neocogi_demo_contracts::ICounterChangedEventAbi::INTERFACE_DESCRIPTOR,
            fields: &[neocogi_com::design::IdlField { name: "value", ty: neocogi_com::design::IdlValueType::Named("example.Missing") }],
        }],
        values: &[],
    };
    /// A provider whose advertised source metadata disagrees with the COM descriptor.
    struct HiddenFactory;
    impl CustomWidgetFactory for HiddenFactory {
        /// The deliberately inconsistent metadata fails during static conversion.
        fn manifest(&self) -> Result<neocogi_ui_document::design::DesignWidgetManifest, String> {
            neocogi_ui_document::design::DesignWidgetManifest::from_idl("hidden.counter", vec![], &HIDDEN)
        }
        /// Registration rejects this provider before construction is possible.
        fn create(&self, _: &Ui, _: &[u8], _: Option<neocogi_com::ComPtr<neocogi_com::events::IEventReceiverAbi>>) -> Result<OwnedWidget<IWidgetAbi>, NcStatus> {
            Err(NC_E_INVALID_ARG)
        }
    }
    assert!(
        CustomWidgetRegistry::new()
            .register(Rc::new(HiddenFactory))
            .unwrap_err()
            .contains("Missing value definition")
    );
}

#[rustfmt::skip]
#[allow(dead_code)]
#[path = "fixtures/shared_bindings.rs"]
mod shared;
#[rustfmt::skip]
#[allow(dead_code)]
#[path = "fixtures/eventless_bindings.rs"]
mod eventless;

/// Two buttons share one concrete receiver implementation; no widget needs a mapper.
#[test]
fn form_fields_need_no_queue_or_handler_generation() {
    let document = Document::from_json(include_bytes!("fixtures/shared.ui.json")).unwrap();
    let source = document.rust_bindings().unwrap();
    assert!(!source.contains("FormQueue"));
    assert!(!source.contains("impl neocogi_com::events::IEventReceiver"));
    assert!(!source.contains("bindings.insert::<"));
    assert!(source.contains("widget_first_1"));
    assert!(source.contains("widget_second_2"));
    fixture(source, "shared_bindings.rs", include_str!("fixtures/shared_bindings.rs"));
}

/// A form with no emitting widgets needs neither a dummy queue nor mapper allocation.
#[test]
fn eventless_form_compiles_without_a_queue_object() {
    let mut document = Document::new("eventless");
    document.root = neocogi_ui_document::Node::new(
        "label",
        neocogi_ui_document::WidgetSpec::TextBlock {
            text: "read only".into(),
            wrapping: neocogi_ui_document::Wrapping::None,
        },
    );
    document.outputs.clear();
    let source = document.rust_bindings().unwrap();
    assert!(!source.contains("struct FormQueue"));
    fixture(source, "eventless_bindings.rs", include_str!("fixtures/eventless_bindings.rs"));
}

#[rustfmt::skip]
#[allow(dead_code)]
#[path = "fixtures/alias_bindings.rs"]
mod alias;

/// Optional fields accept Rust reexports; event revisions are checked on actual event queries.
#[test]
fn form_fields_allow_rust_aliases_and_independent_event_revisions() {
    use neocogi_ui_document::{CustomWidgetFactory, CustomWidgetRegistry, Node, WidgetSpec};
    use neocogi_ui_document::design::DesignWidgetManifest;
    use std::rc::Rc;

    /// Pure metadata fixture; no widget is constructed during source generation.
    struct Catalog(DesignWidgetManifest);
    impl CustomWidgetFactory for Catalog {
        /// Returns one checked descriptor with alternate public reexport paths.
        fn manifest(&self) -> Result<DesignWidgetManifest, String> {
            Ok(self.0.clone())
        }
        /// Construction is deliberately unavailable in this generator-only test.
        fn create(
            &self,
            _: &neocogi_ui::Ui,
            _: &[u8],
            _: Option<neocogi_com::ComPtr<neocogi_com::events::IEventReceiverAbi>>,
        ) -> Result<neocogi_ui::OwnedWidget<neocogi_ui::IWidgetAbi>, neocogi_com::NcStatus> {
            Err(neocogi_com::NC_E_INVALID_ARG)
        }
    }
    let mut metadata = DesignWidgetManifest::from_idl("alias.button", vec![], &neocogi_ui::ButtonAbi::IDL_INTERFACE).unwrap();
    metadata.interface.rust_abi = "neocogi_ui::nodes::ButtonAbi".into();
    for value in &mut metadata.interface.values {
        value.rust_type = value.rust_type.replace("neocogi_ui_nodes_contracts::", "neocogi_ui::nodes::");
    }
    let mut registry = CustomWidgetRegistry::new();
    registry.register(Rc::new(Catalog(metadata.clone()))).unwrap();
    let mut document = Document::from_json(include_bytes!("fixtures/shared.ui.json")).unwrap();
    document.root.children[1].node = Node::new(
        "alias",
        WidgetSpec::Custom {
            kind: "alias.button".into(),
            settings: vec![],
        },
    );
    let source = document.rust_bindings_with(&registry).unwrap();
    assert!(!source.contains("impl neocogi_com::events::IEventReceiver"));
    fixture(source, "alias_bindings.rs", include_str!("fixtures/alias_bindings.rs"));

    // Event revisions negotiate against actual event objects when queried, not
    // against the widget constructor's common receiver.
    metadata.interface.events[0].signature[0] ^= 1;
    let mut alternate = CustomWidgetRegistry::new();
    alternate.register(Rc::new(Catalog(metadata))).unwrap();
}
