//! Real loaded controls exercise publication, binding, authored defaults, and stale handles.
use super::*;
use neocogi_ui_document::{*, contracts::SetText};

/// Prepares the single constructor binding used by these behavioral fixtures.
fn receiver<C: neocogi_com::events::EventSource, M: 'static>(
    key: &str,
    sender: std::sync::mpsc::Sender<M>,
    map: impl FnMut(neocogi_com::events::EventOf<C>) -> M + 'static,
) -> Result<EventBindings, NcStatus> {
    let mut events = EventBindings::new();
    events.insert::<C, M>(key, sender, map)?;
    Ok(events)
}

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
    let loaded = slot.reload(&document, Resources::new(), |_| Ok(EventBindings::new()), |_, _| Ok(())).unwrap();
    let mut title = slot.component().unwrap().input::<SetText>("title").unwrap();
    title.send(SetText { text: "Application value".into() }).unwrap();
    assert_eq!(Document::from_binary(&slot.component().unwrap().save().unwrap()).unwrap(), document);
    assert!(slot.component().unwrap().input::<contracts::SetChecked>("title").is_err());
    let mut invalid = document.clone();
    invalid.root.children[0].node.style.font = Some("not-a-real-font".into());
    assert!(slot.reload(&invalid, Resources::new(), |_| Ok(EventBindings::new()), |_, _| Ok(())).is_err());
    assert!(slot.accepts(loaded.generation));
    title.send(SetText { text: "Still alive".into() }).unwrap();
    assert!(
        slot.reload(
            &document,
            Resources::new(),
            |_| Ok(EventBindings::new()),
            |_, _| Err(neocogi_com::NC_E_INVALID_ARG)
        )
        .is_err()
    );
    title
        .send(SetText {
            text: "Still alive after binding failure".into(),
        })
        .unwrap();
    fixture.frame(VIEW).unwrap();
    let replacement = slot.reload(&document, Resources::new(), |_| Ok(EventBindings::new()), |_, _| Ok(())).unwrap();
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
    let bind = |generation| receiver::<ButtonAbi, _>("save", sender.clone(), move |_: neocogi_ui::button::Event| generation);
    slot.reload(&document, Resources::new(), bind, |_, _| Ok(())).unwrap();
    fixture.frame(VIEW).unwrap();
    pointer(&fixture.ui, Point { x: 30, y: 30 }, true);
    pointer(&fixture.ui, Point { x: 30, y: 30 }, false);
    fixture.frame(VIEW).unwrap();
    let old = events.try_recv().unwrap();
    assert!(slot.accepts(old));
    slot.reload(&document, Resources::new(), bind, |_, _| Ok(())).unwrap();
    assert!(!slot.accepts(old));
    fixture.frame(VIEW).unwrap();
    pointer(&fixture.ui, Point { x: 30, y: 30 }, true);
    pointer(&fixture.ui, Point { x: 30, y: 30 }, false);
    fixture.frame(VIEW).unwrap();
    assert!(slot.accepts(events.try_recv().unwrap()));
}

