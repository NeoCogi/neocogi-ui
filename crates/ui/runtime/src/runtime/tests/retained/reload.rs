//! Real loaded controls exercise publication, binding, authored defaults, and stale handles.
use super::*;
use neocogi_ui_document::*;

/// Prepares the single constructor binding used by these behavioral fixtures.
fn receiver<C: neocogi_com::events::EventSource>(key: &str, queue: ComPtr<C::Queue>) -> Result<EventBindings, NcStatus> {
    let mut events = EventBindings::new();
    events.insert_queue::<C>(key, queue.as_com_ref())?;
    Ok(events)
}

/// Mounts a document slot in an ordinary window using the production runtime.
fn slot(ui: &Ui) -> ReloadSlot {
    slot_with_grid_limits(ui, GridLimits { max_columns: 64, max_rows: 256 })
}

/// Selects an immutable host policy for reload tests that exercise exact Grid bounds.
fn slot_with_grid_limits(ui: &Ui, limits: GridLimits) -> ReloadSlot {
    let root_owner = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let mut root = root_owner.handle();
    let owner = window(ui, "runtime documents", Rect { x: 10, y: 10, width: 400, height: 400 }, root_owner);
    let mut slot = ReloadSlot::new(ui.clone(), owner, limits).unwrap();
    root.insert(0, slot.take_widget().unwrap(), LinearEdge::new(LayoutTrack::Flex { weight: 1.0 }))
        .unwrap();
    slot
}

/// Bad documents, unavailable resources and failed binding all preserve the published component.
#[test]
fn failed_reload_preserves_live_widget_handles_and_authored_save() {
    let mut fixture = Headless::standard();
    let mut slot = slot(&fixture.ui);
    let document = Document::new("test");
    let loaded = slot.reload(&document, Resources::new(), |_| Ok(EventBindings::new()), |_, _| Ok(())).unwrap();
    let mut title = slot.component().unwrap().widget::<TextBlockAbi>("title").unwrap();
    title.set_text("Application value").unwrap();
    assert_eq!(title.text().unwrap(), "Application value");
    assert_eq!(Document::from_binary(&slot.component().unwrap().save().unwrap()).unwrap(), document);
    assert!(slot.component().unwrap().widget::<CheckboxAbi>("title").is_err());
    let mut invalid = document.clone();
    invalid.root.children[0].node.style.font = Some("not-a-real-font".into());
    assert!(slot.reload(&invalid, Resources::new(), |_| Ok(EventBindings::new()), |_, _| Ok(())).is_err());
    assert!(slot.accepts(loaded.generation));
    title.set_text("Still alive").unwrap();
    assert!(
        slot.reload(
            &document,
            Resources::new(),
            |_| Ok(EventBindings::new()),
            |_, _| Err(neocogi_com::NC_E_INVALID_ARG)
        )
        .is_err()
    );
    title.set_text("Still alive after binding failure").unwrap();
    fixture.frame(VIEW).unwrap();
    let replacement = slot.reload(&document, Resources::new(), |_| Ok(EventBindings::new()), |_, _| Ok(())).unwrap();
    assert!(!slot.accepts(loaded.generation));
    assert!(slot.accepts(replacement.generation));
    assert_eq!(title.set_text("Stale handle"), Err(UI_E_CLOSED));
    assert_eq!(title.text(), Err(UI_E_CLOSED));
    fixture.frame(VIEW).unwrap();
}

