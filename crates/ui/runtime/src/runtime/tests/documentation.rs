//! Public walkthroughs run against real providers instead of copied test examples.

#[path = "../../../../api/docs/button_events.rs"]
mod button_events;

/// The exact rustdoc source must mount, deliver, and expire its weak event origin.
#[test]
fn documented_button_walkthrough_activates_and_drops_queued_origin() {
    let ui = nc_ui_nodes::bind_builtin_nodes(super::runtime()).unwrap();
    button_events::walkthrough(&ui, super::SIZE).unwrap();
}
