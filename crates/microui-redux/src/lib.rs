//! Statically linked, typed Rust clients for microui COM plugins.
//!
//! Widget and application generics remain local to the host; implementation
//! state, retained topology, native windows, and GPU ownership live in plugins.
//! This crate depends only on the COM object support and shared UI contracts.

#![deny(missing_docs)]

/// Typed clients and concrete UI interface definitions.
pub mod com;
pub use com::*;

/// Application-owned typed semantic events and deterministic COM delivery.
pub mod events;
pub use events::{EventSession, EventHandler};

/// Exact native-path snapshots and static platform conversion.
pub mod path;
pub use path::NativePath;

/// Compact menu declarations and live item clients.
pub mod menu;
pub use menu::{Menus, MenuBuilder, MenuItem};

/// Standard native-path file-picker requests and clients.
pub mod file_dialog;
pub use file_dialog::{FileDialog, FileDialogRequest};
