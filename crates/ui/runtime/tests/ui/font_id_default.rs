//! A FontId cannot be fabricated without the atlas owner that validates its slot.

use nc_ui::FontId;

fn main() {
    let _font = FontId::default();
}
