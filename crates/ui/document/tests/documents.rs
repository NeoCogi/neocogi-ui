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
