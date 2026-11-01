//! Bounded popup regressions drive generated interfaces, real layout and final GFX output.
use super::*;

/// Public operations plus read-only access to allocations committed by the real forest.
struct PopupFixture {
    /// The actual bound nodes provider and core facade.
    ui: neocogi_ui::Ui,
    /// Construction capability used for standalone semantic choice models.
    runtime: ComPtr<UiRuntimeAbi>,
    /// Observation only; no test mutates forest geometry or routes private input.
    storage: Rc<RuntimeStorage>,
}
impl PopupFixture {
    /// Uses a validated font with small glyph bitmaps even when line distance is very large.
    fn new(line_height: usize) -> Self {
        let mut data = crate::test_support::test_asset_data_with_font_sizes(&[("body", line_height)]);
        data.tables.metrics.padding = 2;
        data.tables.metrics.scrollbar_size = 12;
        data.tables.metrics.thumb_size = 8;
        let source = data.into_bundle().unwrap();
        let gfx = crate::test_support::gfx_factory(crate::test_support::host_runtime());
        let value = create_runtime_value(source.as_com_ref(), gfx.as_com_ref()).unwrap();
        let storage = value.storage.clone();
        let mut runtime: ComPtr<UiRuntimeAbi> = Runtime::from_value(value).query().unwrap();
        runtime.set_render_host(neocogi_gfx_support::testing::host().as_com_ref()).unwrap();
        let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
        Self { ui, runtime, storage }
    }

    /// Reads only final allocations after update; geometry is never predicted by a test solver.
    fn popups(&self) -> Vec<crate::Recti> {
        self.storage.context().unwrap().window_manager.test_popup_rects()
    }

    /// A normal owner intentionally smaller than the drawable: its popup may overlay the frame.
    fn owner(&self) -> ComPtr<IWindowAbi> {
        let widget = self.ui.widgets().text_block("a", TextWrapping::None).unwrap();
        self.ui.window("popup owner", RECT, widget).unwrap()
    }

    /// Creates the public core model with nontruncated stable IDs and shared UTF-8 storage.
    fn choices(&self, owner: &ComPtr<IWindowAbi>, count: u32) -> ComPtr<IChoiceMenuAbi> {
        let entries: Vec<_> = (0..count)
            .map(|id| ChoiceMenuEntry::Choice {
                id: u64::from(id),
                label: MenuTextRange { first: 0, count: 1 },
            })
            .collect();
        self.runtime
            .query::<IChoiceMenusAbi>()
            .unwrap()
            .create_choice_menu(
                owner.as_com_ref(),
                NcCopyFromBufferOffset::new(0, b"a"),
                NcCopyFromBufferOffset::new(0, &entries),
                if count == 0 { ChoiceSelection::None } else { ChoiceSelection::Selected(0) },
                neocogi_com::NcValueRef::new(&EventBinding::Unobserved).unwrap(),
            )
            .unwrap()
    }
}

/// Queues ordinary public input; all intervening layouts and menu actions remain production owned.
fn key(ui: &neocogi_ui::Ui, key: KeyCode) {
    ui.input(
        UiInput::Key(KeyStroke {
            key,
            value: 0,
            pressed: true,
            repeat: false,
            modifiers: 0,
        }),
        "",
    )
    .unwrap();
}