/// Typed events retain their payload and application identity after a real pointer click and reload.
#[test]
fn outputs_rebind_and_queued_old_events_have_distinct_generations() {
    let mut fixture = Headless::standard();
    let mut slot = slot(&fixture.ui);
    let mut document = Document::new("button");
    document.root = Node::new("save", WidgetSpec::Button { text: "Save".into() });
    let (sender, events) = std::sync::mpsc::channel();
    let bind = |generation| {
        receiver::<ButtonAbi>(
            "save",
            neocogi_ui::test_event_queue!(button, Generation, sender.clone(), |queue, _origin, _| queue.generation, { generation: Generation = generation })
                .unwrap(),
        )
    };
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

/// Every built-in must construct, expose its concrete widget interface, render, save, and retire.
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
        let document = Document {
            version: VERSION,
            name: name.into(),
            root,
            outputs,
        };
        slot.reload(
            &document,
            Resources::new(),
            |_| {
                let (sender, _events) = std::sync::mpsc::channel::<()>();
                macro_rules! bind {
                    ($kind:ident, $abi:ty) => {
                        receiver::<$abi>(
                            &document.root.key,
                            neocogi_ui::test_event_queue!($kind, (), sender.clone(), |_queue, _origin, _| ()).unwrap(),
                        )
                    };
                }
                match &document.root.widget {
                    WidgetSpec::Button { .. } => bind!(button, ButtonAbi),
                    WidgetSpec::Checkbox { .. } => bind!(checkbox, CheckboxAbi),
                    WidgetSpec::Textbox { .. } => bind!(textbox, TextboxAbi),
                    WidgetSpec::TextArea { .. } => bind!(text_area, TextAreaAbi),
                    WidgetSpec::Slider { .. } => bind!(slider, SliderAbi),
                    WidgetSpec::Number { .. } => bind!(number, NumberAbi),
                    WidgetSpec::ListItem { .. } => bind!(list_item, ListItemAbi),
                    WidgetSpec::ListBox { .. } => bind!(list_box, ListBoxAbi),
                    WidgetSpec::Scrollbar { .. } => bind!(scrollbar, ScrollbarAbi),
                    WidgetSpec::Combo { .. } => bind!(combo, ComboAbi),
                    WidgetSpec::Split { .. } => bind!(split_view, SplitViewAbi),
                    _ => Ok(EventBindings::new()),
                }
            },
            |component, _| {
                // Resolve the actual object projection used by generated Form fields.
                // No setter catalog or independent command receiver participates.
                match &document.root.widget {
                    WidgetSpec::Button { .. } => component.widget::<ButtonAbi>("root").map(drop),
                    WidgetSpec::Checkbox { .. } => component.widget::<CheckboxAbi>("root").map(drop),
                    WidgetSpec::Textbox { .. } => component.widget::<TextboxAbi>("root").map(drop),
                    WidgetSpec::TextBlock { .. } => component.widget::<TextBlockAbi>("root").map(drop),
                    WidgetSpec::TextArea { .. } => component.widget::<TextAreaAbi>("root").map(drop),
                    WidgetSpec::Slider { .. } => component.widget::<SliderAbi>("root").map(drop),
                    WidgetSpec::Number { .. } => component.widget::<NumberAbi>("root").map(drop),
                    WidgetSpec::ColorSwatch { .. } => component.widget::<ColorSwatchAbi>("root").map(drop),
                    WidgetSpec::ListItem { .. } => component.widget::<ListItemAbi>("root").map(drop),
                    WidgetSpec::ListBox { .. } => component.widget::<ListBoxAbi>("root").map(drop),
                    WidgetSpec::Scrollbar { .. } => component.widget::<ScrollbarAbi>("root").map(drop),
                    WidgetSpec::Combo { .. } => component.widget::<ComboAbi>("root").map(drop),
                    WidgetSpec::Linear { .. } => component.widget::<LinearAbi>("root").map(drop),
                    WidgetSpec::Grid { .. } => component.widget::<GridAbi>("root").map(drop),
                    WidgetSpec::Split { .. } => component.widget::<SplitViewAbi>("root").map(drop),
                    WidgetSpec::Scroll { .. } => component.widget::<ScrollAreaAbi>("root").map(drop),
                    WidgetSpec::Form { .. } => component.widget::<FormAbi>("root").map(drop),
                    WidgetSpec::Disclosure { .. } => component.widget::<DisclosureAbi>("root").map(drop),
                    WidgetSpec::Custom { .. } => unreachable!("built-in fixture"),
                }
            },
        )
        .unwrap_or_else(|error| panic!("{name}: {error}"));
        fixture.frame(VIEW).unwrap();
        assert_eq!(Document::from_binary(&slot.component().unwrap().save().unwrap()).unwrap(), document);
    }
}

