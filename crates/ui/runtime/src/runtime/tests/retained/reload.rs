//! Real loaded controls exercise publication, binding, authored defaults, and stale handles.
use super::*;
use neocogi_ui_document::{*, contracts::SetText};
use neocogi_com_events::bind_output;

/// Mounts a document slot in an ordinary window using the production runtime.
fn slot(ui: &Ui) -> ReloadSlot {
    let mut root = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let owner = window(ui, "runtime documents", Rect { x: 10, y: 10, width: 400, height: 400 }, &root);
    let slot = ReloadSlot::new(ui.clone(), owner).unwrap();
    root.insert(0, slot.widget(), LinearEdge::new(LayoutTrack::Flex { weight: 1.0 }))
        .unwrap()
        .finish()
        .unwrap();
    slot
}

/// Bad documents, unavailable resources and failed binding all preserve the published component.
#[test]
fn failed_reload_preserves_live_inputs_and_authored_save() {
    let mut fixture = Headless::standard();
    let mut slot = slot(&fixture.ui);
    let document = Document::new("test");
    let loaded = slot.reload(&document, Resources::new(), |_, _| Ok(())).unwrap();
    let mut title = slot.component().unwrap().input::<SetText>("title").unwrap();
    title.send(SetText { text: "Application value".into() }).unwrap();
    assert_eq!(Document::from_binary(&slot.component().unwrap().save().unwrap()).unwrap(), document);
    assert!(slot.component().unwrap().input::<contracts::SetChecked>("title").is_err());
    let mut invalid = document.clone();
    invalid.root.children[0].node.style.font = Some("not-a-real-font".into());
    assert!(slot.reload(&invalid, Resources::new(), |_, _| Ok(())).is_err());
    assert!(slot.accepts(loaded.generation));
    title.send(SetText { text: "Still alive".into() }).unwrap();
    assert!(slot.reload(&document, Resources::new(), |_, _| Err(neocogi_com::NC_E_INVALID_ARG)).is_err());
    title
        .send(SetText {
            text: "Still alive after binding failure".into(),
        })
        .unwrap();
    fixture.frame(VIEW).unwrap();
    let replacement = slot.reload(&document, Resources::new(), |_, _| Ok(())).unwrap();
    assert!(!slot.accepts(loaded.generation));
    assert!(slot.accepts(replacement.generation));
    assert!(title.send(SetText { text: "Stale input".into() }).is_err());
    fixture.frame(VIEW).unwrap();
}

/// Typed events retain their payload and application identity after a real pointer click and reload.
#[test]
fn outputs_rebind_and_queued_old_events_have_distinct_generations() {
    let mut fixture = Headless::standard();
    let mut slot = slot(&fixture.ui);
    let mut document = Document::new("button");
    document.root = Node::new("save", WidgetSpec::Button { text: "Save".into() });
    document.inputs.clear();
    let (sender, events) = std::sync::mpsc::channel();
    let bind = |component: &Component, generation| {
        bind_output(component.outputs()?.as_com_ref(), "save_requested", sender.clone(), move |_: Activation| {
            generation
        })
    };
    slot.reload(&document, Resources::new(), bind).unwrap();
    fixture.frame(VIEW).unwrap();
    pointer(&fixture.ui, Point { x: 30, y: 30 }, true);
    pointer(&fixture.ui, Point { x: 30, y: 30 }, false);
    fixture.frame(VIEW).unwrap();
    let old = events.try_recv().unwrap();
    assert!(slot.accepts(old));
    slot.reload(&document, Resources::new(), bind).unwrap();
    assert!(!slot.accepts(old));
    fixture.frame(VIEW).unwrap();
    pointer(&fixture.ui, Point { x: 30, y: 30 }, true);
    pointer(&fixture.ui, Point { x: 30, y: 30 }, false);
    fixture.frame(VIEW).unwrap();
    assert!(slot.accepts(events.try_recv().unwrap()));
}