/// Positive configuration, row caps, flipping, frame restriction and silent live edits cross COM.
#[test]
fn choice_popup_caps_and_frame_fitting_are_committed_together() {
    for count in [0, 1, 9, 10, 11, 64] {
        let fixture = PopupFixture::new(10);
        let owner = fixture.owner();
        let mut menu = fixture.choices(&owner, count);
        assert_eq!(menu.max_visible_items().unwrap(), 10);
        assert_eq!(menu.set_max_visible_items(0), Err(NC_E_INVALID_ARG));
        assert_eq!(menu.max_visible_items().unwrap(), 10);
        menu.show_at(Rect { x: 100, y: 100, width: 120, height: 24 }).unwrap();
        fixture.ui.update(SIZE).unwrap();
        if count == 0 {
            assert!(fixture.popups().is_empty());
            continue;
        }
        let rect = fixture.popups()[0];
        assert_eq!((rect.x, rect.y, rect.width, rect.height), (100, 124, 120, count.min(10) as i32 * 12 + 2));
        assert!(rect.y + rect.height <= SIZE.height - 10);
        assert_eq!(menu.choice_count().unwrap(), u64::from(count));
        menu.set_max_visible_items(3).unwrap();
        fixture.ui.update(SIZE).unwrap();
        assert_eq!(fixture.popups()[0].height, count.min(3) as i32 * 12 + 2);
        assert_eq!(menu.selection().unwrap(), ChoiceSelection::Selected(0));
        menu.set_max_visible_items(u32::MAX).unwrap();
        menu.show_at(Rect { x: 100, y: 380, width: 120, height: 24 }).unwrap();
        fixture.ui.update(SIZE).unwrap();
        let above = fixture.popups()[0];
        assert!(above.y < 380);
        assert!(above.y + above.height <= 380);
        assert_eq!((above.height - 2) % 12, 0);
        let small = Size { width: 180, height: 80 };
        fixture.ui.update(small).unwrap();
        let shrunk = fixture.popups()[0];
        assert!(shrunk.x >= 0 && shrunk.x + shrunk.width <= 180);
        assert!(shrunk.y >= 0 && shrunk.y + shrunk.height <= 80);
        let (renderer, probe) = renderer();
        fixture.ui.render(small, renderer.as_com_ref()).unwrap();
        assert!(probe.vertices.borrow().iter().all(|v| v.x >= 0.0 && v.x <= 180.0 && v.y >= 0.0 && v.y <= 80.0));
    }
}

/// A real last-row click proves the content range wasn't truncated before viewport translation.
#[test]
fn bounded_combo_range_reveals_and_activates_the_final_stable_id() {
    let fixture = PopupFixture::new(39_998);
    let owner = fixture.owner();
    let mut menu = fixture.choices(&owner, 64);
    menu.show_at(Rect { x: 100, y: 100, width: 120, height: 24 }).unwrap();
    let size = Size { width: 640, height: 131_072 };
    fixture.ui.update(size).unwrap();
    key(&fixture.ui, KeyCode::End);
    fixture.ui.update(size).unwrap();
    assert_eq!(menu.selection().unwrap(), ChoiceSelection::Selected(0), "navigation is not commitment");
    let popup = fixture.popups()[0];
    assert_eq!(popup.height, 40_002);
    let position = Point { x: popup.x + 5, y: popup.y + 1 + 5 };
    fixture.ui.input(UiInput::Down { position, buttons: 1 }, "").unwrap();
    fixture.ui.input(UiInput::Up { position, buttons: 1 }, "").unwrap();
    fixture.ui.update(size).unwrap();
    assert_eq!(menu.selection().unwrap(), ChoiceSelection::Selected(63));
    assert!(fixture.popups().is_empty());
    menu.show_at(Rect { x: 100, y: 100, width: 120, height: 24 }).unwrap();
    fixture.ui.update(size).unwrap();
    let (renderer, probe) = renderer();
    fixture.ui.render(size, renderer.as_com_ref()).unwrap();
    assert!(probe.vertices.borrow().len() < 10_000, "only the bounded visible range may paint");
    assert!(probe.vertices.borrow().iter().all(|v| v.x.is_finite() && v.y >= 0.0 && v.y <= 131_072.0));
}

/// Scrolling a parent retires its obsolete child suffix without invalidating traversal indices.
#[test]
fn parent_menu_scrolling_reanchors_or_dismisses_its_submenu() {
    let fixture = PopupFixture::new(10);
    let owner = fixture.owner();
    let menus = fixture.ui.menus().unwrap();
    let builder = menus.create("commands").unwrap();
    let branch = builder.submenu("branch").unwrap();
    branch.item("a").unwrap();
    branch.item("b").unwrap();
    // One branch, its two leaves and these rows consume exactly 64 entries.
    for _ in 0..61 {
        builder.item("a").unwrap();
    }
    let mut popup = menus.popup(owner.as_com_ref(), &builder).unwrap();
    popup.show_at(Rect { x: 20, y: 20, width: 1, height: 1 }).unwrap();
    key(&fixture.ui, KeyCode::Home);
    key(&fixture.ui, KeyCode::ArrowRight);
    fixture.ui.update(SIZE).unwrap();
    let before = fixture.popups();
    assert_eq!(before.len(), 2);
    assert_eq!(before[1].x, before[0].x + before[0].width, "child attaches beyond the parent's scrollbar");
    let position = Point { x: before[0].x + 3, y: before[0].y + 3 };
    fixture.ui.input(UiInput::Pointer(position), "").unwrap();
    fixture.ui.input(UiInput::Scroll(Point { x: 0, y: 600 }), "").unwrap();
    fixture.ui.update(SIZE).unwrap();
    assert_eq!(fixture.popups().len(), 1);
    key(&fixture.ui, KeyCode::Home);
    key(&fixture.ui, KeyCode::ArrowRight);
    fixture.ui.update(SIZE).unwrap();
    assert_eq!(fixture.popups().len(), 2);
    popup.close().unwrap();
    fixture.ui.update(SIZE).unwrap();
    assert!(fixture.popups().is_empty());
}