/// Concrete setters validate before mutation, preserve authored saves and emit no user events.
#[test]
fn widget_setters_validate_values_and_remain_separate_from_event_delivery() {
    let fixture = Headless::standard();
    let mut slot = slot(&fixture.ui);
    for widget in [
        WidgetSpec::Checkbox { text: "check".into(), checked: false },
        WidgetSpec::Slider {
            value: 1.0,
            low: 0.0,
            high: 10.0,
            step: 1.0,
            precision: 0,
        },
        WidgetSpec::Number { value: 1.0, step: 1.0, precision: 0 },
        WidgetSpec::Combo {
            labels: vec!["one".into()],
            selected: Some(0),
            placeholder: "none".into(),
        },
        WidgetSpec::Disclosure {
            text: "details".into(),
            tree: false,
            expanded: false,
        },
    ] {
        let document = Document {
            version: VERSION,
            name: "widget methods".into(),
            root: Node::new("root", widget),
            outputs: vec![],
        };
        let (sender, events) = std::sync::mpsc::channel();
        slot.reload(
            &document,
            Resources::new(),
            |_| match &document.root.widget {
                WidgetSpec::Checkbox { .. } => receiver::<CheckboxAbi>(
                    "root",
                    neocogi_ui::test_event_queue!(checkbox, (), sender.clone(), |_queue, _origin, _| ()).unwrap(),
                ),
                WidgetSpec::Slider { .. } => receiver::<SliderAbi>(
                    "root",
                    neocogi_ui::test_event_queue!(slider, (), sender.clone(), |_queue, _origin, _| ()).unwrap(),
                ),
                WidgetSpec::Number { .. } => receiver::<NumberAbi>(
                    "root",
                    neocogi_ui::test_event_queue!(number, (), sender.clone(), |_queue, _origin, _| ()).unwrap(),
                ),
                WidgetSpec::Combo { .. } => receiver::<ComboAbi>(
                    "root",
                    neocogi_ui::test_event_queue!(combo, (), sender.clone(), |_queue, _origin, _| ()).unwrap(),
                ),
                WidgetSpec::Disclosure { .. } => Ok(EventBindings::new()),
                _ => unreachable!("setter fixture"),
            },
            |_, _| Ok(()),
        )
        .unwrap();
        let component = slot.component().unwrap();
        component.root().unwrap().set_flags(WidgetOptions::NO_INTERACT).unwrap();
        // Keep numeric validation identical for both concrete widget projections.
        macro_rules! numeric {
            ($abi:ty) => {{
                let mut control = component.widget::<$abi>("root").unwrap();
                control.set_value(3.0).unwrap();
                assert_eq!(control.value().unwrap(), 3.0);
                for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                    assert!(control.set_value(invalid).is_err());
                    assert_eq!(control.value().unwrap(), 3.0);
                }
            }};
        }
        match &document.root.widget {
            WidgetSpec::Checkbox { .. } => {
                let mut control = component.widget::<CheckboxAbi>("root").unwrap();
                control.set_checked(true).unwrap();
                assert!(control.checked().unwrap());
            }
            WidgetSpec::Slider { .. } => numeric!(SliderAbi),
            WidgetSpec::Number { .. } => numeric!(NumberAbi),
            WidgetSpec::Combo { .. } => {
                let mut control = component.widget::<ComboAbi>("root").unwrap();
                control.replace_items(&["one", "two"], ChoiceSelection::Selected(1)).unwrap();
                assert_eq!(control.display_text().unwrap(), "two");
                // Invalid replacement selection must preserve the entire old model.
                assert!(control.replace_items(&["replacement"], ChoiceSelection::Selected(1)).is_err());
                assert_eq!(control.choice_count().unwrap(), 2);
                assert_eq!(control.choice_label(0).unwrap(), "one");
                assert_eq!(control.choice_label(1).unwrap(), "two");
                assert!(matches!(control.selection().unwrap(), ChoiceSelection::Selected(1)));
            }
            WidgetSpec::Disclosure { .. } => {
                let mut control = component.widget::<DisclosureAbi>("root").unwrap();
                control.set_expanded(true).unwrap();
                assert!(control.expanded().unwrap());
            }
            _ => unreachable!("setter fixture"),
        }
        assert_eq!(events.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty));
        assert_eq!(Document::from_binary(&component.save().unwrap()).unwrap(), document);
    }
}

