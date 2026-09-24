//! Persistence and validation tests use actual authored shapes, including adversarial bytes.
use neocogi_ui_document::*;

/// Text edits preserve private keys and public application names across both representations.
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

/// Invalid public contracts fail before creating any concrete receiver or widget.
#[test]
fn invalid_names_layouts_and_bindings_are_rejected() {
    let mut document = Document::new("test");
    document.root.children[1].node.key = "title".into();
    assert!(document.validate().is_err());
    let mut document = Document::new("test");
    document.outputs[0].node = "title".into();
    assert!(document.validate().is_err());
    let mut document = Document::new("test");
    document.inputs[0].kind = InputKind::Checked;
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
        document.inputs.clear();
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
