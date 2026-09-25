//! Typed authored UI documents, COM reconstruction, and replace-on-success reload.
#![deny(missing_docs)]

/// Portable, explicit initialization schemas shared with the designer.
pub mod model;
pub use model::*;

/// Generated opaque component and concrete application command contracts.
pub use neocogi_ui_document_contracts as contracts;
mod bindings;
mod wiring;
pub use wiring::{RouteDiagnostic, RouteEffect};
mod rust_bindings;
mod build;
pub use bindings::{Command, InputHandle};
mod runtime;
pub use runtime::{Component, DOCUMENT_TYPE, Loader, Resources};
mod reload;
pub use reload::{Generation, ReloadSlot, Reloaded};
