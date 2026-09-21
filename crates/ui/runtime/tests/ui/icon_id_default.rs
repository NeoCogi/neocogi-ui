//! An IconId cannot be fabricated without the atlas owner that validates its slot.

use nc_ui::IconId;

fn main() {
    let _icon = IconId::default();
}