/// The header forwards configuration, and failed edits cannot dirty or change its current model.
#[test]
fn combo_forwards_visible_limit_without_duplicating_semantic_state() {
    let fixture = PopupFixture::new(10);
    let owner = fixture.owner();
    let combo = fixture.ui.combo_box_unobserved(owner.as_com_ref(), &["a", "b"], ChoiceSelection::None).unwrap();
    let mut combo = combo;
    assert_eq!(combo.max_visible_items().unwrap(), 10);
    assert_eq!(combo.set_max_visible_items(0), Err(NC_E_INVALID_ARG));
    assert_eq!(combo.max_visible_items().unwrap(), 10);
    combo.set_max_visible_items(1).unwrap();
    assert_eq!(combo.max_visible_items().unwrap(), 1);
    combo.replace_items(&["a", "b", "a"], ChoiceSelection::Selected(2)).unwrap();
    assert_eq!(combo.max_visible_items().unwrap(), 1);
    assert_eq!(combo.selection().unwrap(), ChoiceSelection::Selected(2));
}

/// General popup content keeps a fixed header/footer while only its authored body scrolls.
#[test]
fn general_anchored_popup_constrains_an_authored_scroll_area_and_validates_atomically() {
    let fixture = PopupFixture::new(10);
    let owner = fixture.owner();
    let list = fixture.ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let mut list_handle = list.handle();
    for index in 0..100 {
        list_handle
            .insert(
                index,
                fixture.ui.widgets().text_block("a", TextWrapping::None).unwrap(),
                LinearEdge::new(LayoutTrack::Fixed { extent: 24 }),
            )
            .unwrap();
    }
    let scrolling = fixture.ui.widgets().scroll_area(list, true).unwrap();
    let mut scrolling_handle = scrolling.handle();
    let shell = fixture.ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let mut shell_handle = shell.handle();
    shell_handle
        .insert(
            0,
            fixture.ui.widgets().text_block("header", TextWrapping::None).unwrap(),
            LinearEdge::new(LayoutTrack::Fixed { extent: 24 }),
        )
        .unwrap();
    shell_handle.insert(1, scrolling, LinearEdge::new(LayoutTrack::Flex { weight: 1.0 })).unwrap();
    shell_handle
        .insert(
            2,
            fixture.ui.widgets().text_block("footer", TextWrapping::None).unwrap(),
            LinearEdge::new(LayoutTrack::Fixed { extent: 24 }),
        )
        .unwrap();
    let mut popup = fixture.ui.popup(owner.as_com_ref(), "list", shell).unwrap();
    let trigger = Rect { x: 100, y: 100, width: 120, height: 24 };
    popup.show_anchored(trigger, PopupSide::Below).unwrap();
    fixture.ui.update(SIZE).unwrap();
    let committed = fixture.popups()[0];
    let scroll = scrolling_handle.info().unwrap();
    assert!(committed.y + committed.height <= SIZE.height - 10);
    assert!(scroll.viewport.height > 0 && scroll.viewport.height <= committed.height - 48);
    assert!(scroll.maximum.y > 0);
    scrolling_handle.scroll_to_end().unwrap();
    fixture.ui.update(SIZE).unwrap();
    assert_eq!(scrolling_handle.info().unwrap().requested.y, scrolling_handle.info().unwrap().maximum.y);
    assert_eq!(popup.show_anchored(Rect { width: -1, ..trigger }, PopupSide::Below), Err(NC_E_INVALID_ARG));
    assert_eq!(popup.show_anchored(trigger, PopupSide::from_raw(255)), Err(neocogi_com::NC_E_INVALID_TAG));
    assert_eq!(popup.visibility().unwrap(), Visibility::Visible);
    fixture.ui.update(SIZE).unwrap();
    assert_eq!(fixture.popups()[0].height, committed.height);
    let (renderer, probe) = renderer();
    fixture.ui.render(SIZE, renderer.as_com_ref()).unwrap();
    assert!(
        probe
            .vertices
            .borrow()
            .iter()
            .all(|vertex| vertex.x >= 0.0 && vertex.x <= 640.0 && vertex.y >= 0.0 && vertex.y <= 400.0)
    );
}