/// Every built-in must construct, bind all declared ports/commands, render, save, and retire.
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
        WidgetSpec::Form { width: 200, height: 200 },
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
                WidgetSpec::Form { .. } => Edge::Position { x: 0, y: 0, width: 100, height: 30 },
                _ => Edge::Pane,
            };
            root.children.push(Child {
                edge,
                node: Node::new("child", WidgetSpec::Button { text: "child".into() }),
            });
        }
        let name = root.widget.name();
        let outputs = root
            .widget
            .event_stream()
            .into_iter()
            .map(|_| Output {
                name: "event".into(),
                node: "root".into(),
            })
            .collect();
        let inputs = InputKind::ALL
            .into_iter()
            .filter(|kind| root.widget.accepts(kind))
            .map(|kind| Input {
                name: kind.label().into(),
                node: "root".into(),
                kind,
            })
            .collect();
        let document = Document {
            version: VERSION,
            name: name.into(),
            root,
            outputs,
            inputs,
        };
        slot.reload(
            &document,
            Resources::new(),
            |_| {
                let (sender, _events) = std::sync::mpsc::channel::<()>();
                macro_rules! bind {
                    ($abi:ty) => {
                        receiver::<$abi, _>(&document.root.key, sender.clone(), |_| ())
                    };
                }
                match &document.root.widget {
                    WidgetSpec::Button { .. } => bind!(ButtonAbi),
                    WidgetSpec::Checkbox { .. } => bind!(CheckboxAbi),
                    WidgetSpec::Textbox { .. } => bind!(TextboxAbi),
                    WidgetSpec::TextArea { .. } => bind!(TextAreaAbi),
                    WidgetSpec::Slider { .. } => bind!(SliderAbi),
                    WidgetSpec::Number { .. } => bind!(NumberAbi),
                    WidgetSpec::ListItem { .. } => bind!(ListItemAbi),
                    WidgetSpec::ListBox { .. } => bind!(ListBoxAbi),
                    WidgetSpec::Scrollbar { .. } => bind!(ScrollbarAbi),
                    WidgetSpec::Combo { .. } => bind!(ComboAbi),
                    WidgetSpec::Split { .. } => bind!(SplitViewAbi),
                    _ => Ok(EventBindings::new()),
                }
            },
            |component, _| {
                use neocogi_ui_document::contracts::*;
                // The declared capability list must resolve to actual typed COM receivers, not merely
                // pass schema validation. Each concrete contract has its own generated signature.
                for input in &document.inputs {
                    match input.kind {
                        InputKind::Text => component.input::<SetText>(&input.name).map(drop),
                        InputKind::Checked => component.input::<SetChecked>(&input.name).map(drop),
                        InputKind::Value => component.input::<SetValue>(&input.name).map(drop),
                        InputKind::Enabled => component.input::<SetEnabled>(&input.name).map(drop),
                        InputKind::Expanded => component.input::<SetExpanded>(&input.name).map(drop),
                        InputKind::Choices => component.input::<SetChoices>(&input.name).map(drop),
                        InputKind::Selection => component.input::<SetSelection>(&input.name).map(drop),
                        InputKind::Offset => component.input::<SetOffset>(&input.name).map(drop),
                        InputKind::Position => component.input::<SetPosition>(&input.name).map(drop),
                    }?;
                }
                Ok(())
            },
        )
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
        slot.reload(&document, Resources::new(), |_| Ok(EventBindings::new()), |_, _| Ok(())).unwrap();
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

/// Enablement changes only interaction intent, preserving application edits made after construction.
#[test]
fn typed_enablement_preserves_live_application_flags() {
    let fixture = Headless::standard();
    let mut slot = slot(&fixture.ui);
    let document = Document {
        version: VERSION,
        name: "live flags".into(),
        root: Node::new("button", WidgetSpec::Button { text: "toggle".into() }),
        outputs: vec![],
        inputs: vec![Input {
            name: "enabled".into(),
            node: "button".into(),
            kind: InputKind::Enabled,
        }],
    };
    slot.reload(&document, Resources::new(), |_| Ok(EventBindings::new()), |_, _| Ok(())).unwrap();
    let component = slot.component().unwrap();
    let mut button = component.widget::<ButtonAbi>("button").unwrap();
    let mut enabled = component.input::<contracts::SetEnabled>("enabled").unwrap();
    assert_eq!(button.as_interface().properties().unwrap().options, WidgetOptions::FRAME);

    // A later application edit removes an initial bit and adds a different bit.
    // Disabling must preserve both decisions rather than restoring authored defaults.
    button.replace_flags(WidgetOptions::ALIGN_RIGHT).unwrap();
    enabled.send(contracts::SetEnabled { enabled: false }).unwrap();
    assert_eq!(
        button.as_interface().properties().unwrap().options,
        WidgetOptions::ALIGN_RIGHT | WidgetOptions::NO_INTERACT
    );

    // The application may also edit presentation while the control is disabled.
    // Enabling changes only NO_INTERACT and repeated enablement is idempotent.
    button
        .replace_flags(WidgetOptions::FRAME | WidgetOptions::ALIGN_CENTER | WidgetOptions::NO_INTERACT)
        .unwrap();
    for _ in 0..2 {
        enabled.send(contracts::SetEnabled { enabled: true }).unwrap();
        assert_eq!(
            button.as_interface().properties().unwrap().options,
            WidgetOptions::FRAME | WidgetOptions::ALIGN_CENTER
        );
    }
}

