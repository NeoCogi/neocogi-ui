//! Persistence and validation tests use actual authored shapes, including adversarial bytes.
use neocogi_ui_document::*;

/// Text edits preserve widget keys and event aliases across both representations.
#[test]
fn authored_documents_round_trip_both_formats() {
    let mut document = Document::new("Editable form");
    document.root.find_mut("title").unwrap().style.font = Some("body".into());
    let json = document.to_json().unwrap();
    assert_eq!(Document::from_json(&json).unwrap(), document);
    let binary = document.to_binary().unwrap();
    assert_eq!(Document::from_binary(&binary).unwrap(), document);
    for end in 0..binary.len() {
        assert!(Document::from_binary(&binary[..end]).is_err());
    }
    let mut trailing = binary;
    trailing.push(0);
    assert!(Document::from_binary(&trailing).is_err());
}

/// Invalid names, topology and event aliases fail before creating any widget.
#[test]
fn invalid_names_layouts_and_event_aliases_are_rejected() {
    let mut document = Document::new("test");
    document.root.children[1].node.key = "title".into();
    assert!(document.validate().is_err());
    let mut document = Document::new("test");
    document.outputs[0].node = "title".into();
    assert!(document.validate().is_err());
    let mut document = Document::new("test");
    document.root.children[0].edge = Edge::Pane;
    assert!(document.validate().is_err());
    let mut document = Document::new("test");
    document.outputs.push(Output {
        name: "other".into(),
        ..document.outputs[0].clone()
    });
    assert!(document.validate().is_err(), "a child may expose only one public event-stream name");
    let mut document = Document::new("test");
    document.root.style.options = Some(u16::MAX);
    assert!(document.validate().is_err());
}

/// Untrusted numeric settings and excessive nesting have explicit finite bounds.
#[test]
fn arithmetic_versions_and_tree_depth_are_bounded() {
    let mut document = Document::new("test");
    document.version += 1;
    assert!(document.validate().is_err());
    let mut document = Document::new("test");
    document.root.children[0].edge = Edge::Linear {
        main: Track::Flex { weight: f32::NAN },
        cross: None,
    };
    assert!(document.validate().is_err());
    let mut document = Document::new("test");
    for i in 0..MAX_DEPTH {
        let child = document.root;
        let mut parent = Node::new(format!("level{i}"), WidgetSpec::Scroll { enabled: true });
        parent.children.push(Child { edge: Edge::Pane, node: child });
        document.root = parent;
    }
    assert!(document.validate().is_err());
}

/// Authoring capabilities reject unsupported resources before runtime construction.
#[test]
fn style_capabilities_match_document_validation() {
    for (widget, font, icon, image) in [
        (WidgetSpec::Button { text: String::new() }, true, true, true),
        (WidgetSpec::ListItem { text: String::new() }, true, true, false),
        (WidgetSpec::ListBox { text: String::new() }, true, false, true),
        (WidgetSpec::Form { width: 100, height: 100 }, false, false, false),
    ] {
        let mut document = Document::new("capabilities");
        document.outputs.clear();
        document.root.children.clear();
        document.root.widget = widget;
        assert_eq!(document.root.widget.supports_font(), font);
        assert_eq!(document.root.widget.supports_icon(), icon);
        assert_eq!(document.root.widget.supports_image(), image);
        document.root.style.font = Some("body".into());
        assert_eq!(document.validate().is_ok(), font);
        document.root.style.font = None;
        document.root.style.icon = Some("accept".into());
        assert_eq!(document.validate().is_ok(), icon);
        document.root.style.icon = None;
        document.root.style.image = Some("image.png".into());
        assert_eq!(document.validate().is_ok(), image);
    }
}

/// Authored source contains no event graph; applications implement event behavior.
#[test]
fn visual_connections_are_not_part_of_the_document_schema() {
    let document = Document::new("application handlers");
    let mut source: serde_json::Value = serde_json::from_slice(&document.to_json().unwrap()).unwrap();
    assert!(source.get("connections").is_none());
    source["connections"] = serde_json::json!([]);
    assert!(Document::from_json(&serde_json::to_vec(&source).unwrap()).is_err());
}

