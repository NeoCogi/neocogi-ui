//! Statically linked, typed Rust clients for microui COM plugins.
//!
//! Widget and application generics remain local to the host; implementation
//! state, retained topology, native windows, and GPU ownership live in plugins.
//! This crate depends only on COM support, shared contracts, and non-plugin GFX bundle helpers.
//!
//! NCIDL-generated records are re-exported directly rather than redeclared by
//! this facade. Handwritten records in this crate own application-only state or
//! turn borrowed variable-length ABI data into queue-safe Rust snapshots; they
//! must not mirror an available generated contract type.

#![deny(missing_docs)]

extern crate self as neocogi_ui;
extern crate alloc;
// Unit-test executables own one runtime just like this package's integration tests.
#[cfg(test)]
extern crate nc_com as _;

/// Typed clients and concrete UI interface definitions.
pub mod com;
pub use com::*;

/// Application-owned typed semantic events and deterministic COM delivery.
pub mod events;

/// Exact native-path snapshots and static platform conversion.
pub mod path;
pub use path::NativePath;

/// Compact menu declarations and live item clients.
pub mod menu;
pub use menu::{Menus, MenuBuilder, MenuItem};

/// Standard native-path file-picker requests and clients.
pub mod file_dialog;
pub use file_dialog::{FileDialog, FileDialogRequest};

/// Immutable prebuilt and builder-produced resource objects using the same bulk contracts.
pub mod assets;

/// Native appearance states shared by asset compilers and retained UI code.
pub mod appearance;
pub use appearance::{ChromeState, ControlState, MenuState, PointerState, SurfaceState};

/// Static application leaf behavior stored inline in one concrete COM object.
pub mod leaf;
pub use leaf::{LeafContent, DispatchingLeafContent, LeafDispatchContext};

/// Owned immutable renderer images with cached measurement dimensions.
pub mod image;
pub use image::Image;

/// UI-owned producer publication, independent of callable COM ABI.
pub use neocogi_ui::abi::core::{EventSourceMetadata, UiEventSource};

/// Shared IDL bindings and their Rust conveniences.
pub mod abi;