/// Selective enablement preserves live presentation edits and never stores derived input flags.
#[test]
fn typed_enablement_preserves_requested_flags_without_capturing_effective_flags() {
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
    slot.reload(&document, Resources::new(), |_| Ok(EventBindings::new()), |_, _| Ok(())).unwrap();
    let component = slot.component().unwrap();
    let root = component.root().unwrap();
    let mut common = root.as_interface().add_ref();
    assert_eq!(common.properties().unwrap().options, WidgetOptions::FRAME | WidgetOptions::GRAB_SCROLL);
    common.clear_flags(WidgetOptions::FRAME).unwrap();
    common.set_flags(WidgetOptions::ALIGN_RIGHT).unwrap();
    let mut enabled = component.input::<contracts::SetEnabled>("enabled").unwrap();
    for value in [false, true] {
        enabled.send(contracts::SetEnabled { enabled: value }).unwrap();
        let expected = match value {
            true => WidgetOptions::ALIGN_RIGHT | WidgetOptions::GRAB_SCROLL,
            false => WidgetOptions::ALIGN_RIGHT | WidgetOptions::GRAB_SCROLL | WidgetOptions::NO_INTERACT,
        };
        assert_eq!(common.properties().unwrap().options, expected);
    }

    // Scrolling's own model contributes temporary input policy independently of
    // requested surface flags. Disablement must remove wheel handling immediately.
    let mut scrolling = common.query::<IScrollingAbi>().unwrap();
    scrolling.set_enabled(false).unwrap();
    assert_eq!(common.properties().unwrap().options, WidgetOptions::ALIGN_RIGHT | WidgetOptions::NO_INTERACT);

    // The reverse transition must not retain the model's temporary NO_INTERACT.
    // An enable command edits intent only; a disabled scroll model remains inert.
    enabled.send(contracts::SetEnabled { enabled: true }).unwrap();
    assert_eq!(common.properties().unwrap().options, WidgetOptions::ALIGN_RIGHT | WidgetOptions::NO_INTERACT);
    scrolling.set_enabled(true).unwrap();
    assert_eq!(common.properties().unwrap().options, WidgetOptions::ALIGN_RIGHT | WidgetOptions::GRAB_SCROLL);
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
    slot.reload(
        &document,
        Resources::new(),
        |generation| receiver::<ButtonAbi, _>("save", sender, move |_: neocogi_ui::button::Event| generation),
        |_, _| Ok(()),
    )
    .unwrap();
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
    assert!(slot.reload(&document, Resources::new(), |_| Ok(EventBindings::new()), |_, _| Ok(())).is_err());
    assert_eq!(slot.component().unwrap().save().unwrap(), bytes);
}

/// Typed preparation and persistence decoding must construct the same authored component contract.
#[test]
fn typed_and_binary_preparation_have_equivalent_authored_state() {
    let fixture = Headless::standard();
    let document = Document::new("preparation paths");
    let loader = Loader::new(fixture.ui.clone(), None, Resources::new()).unwrap();
    let typed = loader.prepare(&document).unwrap();
    let binary = loader.load(&document.to_binary().unwrap()).unwrap();
    assert_ne!(
        typed.root().unwrap().as_interface().as_raw().object,
        binary.root().unwrap().as_interface().as_raw().object
    );
    for component in [&typed, &binary] {
        component
            .input::<SetText>("title")
            .unwrap()
            .send(SetText { text: "runtime override".into() })
            .unwrap();
        assert_eq!(Document::from_binary(&component.save().unwrap()).unwrap(), document);
    }
}

