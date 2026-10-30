//! Admission tests use the real loader, generated catalog calls and allocation counters.
use neocogi_ui::UiEventSource;
use super::*;
use neocogi_ui_document::{*, design::DesignWidgetManifest};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

/// Concrete products let admission facts be tested against actual constructed widget interfaces.
#[derive(Clone, Copy)]
enum CatalogProduct {
    /// Advertises ordinary activation events.
    Button,
    /// Intentionally silent, yet constructible without a display alias or receiver.
    Text,
}

/// Independently returned descriptor bytes and the concrete constructor they describe.
struct CatalogEntry {
    /// Raw initialized transport can deliberately bypass the producer's encode validator.
    bytes: Vec<u8>,
    /// Real product, never a second fake widget implementation.
    product: CatalogProduct,
}

#[neocogi_com::com_object(interfaces(neocogi_ui_document::contracts::IDesignWidgetCatalog))]
/// Multi-kind transport exercises whole-catalog publication and whole-document preflight.
struct ContractCatalog {
    /// Deliberately independent of the initialized output prefix for protocol admission tests.
    extra_count: u32,
    /// Production factory/client pair for all constructed products.
    ui: Ui,
    /// Ordered source records returned independently through generated describe thunks.
    entries: Vec<CatalogEntry>,
    /// Counts every constructor entry, including one that would later reject its arguments.
    calls: Arc<AtomicUsize>,
}
impl neocogi_ui_document::contracts::IDesignWidgetCatalog for ContractCatalog {
    fn widget_count(&self) -> Result<u32, NcStatus> {
        Ok(self.entries.len() as u32)
    }
    fn describe(&self, index: u32, mut output: neocogi_com::NcOutputWriter<'_, u8>) -> Result<u32, NcStatus> {
        if output.offset() != 0 {
            return Err(neocogi_com::NC_E_INVALID_ARG);
        }
        let entry = self.entries.get(index as usize).ok_or(neocogi_com::NC_E_OUT_OF_RANGE)?;
        if output.capacity() >= entry.bytes.len() {
            output.extend_from_slice(&entry.bytes)?;
        }
        Ok(entry.bytes.len() as u32 + self.extra_count)
    }
    fn create_editor(
        &self,
        _: u32,
        _: neocogi_com::ComRef<'_, UiFactoryAbi>,
        _: neocogi_com::NcCopyFromBufferOffset<'_, u8>,
    ) -> Result<neocogi_com::ComBox<neocogi_ui_document::contracts::IDesignSettingsEditorAbi>, NcStatus> {
        Err(neocogi_com::NC_E_NOINTERFACE)
    }
    fn create_widget(
        &self,
        index: u32,
        _: neocogi_com::ComRef<'_, IWidgetHostAbi>,
        settings: neocogi_com::NcCopyFromBufferOffset<'_, u8>,
        events: neocogi_com::NcValueRef<'_, neocogi_ui::events::EventBinding>,
    ) -> Result<neocogi_com::ComBox<IWidgetAbi>, NcStatus> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        if settings.offset() != 0 {
            return Err(neocogi_com::NC_E_INVALID_ARG);
        }
        let entry = self.entries.get(index as usize).ok_or(neocogi_com::NC_E_OUT_OF_RANGE)?;
        let text: String = design::decode_settings(settings.as_slice()).map_err(|_| neocogi_com::NC_E_INVALID_ARG)?;
        let receiver = match events.to_owned()? {
            neocogi_ui::events::EventBinding::Unobserved => None,
            neocogi_ui::events::EventBinding::Observed(receiver) => Some(receiver),
        };
        match entry.product {
            CatalogProduct::Button => self
                .ui
                .widgets()
                .button_with_queue(&text, receiver.as_ref().map(ComPtr::as_com_ref))?
                .into_interface()
                .query()
                .map_err(|(status, _owner)| status),
            CatalogProduct::Text if receiver.is_none() => self
                .ui
                .widgets()
                .text_block(&text, TextWrapping::None)?
                .into_interface()
                .query()
                .map_err(|(status, _owner)| status),
            CatalogProduct::Text => Err(neocogi_com::NC_E_INVALID_ARG),
        }
    }
}

