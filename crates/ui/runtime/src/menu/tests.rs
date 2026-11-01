//
// Copyright 2023-Present (c) Raja Lehtihet & Wael El Oraiby
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
// this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
// this list of conditions and the following disclaimer in the documentation
// and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
// may be used to endorse or promote products derived from this software without
// specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
// LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
// CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
// SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
// CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGE.

//! Structural and geometry regressions for compact menu surfaces.

use super::*;

use crate::test_support::{test_atlas, test_skin};
use crate::ui_node::text_layout::control_text_position_with_font;

/// Proves that an explicitly empty bar cannot reserve a blank interactive strip.
#[test]
fn empty_bar_has_zero_geometry() {
    // All later phases consume this same cached layout, so zero measurement also means no slot can
    // paint, hit, or anchor a popup even if the surrounding linear shell stretches horizontally.
    let atlas = test_atlas();
    let style = test_skin(&atlas);
    let layout = layout_bar(&[], &style, &atlas, Vec::new());
    assert_eq!((layout.size.width, layout.size.height), (0, 0));
    assert!(layout.slots.is_empty());
    assert_eq!(layout.marker_width, 0);
}

/// Proves that the shared popup text region reserves unclipped, non-overlapping trailing content.
#[test]
fn popup_text_region_separates_labels_from_shortcuts_and_submenu_arrows() {
    // Use deterministic test-atlas glyph advances and include both trailing-content variants in
    // one popup so production layout must choose shared label and trailing maxima.
    let atlas = test_atlas();
    let style = test_skin(&atlas);
    let (_, item) = MenuItem::create(MenuItemParameters::new("aaaa").shortcut_hint("bbbb"), None);
    let rows = vec![MenuSlot::Item(item.record), MenuSlot::Branch { label: "aaaaaa".into() }];
    let layout = layout_popup(&rows, &style, &atlas, Vec::new());
    let padding = style.metrics().padding.max(1);

    // The item label and right-aligned shortcut are positioned by the same production helper and
    // clipped by the same production region. Their measured extents must fit without intersecting.
    let item_region = text_region(layout.slots[0].local_rect(0, 0), layout.marker_width);
    let item_font = style.resolve_font(&atlas, &FontRef::Role(FontRole::Body));
    let label_size = atlas.get_text_size(item_font, "aaaa");
    let hint_size = atlas.get_text_size(item_font, "bbbb");
    let label_position = control_text_position_with_font(&style, &atlas, item_font, "aaaa", item_region, WidgetOption::NONE);
    let hint_position = control_text_position_with_font(&style, &atlas, item_font, "bbbb", item_region, WidgetOption::ALIGN_RIGHT);
    let item_right = item_region.x + item_region.width;
    assert!(label_position.x >= item_region.x && label_position.x + label_size.width <= item_right);
    assert!(hint_position.x >= item_region.x && hint_position.x + hint_size.width <= item_right);
    assert!(label_position.x + label_size.width <= hint_position.x, "label and shortcut must not overlap");

    // A submenu label uses the identical horizontal region while its arrow uses the production
    // trailing rectangle. Both must stay inside the row, with at least the intended padding gap.
    let branch_row = layout.slots[1].local_rect(0, 0);
    let branch_region = text_region(branch_row, layout.marker_width);
    let branch_size = atlas.get_text_size(item_font, "aaaaaa");
    let branch_position = control_text_position_with_font(&style, &atlas, item_font, "aaaaaa", branch_region, WidgetOption::NONE);
    let arrow_size = atlas.get_icon_size(style.resolve_icon_role(&atlas, crate::IconRole::Expand));
    let arrow = trailing_rect(branch_region, arrow_size, padding);
    let branch_right = branch_position.x + branch_size.width;
    assert_eq!((item_region.x, item_region.width), (branch_region.x, branch_region.width));
    assert!(branch_right.saturating_add(padding) <= arrow.x, "submenu label and arrow must retain their gap");
    assert!(arrow.x >= branch_region.x && arrow.x + arrow.width <= branch_region.x + branch_region.width);
    assert!(arrow.y >= branch_row.y && arrow.y + arrow.height <= branch_row.y + branch_row.height);
}

/// Proves that compact menu traversal skips inert rows and wraps without a parallel index list.
#[test]
fn keyboard_selection_skips_disabled_items_and_separators_with_wrapping() {
    let (_, disabled) = MenuItem::create(MenuItemParameters::new("disabled").disabled(), None);
    let (_, enabled) = MenuItem::create(MenuItemParameters::new("enabled"), None);
    let rows = vec![
        MenuSlot::Item(disabled.record),
        MenuSlot::Separator,
        MenuSlot::Branch { label: "branch".into() },
        MenuSlot::Item(enabled.record),
    ];
    let mut surface = MenuSurface::new(rows, true);

    assert!(surface.focus_first());
    assert_eq!(surface.keyboard_slot(), Some(2));
    assert!(matches!(surface.activate_keyboard_slot(), Some(MenuAction::OpenSlot(2))));
    assert!(surface.move_keyboard_focus(true));
    assert_eq!(surface.keyboard_slot(), Some(3));
    assert!(surface.move_keyboard_focus(true));
    assert_eq!(surface.keyboard_slot(), Some(2), "forward traversal must wrap");
    assert!(surface.move_keyboard_focus(false));
    assert_eq!(surface.keyboard_slot(), Some(3), "reverse traversal must wrap");
}

