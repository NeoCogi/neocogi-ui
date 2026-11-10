//! Canonical UI contracts, portable values and typed clients for NeoCogi services.
//!
//! Widget and application generics remain local to the host; implementation
//! state, retained topology, native windows, and GPU ownership live in plugins.
//! This crate owns shared definitions and immutable value producers; concrete UI,
//! graphics construction and theme parsing belong to the static implementation.
//!
//! NCIDL-generated records are re-exported directly rather than redeclared by
//! this facade. Handwritten records in this crate own application-only state or
//! turn borrowed variable-length ABI data into queue-safe Rust snapshots; they
//! must not mirror an available generated contract type.

#![deny(missing_docs)]

extern crate self as neocogi_ui_contracts;
extern crate alloc;
// Unit-test executables own one runtime just like this package's integration tests.
#[cfg(test)]
extern crate neocogi_com_host as _;

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
pub use menu::{Menus, MenuBuilder, MenuItem, MAX_MENU_ENTRIES, MAX_MENU_DEPTH};

/// Standard native-path file-picker requests and clients.
pub mod file_dialog;
pub use file_dialog::{FileDialog, FileDialogRequest};

/// Immutable prebuilt and builder-produced resource objects using the same bulk contracts.
pub mod assets;

/// Native appearance states shared by asset compilers and retained UI code.
pub use appearance::{ChromeState, ControlState, MenuState, PointerState, SurfaceState};

/// Static application leaf behavior stored inline in one concrete COM object.
pub mod leaf;
pub use leaf::{LeafContent, DispatchingLeafContent, LeafDispatchContext};

/// Owned immutable renderer images with cached measurement dimensions.
pub mod image;
pub use image::Image;

/// UI-owned producer publication, independent of callable COM ABI.
pub use neocogi_ui_contracts::abi::core::{EventSourceMetadata, UiEventSource};

/// Shared IDL bindings and their Rust conveniences.
pub mod abi;

/// Bounded UI coordinates and checked rs-math3d conversions shared by clients and providers.
pub mod geometry;

/// Shared typed appearance policy and its generated authoring contracts.
pub mod appearance;
/// Consumer-owned schema generation; enabled only by build dependencies.
#[cfg(feature = "build-support")]
#[path = "../build_support/mod.rs"]
pub mod build;
/// Owned design metadata and bounded codecs used by plugins and document authors.
pub mod design;
/// Canonical immutable graphics values, validation and publication helpers.
pub mod gfx;
/// Typed activation helpers for the one consolidated library root.
pub mod services;
