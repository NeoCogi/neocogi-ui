//! Window, focus, capture and retained-layout policies observed through the public runtime.
use super::*;
use neocogi_ui_support::test_support::events::TestApp;

/// Places leaves in a production Linear whose allocations are fixed and easy to target.
fn column(ui: &Ui, children: &[Widget<IWidgetAbi>]) -> LinearWidget {
    let mut column = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    for (index, child) in children.iter().enumerate() {
        column
            .insert(index as u32, child, LinearEdge::new(LayoutTrack::Fixed { extent: 30 }))
            .unwrap()
            .finish()
            .unwrap();
    }
    column
}

/// Reads the last complete snapshot without retaining a callback borrow.
fn update(observed: &Rc<RefCell<Observations>>) -> WidgetUpdate {
    observed.borrow().updates.last().unwrap().0
}

/// Tab traversal follows retained sibling order, skips noninteractive nodes, and wraps both ways.
#[test]
fn sequential_focus_wraps_in_retained_order_and_skips_ineligible_nodes() {
    let mut fixture = Headless::standard();
    let (first, a) = probe::<2>(&fixture.ui, Size::default(), WidgetOptions::NONE);
    let (disabled, b) = probe::<2>(&fixture.ui, Size::default(), WidgetOptions::NO_INTERACT);
    let (last, c) = probe::<2>(&fixture.ui, Size::default(), WidgetOptions::NONE);
    let column = column(&fixture.ui, &[first, disabled, last]);
    let _window = window(&fixture.ui, "focus", Rect { x: 10, y: 10, width: 200, height: 140 }, &column);
    for (modifiers, expected) in [(0, [true, false]), (0, [false, true]), (0, [true, false]), (4, [false, true])] {
        key(&fixture.ui, KeyCode::Tab, 0, modifiers);
        fixture.frame(VIEW).unwrap();
        assert_eq!([update(&a).focused, update(&c).focused], expected);
        assert!(!update(&b).focused);
    }
}

/// A captured leaf continues receiving drag outside its root until release; keyboard focus persists.
#[test]
fn capture_is_exclusive_across_windows_and_release_preserves_keyboard_focus() {
    let mut fixture = Headless::standard();
    let (first, a) = probe::<1>(&fixture.ui, Size::default(), WidgetOptions::NONE);
    let (second, b) = probe::<1>(&fixture.ui, Size::default(), WidgetOptions::NONE);
    let _first = window(&fixture.ui, "first", Rect { x: 10, y: 10, width: 120, height: 90 }, &first);
    let _second = window(&fixture.ui, "second", Rect { x: 200, y: 10, width: 120, height: 90 }, &second);
    fixture.frame(VIEW).unwrap();
    pointer(&fixture.ui, Point { x: 30, y: 30 }, true);
    fixture.frame(VIEW).unwrap();
    fixture.ui.input(UiInput::Pointer(Point { x: 230, y: 30 }), "").unwrap();
    fixture.frame(VIEW).unwrap();
    assert!(update(&a).active);
    assert!(!update(&b).active);
    assert!(matches!(a.borrow().updates.last().unwrap().1, WidgetEvent::Drag { .. }));
    pointer(&fixture.ui, Point { x: 230, y: 30 }, false);
    fixture.frame(VIEW).unwrap();
    assert!(!update(&a).active);
    assert!(update(&a).focused);
    pointer(&fixture.ui, Point { x: 230, y: 30 }, true);
    fixture.frame(VIEW).unwrap();
    assert!(update(&b).active);
    assert!(!update(&a).active);
}

/// Host cancellation and hiding each revoke capture without resurrecting it when a root returns.
#[test]
fn cancel_and_visibility_changes_revoke_pointer_capture() {
    for hide in [false, true] {
        let mut fixture = Headless::standard();
        let (leaf, observed) = probe::<1>(&fixture.ui, Size::default(), WidgetOptions::NONE);
        let owner = window(&fixture.ui, "capture", Rect { x: 10, y: 10, width: 120, height: 90 }, &leaf);
        pointer(&fixture.ui, Point { x: 30, y: 30 }, true);
        fixture.frame(VIEW).unwrap();
        assert!(update(&observed).active);
        if hide {
            owner.set_visibility(Visibility::Hidden).unwrap();
            fixture.frame(VIEW).unwrap();
            owner.set_visibility(Visibility::Visible).unwrap();
        } else {
            fixture.ui.input(UiInput::Cancel, "").unwrap();
        }
        fixture.ui.input(UiInput::Pointer(Point { x: 70, y: 40 }), "").unwrap();
        fixture.frame(VIEW).unwrap();
        assert!(!update(&observed).active);
    }
}