/// Removed command declarations fail explicitly; no loader may silently discard setter exports.
#[test]
fn removed_inputs_and_previous_document_revisions_are_rejected() {
    let document = Document::new("widget methods");
    let source: serde_json::Value = serde_json::from_slice(&document.to_json().unwrap()).unwrap();
    assert!(source.get("inputs").is_none());
    for obsolete in [serde_json::json!([]), serde_json::json!([
        {"name": "title", "node": "title", "kind": "Text"}
    ])] {
        let mut legacy = source.clone();
        legacy["inputs"] = obsolete;
        let error = Document::from_json(&serde_json::to_vec(&legacy).unwrap()).unwrap_err();
        assert!(error.contains("unknown field `inputs`"), "{error}");
    }
    let mut legacy = source;
    legacy["version"] = serde_json::json!(VERSION - 1);
    assert!(Document::from_json(&serde_json::to_vec(&legacy).unwrap()).unwrap_err().contains("Unsupported UI document revision"));

    // The persistence envelope and inner document each carry an independent
    // revision. Reject either mismatch before publishing any widget tree.
    use neocogi_com::{binary, persistence::ObjectRecord};
    let bytes = document.to_binary().unwrap();
    let mut envelope: ObjectRecord = binary::from_slice(&bytes).unwrap();
    envelope.schema_version = VERSION - 1;
    assert!(Document::from_binary(&binary::to_vec(&envelope).unwrap()).unwrap_err().contains("Unsupported UI component recipe or revision"));
    envelope.schema_version = VERSION;
    let mut legacy = document;
    legacy.version = VERSION - 1;
    envelope.state = binary::to_vec(&legacy).unwrap();
    assert!(Document::from_binary(&binary::to_vec(&envelope).unwrap()).unwrap_err().contains("Unsupported UI document revision"));
}

/// The outer state's Vec<u8> is charged as values, even when inner text fits its
/// byte policy. Independent exact counts detect a return of the old bypass.
#[test]
fn binary_admission_includes_outer_state_and_matches_saved_round_trip() {
    let mut document = Document::new("");
    let base = neocogi_com::binary::encoded_len_with_limits(&document, neocogi_com::binary::Limits::default()).unwrap();
    // Envelope: one record, UUID sequence + 16 bytes, revision, state sequence,
    // and empty children sequence = 21 values before the state's byte values.
    document.name = "x".repeat(1_000_000 - 21 - base);
    document.validate_binary().unwrap();
    let bytes = document.to_binary().unwrap();
    assert_eq!(Document::from_binary(&bytes).unwrap(), document);
    document.name.push('x');
    document.validate().unwrap(); // Shape and JSON policy remain distinct.
    assert!(document.to_json().is_ok());
    assert!(document.validate_binary().is_err());
    assert!(document.to_binary().is_err());
}

/// Whole-document accounting cannot be evaded through multiple moderate strings;
/// already allocated caller inputs do not authorize a second unbounded copy.
#[test]
fn binary_admission_charges_aggregate_fields_and_portable_text_growth() {
    let mut document = Document::new("");
    document.name = "a".repeat(600_000);
    document.root.children[0].node.widget = WidgetSpec::TextBlock { text: "b".repeat(600_000), wrapping: Wrapping::None };
    assert!(document.validate_binary().is_err());
    document.name.clear();
    document.validate_binary().unwrap();
    assert!(document.validate_binary_with_text_reserve(600_000).is_err());
    assert!(document.validate_binary_with_text_reserve(usize::MAX).is_err());
    document.validate_binary_with_text_reserve(1).unwrap();
}

/// Pretty JSON has its own symmetric byte envelope. Exact acceptance uses actual
/// escaping, not a sum of Rust string lengths, and never changes binary policy.
#[test]
fn json_byte_limit_counts_escaped_text_and_aggregate_overhead() {
    let mut document = Document::new("");
    let base = document.to_json().unwrap().len();
    let available = MAX_BYTES - base;
    document.name = "\n".repeat(available / 2);
    if available % 2 != 0 { document.name.push('x'); }
    let exact = document.to_json().unwrap();
    assert_eq!(exact.len(), MAX_BYTES);
    assert_eq!(Document::from_json(&exact).unwrap(), document);
    document.name.push('x');
    assert!(document.to_json().is_err());
    let mut excessive = exact;
    excessive.push(b' ');
    assert!(Document::from_json(&excessive).is_err());
}
