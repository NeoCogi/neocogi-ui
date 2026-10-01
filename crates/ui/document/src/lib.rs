//! Typed authored UI documents, COM reconstruction, and replace-on-success reload.
#![deny(missing_docs)]

/// Portable, explicit initialization schemas shared with the designer.
pub mod model;
pub use model::*;

/// Generated component lookup and custom-widget discovery contracts.
pub use neocogi_ui_document_contracts as contracts;
mod event_bindings;
pub use event_bindings::EventBindings;
/// Owned COM-transportable authoring metadata for custom widgets.
pub use neocogi_ui_design as design;
mod custom;
mod discovery;
pub use discovery::MAX_CATALOG_WIDGETS;
pub use custom::{CustomWidgetFactory, CustomWidgetRegistry};
mod build;
mod rust_bindings;
mod runtime;
pub use runtime::{Component, DOCUMENT_TYPE, Loader, Resources};
mod reload;
pub use reload::{Generation, ReloadIds, ReloadSlot, Reloaded};