/// Removing a focused/captured subtree cannot transfer its interaction identity to its replacement.
#[test]
fn replacing_a_captured_child_closes_it_and_does_not_transfer_focus() {
    let mut fixture = Headless::standard();
    let (first, a) = probe::<1>(&fixture.ui, Size::default(), WidgetOptions::NONE);
    let mut column = column(&fixture.ui, &[first]);
    let _owner = window(&fixture.ui, "replace", Rect { x: 10, y: 10, width: 200, height: 100 }, &column);
    pointer(&fixture.ui, Point { x: 30, y: 20 }, true);
    fixture.frame(VIEW).unwrap();
    assert!(update(&a).active);
    let (replacement, b) = probe::<1>(&fixture.ui, Size::default(), WidgetOptions::NONE);
    column
        .replace(0, &replacement, LinearEdge::new(LayoutTrack::Fixed { extent: 30 }))
        .unwrap()
        .finish()
        .unwrap();
    assert!(a.borrow().closed);
    fixture.ui.input(UiInput::Pointer(Point { x: 70, y: 20 }), "").unwrap();
    pointer(&fixture.ui, Point { x: 70, y: 20 }, false);
    fixture.frame(VIEW).unwrap();
    assert!(!update(&b).active);
    assert!(!update(&b).focused);
    assert!(
        !b.borrow()
            .updates
            .iter()
            .any(|(_, event)| matches!(event, WidgetEvent::Drag { .. } | WidgetEvent::Up { .. }))
    );
}

/// Sibling raising changes actual painter order inside a fixed layer, leaving higher bands last.
#[test]
fn raising_changes_compositor_order_only_within_the_windows_layer() {
    let mut fixture = Headless::standard();
    let mut windows = Vec::new();
    let colors = [0xff113355, 0xff224466, 0xff335577];
    for (index, color) in colors.into_iter().enumerate() {
        let (leaf, observed) = probe::<0>(&fixture.ui, Size::default(), WidgetOptions::NONE);
        observed.borrow_mut().color = Some(color);
        let owner = window(&fixture.ui, "order", Rect { x: 10, y: 10, width: 100, height: 80 }, &leaf);
        owner.set_layer(if index == 2 { 3 } else { 2 }).unwrap();
        windows.push(owner);
    }
    fixture.frame(VIEW).unwrap();
    let order = |fixture: &Headless| {
        let mut values: Vec<_> = fixture
            .snapshot()
            .vertices
            .iter()
            .map(|vertex| vertex.rgba)
            .filter(|value| colors.contains(value))
            .collect();
        values.dedup();
        values
    };
    assert_eq!(order(&fixture), colors);
    windows[0].raise().unwrap();
    fixture.frame(VIEW).unwrap();
    assert_eq!(order(&fixture), [colors[1], colors[0], colors[2]]);
}

/// Child windows inherit the owner's layer and reject independent assignment; dialogs are modal.
#[test]
fn managed_surface_layers_follow_family_policy_and_reject_independent_assignment() {
    let mut fixture = Headless::standard();
    let body = fixture.ui.widgets().button_unobserved("parent").unwrap();
    let owner = window(&fixture.ui, "owner", Rect { x: 10, y: 10, width: 200, height: 150 }, &body);
    let child_body = fixture.ui.widgets().button_unobserved("child").unwrap();
    let child = fixture
        .ui
        .child_window(owner.as_com_ref(), "child", Rect { x: 20, y: 20, width: 100, height: 80 }, &child_body)
        .unwrap();
    let dialog_body = fixture.ui.widgets().button_unobserved("dialog").unwrap();
    let dialog = fixture
        .ui
        .dialog(owner.as_com_ref(), "dialog", Rect { x: 30, y: 30, width: 100, height: 80 }, &dialog_body)
        .unwrap();
    owner.set_layer(2).unwrap();
    assert_eq!(child.snapshot().unwrap().layer.number, 2);
    assert_eq!(child.set_layer(3), Err(UI_E_POLICY));
    assert_eq!(dialog.set_layer(3), Err(UI_E_POLICY));
    assert_eq!(dialog.snapshot().unwrap().layer.kind, LayerKind::Modal);
    owner.set_layer(4).unwrap();
    assert_eq!(child.snapshot().unwrap().layer.number, 4);
    fixture.frame(VIEW).unwrap();
}