/// Enablement changes only interaction intent, preserving application edits made after construction.
#[test]
fn widget_flag_enablement_preserves_live_application_flags() {
    let fixture = Headless::standard();
    let mut slot = slot(&fixture.ui);
    let document = Document {
        version: VERSION,
        name: "live flags".into(),
        root: Node::new("button", WidgetSpec::Button { text: "toggle".into() }),
        outputs: vec![],
    };
    slot.reload(&document, Resources::new(), |_| Ok(EventBindings::new()), |_, _| Ok(())).unwrap();
    let component = slot.component().unwrap();
    let mut button = component.widget::<ButtonAbi>("button").unwrap();
    assert_eq!(button.as_interface().properties().unwrap().options, WidgetOptions::FRAME);

    // A later application edit removes an initial bit and adds a different bit.
    // Disabling must preserve both decisions rather than restoring authored defaults.
    button.replace_flags(WidgetOptions::ALIGN_RIGHT).unwrap();
    button.set_flags(WidgetOptions::NO_INTERACT).unwrap();
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
        button.clear_flags(WidgetOptions::NO_INTERACT).unwrap();
        assert_eq!(
            button.as_interface().properties().unwrap().options,
            WidgetOptions::FRAME | WidgetOptions::ALIGN_CENTER
        );
    }
}

