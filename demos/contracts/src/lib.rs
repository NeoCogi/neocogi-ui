//! Generated application-specific contracts shared by demo hosts and renderers.
//!
//! External math types are converted by ordinary Rust helper implementations;
//! the ABI itself contains only IDL-owned copied vector and matrix layouts.

#![no_std]
#![deny(missing_docs)]

include!(concat!(env!("OUT_DIR"), "/neocogi_demo.rs"));

/// Compatibility namespace for the demo extension's generated declarations.
pub mod extension {
    pub use crate::{Counter, CounterAbi, ExtensionProvider, ExtensionProviderAbi, IInset, IInsetAbi, IWidgetProbe, IWidgetProbeAbi, WidgetCalls};
}

mod helpers;