/// An outside press dismisses the popup before selecting and delivering to the revealed owner.
#[test]
fn outside_popup_press_dismisses_then_routes_once_and_popup_reuses_its_content() {
    let mut fixture = Headless::standard();
    let (leaf, observed) = probe::<1>(&fixture.ui, Size::default(), WidgetOptions::NONE);
    let owner = window(&fixture.ui, "owner", Rect { x: 10, y: 10, width: 250, height: 180 }, &leaf);
    let (popup_leaf, popup_observed) = probe::<1>(&fixture.ui, Size::default(), WidgetOptions::NONE);
    let mut popup = fixture.ui.popup(owner.as_com_ref(), "popup", &popup_leaf).unwrap();
    popup.set_options(PLAIN).unwrap();
    let anchor = Rect { x: 100, y: 60, width: 100, height: 80 };
    popup.show_at(anchor).unwrap();
    fixture.frame(VIEW).unwrap();
    pointer(&fixture.ui, Point { x: 30, y: 30 }, true);
    fixture.frame(VIEW).unwrap();
    assert_eq!(popup.visibility(), Ok(Visibility::Hidden));
    assert!(update(&observed).active);
    assert_eq!(
        observed
            .borrow()
            .updates
            .iter()
            .filter(|(_, event)| matches!(event, WidgetEvent::Down { .. }))
            .count(),
        1
    );
    assert!(!popup_observed.borrow().closed);
    pointer(&fixture.ui, Point { x: 30, y: 30 }, false);
    popup.show_at(anchor).unwrap();
    fixture.frame(VIEW).unwrap();
    assert_eq!(popup.visibility(), Ok(Visibility::Visible));
    owner.close().unwrap();
    assert_eq!(popup.show_at(anchor), Err(UI_E_CLOSED));
    assert!(popup_observed.borrow().closed);
}

/// Showing another popup dismisses the previous branch atomically, and hiding the owner dismisses both.
#[test]
fn one_popup_branch_is_active_per_runtime_and_owner_visibility_controls_it() {
    let mut fixture = Headless::standard();
    let body = fixture.ui.widgets().button_unobserved("body").unwrap();
    let owner = window(&fixture.ui, "owner", Rect { x: 10, y: 10, width: 250, height: 180 }, &body);
    let first = fixture.ui.widgets().button_unobserved("first").unwrap();
    let second = fixture.ui.widgets().button_unobserved("second").unwrap();
    let mut a = fixture.ui.popup(owner.as_com_ref(), "a", &first).unwrap();
    let mut b = fixture.ui.popup(owner.as_com_ref(), "b", &second).unwrap();
    a.show_at(Rect { x: 40, y: 40, width: 100, height: 80 }).unwrap();
    b.show_at(Rect { x: 60, y: 60, width: 100, height: 80 }).unwrap();
    assert_eq!(a.visibility(), Ok(Visibility::Hidden));
    assert_eq!(b.visibility(), Ok(Visibility::Visible));
    owner.set_visibility(Visibility::Hidden).unwrap();
    assert_eq!(b.visibility(), Ok(Visibility::Hidden));
    owner.set_visibility(Visibility::Visible).unwrap();
    fixture.frame(VIEW).unwrap();
    assert_eq!(b.visibility(), Ok(Visibility::Hidden));
}