/// Selective enablement preserves live presentation edits and never stores derived input flags.
#[test]
fn widget_flag_enablement_preserves_requested_flags_without_capturing_effective_flags() {
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
    };
    slot.reload(&document, Resources::new(), |_| Ok(EventBindings::new()), |_, _| Ok(())).unwrap();
    let component = slot.component().unwrap();
    let root = component.root().unwrap();
    let mut common = root.as_interface().clone();
    assert_eq!(common.properties().unwrap().options, WidgetOptions::FRAME | WidgetOptions::GRAB_SCROLL);
    common.clear_flags(WidgetOptions::FRAME).unwrap();
    common.set_flags(WidgetOptions::ALIGN_RIGHT).unwrap();
    for value in [false, true] {
        match value {
            true => common.clear_flags(WidgetOptions::NO_INTERACT),
            false => common.set_flags(WidgetOptions::NO_INTERACT),
        }
        .unwrap();
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
    // Clearing the requested bit edits intent only; a disabled scroll model remains inert.
    common.clear_flags(WidgetOptions::NO_INTERACT).unwrap();
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
    let bytes = document.to_binary().unwrap();
    assert_eq!(Document::from_binary(&bytes).unwrap(), document);
    let (sender, events) = std::sync::mpsc::channel();
    slot.reload(
        &document,
        Resources::new(),
        |generation| {
            receiver::<ButtonAbi>(
                "save",
                neocogi_ui::test_event_queue!(button, Generation, sender, |queue, _origin, _| queue.generation, { generation: Generation = generation })
                    .unwrap(),
            )
        },
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
    let loader = Loader::new(fixture.ui.clone(), None, Resources::new(), neocogi_ui::GridLimits { max_columns: 64, max_rows: 256 }).unwrap();
    let typed = loader.compile(&document).unwrap();
    let binary = loader.load(&document.to_binary().unwrap()).unwrap();
    assert_ne!(
        typed.root().unwrap().as_interface().as_raw().object,
        binary.root().unwrap().as_interface().as_raw().object
    );
    for component in [&typed, &binary] {
        let mut title = component.widget::<TextBlockAbi>("title").unwrap();
        title.set_text("runtime override").unwrap();
        assert_eq!(title.text().unwrap(), "runtime override");
        assert_eq!(Document::from_binary(&component.save().unwrap()).unwrap(), document);
    }
}

/// Direct widget setters preserve unrelated configuration, validate before mutation, and retire.
#[test]
fn selection_offset_and_position_setters_preserve_configuration_and_retire() {
    let fixture = Headless::standard();
    let mut slot = slot(&fixture.ui);
    let mut document = Document {
        version: VERSION,
        name: "focused setters".into(),
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
    };
    let (sender, events) = std::sync::mpsc::channel();
    slot.reload(
        &document,
        Resources::new(),
        |_| {
            receiver::<ComboAbi>(
                "root",
                neocogi_ui::test_event_queue!(combo, (), sender.clone(), |_queue, _origin, _| ()).unwrap(),
            )
        },
        |_, _| Ok(()),
    )
    .unwrap();
    let component = slot.component().unwrap();
    let mut combo = component.widget::<ComboAbi>("root").unwrap();
    combo.set_selection(ChoiceSelection::Selected(1)).unwrap();
    assert!(matches!(combo.selection().unwrap(), ChoiceSelection::Selected(1)));
    for selected in [2, u64::MAX] {
        assert!(combo.set_selection(ChoiceSelection::Selected(selected)).is_err());
        assert!(matches!(combo.selection().unwrap(), ChoiceSelection::Selected(1)));
    }
    combo.set_selection(ChoiceSelection::None).unwrap();
    assert!(matches!(combo.selection().unwrap(), ChoiceSelection::None));
    assert!(events.try_recv().is_err());

    document.root.widget = WidgetSpec::Scrollbar {
        vertical: true,
        viewport: 10,
        content: 100,
        offset: 5,
    };
    slot.reload(
        &document,
        Resources::new(),
        |_| {
            receiver::<ScrollbarAbi>(
                "root",
                neocogi_ui::test_event_queue!(scrollbar, (), sender.clone(), |_queue, _origin, _| ()).unwrap(),
            )
        },
        |_, _| Ok(()),
    )
    .unwrap();
    assert!(combo.set_selection(ChoiceSelection::Selected(0)).is_err());
    let component = slot.component().unwrap();
    let mut scrollbar = component.widget::<ScrollbarAbi>("root").unwrap();
    for (requested, expected) in [(50, 50), (i32::MAX, 90), (-1, 0)] {
        scrollbar.set_offset(requested).unwrap();
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
    slot.reload(
        &document,
        Resources::new(),
        |_| {
            receiver::<SplitViewAbi>(
                "root",
                neocogi_ui::test_event_queue!(split_view, (), sender.clone(), |_queue, _origin, _| ()).unwrap(),
            )
        },
        |_, _| Ok(()),
    )
    .unwrap();
    assert!(scrollbar.set_offset(0).is_err());
    let component = slot.component().unwrap();
    let mut split = component.widget::<SplitViewAbi>("root").unwrap();
    // Preserve live configuration as well as authored defaults when setting subsequent positions.
    let mut settings = split.settings().unwrap();
    settings.minimum_first = 40;
    split.set_settings(settings).unwrap();
    split.set_position(150).unwrap();
    let settings = split.settings().unwrap();
    assert_eq!(
        (settings.position, settings.minimum_first, settings.minimum_second, settings.handle_width),
        (150, 40, 30, 6)
    );
    assert!(split.set_position(-1).is_err());
    assert_eq!(split.settings().unwrap().position, 150);
    assert!(events.try_recv().is_err());
    assert_eq!(Document::from_binary(&component.save().unwrap()).unwrap(), document);
    slot.reload(&document, Resources::new(), |_| Ok(EventBindings::new()), |_, _| Ok(())).unwrap();
    assert!(split.set_position(100).is_err());
}

/// A self-targeting activation applies only after input traversal, without a reentrant borrow.
#[test]
fn application_handles_real_click_and_closed_form_retires() {
    let mut fixture = Headless::standard();
    let mut slot = slot(&fixture.ui);
    let mut document = Document::new("private action");
    document.root = Node::new("button", WidgetSpec::Button { text: "Before".into() });
    document.outputs.clear();
    // Application behavior is queued explicitly; documents do not contain executable routes.
    let (sender, messages) = std::sync::mpsc::channel();
    let bytes = document.to_binary().unwrap();
    assert_eq!(Document::from_binary(&bytes).unwrap(), document);
    slot.reload(
        &document,
        Resources::new(),
        |_| {
            receiver::<ButtonAbi>(
                "button",
                neocogi_ui::test_event_queue!(button, (), sender.clone(), |_queue, _origin, _| ()).unwrap(),
            )
        },
        |_, _| Ok(()),
    )
    .unwrap();
    let component = slot.component().unwrap();
    let mut button = component.widget::<ButtonAbi>("button").unwrap();
    let mut root = component.root().unwrap().node_interface().unwrap();
    fixture.frame(VIEW).unwrap();
    pointer(&fixture.ui, Point { x: 30, y: 30 }, true);
    pointer(&fixture.ui, Point { x: 30, y: 30 }, false);
    fixture.frame(VIEW).unwrap();
    assert_eq!(button.label().unwrap(), "Before");
    messages.try_recv().unwrap();
    button.set_label("After").unwrap();
    assert_eq!(button.label().unwrap(), "After");
    assert_eq!(slot.component().unwrap().save().unwrap(), bytes);
    assert_eq!(root.dispatch_next(), Ok(false));
    slot.reload(&document, Resources::new(), |_| Ok(EventBindings::new()), |_, _| Ok(())).unwrap();
    // Container closure queues retirement until the next safe runtime boundary.
    fixture.frame(VIEW).unwrap();
    assert_eq!(root.dispatch_next(), Err(UI_E_CLOSED));
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
    // One application handler may issue commands to multiple typed widget fields.
    let (sender, events) = std::sync::mpsc::channel();
    slot.reload(
        &document,
        Resources::new(),
        |_| {
            receiver::<CheckboxAbi>(
                "source",
                neocogi_ui::test_event_queue!(checkbox, bool, sender, |_queue, _origin, neocogi_ui::checkbox::Event::Changed(event)| event
                    .checked)
                .unwrap(),
            )
        },
        |_, _| Ok(()),
    )
    .unwrap();
    fixture.frame(VIEW).unwrap();
    let form = slot.component().unwrap().root().unwrap().as_interface().clone().query::<FormAbi>().unwrap();
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
    // The application matches the enum and edits its preview field after traversal.
    let (sender, events) = std::sync::mpsc::channel();
    slot.reload(
        &document,
        Resources::new(),
        |_| {
            receiver::<TextboxAbi>(
                "name",
                neocogi_ui::test_event_queue!(textbox, neocogi_ui::textbox::Event, sender, |_queue, _origin, event| event).unwrap(),
            )
        },
        |_, _| Ok(()),
    )
    .unwrap();
    fixture.frame(VIEW).unwrap();
    let form = slot.component().unwrap().root().unwrap().as_interface().clone().query::<FormAbi>().unwrap();
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
    let loader = Loader::new(fixture.ui.clone(), None, Resources::new(), neocogi_ui::GridLimits { max_columns: 64, max_rows: 256 }).unwrap();
    let document = Document::new("constructor bindings");
    for (key, wrong_type) in [("missing", false), ("title", false), ("save", true)] {
        let (sender, messages) = std::sync::mpsc::channel::<()>();
        let mut events = EventBindings::new();
        match wrong_type {
            true => events
                .insert_queue::<TextboxAbi>(
                    key,
                    neocogi_ui::test_event_queue!(textbox, (), sender, |_queue, _origin, _| ())
                        .unwrap()
                        .as_com_ref(),
                )
                .unwrap(),
            false => events
                .insert_queue::<ButtonAbi>(
                    key,
                    neocogi_ui::test_event_queue!(button, (), sender, |_queue, _origin, _| ()).unwrap().as_com_ref(),
                )
                .unwrap(),
        }
        assert!(loader.compile_with_events(&document, events).is_err());
        assert!(
            matches!(messages.try_recv(), Err(std::sync::mpsc::TryRecvError::Disconnected)),
            "rejected {key} mapping leaked"
        );
    }
    let (sender, messages) = std::sync::mpsc::channel::<()>();
    let mut events = EventBindings::new();
    events
        .insert_queue::<ButtonAbi>(
            "save",
            neocogi_ui::test_event_queue!(button, (), sender, |_queue, _origin, _| ()).unwrap().as_com_ref(),
        )
        .unwrap();
    let (replacement, rejected) = std::sync::mpsc::channel::<()>();
    assert_eq!(
        events.insert_queue::<ButtonAbi>(
            "save",
            neocogi_ui::test_event_queue!(button, (), replacement, |_queue, _origin, _| ())
                .unwrap()
                .as_com_ref()
        ),
        Err(neocogi_com::NC_E_INVALID_ARG)
    );
    assert!(matches!(rejected.try_recv(), Err(std::sync::mpsc::TryRecvError::Disconnected)));
    let component = loader.compile_with_events(&document, events).unwrap();
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
    let loader = Loader::new(fixture.ui.clone(), None, Resources::new(), neocogi_ui::GridLimits { max_columns: 64, max_rows: 256 }).unwrap();
    for binary in [false, true] {
        let (sender, messages) = std::sync::mpsc::channel::<()>();
        let events = receiver::<ButtonAbi>("save", neocogi_ui::test_event_queue!(button, (), sender, |_queue, _origin, _| ()).unwrap()).unwrap();
        let candidate = match binary {
            true => loader.load_with_events(&bytes, events).unwrap(),
            false => loader.compile_with_events(&document, events).unwrap(),
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
            |_| receiver::<ButtonAbi>("save", neocogi_ui::test_event_queue!(button, (), sender, |_queue, _origin, _| ()).unwrap()),
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

/// Root retention is ordinary COM ownership, independent of compiled recipe metadata.
#[test]
fn compiled_metadata_can_drop_before_the_actual_root_is_mounted() {
    for binary in [false, true] {
        let fixture = Headless::standard();
        let document = Document::new("independent recipe lifetime");
        let loader = Loader::new(fixture.ui.clone(), None, Resources::new(), neocogi_ui::GridLimits { max_columns: 64, max_rows: 256 }).unwrap();
        let (sender, messages) = std::sync::mpsc::channel::<()>();
        let events = receiver::<ButtonAbi>("save", neocogi_ui::test_event_queue!(button, (), sender, |_queue, _origin, _| ()).unwrap()).unwrap();
        let mut compiled = match binary {
            true => loader.load_with_events(&document.to_binary().unwrap(), events).unwrap(),
            false => loader.compile_with_events(&document, events).unwrap(),
        };
        let root = compiled.take_root().unwrap();
        let button = compiled.widget::<ButtonAbi>("save").unwrap();
        drop(compiled);
        assert!(button.label().is_ok());
        let window = fixture
            .ui
            .window("actual compiled root", Rect { x: 10, y: 10, width: 400, height: 300 }, root)
            .unwrap();
        assert!(button.label().is_ok(), "the runtime owns the mounted tree");
        assert!(matches!(messages.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty)));
        window.close().unwrap();
        assert_eq!(button.label(), Err(UI_E_CLOSED));
        assert!(matches!(messages.try_recv(), Err(std::sync::mpsc::TryRecvError::Disconnected)));
    }
}

/// Rejected publication closes a complete candidate even when validation retained its root.
#[test]
fn rejected_reload_closes_retained_candidate_handles_without_touching_the_live_form() {
    let fixture = Headless::standard();
    let mut slot = slot(&fixture.ui);
    let document = Document::new("explicit candidate rollback");
    let active = slot.reload(&document, Resources::new(), |_| Ok(EventBindings::new()), |_, _| Ok(())).unwrap();
    let previous = slot.component().unwrap().widget::<ButtonAbi>("save").unwrap();
    let (sender, messages) = std::sync::mpsc::channel::<()>();
    let mut retained = None;
    let result = slot.reload(
        &document,
        Resources::new(),
        |_| receiver::<ButtonAbi>("save", neocogi_ui::test_event_queue!(button, (), sender, |_queue, _origin, _| ()).unwrap()),
        |candidate, _| {
            retained = Some((candidate.root()?, candidate.widget::<ButtonAbi>("save")?));
            Err(neocogi_com::NC_E_INVALID_ARG)
        },
    );
    assert!(result.is_err());
    assert!(slot.accepts(active.generation));
    assert!(previous.label().is_ok());
    let (root, button) = retained.unwrap();
    assert_eq!(root.node_interface().unwrap().properties(), Err(UI_E_CLOSED));
    assert_eq!(button.label(), Err(UI_E_CLOSED));
    assert!(matches!(messages.try_recv(), Err(std::sync::mpsc::TryRecvError::Disconnected)));
}

#[path = "form_methods.rs"]
mod form_methods;

/// A failed publication restores the compiled form's root and keeps all lookups
/// valid. Retrying the same component requires no reconstruction or new binding.
#[test]
fn failed_component_mount_returns_its_unmounted_root_for_retry() {
    let fixture = Headless::standard();
    let loader = Loader::new(fixture.ui.clone(), None, Resources::new(), neocogi_ui::GridLimits { max_columns: 64, max_rows: 256 }).unwrap();
    let mut component = loader.compile(&Document::new("retry")).unwrap();
    let root = component.root().unwrap();
    let invalid = Rect { x: 0, y: 0, width: -1, height: 100 };
    let status = match fixture.ui.window("invalid", invalid, &mut component) {
        Err((status, _component)) => status,
        Ok(_) => panic!("negative window extent must be rejected"),
    };
    assert_eq!(status, neocogi_com::NC_E_INVALID_ARG);
    assert!(root.as_interface().is_alive());
    let window = fixture.ui.window("retry", Rect { x: 0, y: 0, width: 200, height: 100 }, &mut component)
        .unwrap_or_else(|(status, _)| panic!("valid retry failed: {status:?}"));
    assert!(component.take_root().is_err(), "success transfers exactly once");
    window.close().unwrap();
    assert!(!root.as_interface().is_alive());
}

/// Builds a compact authored Grid; runtime limits deliberately do not appear in its recipe.
fn grid_document(spans: &[(u32, u32)]) -> Document {
    let mut document = Document::new("bounded grid");
    document.outputs.clear();
    document.root = Node::new("grid", WidgetSpec::Grid {
        columns: vec![Track::Content], rows: vec![Track::Content],
    });
    for (index, &(columns, rows)) in spans.iter().enumerate() {
        document.root.children.push(Child {
            edge: Edge::Grid { columns, rows },
            node: Node::new(format!("child{index}"), WidgetSpec::TextBlock { text: "cell".into(), wrapping: Wrapping::None }),
        });
    }
    document
}

/// Typed and persisted documents both obey the host's policy, including cumulative implicit rows.
#[test]
fn loaded_grid_uses_host_limits_on_both_construction_paths() {
    let fixture = Headless::standard();
    let limits = GridLimits { max_columns: 2, max_rows: 2 };
    let loader = Loader::new(fixture.ui.clone(), None, Resources::new(), limits).unwrap();
    let exact = grid_document(&[(1, 2)]);
    let typed = loader.compile(&exact).unwrap();
    let binary = loader.load(&exact.to_binary().unwrap()).unwrap();
    for component in [typed, binary] {
        assert_eq!(component.widget::<GridAbi>("grid").unwrap().limits().unwrap(), limits);
        assert_eq!(Document::from_binary(&component.save().unwrap()).unwrap(), exact);
    }
    let mut excessive_columns = exact.clone();
    if let WidgetSpec::Grid { columns, .. } = &mut excessive_columns.root.widget {
        *columns = vec![Track::Content; 3];
    }
    let mut excessive_rows = exact.clone();
    if let WidgetSpec::Grid { rows, .. } = &mut excessive_rows.root.widget {
        *rows = vec![Track::Content; 3];
    }
    for document in [grid_document(&[(1, u32::MAX)]), grid_document(&[(1, 2), (1, 1)]), excessive_columns, excessive_rows] {
        // The portable recipe can be valid for another host. Admission, not
        // serialization, applies this application's specific resource ceilings.
        let bytes = document.to_binary().unwrap();
        let json = Document::from_json(&document.to_json().unwrap()).unwrap();
        assert!(loader.compile(&json).is_err());
        assert_eq!(loader.load(&bytes).err(), Some(neocogi_com::NC_E_OUT_OF_RANGE));
    }
    let larger = Loader::new(fixture.ui.clone(), None, Resources::new(), GridLimits { max_columns: 2, max_rows: 3 }).unwrap();
    assert!(larger.compile(&grid_document(&[(1, 2), (1, 1)])).is_ok());
}

/// An over-budget reload must not invalidate handles or replace the authored live component.
#[test]
fn rejected_grid_reload_preserves_published_component() {
    let mut fixture = Headless::standard();
    let mut slot = slot_with_grid_limits(&fixture.ui, GridLimits { max_columns: 2, max_rows: 2 });
    let document = grid_document(&[(1, 2)]);
    let committed = slot.reload(&document, Resources::new(), |_| Ok(EventBindings::new()), |_, _| Ok(())).unwrap();
    let mut old = slot.component().unwrap().widget::<TextBlockAbi>("child0").unwrap();
    let saved = slot.component().unwrap().save().unwrap();
    assert!(slot.reload(&grid_document(&[(1, 2), (1, 1)]), Resources::new(), |_| Ok(EventBindings::new()), |_, _| Ok(())).is_err());
    assert!(slot.accepts(committed.generation));
    assert_eq!(slot.component().unwrap().save().unwrap(), saved);
    old.set_text("Still owned by the committed Grid").unwrap();
    fixture.frame(VIEW).unwrap();
}