/// Focused commands preserve unrelated widget configuration, validate before mutation, and retire.
#[test]
fn selection_offset_and_position_commands_preserve_configuration_and_retire() {
    use neocogi_ui_document::contracts::{SetOffset, SetPosition, SetSelection};
    let fixture = Headless::standard();
    let mut slot = slot(&fixture.ui);
    let mut document = Document {
        version: VERSION,
        name: "focused commands".into(),
        root: Node::new(
            "root",
            WidgetSpec::Combo {
                labels: vec!["one".into(), "two".into()],
                selected: Some(0),
                placeholder: "empty".into(),
            },
        ),
        outputs: vec![Output {
            name: "changed".into(),
            node: "root".into(),
        }],
        inputs: vec![Input {
            name: "set".into(),
            node: "root".into(),
            kind: InputKind::Selection,
        }],
    };
    let (sender, events) = std::sync::mpsc::channel();
    slot.reload(
        &document,
        Resources::new(),
        |_| receiver::<ComboAbi, _>("root", sender.clone(), |_| ()),
        |_, _| Ok(()),
    )
    .unwrap();
    let component = slot.component().unwrap();
    let combo = Widget::from_interface(component.root().unwrap().as_interface().add_ref().query::<ComboAbi>().unwrap());
    let mut selection = component.input::<SetSelection>("set").unwrap();
    selection.send(SetSelection { selected: 1 }).unwrap();
    assert!(matches!(combo.selection().unwrap(), ChoiceSelection::Selected(1)));
    for selected in [-2, 2, i32::MAX] {
        assert!(selection.send(SetSelection { selected }).is_err());
        assert!(matches!(combo.selection().unwrap(), ChoiceSelection::Selected(1)));
    }
    selection.send(SetSelection { selected: -1 }).unwrap();
    assert!(matches!(combo.selection().unwrap(), ChoiceSelection::None));
    assert!(events.try_recv().is_err());

    document.root.widget = WidgetSpec::Scrollbar {
        vertical: true,
        viewport: 10,
        content: 100,
        offset: 5,
    };
    document.inputs[0].kind = InputKind::Offset;
    slot.reload(
        &document,
        Resources::new(),
        |_| receiver::<ScrollbarAbi, _>("root", sender.clone(), |_| ()),
        |_, _| Ok(()),
    )
    .unwrap();
    assert!(selection.send(SetSelection { selected: 0 }).is_err());
    let component = slot.component().unwrap();
    let scrollbar = component.root().unwrap().as_interface().add_ref().query::<ScrollbarAbi>().unwrap();
    let mut offset = component.input::<SetOffset>("set").unwrap();
    for (requested, expected) in [(50, 50), (i32::MAX, 90), (-1, 0)] {
        offset.send(SetOffset { offset: requested }).unwrap();
        let range = scrollbar.range().unwrap();
        assert_eq!((range.viewport, range.content, range.offset), (10, 100, expected));
    }
    assert!(events.try_recv().is_err());

    document.root.widget = WidgetSpec::Split {
        vertical: false,
        position: 100,
        minimum_first: 20,
        minimum_second: 30,
        handle: 6,
    };
    document.inputs[0].kind = InputKind::Position;
    slot.reload(
        &document,
        Resources::new(),
        |_| receiver::<SplitViewAbi, _>("root", sender.clone(), |_| ()),
        |_, _| Ok(()),
    )
    .unwrap();
    assert!(offset.send(SetOffset { offset: 0 }).is_err());
    let component = slot.component().unwrap();
    let mut split = component.root().unwrap().as_interface().add_ref().query::<SplitViewAbi>().unwrap();
    // Preserve live configuration as well as authored defaults when sending subsequent positions.
    let mut settings = split.settings().unwrap();
    settings.minimum_first = 40;
    split.set_settings(settings).unwrap();
    let mut position = component.input::<SetPosition>("set").unwrap();
    position.send(SetPosition { position: 150 }).unwrap();
    let settings = split.settings().unwrap();
    assert_eq!(
        (settings.position, settings.minimum_first, settings.minimum_second, settings.handle_width),
        (150, 40, 30, 6)
    );
    assert!(position.send(SetPosition { position: -1 }).is_err());
    assert_eq!(split.settings().unwrap().position, 150);
    assert!(events.try_recv().is_err());
    assert_eq!(Document::from_binary(&component.save().unwrap()).unwrap(), document);
    slot.reload(&document, Resources::new(), |_| Ok(EventBindings::new()), |_, _| Ok(())).unwrap();
    assert!(position.send(SetPosition { position: 100 }).is_err());
}