/// Each nested child inherits the intersection of ancestor content clips in drawable coordinates.
#[test]
fn child_clipping_accumulates_and_can_be_disabled_without_changing_child_geometry() {
    for clipping in [ChildClip::Content, ChildClip::None] {
        let mut fixture = Headless::standard();
        let body = fixture.ui.widgets().button_unobserved("body").unwrap();
        let owner = window(&fixture.ui, "owner", Rect { x: 20, y: 20, width: 100, height: 100 }, &body);
        owner.set_child_clip(clipping).unwrap();
        let (leaf, observed) = probe::<0>(&fixture.ui, Size::default(), WidgetOptions::NONE);
        let child = fixture
            .ui
            .child_window(owner.as_com_ref(), "child", Rect { x: 100, y: 100, width: 80, height: 80 }, &leaf)
            .unwrap();
        child.set_options(PLAIN).unwrap();
        fixture.frame(VIEW).unwrap();
        let state = update(&observed);
        assert_eq!((state.screen.x, state.screen.y, state.screen.width, state.screen.height), (100, 100, 80, 80));
        let extent = if clipping == ChildClip::Content { 20 } else { 80 };
        assert_eq!((state.clip.x, state.clip.y, state.clip.width, state.clip.height), (0, 0, extent, extent));
    }
}

/// Failed cross-runtime creation does not consume the content's only mount claim.
#[test]
fn foreign_surface_owner_rejection_preserves_content_for_retry() {
    let mut fixture = Headless::standard();
    let other = Headless::standard();
    let foreign_body = other.ui.widgets().button_unobserved("foreign").unwrap();
    let foreign = window(&other.ui, "foreign", Rect { x: 0, y: 0, width: 100, height: 80 }, &foreign_body);
    let candidate = fixture.ui.widgets().button_unobserved("candidate").unwrap();
    assert_eq!(
        fixture
            .ui
            .child_window(foreign.as_com_ref(), "bad", Rect { x: 0, y: 0, width: 100, height: 80 }, &candidate)
            .unwrap_err(),
        UI_E_OWNER
    );
    let owner = window(&fixture.ui, "retry", Rect { x: 10, y: 10, width: 100, height: 80 }, &candidate);
    fixture.frame(VIEW).unwrap();
    owner.close().unwrap();
    assert_eq!(candidate.label(), Err(UI_E_CLOSED));
}

/// A deep family changes visibility and inherited layer consistently without exposing private roots.
#[test]
fn deep_window_family_preserves_inherited_policy_through_mutation_and_teardown() {
    let mut fixture = Headless::standard();
    let body = fixture.ui.widgets().button_unobserved("root").unwrap();
    let root = window(&fixture.ui, "root", Rect { x: 10, y: 10, width: 120, height: 100 }, &body);
    let mut deepest = root.clone();
    for _ in 0..48 {
        let body = fixture.ui.widgets().button_unobserved("child").unwrap();
        deepest = fixture
            .ui
            .child_window(deepest.as_com_ref(), "child", Rect { x: 10, y: 10, width: 100, height: 80 }, &body)
            .unwrap();
    }
    root.set_layer(3).unwrap();
    assert_eq!(deepest.snapshot().unwrap().layer.number, 3);
    root.set_visibility(Visibility::Hidden).unwrap();
    assert_eq!(deepest.snapshot().unwrap().visibility, Visibility::Hidden);
    root.set_visibility(Visibility::Visible).unwrap();
    fixture.frame(VIEW).unwrap();
    assert_eq!(deepest.snapshot().unwrap().visibility, Visibility::Visible);
    root.close().unwrap();
    assert_eq!(deepest.snapshot().unwrap_err(), UI_E_CLOSED);
}

/// Title dragging produces copied geometry events; the close affordance hides the retained window.
#[test]
fn title_drag_and_close_deliver_window_events_through_the_production_session() {
    let mut fixture = Headless::standard();
    let body = fixture.ui.widgets().button_unobserved("body").unwrap();
    let owner = fixture.ui.window("chrome", Rect { x: 30, y: 30, width: 180, height: 120 }, &body).unwrap();
    let (events_tx, events) = std::sync::mpsc::channel();
    let observations = Vec::new();
    neocogi_ui::events::bind(neocogi_ui::events::window_events(owner.as_com_ref()).unwrap(), events_tx.clone(), |event| event).unwrap();
    fixture.frame(VIEW).unwrap();
    pointer(&fixture.ui, Point { x: 70, y: 40 }, true);
    fixture.ui.input(UiInput::Pointer(Point { x: 80, y: 48 }), "").unwrap();
    pointer(&fixture.ui, Point { x: 80, y: 48 }, false);
    let app = TestApp::new(observations, events, |state, message| {
        state.push(message);
        Ok(())
    });
    fixture.ui.update_with_events(VIEW, app.as_interface()).unwrap();
    let moved = owner.snapshot().unwrap().rect;
    assert_eq!((moved.x, moved.y, moved.width, moved.height), (40, 38, 180, 120));
    assert!(matches!(app.state().as_slice(), [WindowEvent::GeometryChanged { rect }] if *rect == moved));
    // The default recipe puts its sole caption control at the trailing edge of the title.
    pointer(
        &fixture.ui,
        Point {
            x: moved.x + moved.width - 10,
            y: moved.y + 10,
        },
        true,
    );
    pointer(
        &fixture.ui,
        Point {
            x: moved.x + moved.width - 10,
            y: moved.y + 10,
        },
        false,
    );
    fixture.ui.update_with_events(VIEW, app.as_interface()).unwrap();
    assert_eq!(owner.snapshot().unwrap().visibility, Visibility::Hidden);
    assert!(matches!(
        app.state().as_slice(),
        [WindowEvent::GeometryChanged { .. }, WindowEvent::Hidden, WindowEvent::CloseRequested]
    ));
    assert!(body.label().is_ok());
}