/// Selection menus retain the committed leaf until pointer hover has a concrete row target.
#[test]
fn choice_hover_does_not_erase_committed_selection() {
    let atlas = test_atlas();
    let style = test_skin(&atlas);
    let rows = vec![
        MenuSlot::Choice {
            id: 11,
            label: "first".into(),
            enabled: true,
        },
        MenuSlot::Choice {
            id: 22,
            label: "second".into(),
            enabled: true,
        },
    ];
    let mut surface = MenuSurface::choices(rows, Some(11));
    let measured = surface.measure(&style, &atlas);
    surface.layout(Recti::new(10, 20, measured.width.max(100), measured.height), Recti::new(0, 0, 300, 300));
    assert_eq!(surface.highlighted_choice_slot(), Some(0));

    let second = surface.slot_rect(1).unwrap();
    let pointer = Vec2i::new(second.x + 1, second.y + 1);
    let route = surface.route_pointer(&UiInputEvent::MouseMove { pos: pointer, delta: Vec2i::new(0, 0) }, true);
    assert!(route.handled);
    assert_eq!(surface.highlighted_choice_slot(), Some(1));

    let outside = Vec2i::new(300, 300);
    surface.route_pointer(&UiInputEvent::MouseMove { pos: outside, delta: Vec2i::new(0, 0) }, true);
    assert_eq!(surface.highlighted_choice_slot(), Some(0));
}

/// Choice activation is stored as a typed leaf identity rather than a command-menu label event.
#[test]
fn choice_submission_and_model_replacement_are_distinct() {
    let rows = vec![
        MenuSlot::Choice {
            id: 11,
            label: "duplicate".into(),
            enabled: true,
        },
        MenuSlot::Choice {
            id: 22,
            label: "duplicate".into(),
            enabled: true,
        },
    ];
    let mut surface = MenuSurface::choices(rows, Some(11));
    assert!(surface.move_keyboard_focus(true));
    assert_eq!(surface.keyboard_slot(), Some(1));
    assert!(matches!(surface.activate_keyboard_slot(), Some(MenuAction::SubmitAndClose(Ok(())))));
    assert_eq!(surface.take_choice_submission(), Some(22));
    assert_eq!(surface.take_choice_submission(), None);

    assert!(surface.replace_choices(
        vec![MenuSlot::Choice {
            id: 33,
            label: "new".into(),
            enabled: true
        }],
        Some(33)
    ));
    assert_eq!(surface.highlighted_choice_slot(), Some(0));
    assert_eq!(surface.take_choice_submission(), None);
}

/// The count cap changes viewport size, never admission, and only overflow gets a gutter.
#[test]
fn choice_viewports_bound_rows_without_truncating_the_model() {
    let atlas = test_atlas();
    let style = test_skin(&atlas);
    for count in [0, 1, 9, 10, 11, 64] {
        let rows = (0..count).map(|id| MenuSlot::Choice { id, label: "a".into(), enabled: true }).collect();
        let mut menu = MenuSurface::choices(rows, None);
        let size = menu.measure(&style, &atlas);
        menu.layout(Recti::new(20, 30, 100, size.height), Recti::new(0, 0, 400, 400));
        assert_eq!(menu.visible_slots().len(), (count as usize).min(10));
        assert_eq!(menu.rows.len(), count as usize);
        assert_eq!(menu.scrollbar.is_some(), count > 10 && style.metrics().scrollbar_size > 0);
        let original = menu.keyboard_slot();
        if count > 10 {
            menu.route_pointer(
                &UiInputEvent::Scroll {
                    pos: Vec2i::new(22, 32),
                    delta: Vec2i::new(0, 1),
                },
                false,
            );
            assert!(menu.offset_y > 0);
            menu.layout(Recti::new(20, 30, 100, size.height), Recti::new(0, 0, 400, 400));
            assert_eq!(menu.keyboard_slot(), original);
            assert!(menu.offset_y > 0, "ordinary layout must not undo wheel movement");
            assert!(menu.take_choice_submission().is_none());
        }
        assert!(menu.set_visible_limit(3));
        let size = menu.measure(&style, &atlas);
        menu.layout(Recti::new(20, 30, 100, size.height), Recti::new(0, 0, 400, 400));
        assert_eq!(menu.visible_slots().len(), (count as usize).min(3));
        assert!(!menu.set_visible_limit(3));
    }
}