/// A self-targeting activation applies only after input traversal, without a reentrant borrow.
#[test]
fn application_handles_real_click_and_closed_form_retires() {
    use neocogi_ui::{Mount, Mountable, IEventOwnerAbi};
    let mut fixture = Headless::standard();
    let mut slot = slot(&fixture.ui);
    let mut document = Document::new("private action");
    document.root = Node::new("button", WidgetSpec::Button { text: "Before".into() });
    document.inputs.clear();
    document.outputs.clear();
    // Application behavior is queued explicitly; documents do not contain executable routes.
    let (sender, messages) = std::sync::mpsc::channel();
    let bytes = document.to_binary().unwrap();
    assert_eq!(Document::from_binary(&bytes).unwrap(), document);
    slot.reload(
        &document,
        Resources::new(),
        |_| receiver::<ButtonAbi, _>("button", sender.clone(), |_| ()),
        |_, _| Ok(()),
    )
    .unwrap();
    let component = slot.component().unwrap();
    let mut button = component.widget::<ButtonAbi>("button").unwrap();
    let mut owner: neocogi_com::ComPtr<IEventOwnerAbi> = match component.mount_description().unwrap() {
        Mount::Component { owner, .. } => owner,
        _ => panic!("document must retain its event owner"),
    };
    fixture.frame(VIEW).unwrap();
    pointer(&fixture.ui, Point { x: 30, y: 30 }, true);
    pointer(&fixture.ui, Point { x: 30, y: 30 }, false);
    fixture.frame(VIEW).unwrap();
    assert_eq!(button.label().unwrap(), "Before");
    messages.try_recv().unwrap();
    button.set_label("After").unwrap();
    assert_eq!(button.label().unwrap(), "After");
    assert_eq!(slot.component().unwrap().save().unwrap(), bytes);
    assert_eq!(owner.dispatch_next(), Ok(false));
    slot.reload(&document, Resources::new(), |_| Ok(EventBindings::new()), |_, _| Ok(())).unwrap();
    // Container closure queues retirement until the next safe runtime boundary.
    fixture.frame(VIEW).unwrap();
    assert_eq!(owner.dispatch_next(), Err(UI_E_CLOSED));
    assert!(button.label().is_err());
}

/// One application consumer receives the complete checkbox event and edits two typed fields.
#[test]
fn application_owns_checkbox_actions_and_complete_event_consumption() {
    let mut fixture = Headless::standard();
    let mut slot = slot(&fixture.ui);
    let mut document = Document::new("forwarded state");
    document.root = Node::new("form", WidgetSpec::Form { width: 300, height: 200 });
    for (key, y) in [("source", 0), ("target", 60), ("second", 120)] {
        document.root.children.push(Child {
            edge: Edge::Position { x: 0, y, width: 200, height: 40 },
            node: Node::new(key, WidgetSpec::Checkbox { text: key.into(), checked: false }),
        });
    }
    document.outputs = vec![Output {
        name: "source_changed".into(),
        node: "source".into(),
    }];
    document.inputs.clear();
    // One application handler may issue commands to multiple typed widget fields.
    let (sender, events) = std::sync::mpsc::channel();
    slot.reload(
        &document,
        Resources::new(),
        |_| receiver::<CheckboxAbi, _>("source", sender, |neocogi_ui::checkbox::Event::Changed(event)| event.checked),
        |_, _| Ok(()),
    )
    .unwrap();
    fixture.frame(VIEW).unwrap();
    let form = slot.component().unwrap().root().unwrap().as_interface().add_ref().query::<FormAbi>().unwrap();
    let children = form.children().unwrap();
    let mut target = Widget::from_interface(children.child(1).unwrap().query::<CheckboxAbi>().unwrap());
    let mut second = Widget::from_interface(children.child(2).unwrap().query::<CheckboxAbi>().unwrap());
    assert!(!target.checked().unwrap());
    assert!(!second.checked().unwrap());
    pointer(&fixture.ui, Point { x: 30, y: 30 }, true);
    pointer(&fixture.ui, Point { x: 30, y: 30 }, false);
    fixture.frame(VIEW).unwrap();
    assert!(!target.checked().unwrap());
    let checked = events.try_recv().unwrap();
    target.set_checked(checked).unwrap();
    second.set_checked(checked).unwrap();
    assert!(target.checked().unwrap());
    assert!(second.checked().unwrap());
}