/// A repeated required count must agree with the independently initialized output prefix.
#[test]
fn catalog_description_count_must_match_the_initialized_bytes() {
    let fixture = Headless::standard();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = CustomWidgetRegistry::new();
    for (extra_count, accepted) in [(0, true), (1, false)] {
        let catalog = ContractCatalog::from_value(ContractCatalog {
            extra_count,
            ui: fixture.ui.clone(),
            calls: calls.clone(),
            entries: vec![CatalogEntry {
                bytes: contract_manifest(if accepted { "count.valid" } else { "count.invalid" }, CatalogProduct::Button)
                    .encode()
                    .unwrap(),
                product: CatalogProduct::Button,
            }],
        });
        assert_eq!(registry.register_catalog(catalog.query().unwrap()).is_ok(), accepted);
    }
    assert_eq!(registry.kinds().collect::<Vec<_>>(), vec!["count.valid"]);
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

/// Derives ordinary kind metadata before adding any deliberately hostile transport shape.
fn contract_manifest(kind: &str, product: CatalogProduct) -> DesignWidgetManifest {
    let source = match product {
        CatalogProduct::Button => &ButtonAbi::EVENT_SOURCE,
        CatalogProduct::Text => &TextBlockAbi::EVENT_SOURCE,
    };
    DesignWidgetManifest::from_source(kind, design::encode_settings("initial").unwrap(), source).unwrap()
}

/// Real COM catalog registration must preserve the entire old registry on a later invalid record.
#[test]
fn deepest_catalog_metadata_round_trips_and_failed_later_describe_publishes_nothing() {
    use design::{DesignField, DesignShape, DesignType, DesignValueType};
    let fixture = Headless::standard();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = CustomWidgetRegistry::new();
    let original = contract_manifest("contract.original", CatalogProduct::Button);
    let catalog = ContractCatalog::from_value(ContractCatalog {
        extra_count: 0,
        ui: fixture.ui.clone(),
        calls: calls.clone(),
        entries: vec![CatalogEntry {
            bytes: original.encode().unwrap(),
            product: CatalogProduct::Button,
        }],
    });
    registry.register_catalog(catalog.query().unwrap()).unwrap();
    let before: Vec<_> = registry.kinds().map(str::to_owned).collect();
    let mut deepest = contract_manifest("contract.deep", CatalogProduct::Button);
    let mut ty = DesignValueType::Scalar("u32".into());
    for _ in 1..design::MAX_DESIGN_EXPRESSION_NODES {
        ty = DesignValueType::Array(Box::new(ty), 1);
    }
    deepest.interface.values.push(DesignType {
        qualified_name: "contract.Deep".into(),
        rust_type: "contract_types::Deep".into(),
        shape: DesignShape::Struct(vec![DesignField { name: "value".into(), ty: ty.clone() }]),
    });
    let mut invalid = deepest.clone();
    invalid.kind = "contract.invalid".into();
    let DesignShape::Struct(fields) = &mut invalid.interface.values.last_mut().unwrap().shape else {
        unreachable!()
    };
    fields[0].ty = DesignValueType::Array(Box::new(ty), 1);
    assert!(invalid.encode().is_err());
    let failing = ContractCatalog::from_value(ContractCatalog {
        extra_count: 0,
        ui: fixture.ui.clone(),
        calls: calls.clone(),
        entries: vec![
            CatalogEntry {
                bytes: deepest.encode().unwrap(),
                product: CatalogProduct::Button,
            },
            CatalogEntry {
                bytes: design::bounded_json::to_vec(&invalid, design::MAX_DESIGN_DESCRIPTOR_BYTES, design::bounded_json::Format::Compact).unwrap(),
                product: CatalogProduct::Button,
            },
        ],
    });
    assert!(registry.register_catalog(failing.query().unwrap()).is_err());
    assert_eq!(registry.kinds().collect::<Vec<_>>(), before.iter().map(String::as_str).collect::<Vec<_>>());
    assert_eq!(registry.interface("contract.original").unwrap(), &original.interface);
    let valid = ContractCatalog::from_value(ContractCatalog {
        extra_count: 0,
        ui: fixture.ui.clone(),
        calls: calls.clone(),
        entries: vec![CatalogEntry {
            bytes: deepest.encode().unwrap(),
            product: CatalogProduct::Button,
        }],
    });
    registry.register_catalog(valid.query().unwrap()).unwrap();
    assert_eq!(registry.interface("contract.deep").unwrap(), &deepest.interface);
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

/// Labels are portable presentation metadata; construction and fixed-tree edits resolve kind facts.
#[test]
fn custom_display_aliases_share_kind_admission_and_metadata_publication() {
    let fixture = Headless::standard();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = CustomWidgetRegistry::new();
    let catalog = ContractCatalog::from_value(ContractCatalog {
        extra_count: 0,
        ui: fixture.ui.clone(),
        calls: calls.clone(),
        entries: vec![
            CatalogEntry {
                bytes: contract_manifest("contract.button", CatalogProduct::Button).encode().unwrap(),
                product: CatalogProduct::Button,
            },
            CatalogEntry {
                bytes: contract_manifest("contract.silent", CatalogProduct::Text).encode().unwrap(),
                product: CatalogProduct::Text,
            },
        ],
    });
    registry.register_catalog(catalog.query().unwrap()).unwrap();
    let mut document = Document::new("custom labels");
    for (key, kind) in [("save", "contract.button"), ("title", "contract.silent")] {
        document.root.find_mut(key).unwrap().widget = registry.default_spec(kind).unwrap();
    }
    document.root.children.reverse(); // Counting Button precedes the silent Custom.
    document.outputs[0].name = "title".into(); // A label may equal another node's key.
    assert_eq!(Document::from_json(&document.to_json().unwrap()).unwrap(), document);
    assert_eq!(Document::from_binary(&document.to_binary().unwrap()).unwrap(), document);
    document.rust_bindings_with(&registry).unwrap();
    let (queue, _inbox) = neocogi_ui::events::channel();
    EventBindings::all(&document, &registry, queue.as_com_ref()).unwrap();
    let loader = Loader::new_with_custom(
        fixture.ui.clone(),
        None,
        Resources::new(),
        registry.clone(),
        GridLimits { max_columns: 64, max_rows: 256 },
    )
    .unwrap();
    let mut component = loader.compile(&document).unwrap();
    let root = component.root().unwrap();
    let mut save = component.widget::<ButtonAbi>("save").unwrap();
    save.set_label("live state").unwrap();
    let mut valid = document.clone();
    valid.name = "new presentation".into();
    valid.outputs[0].name = "another label".into();
    component.replace_metadata(&valid).unwrap();
    let published = component.save(&crate::test_support::host_runtime()).unwrap();
    let mut invalid = valid.clone();
    invalid.outputs[0].node = "title".into();
    assert!(
        Document::from_binary(&invalid.to_binary().unwrap()).is_ok(),
        "pure persistence cannot resolve custom kind events"
    );
    assert!(component.replace_metadata(&invalid).is_err());
    assert_eq!(component.save(&crate::test_support::host_runtime()).unwrap(), published);
    assert!(root.as_interface().same_object(component.root().unwrap().as_interface()));
    assert_eq!(save.label().unwrap(), "live state");
    calls.store(0, Ordering::Relaxed);
    assert!(loader.compile(&invalid).is_err());
    assert!(loader.load(&invalid.to_binary().unwrap()).is_err());
    assert!(invalid.rust_bindings_with(&registry).is_err());
    assert!(EventBindings::all(&invalid, &registry, queue.as_com_ref()).is_err());
    assert_eq!(
        calls.load(Ordering::Relaxed),
        0,
        "a later silent alias must reject before the earlier Button constructor"
    );
    let mut unknown = document.clone();
    unknown.root.find_mut("save").unwrap().widget = WidgetSpec::Custom {
        kind: "contract.missing".into(),
        settings: vec![],
    };
    assert!(loader.compile(&unknown).is_err());
    assert!(unknown.rust_bindings_with(&registry).is_err());
    assert!(EventBindings::all(&unknown, &registry, queue.as_com_ref()).is_err());
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    invalid.outputs.clear();
    loader.compile(&invalid).unwrap(); // Silent Custom remains legal without an alias.
    assert_eq!(calls.load(Ordering::Relaxed), 2);
}

/// A real COM catalog whose constructor calls can be observed independently of
/// factory lifetime. Atomics are test instrumentation shared with a COM receiver.
#[neocogi_com::com_object(interfaces(neocogi_ui_document::contracts::IDesignWidgetCatalog))]
struct BudgetCatalog {
    /// Runtime services used to create the advertised built-in concrete product.
    ui: Ui,
    /// Prevalidated descriptor returned through the ordinary two-call COM protocol.
    description: Vec<u8>,
    /// Counts entry, so even a rejected provider invocation would fail admission tests.
    calls: Arc<AtomicUsize>,
}
impl neocogi_ui_document::contracts::IDesignWidgetCatalog for BudgetCatalog {
    /// This test provider offers construction/metadata only, without a settings editor.
    fn create_editor(
        &self,
        _: u32,
        _: neocogi_com::ComRef<'_, neocogi_ui::UiFactoryAbi>,
        _: neocogi_com::NcCopyFromBufferOffset<'_, u8>,
    ) -> Result<neocogi_com::ComBox<neocogi_ui_document::contracts::IDesignSettingsEditorAbi>, neocogi_com::NcStatus> {
        Err(neocogi_com::NC_E_NOINTERFACE)
    }

    /// This fixture exposes exactly one independently discoverable kind.
    fn widget_count(&self) -> Result<u32, NcStatus> {
        Ok(1)
    }

    /// Copies the same stable descriptor on length query and subsequent read.
    fn describe(&self, index: u32, mut output: neocogi_com::NcOutputWriter<'_, u8>) -> Result<u32, NcStatus> {
        if index != 0 || output.offset() != 0 {
            return Err(neocogi_com::NC_E_INVALID_ARG);
        }
        if output.capacity() >= self.description.len() {
            output.extend_from_slice(&self.description)?;
        }
        Ok(self.description.len() as u32)
    }

    /// Crosses real generated thunks; any entry is counted before validating input.
    fn create_widget(
        &self,
        index: u32,
        _host: neocogi_com::ComRef<'_, IWidgetHostAbi>,
        settings: neocogi_com::NcCopyFromBufferOffset<'_, u8>,
        _events: neocogi_com::NcValueRef<'_, neocogi_ui::events::EventBinding>,
    ) -> Result<neocogi_com::ComBox<IWidgetAbi>, NcStatus> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        if index != 0 || settings.offset() != 0 {
            return Err(neocogi_com::NC_E_INVALID_ARG);
        }
        let text: String = design::decode_settings(settings.as_slice()).map_err(|_| neocogi_com::NC_E_INVALID_ARG)?;
        self.ui
            .widgets()
            .text_block(&text, TextWrapping::None)?
            .into_interface()
            .query::<IWidgetAbi>()
            .map_err(|(status, _owner)| status)
    }
}

/// Registers through COM discovery, retaining only the observer used for assertions.
fn catalog_loader(ui: &Ui) -> (Loader, Document, Arc<AtomicUsize>) {
    let settings = design::encode_settings("").unwrap();
    let manifest = DesignWidgetManifest::from_source("test.text", settings.clone(), &TextBlockAbi::EVENT_SOURCE).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let catalog = BudgetCatalog::from_value(BudgetCatalog {
        ui: ui.clone(),
        description: manifest.encode().unwrap(),
        calls: calls.clone(),
    });
    let mut registry = CustomWidgetRegistry::new();
    registry.register_catalog(catalog.query().unwrap()).unwrap();
    assert!(registry.create_editor("test.text", ui, &settings).unwrap().is_none());
    let loader = Loader::new_with_custom(ui.clone(), None, Resources::new(), registry, GridLimits { max_columns: 64, max_rows: 256 }).unwrap();
    let document = Document {
        version: VERSION,
        name: "catalog admission".into(),
        outputs: vec![],
        root: Node::new("custom", WidgetSpec::Custom { kind: "test.text".into(), settings }),
    };
    (loader, document, calls)
}

/// Whole-document and custom-property admission must precede any provider entry,
/// not just validate each node immediately before its own construction.
#[test]
fn oversized_typed_recipes_and_settings_never_enter_catalog_constructors() {
    let fixture = Headless::standard();
    let (loader, mut document, calls) = catalog_loader(&fixture.ui);
    let valid = loader.compile(&document).unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    let bytes = valid.save(&crate::test_support::host_runtime()).unwrap();
    assert_eq!(Document::from_binary(&bytes).unwrap(), document);
    loader.load(&bytes).unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 2);

    document.name = "x".repeat(2 * 1024 * 1024);
    let measured = crate::test_support::AllocationMeasurement::begin();
    let rejected = loader.compile(&document);
    let allocations = measured.finish();
    assert!(rejected.is_err());
    assert!(allocations.bytes < 16 * 1024, "rejection copied recipe: {allocations:?}");
    assert_eq!(calls.load(Ordering::Relaxed), 2);

    document.name.clear();
    if let WidgetSpec::Custom { settings, .. } = &mut document.root.widget {
        settings.resize(design::MAX_SETTINGS_BYTES + 1, 0);
    }
    let measured = crate::test_support::AllocationMeasurement::begin();
    let rejected = loader.compile(&document);
    let allocations = measured.finish();
    assert!(rejected.is_err());
    assert!(allocations.bytes < 16 * 1024, "rejection encoded settings: {allocations:?}");
    assert_eq!(calls.load(Ordering::Relaxed), 2);
}

