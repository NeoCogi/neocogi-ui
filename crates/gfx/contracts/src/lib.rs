//! Renderer-neutral pixel decoding and portable resource-baking contracts.
//!
//! GFX services own CPU decoding, font rasterization, and atlas packing. They
//! own the immutable portable visual bundle consumed by UI. Services return
//! that bundle or renderer-owned textures and expose no JSON syntax tree,
//! theme-selection policy, UI tree, or native graphics handle.

#![no_std]
#![deny(missing_docs)]

include!(concat!(env!("OUT_DIR"), "/neocogi_gfx.rs"));

mod helpers;
pub use helpers::*;