/// One text event enum reaches the form; its changed variant updates a visual target
/// while both changed and submitted remain visible through the sole public stream.
#[test]
fn textbox_enum_stream_drives_form_action_and_public_sink() {
    let mut fixture = Headless::standard();
    let mut slot = slot(&fixture.ui);
    let mut document = Document::new("text event owner");
    document.root = Node::new("form", WidgetSpec::Form { width: 300, height: 200 });
    document.root.children.push(Child {
        edge: Edge::Position { x: 0, y: 0, width: 200, height: 40 },
        node: Node::new("name", WidgetSpec::Textbox { text: String::new() }),
    });
    document.root.children.push(Child {
        edge: Edge::Position { x: 0, y: 60, width: 200, height: 40 },
        node: Node::new(
            "preview",
            WidgetSpec::TextBlock {
                text: String::new(),
                wrapping: Wrapping::None,
            },
        ),
    });
    document.outputs = vec![Output {
        name: "name_event".into(),
        node: "name".into(),
    }];
    document.inputs.clear();
    // The application matches the enum and edits its preview field after traversal.
    let (sender, events) = std::sync::mpsc::channel();
    slot.reload(
        &document,
        Resources::new(),
        |_| receiver::<TextboxAbi, _>("name", sender, |event: neocogi_ui::textbox::Event| event),
        |_, _| Ok(()),
    )
    .unwrap();
    fixture.frame(VIEW).unwrap();
    let form = slot.component().unwrap().root().unwrap().as_interface().add_ref().query::<FormAbi>().unwrap();
    let children = form.children().unwrap();
    let mut preview = Widget::from_interface(children.child(1).unwrap().query::<TextBlockAbi>().unwrap());
    pointer(&fixture.ui, Point { x: 30, y: 30 }, true);
    pointer(&fixture.ui, Point { x: 30, y: 30 }, false);
    fixture.ui.input(UiInput::Text, "Ada").unwrap();
    fixture.frame(VIEW).unwrap();
    assert_eq!(preview.text().unwrap(), "");
    match events.try_recv().unwrap() {
        neocogi_ui::textbox::Event::Changed(value) => {
            assert_eq!(value.cursor, 3);
            preview.set_text(&value.text).unwrap();
        }
        neocogi_ui::textbox::Event::Submitted(_) => panic!("edit must arrive before submit"),
    }
    assert_eq!(preview.text().unwrap(), "Ada");
    key(&fixture.ui, KeyCode::Enter, 0, 0);
    fixture.frame(VIEW).unwrap();
    assert!(matches!(events.try_recv(), Ok(neocogi_ui::textbox::Event::Submitted(value)) if value.text == "Ada"));
}

