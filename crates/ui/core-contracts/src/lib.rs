//! Generated core COM contracts plus Rust-only conveniences for UI consumers.
//!
//! The `.ncidl` source owns every ABI declaration and its stable identity. This
//! crate adds only ordinary Rust validation, defaults, and semantic helpers;
//! none of those conveniences are stored in the native contract description.

#![no_std]
#![deny(missing_docs)]

include!(concat!(env!("OUT_DIR"), "/neocogi_ui_core.rs"));

// Preserve the facade's established Rust import surface while declaration
// ownership and native metadata live exclusively in `neocogi.renderer`.
pub use neocogi_renderer_contracts::*;

mod helpers;
pub use helpers::*;