/// Right, bottom and corner capture change only their declared resize axes.
#[test]
fn chrome_resize_regions_preserve_unselected_axes() {
    for (position, width, height) in [
        (Point { x: 209, y: 90 }, 195, 120),
        (Point { x: 100, y: 149 }, 180, 129),
        (Point { x: 209, y: 149 }, 195, 129),
    ] {
        let mut fixture = Headless::standard();
        let (leaf, observed) = probe::<1>(&fixture.ui, Size::default(), WidgetOptions::NONE);
        let owner = fixture.ui.window("resize", Rect { x: 30, y: 30, width: 180, height: 120 }, &leaf).unwrap();
        fixture.frame(VIEW).unwrap();
        pointer(&fixture.ui, position, true);
        fixture.ui.input(UiInput::Pointer(Point { x: position.x + 15, y: position.y + 9 }), "").unwrap();
        pointer(&fixture.ui, Point { x: position.x + 15, y: position.y + 9 }, false);
        fixture.frame(VIEW).unwrap();
        let rect = owner.snapshot().unwrap().rect;
        assert_eq!((rect.x, rect.y, rect.width, rect.height), (30, 30, width, height));
        assert!(
            !observed
                .borrow()
                .updates
                .iter()
                .any(|(_, event)| matches!(event, WidgetEvent::Down { .. } | WidgetEvent::Drag { .. }))
        );
    }
}

/// A visible dialog is the only pointer group; hiding or raising dialogs changes that group.
#[test]
fn modal_stack_suspends_underlying_pointer_routing_and_reveals_the_next_dialog() {
    let mut fixture = Headless::standard();
    let (body, background) = probe::<1>(&fixture.ui, Size::default(), WidgetOptions::NONE);
    let owner = window(&fixture.ui, "owner", Rect { x: 10, y: 10, width: 250, height: 180 }, &body);
    let (first, a) = probe::<1>(&fixture.ui, Size::default(), WidgetOptions::NONE);
    let (second, b) = probe::<1>(&fixture.ui, Size::default(), WidgetOptions::NONE);
    let rect = Rect { x: 100, y: 60, width: 100, height: 80 };
    let first = fixture.ui.dialog(owner.as_com_ref(), "first", rect, &first).unwrap();
    let second = fixture.ui.dialog(owner.as_com_ref(), "second", rect, &second).unwrap();
    for dialog in [&first, &second] {
        dialog.set_options(PLAIN).unwrap();
        dialog.set_visibility(Visibility::Visible).unwrap();
    }
    pointer(&fixture.ui, Point { x: 30, y: 30 }, true);
    pointer(&fixture.ui, Point { x: 30, y: 30 }, false);
    fixture.frame(VIEW).unwrap();
    assert!(!background.borrow().updates.iter().any(|(_, event)| matches!(event, WidgetEvent::Down { .. })));
    for expected_first in [false, true, false] {
        if expected_first {
            first.raise().unwrap();
        } else {
            first.set_visibility(Visibility::Hidden).unwrap();
        }
        pointer(&fixture.ui, Point { x: 130, y: 80 }, true);
        fixture.frame(VIEW).unwrap();
        assert!(update(if expected_first { &a } else { &b }).active);
        pointer(&fixture.ui, Point { x: 130, y: 80 }, false);
        fixture.frame(VIEW).unwrap();
        if !expected_first {
            first.set_visibility(Visibility::Visible).unwrap();
        }
    }
}