/// Constructor tables reject ambiguous ownership and incompatible event types before publication.
#[test]
fn construction_bindings_reject_duplicate_unknown_nonemitting_and_wrong_typed_sources() {
    let fixture = Headless::standard();
    let loader = Loader::new(fixture.ui.clone(), None, Resources::new()).unwrap();
    let document = Document::new("constructor bindings");
    for (key, wrong_type) in [("missing", false), ("title", false), ("save", true)] {
        let (sender, messages) = std::sync::mpsc::channel::<()>();
        let mut events = EventBindings::new();
        match wrong_type {
            true => events.insert::<TextboxAbi, _>(key, sender, |_| ()).unwrap(),
            false => events.insert::<ButtonAbi, _>(key, sender, |_| ()).unwrap(),
        }
        assert!(loader.prepare_with_events(&document, events).is_err());
        assert!(
            matches!(messages.try_recv(), Err(std::sync::mpsc::TryRecvError::Disconnected)),
            "rejected {key} mapping leaked"
        );
    }
    let (sender, messages) = std::sync::mpsc::channel::<()>();
    let mut events = EventBindings::new();
    events.insert::<ButtonAbi, _>("save", sender, |_| ()).unwrap();
    let (replacement, rejected) = std::sync::mpsc::channel::<()>();
    assert_eq!(events.insert::<ButtonAbi, _>("save", replacement, |_| ()), Err(neocogi_com::NC_E_INVALID_ARG));
    assert!(matches!(rejected.try_recv(), Err(std::sync::mpsc::TryRecvError::Disconnected)));
    let component = loader.prepare_with_events(&document, events).unwrap();
    let retained = component.widget::<ButtonAbi>("save").unwrap();
    assert!(matches!(messages.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty)));
    drop(component);
    assert!(matches!(messages.try_recv(), Err(std::sync::mpsc::TryRecvError::Disconnected)));
    assert_eq!(retained.label(), Err(UI_E_CLOSED));
}

/// Both loading paths install the same receiver before returning a candidate; live bindings never enter saved bytes.
#[test]
fn typed_and_binary_candidates_own_constructor_mappings_and_preserve_recipe_bytes() {
    let fixture = Headless::standard();
    let document = Document::new("observed recipe");
    let bytes = document.to_binary().unwrap();
    let loader = Loader::new(fixture.ui.clone(), None, Resources::new()).unwrap();
    for binary in [false, true] {
        let (sender, messages) = std::sync::mpsc::channel::<()>();
        let events = receiver::<ButtonAbi, _>("save", sender, |_| ()).unwrap();
        let candidate = match binary {
            true => loader.load_with_events(&bytes, events).unwrap(),
            false => loader.prepare_with_events(&document, events).unwrap(),
        };
        let retained = candidate.widget::<ButtonAbi>("save").unwrap();
        assert_eq!(candidate.save().unwrap(), bytes);
        assert!(matches!(messages.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty)));
        drop(candidate);
        assert_eq!(retained.label(), Err(UI_E_CLOSED));
        assert!(matches!(messages.try_recv(), Err(std::sync::mpsc::TryRecvError::Disconnected)));
    }
}

/// A postconstruction field-check failure closes all installed mappings without replacing the old form.
#[test]
fn rejected_form_field_validation_releases_new_receivers_and_keeps_previous_generation() {
    let fixture = Headless::standard();
    let mut slot = slot(&fixture.ui);
    let document = Document::new("reject candidate");
    let active = slot.reload(&document, Resources::new(), |_| Ok(EventBindings::new()), |_, _| Ok(())).unwrap();
    let previous = slot.component().unwrap().widget::<ButtonAbi>("save").unwrap();
    let (sender, messages) = std::sync::mpsc::channel::<()>();
    assert!(
        slot.reload(
            &document,
            Resources::new(),
            |_| receiver::<ButtonAbi, _>("save", sender, |_| ()),
            |candidate, _| {
                // Construction succeeded and owns the mapping, but the application's concrete field is wrong.
                candidate.widget::<TextboxAbi>("save").map(drop)
            }
        )
        .is_err()
    );
    assert!(slot.accepts(active.generation));
    assert!(previous.label().is_ok());
    assert!(matches!(messages.try_recv(), Err(std::sync::mpsc::TryRecvError::Disconnected)));
}
