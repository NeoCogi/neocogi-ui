//! A FontId cannot be fabricated without the atlas owner that validates its slot.

use neocogi_ui::runtime::FontId;

fn main() {
    let _font = FontId::default();
}
