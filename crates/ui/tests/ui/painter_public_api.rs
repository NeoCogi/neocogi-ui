//! Applications record retained drawing through Painter rather than owning a display list.

use neocogi_ui::runtime::prelude::{color, rect};
use neocogi_ui::runtime::render::Painter;

#[allow(dead_code)]
fn record_supported_drawing(painter: &mut Painter<'_>) {
    // A concrete public operation proves Painter and its argument types remain importable.
    painter.fill_rect(rect(0, 0, 8, 8), color(255, 255, 255, 255)).unwrap();
}

fn main() {}
