//! An IconId cannot be fabricated without the atlas owner that validates its slot.

use neocogi_ui::runtime::IconId;

fn main() {
    let _icon = IconId::default();
}
