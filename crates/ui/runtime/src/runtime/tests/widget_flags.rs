//! Flag mutations exercise generated COM calls, shared widget state and lifecycle authority.
use super::*;

/// Replacements, additions and removals compose on live state and retain pending invalidation.
#[test]
fn widget_flag_edits_preserve_requested_state_and_sticky_invalidation() {
    let runtime = runtime();
    let button = button(&runtime);
    let mut widget: WeakComBox<IWidgetAbi> = button.downgrade().query().unwrap();
    widget.take_change().unwrap();

    // Replacement clears omitted constructor bits, while selective edits preserve new ones.
    widget.replace_flags(WidgetOptions::ALIGN_RIGHT).unwrap();
    widget.set_flags(WidgetOptions::NO_INTERACT).unwrap();
    assert_eq!(button.properties().unwrap().options, WidgetOptions::ALIGN_RIGHT | WidgetOptions::NO_INTERACT);
    widget.clear_flags(WidgetOptions::NO_INTERACT).unwrap();
    assert_eq!(button.properties().unwrap().options, WidgetOptions::ALIGN_RIGHT);

    // Neither a redundant set, an absent clear nor an identical replacement may erase dirty state.
    widget.set_flags(WidgetOptions::ALIGN_RIGHT).unwrap();
    widget.clear_flags(WidgetOptions::FRAME).unwrap();
    widget.replace_flags(WidgetOptions::ALIGN_RIGHT).unwrap();
    assert_eq!(widget.take_change().unwrap(), GeometryChange::Changed);
    for _ in 0..2 {
        widget.set_flags(WidgetOptions::ALIGN_RIGHT).unwrap();
        widget.clear_flags(WidgetOptions::FRAME).unwrap();
        widget.replace_flags(WidgetOptions::ALIGN_RIGHT).unwrap();
        assert_eq!(widget.take_change().unwrap(), GeometryChange::NoChange);
    }

    // A real addition and removal each invalidate once; replacement intentionally clears all bits.
    widget.set_flags(WidgetOptions::FRAME).unwrap();
    assert_eq!(widget.take_change().unwrap(), GeometryChange::Changed);
    widget.clear_flags(WidgetOptions::FRAME).unwrap();
    assert_eq!(widget.take_change().unwrap(), GeometryChange::Changed);
    widget.replace_flags(WidgetOptions::NONE).unwrap();
    assert_eq!(button.properties().unwrap().options, WidgetOptions::NONE);
    assert_eq!(widget.take_change().unwrap(), GeometryChange::Changed);
}

/// Unknown masks fail before mutation, and every operation respects object and host closure.
#[test]
fn widget_flag_edits_reject_unknown_masks_and_closed_objects() {
    let runtime = runtime();
    let owner = button(&runtime);
    let mut widget: WeakComBox<IWidgetAbi> = owner.downgrade().query().unwrap();
    widget.take_change().unwrap();
    widget.set_flags(WidgetOptions::ALIGN_RIGHT).unwrap();
    let before = widget.properties().unwrap();
    for mask in [WidgetOptions::from_raw(8), WidgetOptions::from_raw(8) | WidgetOptions::FRAME] {
        assert_eq!(widget.replace_flags(mask), Err(neocogi_com::NC_E_INVALID_TAG));
        assert_eq!(widget.set_flags(mask), Err(neocogi_com::NC_E_INVALID_TAG));
        // Clearing an unknown bit must fail even though masking it out could appear harmless.
        assert_eq!(widget.clear_flags(mask), Err(neocogi_com::NC_E_INVALID_TAG));
        assert_eq!(widget.properties().unwrap(), before);
    }
    assert_eq!(widget.take_change().unwrap(), GeometryChange::Changed);
    assert_eq!(widget.clear_flags(WidgetOptions::from_raw(8)), Err(neocogi_com::NC_E_INVALID_TAG));
    assert_eq!(widget.take_change().unwrap(), GeometryChange::NoChange);

    drop(owner);
    assert_eq!(widget.replace_flags(WidgetOptions::NONE), Err(neocogi_com::NC_E_DROPPED));
    assert_eq!(widget.set_flags(WidgetOptions::FRAME), Err(neocogi_com::NC_E_DROPPED));
    assert_eq!(widget.clear_flags(WidgetOptions::NONE), Err(neocogi_com::NC_E_DROPPED));

    // Unmounted handles are also invalidated when their allocating runtime closes.
    let mut detached: ComBox<IWidgetAbi> = button(&runtime).query().unwrap();
    runtime.close().unwrap();
    assert_eq!(detached.replace_flags(WidgetOptions::NONE), Err(UI_E_CLOSED));
    assert_eq!(detached.set_flags(WidgetOptions::FRAME), Err(UI_E_CLOSED));
    assert_eq!(detached.clear_flags(WidgetOptions::NONE), Err(UI_E_CLOSED));
}