/// Both export formats reject large borrowed fields with only small diagnostics
/// and bounded structural indexes; they do not materialize rejected output buffers.
#[test]
fn oversized_document_exports_do_not_allocate_proportional_buffers() {
    let mut document = Document::new("small");
    for bytes in [2 * 1024 * 1024, MAX_BYTES + 1] {
        document.name = "x".repeat(bytes);
        let measured = crate::test_support::AllocationMeasurement::begin();
        let rejected = document.to_binary();
        let allocations = measured.finish();
        assert!(rejected.is_err());
        assert!(allocations.bytes < 16 * 1024, "binary allocated a rejected buffer: {allocations:?}");
    }
    let measured = crate::test_support::AllocationMeasurement::begin();
    let rejected = document.to_json();
    let allocations = measured.finish();
    assert!(rejected.is_err());
    assert!(allocations.bytes < 16 * 1024, "JSON allocated a rejected buffer: {allocations:?}");
}

/// Exact binary admission survives real construction, snapshot and decoding; a
/// one-byte excess fails even though the document remains under the JSON ceiling.
#[test]
fn exact_binary_recipe_constructs_and_saves_but_next_byte_is_rejected() {
    let fixture = Headless::standard();
    let loader = Loader::new(fixture.ui.clone(), None, Resources::new(), GridLimits { max_columns: 64, max_rows: 256 }).unwrap();
    let mut document = Document::new("");
    let base = neocogi_com::binary::encoded_len_with_limits(&document, neocogi_com::binary::Limits::default()).unwrap();
    document.name = "x".repeat(1_000_000 - 21 - base);
    let accepted = loader.compile(&document).unwrap();
    assert_eq!(
        Document::from_binary(&accepted.save(&crate::test_support::host_runtime()).unwrap()).unwrap(),
        document
    );
    document.name.push('x');
    assert!(loader.compile(&document).is_err());
}

