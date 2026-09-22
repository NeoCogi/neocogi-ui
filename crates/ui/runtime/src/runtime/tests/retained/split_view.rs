//! Split behavior crosses the actual factory, topology, routing, and geometry boundaries.
use super::*;

/// Unequal minima expose reversed pane constraints in either orientation.
fn settings(axis: SplitAxis) -> SplitOptions {
    SplitOptions {
        axis,
        position: 120,
        minimum_first: 80,
        minimum_second: 100,
        handle_width: 8,
    }
}

/// Drags the real divider, observes child allocations, and preserves requests across window resize.
#[test]
fn split_drag_clamps_panes_and_preserves_requested_width_on_resize() {
    let mut fixture = Headless::standard();
    let (first, first_state) = probe::<0>(&fixture.ui, Size { width: 10, height: 10 }, WidgetOptions::NONE);
    let (second, second_state) = probe::<0>(&fixture.ui, Size { width: 10, height: 10 }, WidgetOptions::NONE);
    let (send, events) = std::sync::mpsc::channel();
    let mut split = fixture
        .ui
        .widgets()
        .split_view(settings(SplitAxis::Horizontal), send, move |neocogi_ui::split_view::Event::Changed(event)| {
            event.position
        })
        .unwrap();
    split.insert(0, &first).unwrap().finish().unwrap();
    split.insert(1, &second).unwrap().finish().unwrap();

    let owner = window(&fixture.ui, "split", Rect { x: 10, y: 10, width: 400, height: 120 }, &split);
    fixture.frame(VIEW).unwrap();
    assert_eq!(first_state.borrow().paints.last().unwrap().rect.width, 120);
    assert_eq!(second_state.borrow().paints.last().unwrap().rect.width, 272);

    // A pane press must never start divider capture through ancestor bubbling.
    pointer(&fixture.ui, Point { x: 50, y: 40 }, true);
    fixture.ui.input(UiInput::Pointer(Point { x: 180, y: 40 }), "").unwrap();
    pointer(&fixture.ui, Point { x: 180, y: 40 }, false);
    fixture.frame(VIEW).unwrap();
    assert_eq!(split.settings().unwrap().position, 120);
    assert!(events.try_recv().is_err());

    // Grab four pixels into the handle: no jump on press, including under capture.
    pointer(&fixture.ui, Point { x: 134, y: 40 }, true);
    fixture.frame(VIEW).unwrap();
    assert_eq!(split.settings().unwrap().position, 120);
    fixture.ui.input(UiInput::Pointer(Point { x: 234, y: 40 }), "").unwrap();
    fixture.frame(VIEW).unwrap();
    assert_eq!(split.settings().unwrap().position, 220);
    assert_eq!(events.try_recv().unwrap(), 220);
    assert_eq!(first_state.borrow().paints.last().unwrap().rect.width, 220);
    assert_eq!(second_state.borrow().paints.last().unwrap().rect.width, 172);

    // Capture carries the gesture beyond the window and clamps the second pane.
    fixture.ui.input(UiInput::Pointer(Point { x: 600, y: 40 }), "").unwrap();
    pointer(&fixture.ui, Point { x: 600, y: 40 }, false);
    fixture.frame(VIEW).unwrap();
    assert_eq!(split.settings().unwrap().position, 292);
    assert_eq!(events.try_recv().unwrap(), 292);
    assert_eq!(second_state.borrow().paints.last().unwrap().rect.width, 100);

    owner.set_rect(Rect { x: 10, y: 10, width: 98, height: 120 }).unwrap();
    fixture.frame(VIEW).unwrap();
    assert_eq!(first_state.borrow().paints.last().unwrap().rect.width, 40);
    assert_eq!(second_state.borrow().paints.last().unwrap().rect.width, 50);
    assert_eq!(split.settings().unwrap().position, 292);
    pointer(&fixture.ui, Point { x: 54, y: 40 }, true);
    fixture.ui.input(UiInput::Pointer(Point { x: 75, y: 40 }), "").unwrap();
    pointer(&fixture.ui, Point { x: 75, y: 40 }, false);
    fixture.frame(VIEW).unwrap();
    assert_eq!(split.settings().unwrap().position, 292, "an immobile divider must preserve the wider request");
    owner.set_rect(Rect { x: 10, y: 10, width: 500, height: 120 }).unwrap();
    fixture.frame(VIEW).unwrap();
    assert_eq!(first_state.borrow().paints.last().unwrap().rect.width, 292);
    assert!(events.try_recv().is_err(), "allocation clamping must not publish a user edit");
}

