//! Statically linked, typed Rust clients for microui COM plugins.
//!
//! Widget and application generics remain local to the host; implementation
//! state, retained topology, native windows, and GPU ownership live in plugins.
//! This crate depends only on the COM object support and shared UI contracts.

#![deny(missing_docs)]

/// Typed clients and concrete UI interface definitions.
pub mod com;
pub use com::*;