/// Every declared built-in mapping must construct, render, save, and retire through the real providers.
#[test]
fn all_builtin_document_controls_construct_and_reload() {
    let mut fixture = Headless::standard();
    let mut slot = slot(&fixture.ui);
    let kinds = vec![
        WidgetSpec::Button { text: "b".into() },
        WidgetSpec::Checkbox { text: "c".into(), checked: true },
        WidgetSpec::Textbox { text: "edit".into() },
        WidgetSpec::TextBlock {
            text: "text".into(),
            wrapping: Wrapping::Word,
        },
        WidgetSpec::TextArea {
            text: "area".into(),
            wrapping: Wrapping::Character,
            scrolling: true,
        },
        WidgetSpec::Slider {
            value: 2.0,
            low: 0.0,
            high: 10.0,
            step: 1.0,
            precision: 1,
        },
        WidgetSpec::Number { value: 1.0, step: 1.0, precision: 1 },
        WidgetSpec::ColorSwatch { text: "color".into(), rgba: 0xff00ffff },
        WidgetSpec::ListItem { text: "item".into() },
        WidgetSpec::ListBox { text: "box".into() },
        WidgetSpec::Scrollbar {
            vertical: true,
            viewport: 10,
            content: 100,
            offset: 5,
        },
        WidgetSpec::Combo {
            labels: vec!["a".into(), "b".into()],
            selected: Some(0),
            placeholder: "select".into(),
        },
        WidgetSpec::Linear {
            direction: Direction::Down,
            cross: Cross::Stretch,
        },
        WidgetSpec::Grid {
            columns: vec![Track::Flex { weight: 1.0 }],
            rows: vec![Track::Content],
        },
        WidgetSpec::Split {
            vertical: false,
            position: 100,
            minimum_first: 20,
            minimum_second: 20,
            handle: 6,
        },
        WidgetSpec::Scroll { enabled: true },
        WidgetSpec::Disclosure {
            text: "Details".into(),
            tree: false,
            expanded: true,
        },
    ];
    for kind in kinds {
        let mut root = Node::new("root", kind);
        if root.widget.container() {
            let edge = match root.widget {
                WidgetSpec::Linear { .. } | WidgetSpec::Disclosure { .. } => Edge::Linear { main: Track::Content, cross: None },
                WidgetSpec::Grid { .. } => Edge::Grid { columns: 1, rows: 1 },
                _ => Edge::Pane,
            };
            root.children.push(Child {
                edge,
                node: Node::new("child", WidgetSpec::Button { text: "child".into() }),
            });
        }
        let name = root.widget.name();
        let document = Document {
            version: VERSION,
            name: name.into(),
            root,
            outputs: vec![],
            inputs: vec![],
        };
        slot.reload(&document, Resources::new(), |_, _| Ok(()))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        fixture.frame(VIEW).unwrap();
        assert_eq!(Document::from_binary(&slot.component().unwrap().save().unwrap()).unwrap(), document);
    }
}

/// Typed input contracts call silent setters and reject malformed replacement models.
#[test]
fn typed_inputs_validate_payloads_and_do_not_emit_user_events() {
    use neocogi_ui_document::contracts::{SetChecked, SetChoices, SetEnabled, SetExpanded, SetValue};
    let fixture = Headless::standard();
    let mut slot = slot(&fixture.ui);
    let cases = [
        (WidgetSpec::Checkbox { text: "check".into(), checked: false }, InputKind::Checked),
        (
            WidgetSpec::Slider {
                value: 1.0,
                low: 0.0,
                high: 10.0,
                step: 1.0,
                precision: 0,
            },
            InputKind::Value,
        ),
        (
            WidgetSpec::Combo {
                labels: vec!["one".into()],
                selected: Some(0),
                placeholder: "none".into(),
            },
            InputKind::Choices,
        ),
        (
            WidgetSpec::Disclosure {
                text: "details".into(),
                tree: false,
                expanded: false,
            },
            InputKind::Expanded,
        ),
    ];
    for (widget, kind) in cases {
        let document = Document {
            version: VERSION,
            name: "input".into(),
            root: Node::new("root", widget),
            outputs: vec![],
            inputs: vec![
                Input {
                    name: "set".into(),
                    node: "root".into(),
                    kind: kind.clone(),
                },
                Input {
                    name: "enabled".into(),
                    node: "root".into(),
                    kind: InputKind::Enabled,
                },
            ],
        };
        slot.reload(&document, Resources::new(), |_, _| Ok(())).unwrap();
        let component = slot.component().unwrap();
        component.input::<SetEnabled>("enabled").unwrap().send(SetEnabled { enabled: false }).unwrap();
        match kind {
            InputKind::Checked => component.input::<SetChecked>("set").unwrap().send(SetChecked { checked: true }).unwrap(),
            InputKind::Value => {
                let mut input = component.input::<SetValue>("set").unwrap();
                input.send(SetValue { value: 3.0 }).unwrap();
                assert!(input.send(SetValue { value: f32::NAN }).is_err());
            }
            InputKind::Choices => {
                let mut input = component.input::<SetChoices>("set").unwrap();
                input
                    .send(SetChoices {
                        text: "onetwo".into(),
                        ends: vec![3, 6],
                        selected: 1,
                    })
                    .unwrap();
                assert!(
                    input
                        .send(SetChoices {
                            text: "é".into(),
                            ends: vec![1],
                            selected: 0
                        })
                        .is_err()
                );
            }
            InputKind::Expanded => component.input::<SetExpanded>("set").unwrap().send(SetExpanded { expanded: true }).unwrap(),
            _ => unreachable!(),
        }
        assert_eq!(Document::from_binary(&component.save().unwrap()).unwrap(), document);
    }
}