/// Variable command rows and uniform choice rows both preserve full aggregate coordinates.
#[test]
fn huge_menu_rows_stay_distinct_until_screen_translation() {
    let atlas = test_atlas();
    let style = test_skin(&atlas);
    let rows = (0..64).map(|id| MenuSlot::Choice { id, label: "a".into(), enabled: true }).collect();
    let mut menu = MenuSurface::choices(rows, Some(63));
    menu.measure(&style, &atlas);
    // Exercise the production committed geometry with a valid wide aggregate.
    for (index, row) in menu.geometry.slots.iter_mut().enumerate() {
        row.y = index as i64 * 40_000;
        row.height = 40_000;
    }
    menu.layout(
        Recti::new(20, 30, 120, 80_000 + menu.geometry.insets.vertical_extent()),
        Recti::new(0, 0, 640, 131_072),
    );
    assert!(menu.offset_y > i64::from(neocogi_ui::geometry::COORDINATE_LIMIT));
    assert_eq!(menu.visible_slots(), 62..64);
    let last = menu.slot_rect(63).unwrap();
    let previous = menu.slot_rect(62).unwrap();
    assert_eq!(last.y - previous.y, 40_000);
    assert!(matches!(
        menu.route_pointer(
            &UiInputEvent::MouseDown {
                pos: Vec2i::new(last.x + 1, last.y + 1),
                button: MouseButton::LEFT
            },
            false
        )
        .action,
        Some(MenuAction::SubmitAndClose(Ok(())))
    ));
    assert_eq!(menu.take_choice_submission(), Some(63));
}

/// Thumb drags preserve a single initial anchor and cannot submit an exposed row on release.
#[test]
fn menu_scrollbar_capture_has_no_row_activation_or_incremental_rounding() {
    let atlas = test_atlas();
    let style = test_skin(&atlas);
    let rows = (0..64).map(|id| MenuSlot::Choice { id, label: "a".into(), enabled: true }).collect();
    let mut menu = MenuSurface::choices(rows, None);
    menu.set_visible_limit(3);
    let size = menu.measure(&style, &atlas);
    let allocation = Recti::new(20, 30, 120, size.height);
    let clip = Recti::new(0, 0, 400, 400);
    menu.layout(allocation, clip);
    let Some(bar) = menu.scrollbar else {
        panic!("fixture requires a visible scrollbar")
    };
    let thumb = bar.thumb();
    let start = Vec2i::new(20 + thumb.x, 30 + thumb.y);
    menu.route_pointer(&UiInputEvent::MouseDown { pos: start, button: MouseButton::LEFT }, false);
    for distance in [1, 2, 10_000] {
        menu.route_pointer(
            &UiInputEvent::MouseDrag {
                pos: Vec2i::new(start.x, start.y + distance),
                delta: Vec2i::new(0, 1),
                buttons: MouseButton::LEFT,
            },
            false,
        );
        menu.layout(allocation, clip);
    }
    assert!(menu.has_capture());
    assert_eq!(menu.visible_slots().end, 64);
    menu.route_pointer(
        &UiInputEvent::MouseUp {
            pos: Vec2i::new(22, 32),
            button: MouseButton::LEFT,
        },
        false,
    );
    assert!(!menu.has_capture());
    assert!(menu.take_choice_submission().is_none());
    assert!(menu.focus_first());
    menu.layout(allocation, clip);
    assert_eq!(menu.offset_y, 0);
    assert!(menu.page_keyboard_focus(true));
    menu.layout(allocation, clip);
    assert!(menu.offset_y > 0);
    assert!(menu.slot_rect(menu.keyboard_slot().unwrap()).is_some());
}

/// Mixed row heights use the same wide content domain as uniform choice panels.
#[test]
fn variable_command_row_measurement_preserves_large_tail_positions() {
    let mut data = crate::test_support::test_asset_data_with_font_sizes(&[("body", 10)]);
    data.tables.metrics.spacing = 100_000_000;
    let atlas = AtlasHandle::from_validated_data(data, None).unwrap();
    let style = test_skin(&atlas);
    let mut rows = Vec::new();
    for _ in 0..32 {
        rows.push(MenuSlot::Separator);
        rows.push(MenuSlot::Branch { label: "a".into() });
    }
    let mut menu = MenuSurface::new(rows, true);
    menu.measure(&style, &atlas);
    assert!(menu.content_height() > i64::from(i32::MAX));
    menu.focus_last();
    menu.layout(Recti::new(10, 20, 120, 300), Recti::new(0, 0, 400, 400));
    let last = menu.slot_rect(63).unwrap();
    assert!(last.y >= 20 && last.y + last.height <= 320);
    assert!(menu.visible_slots().len() < 10);
    let route = menu.route_pointer(
        &UiInputEvent::MouseDown {
            pos: Vec2i::new(last.x + 1, last.y + 1),
            button: MouseButton::LEFT,
        },
        false,
    );
    assert!(matches!(route.action, Some(MenuAction::OpenSlot(63))));
}
