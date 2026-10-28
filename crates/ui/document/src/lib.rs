//! Typed authored UI documents, COM reconstruction, and replace-on-success reload.
#![deny(missing_docs)]

/// Portable, explicit initialization schemas shared with the designer.
pub mod model;
pub use model::*;

/// Generated component lookup and custom-widget discovery contracts.
pub use neocogi_ui::abi::document as contracts;
mod event_bindings;
pub use event_bindings::{EventBindings, DocumentEvent, document_receiver};
/// Owned COM-transportable authoring metadata for custom widgets.
pub use neocogi_ui_design as design;
mod custom;
mod discovery;
pub use discovery::{MAX_CATALOG_WIDGETS, editor_settings};
pub use custom::{CustomWidgetFactory, CustomWidgetRegistry};
mod build;
mod runtime;
mod rust_bindings;
pub use runtime::{Component, DOCUMENT_TYPE, Loader, Resources};
mod reload;
pub use reload::{Generation, ReloadIds, ReloadSlot, Reloaded};

// This unit-test executable owns one runtime; normal library builds never embed it.
#[cfg(test)]
extern crate neocogi_com_host as _;

/// Local receiver regression bindings; no separate contract package.
#[cfg(test)]
pub mod receiver_fixture {
    include!(env!("IDL_RETAINER_BINDINGS"));
}