/// Programmatic enablement must not freeze effective wheel eligibility into authored option bits.
#[test]
fn typed_enablement_preserves_authored_options_without_capturing_dynamic_routing() {
    let fixture = Headless::standard();
    let mut slot = slot(&fixture.ui);
    let mut root = Node::new("scroll", WidgetSpec::Scroll { enabled: true });
    root.children.push(Child {
        edge: Edge::Pane,
        node: Node::new("content", WidgetSpec::Button { text: "content".into() }),
    });
    let document = Document {
        version: VERSION,
        name: "routing".into(),
        root,
        outputs: vec![],
        inputs: vec![Input {
            name: "enabled".into(),
            node: "scroll".into(),
            kind: InputKind::Enabled,
        }],
    };
    slot.reload(&document, Resources::new(), |_, _| Ok(())).unwrap();
    let component = slot.component().unwrap();
    let root = component.root().unwrap();
    assert!(root.as_interface().properties().unwrap().options.contains(WidgetOptions::GRAB_SCROLL));
    component
        .input::<contracts::SetEnabled>("enabled")
        .unwrap()
        .send(contracts::SetEnabled { enabled: true })
        .unwrap();
    root.as_interface().add_ref().query::<IScrollingAbi>().unwrap().set_enabled(false).unwrap();
    assert!(!root.as_interface().properties().unwrap().options.contains(WidgetOptions::GRAB_SCROLL));
}

/// A fixed-coordinate form uses the same geometry for rendering and pointer routing.
#[test]
fn authored_form_rectangles_survive_binary_reload_and_route_real_clicks() {
    let mut fixture = Headless::standard();
    let mut slot = slot(&fixture.ui);
    let mut document = Document::new("fixed form");
    document.root = Node::new("form", WidgetSpec::Form { width: 360, height: 300 });
    document.root.children.push(Child {
        edge: Edge::Position { x: 120, y: 80, width: 100, height: 32 },
        node: Node::new("save", WidgetSpec::Button { text: "Save".into() }),
    });
    document.inputs.clear();
    let bytes = document.to_binary().unwrap();
    assert_eq!(Document::from_binary(&bytes).unwrap(), document);
    let (sender, events) = std::sync::mpsc::channel();
    slot.reload(&document, Resources::new(), |component, generation| {
        bind_output(component.outputs()?.as_com_ref(), "save_requested", sender, move |_: Activation| generation)
    }).unwrap();
    fixture.frame(VIEW).unwrap();
    // The first point is inside the form but outside its authored button.
    pointer(&fixture.ui, Point { x: 30, y: 30 }, true);
    pointer(&fixture.ui, Point { x: 30, y: 30 }, false);
    fixture.frame(VIEW).unwrap();
    assert!(events.try_recv().is_err());
    pointer(&fixture.ui, Point { x: 160, y: 120 }, true);
    pointer(&fixture.ui, Point { x: 160, y: 120 }, false);
    fixture.frame(VIEW).unwrap();
    assert!(slot.accepts(events.try_recv().unwrap()));
    // Bad arithmetic must be rejected before it can replace the live component.
    document.root.children[0].edge = Edge::Position { x: i32::MAX, y: 0, width: 2, height: 20 };
    assert!(slot.reload(&document, Resources::new(), |_, _| Ok(())).is_err());
    assert_eq!(slot.component().unwrap().save().unwrap(), bytes);
}
