//! Generated COM contracts for optional UI asset-authoring services.
//!
//! The generated bindings re-export every explicitly imported `neocogi.ui.core`
//! declaration with its original identity. This crate declares only the
//! construction services implemented by the optional assets plugin.

#![no_std]
#![deny(missing_docs)]

include!(concat!(env!("OUT_DIR"), "/neocogi_ui_assets.rs"));
