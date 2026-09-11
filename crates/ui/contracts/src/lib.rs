//! Generated COM contracts plus Rust-only conveniences for UI consumers.
//!
//! The `.ncidl` source owns every ABI declaration and its stable identity. This
//! crate adds only ordinary Rust validation, defaults, and semantic helpers;
//! none of those conveniences are stored in the native contract description.

#![no_std]
#![deny(missing_docs)]

include!(concat!(env!("OUT_DIR"), "/neocogi_ui.rs"));

/// Semantic appearance roles and state ordering shared by assets and widgets.
pub mod appearance;

#[path = "widget_support.rs"]
/// Rust-only widget implementation state built on generated contract values.
pub mod widget;

/// Named flat-palette authoring values generated from the canonical IDL.
pub mod palette {
    pub use crate::FlatPalette;
}

mod helpers;
pub use helpers::*;

/// Semantic names shared by asset recipes and runtime resource resolution.
mod resource_roles;

/// Plain layout and window geometry used by theme authoring and resolved skins.
mod skin;
pub use skin::{CaptionButtonSide, CaptionButtonsSkin, SkinMetrics, TitleBackdropSkin, WindowChromeSkin, WindowTitleAlignment};