/// Rejected resource admission cannot replace the active recipe, generation or
/// mounted widgets. Candidate field-binding is never called on that failure.
#[test]
fn unsaveable_reload_preserves_published_form_and_queued_identity() {
    let fixture = Headless::standard();
    let root = fixture.ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let parent = window(&fixture.ui, "admission", Rect { x: 0, y: 0, width: 100, height: 100 }, root);
    let mut ids = ReloadIds::new();
    let mut slot = ReloadSlot::new(&mut ids, fixture.ui.clone(), parent, GridLimits { max_columns: 64, max_rows: 256 }).unwrap();
    let mut document = Document::new("accepted");
    let first = slot.reload(&document, Resources::new(), |_| Ok(EventBindings::new()), |_, _| Ok(())).unwrap();
    let mut title = slot.component().unwrap().widget::<TextBlockAbi>("title").unwrap();
    let saved = slot.component().unwrap().save(&crate::test_support::host_runtime()).unwrap();
    document.name = "x".repeat(2 * 1024 * 1024);
    assert!(
        slot.reload(
            &document,
            Resources::new(),
            |_| Ok(EventBindings::new()),
            |_, _| panic!("rejected recipe reached binding")
        )
        .is_err()
    );
    assert!(slot.accepts(first.generation));
    assert_eq!(slot.component().unwrap().save(&crate::test_support::host_runtime()).unwrap(), saved);
    title.set_text("still live").unwrap();
}
