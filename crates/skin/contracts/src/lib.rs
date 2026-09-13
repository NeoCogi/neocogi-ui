//! Portable appearance and external theme-compilation contracts.
//!
//! Skin services transform immutable resource bundles. The ABI carries typed
//! palettes, metrics, native theme identity, and completed bundle capabilities;
//! parsing trees, JSON values, decoded pixels, and renderer handles stay private.

#![no_std]
#![deny(missing_docs)]

include!(concat!(env!("OUT_DIR"), "/neocogi_skin.rs"));
