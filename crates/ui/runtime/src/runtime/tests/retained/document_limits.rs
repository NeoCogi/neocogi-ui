//! Admission tests use the real loader, generated catalog calls and allocation counters.
use super::*;
use neocogi_ui_document::{*, design::{CustomValue, DesignWidgetManifest}};
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

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
    /// This fixture exposes exactly one independently discoverable kind.
    fn widget_count(&self) -> Result<u32, NcStatus> { Ok(1) }

    /// Copies the same stable descriptor on length query and subsequent read.
    fn describe(&self, index: u32, mut output: neocogi_com::NcCopyToBufferOffset<'_, u8>) -> Result<u32, NcStatus> {
        if index != 0 || output.offset() != 0 { return Err(neocogi_com::NC_E_INVALID_ARG); }
        if output.len() >= self.description.len() {
            output.as_mut_slice()[..self.description.len()].copy_from_slice(&self.description);
        }
        Ok(self.description.len() as u32)
    }

    /// Crosses real generated thunks; any entry is counted before validating input.
    fn create_widget(
        &self, index: u32, _host: neocogi_com::ComRef<'_, IWidgetHostAbi>,
        properties: neocogi_com::NcCopyFromBufferOffset<'_, u8>,
        _events: neocogi_com::NcValueRef<'_, contracts::WidgetEventBinding>,
    ) -> Result<neocogi_com::ComBox<IWidgetAbi>, NcStatus> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        if index != 0 || properties.offset() != 0 { return Err(neocogi_com::NC_E_INVALID_ARG); }
        let properties = design::decode_properties(properties.as_slice()).map_err(|_| neocogi_com::NC_E_INVALID_ARG)?;
        let Some(CustomProperty { value: CustomValue::Text(text), .. }) = properties.first() else {
            return Err(neocogi_com::NC_E_INVALID_ARG);
        };
        self.ui.widgets().text_block(text, TextWrapping::None)?.into_interface()
            .query::<IWidgetAbi>().map_err(|(status, _owner)| status)
    }
}

/// Registers through COM discovery, retaining only the observer used for assertions.
fn catalog_loader(ui: &Ui) -> (Loader, Document, Arc<AtomicUsize>) {
    let properties = vec![
        CustomProperty { name: "first".into(), value: CustomValue::Text(String::new()) },
        CustomProperty { name: "second".into(), value: CustomValue::Text(String::new()) },
    ];
    let manifest = DesignWidgetManifest::from_idl("test.text", properties.clone(), &TextBlockAbi::IDL_INTERFACE).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let catalog = BudgetCatalog::from_value(BudgetCatalog { ui: ui.clone(), description: manifest.encode().unwrap(), calls: calls.clone() });
    let mut registry = CustomWidgetRegistry::new();
    registry.register_catalog(catalog.query().unwrap()).unwrap();
    let loader = Loader::new_with_custom(ui.clone(), None, Resources::new(), registry, GridLimits { max_columns: 64, max_rows: 256 }).unwrap();
    let document = Document {
        version: VERSION, name: "catalog admission".into(), outputs: vec![],
        root: Node::new("custom", WidgetSpec::Custom { kind: "test.text".into(), properties }),
    };
    (loader, document, calls)
}

/// Whole-document and custom-property admission must precede any provider entry,
/// not just validate each node immediately before its own construction.
#[test]
fn oversized_typed_recipes_and_properties_never_enter_catalog_constructors() {
    let fixture = Headless::standard();
    let (loader, mut document, calls) = catalog_loader(&fixture.ui);
    let valid = loader.compile(&document).unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    let bytes = valid.save().unwrap();
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
    if let WidgetSpec::Custom { properties, .. } = &mut document.root.widget {
        for property in properties { property.value = CustomValue::Text("x".repeat(40_000)); }
    }
    let measured = crate::test_support::AllocationMeasurement::begin();
    let rejected = loader.compile(&document);
    let allocations = measured.finish();
    assert!(rejected.is_err());
    assert!(allocations.bytes < 16 * 1024, "rejection encoded properties: {allocations:?}");
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
    assert_eq!(Document::from_binary(&accepted.save().unwrap()).unwrap(), document);
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
    let saved = slot.component().unwrap().save().unwrap();
    document.name = "x".repeat(2 * 1024 * 1024);
    assert!(slot.reload(&document, Resources::new(), |_| Ok(EventBindings::new()), |_, _| panic!("rejected recipe reached binding")).is_err());
    assert!(slot.accepts(first.generation));
    assert_eq!(slot.component().unwrap().save().unwrap(), saved);
    title.set_text("still live").unwrap();
}
