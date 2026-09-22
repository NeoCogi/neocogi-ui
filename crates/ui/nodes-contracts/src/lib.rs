//! Generated concrete built-in widget and container COM contracts.
//!
//! Shared node, layout, popup, and host protocols remain in `neocogi.ui.core`.
//! This module owns concrete class and factory identities only.

#![no_std]
#![deny(missing_docs)]

include!(concat!(env!("OUT_DIR"), "/neocogi_ui_nodes.include.rs"));

mod helpers;