/// Child-owned popups respect ancestor clipping; a finite tiny/zero frame never becomes unbounded.
#[test]
fn inherited_clips_tiny_frames_and_resize_keep_popup_geometry_finite() {
    let fixture = PopupFixture::new(10);
    let owner = fixture.owner();
    owner.set_rect(Rect { x: 0, y: 0, width: 180, height: 90 }).unwrap();
    owner
        .set_options(WindowOptions::NO_TITLE | WindowOptions::NO_PADDING | WindowOptions::NO_RESIZE)
        .unwrap();
    owner.set_child_clip(ChildClip::Content).unwrap();
    let child_root = fixture.ui.widgets().text_block("a", TextWrapping::None).unwrap();
    let child = fixture
        .ui
        .child_window(owner.as_com_ref(), "child", Rect { x: 10, y: 10, width: 60, height: 60 }, child_root)
        .unwrap();
    let mut menu = fixture.choices(&child, 64);
    menu.show_at(Rect { x: 0, y: 0, width: 120, height: 24 }).unwrap();
    for size in [SIZE, Size { width: 3, height: 5 }, SIZE] {
        fixture.ui.update(size).unwrap();
        let popup = fixture.popups()[0];
        assert!(popup.x >= 0 && popup.y >= 0);
        assert!(popup.x + popup.width <= size.width.min(180));
        assert!(popup.y + popup.height <= size.height.min(90));
        if size.width == 3 {
            assert!(popup.height > 0, "tiny frame keeps a clipped positive body");
        }
    }
    // Global drawable zero is rejected by the existing runtime contract; test a
    // genuinely finite zero inherited clip inside a valid drawable instead.
    assert_eq!(fixture.ui.update(Size { width: 0, height: 0 }), Err(NC_E_INVALID_ARG));
    owner.set_rect(Rect { x: 0, y: 0, width: 0, height: 0 }).unwrap();
    fixture.ui.update(SIZE).unwrap();
    let empty = fixture.popups()[0];
    assert_eq!((empty.width, empty.height), (0, 0));
    key(&fixture.ui, KeyCode::End);
    key(&fixture.ui, KeyCode::Enter);
    fixture.ui.update(SIZE).unwrap();
    assert_eq!(
        menu.selection().unwrap(),
        ChoiceSelection::Selected(0),
        "an empty body cannot activate an invisible row"
    );
    assert_eq!(fixture.popups().len(), 1);
    owner.set_rect(Rect { x: 0, y: 0, width: 180, height: 90 }).unwrap();
    fixture.ui.update(SIZE).unwrap();
    assert!(fixture.popups()[0].height > 0);
    key(&fixture.ui, KeyCode::Escape);
    fixture.ui.update(SIZE).unwrap();
    assert!(fixture.popups().is_empty());
}

