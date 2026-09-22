//! Generated application-specific contracts for the independent widget extension.
//!
//! Mesh rendering belongs to the separate `neocogi.demo.render` module so a
//! renderer plugin never imports the UI widget contract graph.

#![no_std]
#![deny(missing_docs)]

include!(concat!(env!("OUT_DIR"), "/neocogi_demo.include.rs"));

mod helpers;
