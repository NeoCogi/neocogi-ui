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

/// Exact fixture equality keeps the compiled artifact synchronized with the generator's output.
#[test]
fn generated_bindings_compile_for_every_public_contract_and_escaped_name() {
    let document = Document::from_json(include_bytes!("fixtures/bindings.ui.json")).unwrap();
    assert_eq!(document.rust_bindings().unwrap(), include_str!("fixtures/full_bindings.rs"));
}

/// Empty public contracts still produce a valid exhaustive dispatcher and a callable binding function.
#[test]
fn empty_public_contract_generates_a_valid_module() {
    let document = Document::from_json(include_bytes!("fixtures/empty.ui.json")).unwrap();
    assert_eq!(document.rust_bindings().unwrap(), include_str!("fixtures/empty_bindings.rs"));
}

/// Generated custom fields and handlers read their event contract from the COM interface.
#[test]
fn custom_widget_contract_generates_checked_field_and_direct_event_binding() {
    use neocogi_com::{NcStatus, NC_E_INVALID_ARG};
    use neocogi_ui::{IWidgetAbi, Ui, Widget};
    use neocogi_ui_document::{Child, CustomProperty, CustomValue, CustomWidgetFactory, CustomWidgetRegistry, Edge, Node, Track, WidgetSpec};
    use std::rc::Rc;

    /// Test provider names the IDL-generated COM description, not a duplicate event schema.
    struct CounterFactory {
        /// Designer-visible constructor defaults.
        defaults: Vec<CustomProperty>,
    }
    impl CustomWidgetFactory for CounterFactory {
        /// Stable document kind for the test fixture.
        fn kind(&self) -> &str { "example.button" }
        /// The same generated projection used by the application's widget field.
        fn interface(&self) -> &'static neocogi_com::design::IdlInterface {
            &neocogi_demo_contracts::CounterAbi::IDL_INTERFACE
        }
        /// Typed defaults accepted by the designer inspector.
        fn defaults(&self) -> &[CustomProperty] { &self.defaults }
        /// Binding generation never constructs a live widget.
        fn create(&self, _: &Ui, _: &[CustomProperty]) -> Result<Widget<IWidgetAbi>, NcStatus> { Err(NC_E_INVALID_ARG) }
    }
    let mut registry = CustomWidgetRegistry::new();
    registry.register(Rc::new(CounterFactory {
        defaults: vec![CustomProperty { name: "label".into(), value: CustomValue::Text("Custom".into()) }],
    })).unwrap();
    let mut document = Document::new("custom");
    document.root.children.push(Child {
        edge: Edge::Linear { main: Track::Content, cross: None },
        node: Node::new("extension", registry.default_spec("example.button").unwrap()),
    });
    assert_eq!(document.rust_bindings_with(&registry).unwrap(), include_str!("fixtures/custom_bindings.rs"));
    assert_eq!(Document::from_json(&document.to_json().unwrap()).unwrap(), document);
    assert_eq!(Document::from_binary(&document.to_binary().unwrap()).unwrap(), document);
    let metadata = &neocogi_demo_contracts::CounterEvent::IDL_MESSAGE;
    let neocogi_com::design::IdlShape::Enum(cases) = metadata.shape else { panic!("CounterEvent must be an enum") };
    assert_eq!(cases.iter().map(|case| case.name).collect::<Vec<_>>(), ["Activated", "Changed"]);
    assert_eq!(cases[1].fields[0].ty, "neocogi.demo.CounterChanged");
    assert!(document.rust_bindings().unwrap_err().contains("not registered"));
    let WidgetSpec::Custom { properties, .. } = &mut document.root.children.last_mut().unwrap().node.widget else { unreachable!() };
    properties[0].value = CustomValue::Integer(2);
    assert!(registry.validate_document(&document).unwrap_err().contains("schema mismatch"));
}

/// Registry registration rejects metadata that conceals an actual COM output setter.
#[test]
fn custom_factory_cannot_hide_a_second_event_contract_in_com_metadata() {
    use neocogi_com::{NcStatus, NC_E_INVALID_ARG, design::IdlInterface};
    use neocogi_ui::{IWidgetAbi, Ui, Widget};
    use neocogi_ui_document::{CustomProperty, CustomWidgetFactory, CustomWidgetRegistry};
    use std::rc::Rc;

    /// Fabricated source metadata intentionally hides Counter's physical event setter.
    static HIDDEN: IdlInterface = IdlInterface {
        abi: &neocogi_demo_contracts::CounterAbi::INTERFACE_DESCRIPTOR,
        chain: &neocogi_demo_contracts::CounterAbi::INTERFACE_CHAIN,
        rust_abi: "neocogi_demo_contracts::CounterAbi",
        rust_outputs: None,
        outputs: &[],
    };
    /// A provider whose advertised source metadata disagrees with the COM descriptor.
    struct HiddenFactory;
    impl CustomWidgetFactory for HiddenFactory {
        /// One nonempty lookup key.
        fn kind(&self) -> &str { "hidden.counter" }
        /// The deliberately inconsistent metadata above.
        fn interface(&self) -> &'static IdlInterface { &HIDDEN }
        /// No constructor properties are needed for this registration check.
        fn defaults(&self) -> &[CustomProperty] { &[] }
        /// Registration rejects this provider before construction is possible.
        fn create(&self, _: &Ui, _: &[CustomProperty]) -> Result<Widget<IWidgetAbi>, NcStatus> { Err(NC_E_INVALID_ARG) }
    }
    assert!(CustomWidgetRegistry::new().register(Rc::new(HiddenFactory)).unwrap_err().contains("disagrees"));
}