/// A disabled upper window still occludes lower windows, preventing click-through.
#[test]
fn disabled_window_blocks_pointer_delivery_to_lower_layers() {
    let mut fixture = Headless::standard();
    let (low, low_state) = probe::<1>(&fixture.ui, Size::default(), WidgetOptions::NONE);
    let (high, high_state) = probe::<1>(&fixture.ui, Size::default(), WidgetOptions::NONE);
    let rect = Rect { x: 10, y: 10, width: 150, height: 100 };
    let lower = window(&fixture.ui, "lower", rect, &low);
    let upper = window(&fixture.ui, "upper", rect, &high);
    lower.set_layer(2).unwrap();
    upper.set_layer(3).unwrap();
    upper.set_options(PLAIN | WindowOptions::DISABLED).unwrap();
    pointer(&fixture.ui, Point { x: 40, y: 40 }, true);
    fixture.frame(VIEW).unwrap();
    assert!(!update(&low_state).active);
    assert!(!update(&high_state).active);
    assert!(!low_state.borrow().updates.iter().any(|(_, event)| matches!(event, WidgetEvent::Down { .. })));
}

/// Cached child measurements survive ordinary ticks and invalidate every affected ancestor on change.
#[test]
fn nested_measurement_cache_reuses_stable_constraints_and_commits_changed_preferences() {
    let mut fixture = Headless::standard();
    let (leaf, observed) = probe::<0>(&fixture.ui, Size { width: 40, height: 20 }, WidgetOptions::NONE);
    let mut inner = fixture.ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Content).unwrap();
    inner.insert(0, &leaf, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    let mut outer = fixture.ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Content).unwrap();
    outer.insert(0, &inner, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    let owner = window(&fixture.ui, "cache", Rect { x: 10, y: 10, width: 200, height: 100 }, &outer);
    owner.set_options(PLAIN | WindowOptions::AUTO_SIZE).unwrap();
    fixture.frame(VIEW).unwrap();
    let measured = observed.borrow().measures;
    for _ in 0..4 {
        fixture.frame(VIEW).unwrap();
    }
    assert_eq!(observed.borrow().measures, measured);
    {
        let mut observed = observed.borrow_mut();
        observed.preferred = Size { width: 80, height: 35 };
        observed.change = true;
    }
    fixture.frame(VIEW).unwrap();
    assert!(observed.borrow().measures > measured);
    assert_eq!((owner.snapshot().unwrap().rect.width, owner.snapshot().unwrap().rect.height), (80, 35));
}

/// A cancelled higher-layer gesture cannot intercept continuation of a new lower-layer press.
#[test]
fn fresh_press_replaces_revoked_capture_across_window_layers() {
    for hide in [false, true] {
        let mut fixture = Headless::standard();
        let (low, a) = probe::<1>(&fixture.ui, Size::default(), WidgetOptions::NONE);
        let (high, b) = probe::<1>(&fixture.ui, Size::default(), WidgetOptions::NONE);
        let lower = window(&fixture.ui, "lower", Rect { x: 10, y: 10, width: 120, height: 100 }, &low);
        let upper = window(&fixture.ui, "upper", Rect { x: 200, y: 10, width: 120, height: 100 }, &high);
        lower.set_layer(2).unwrap();
        upper.set_layer(3).unwrap();
        pointer(&fixture.ui, Point { x: 230, y: 30 }, true);
        fixture.frame(VIEW).unwrap();
        assert!(update(&b).active);
        if hide {
            upper.set_visibility(Visibility::Hidden).unwrap();
        } else {
            fixture.ui.input(UiInput::Cancel, "").unwrap();
        }
        pointer(&fixture.ui, Point { x: 30, y: 30 }, true);
        fixture.frame(VIEW).unwrap();
        if hide {
            upper.set_visibility(Visibility::Visible).unwrap();
        }
        fixture.ui.input(UiInput::Pointer(Point { x: 70, y: 40 }), "").unwrap();
        fixture.frame(VIEW).unwrap();
        assert!(matches!(a.borrow().updates.last().unwrap().1, WidgetEvent::Drag { .. }));
        assert!(update(&a).active);
        assert!(!update(&b).active);
    }
}