/// Focused arrows follow spatial direction, while cancellation and disable retire old gestures.
#[test]
fn split_keyboard_and_capture_loss_use_core_routing_on_both_axes() {
    for axis in [SplitAxis::Horizontal, SplitAxis::Vertical] {
        let mut fixture = Headless::standard();
        let mut split = fixture.ui.widgets().split_view_unobserved(settings(axis)).unwrap();
        for index in 0..2 {
            split
                .insert(index, &fixture.ui.widgets().text_block("pane", TextWrapping::None).unwrap())
                .unwrap()
                .finish()
                .unwrap();
        }
        let rect = match axis {
            SplitAxis::Horizontal => Rect { x: 10, y: 10, width: 400, height: 120 },
            _ => Rect { x: 10, y: 10, width: 120, height: 350 },
        };
        let _owner = window(&fixture.ui, "keyboard", rect, &split);
        fixture.frame(VIEW).unwrap();
        key(&fixture.ui, KeyCode::Tab, 0, 0);
        key(
            &fixture.ui,
            if axis == SplitAxis::Horizontal {
                KeyCode::ArrowRight
            } else {
                KeyCode::ArrowDown
            },
            0,
            0,
        );
        fixture.frame(VIEW).unwrap();
        assert_eq!(split.settings().unwrap().position, 128);
        let grab = match axis {
            SplitAxis::Horizontal => Point { x: 142, y: 40 },
            _ => Point { x: 40, y: 142 },
        };
        pointer(&fixture.ui, grab, true);
        fixture.frame(VIEW).unwrap();
        fixture.ui.input(UiInput::Cancel, "").unwrap();
        fixture.ui.input(UiInput::Pointer(Point { x: 300, y: 300 }), "").unwrap();
        fixture.frame(VIEW).unwrap();
        assert_eq!(split.settings().unwrap().position, 128);
        pointer(&fixture.ui, grab, true);
        fixture.frame(VIEW).unwrap();
        split.set_options(WidgetOptions::NO_INTERACT).unwrap();
        fixture.ui.input(UiInput::Pointer(Point { x: 300, y: 300 }), "").unwrap();
        pointer(&fixture.ui, Point { x: 300, y: 300 }, false);
        fixture.frame(VIEW).unwrap();
        split.set_options(WidgetOptions::NONE).unwrap();
        fixture.frame(VIEW).unwrap();
        assert_eq!(split.settings().unwrap().position, 128);
    }
}

/// Invalid construction/settings/attachments preserve state; removal and close retire owned panes.
#[test]
fn split_topology_and_settings_are_validated_before_mutation() {
    let mut fixture = Headless::standard();
    let initial = settings(SplitAxis::Horizontal);
    for bad in [
        SplitOptions {
            axis: SplitAxis::from_raw(255),
            ..initial
        },
        SplitOptions { position: -1, ..initial },
        SplitOptions { minimum_first: -1, ..initial },
        SplitOptions { minimum_second: -1, ..initial },
        SplitOptions { handle_width: 0, ..initial },
    ] {
        assert!(matches!(fixture.ui.widgets().split_view_unobserved(bad), Err(neocogi_com::NC_E_INVALID_ARG)));
    }
    let mut split = fixture.ui.widgets().split_view_unobserved(initial).unwrap();
    let (first, first_state) = probe::<0>(&fixture.ui, Size { width: 10, height: 10 }, WidgetOptions::NONE);
    let (second, second_state) = probe::<0>(&fixture.ui, Size { width: 10, height: 10 }, WidgetOptions::NONE);
    let extra = fixture.ui.widgets().button_unobserved("unattached").unwrap();
    split.insert(0, &first).unwrap().finish().unwrap();
    split.insert(1, &second).unwrap().finish().unwrap();
    assert!(matches!(split.insert(2, &extra), Err(neocogi_com::NC_E_OUT_OF_RANGE)));
    assert_eq!(split.children().unwrap().child_count().unwrap(), 2);
    assert_eq!(
        split.set_settings(SplitOptions { handle_width: -1, ..initial }),
        Err(neocogi_com::NC_E_INVALID_ARG)
    );
    assert_eq!(split.settings().unwrap(), initial);
    // Rejected insertion must leave the third node available to another owner.
    let mut other = fixture.ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    other.insert(0, &extra, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    let owner = window(&fixture.ui, "topology", Rect { x: 10, y: 10, width: 400, height: 120 }, &split);
    fixture.frame(VIEW).unwrap();
    split.remove(0).unwrap().finish().unwrap();
    fixture.frame(VIEW).unwrap();
    assert!(first_state.borrow().closed);
    assert_eq!(second_state.borrow().paints.last().unwrap().rect.width, 400);
    owner.close().unwrap();
    assert!(second_state.borrow().closed);
    assert_eq!(split.settings(), Err(UI_E_CLOSED));
}