/// A cap-only edit preserves the active branch and transient candidate, including its visibility.
#[test]
fn changing_the_cap_retains_the_open_choice_path_and_visible_candidate() {
    let fixture = PopupFixture::new(10);
    let owner = fixture.owner();
    let mut entries = vec![ChoiceMenuEntry::Submenu {
        label: MenuTextRange { first: 0, count: 1 },
        children: 30,
    }];
    entries.extend((100..130).map(|id| ChoiceMenuEntry::Choice {
        id,
        label: MenuTextRange { first: 0, count: 1 },
    }));
    let mut menu = fixture
        .runtime
        .query::<IChoiceMenusAbi>()
        .unwrap()
        .create_choice_menu(
            owner.as_com_ref(),
            NcCopyFromBufferOffset::new(0, b"a"),
            NcCopyFromBufferOffset::new(0, &entries),
            ChoiceSelection::Selected(100),
            neocogi_com::NcValueRef::new(&EventBinding::Unobserved).unwrap(),
        )
        .unwrap();
    menu.show_at(Rect { x: 100, y: 100, width: 120, height: 24 }).unwrap();
    fixture.ui.update(SIZE).unwrap();
    key(&fixture.ui, KeyCode::End);
    fixture.ui.update(SIZE).unwrap();
    menu.set_max_visible_items(3).unwrap();
    fixture.ui.update(SIZE).unwrap();
    let popups = fixture.popups();
    assert_eq!(popups.len(), 2, "count edits must not rebuild/dismiss the current branch");
    assert_eq!(popups[1].height, 3 * 12 + 2);
    // The last candidate must remain visible after shrinking from ten rows to three.
    let position = Point {
        x: popups[1].x + 3,
        y: popups[1].y + 1 + 2 * 12 + 1,
    };
    fixture.ui.input(UiInput::Down { position, buttons: 1 }, "").unwrap();
    fixture.ui.input(UiInput::Up { position, buttons: 1 }, "").unwrap();
    fixture.ui.update(SIZE).unwrap();
    assert_eq!(menu.selection().unwrap(), ChoiceSelection::Selected(129));
}

/// Replacing a captured model or changing its track never sends a physical release behind it.
#[test]
fn invalidated_scrollbar_capture_consumes_the_release_tail() {
    for replace in [false, true] {
        let fixture = PopupFixture::new(10);
        let mut button = button(&fixture.runtime);
        button.replace_flags(WidgetOptions::NONE).unwrap();
        let probe = Rc::new(WidgetProbe::default());
        let leaf = RetainingWidget::into_box(RetainingWidget {
            button,
            runtime: fixture.runtime.clone(),
            probe: probe.clone(),
        });
        let owner = fixture
            .ui
            .window("underlying", RECT, neocogi_ui::OwnedWidget::from_interface(leaf.query::<IWidgetAbi>().unwrap()))
            .unwrap();
        let mut menu = fixture.choices(&owner, 64);
        menu.show_at(Rect { x: 100, y: 100, width: 120, height: 24 }).unwrap();
        fixture.ui.update(SIZE).unwrap();
        let popup = fixture.popups()[0];
        let thumb = Point {
            x: popup.x + popup.width - 4,
            y: popup.y + 3,
        };
        fixture.ui.input(UiInput::Down { position: thumb, buttons: 1 }, "").unwrap();
        fixture.ui.update(SIZE).unwrap();
        if replace {
            let entries = [ChoiceMenuEntry::Choice {
                id: 200,
                label: MenuTextRange { first: 0, count: 1 },
            }];
            menu.replace(
                NcCopyFromBufferOffset::new(0, b"a"),
                NcCopyFromBufferOffset::new(0, &entries),
                ChoiceSelection::Selected(200),
            )
            .unwrap();
        } else {
            menu.set_max_visible_items(1).unwrap();
        }
        fixture.ui.update(SIZE).unwrap();
        probe.events.borrow_mut().clear();
        fixture
            .ui
            .input(
                UiInput::Up {
                    position: Point { x: 40, y: 80 },
                    buttons: 1,
                },
                "",
            )
            .unwrap();
        fixture.ui.update(SIZE).unwrap();
        assert!(
            !probe.events.borrow().iter().any(|event| matches!(event, WidgetEvent::Up { .. })),
            "the revoked menu still owns its release tail"
        );
        menu.hide().unwrap();
        fixture.ui.update(SIZE).unwrap();
        fixture
            .ui
            .input(
                UiInput::Down {
                    position: Point { x: 40, y: 80 },
                    buttons: 1,
                },
                "",
            )
            .unwrap();
        fixture
            .ui
            .input(
                UiInput::Up {
                    position: Point { x: 40, y: 80 },
                    buttons: 1,
                },
                "",
            )
            .unwrap();
        fixture.ui.update(SIZE).unwrap();
        assert!(
            probe.events.borrow().iter().any(|event| matches!(event, WidgetEvent::Up { .. })),
            "fresh gestures route normally after revocation"
        );
    }
}
