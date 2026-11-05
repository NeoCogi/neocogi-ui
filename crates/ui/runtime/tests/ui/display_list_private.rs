//! Applications must not bypass traversal by importing its implementation-owned display list.
#![allow(unused_imports)]

// A glob tests export visibility without a dependency-definition path in the diagnostic.
// Those paths differ between trimmed checkout builds and ordinary archived consumers.
use nc_ui::render::*;

fn main() {
    let _: Option<DisplayList> = None;
}
