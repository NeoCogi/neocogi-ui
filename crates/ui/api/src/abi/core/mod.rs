//! Generated core COM contracts plus Rust-only conveniences for UI consumers.
//!
//! The `.ncidl` source owns every ABI declaration and its stable identity. This
//! module adds only ordinary Rust validation, defaults, and semantic helpers;
//! none of those conveniences are stored in the native contract description.

#![deny(missing_docs)]

extern crate alloc;
#[cfg(test)]
extern crate std;

include!(env!("IDL_NEOCOGI_UI_CORE"));

// Preserve the facade's established Rust import surface while declaration
// ownership and native metadata live exclusively in `neocogi.renderer`.
pub use neocogi_render_core::abi::*;
pub use neocogi_render_core::platform::*;
pub use neocogi_gfx_support::abi::*;

mod helpers;
pub use helpers::*;

mod outcomes;

pub mod events;

// Event publication is UI authoring policy, separate from callable IDL ABI.
crate::abi::core::ui_event_sources! {
    IFileDialogAbi => [IFileAcceptedEventAbi, IFileCancelledEventAbi];
    IChoiceMenuAbi => [IChoiceChangedEventAbi];
    IMenuItemAbi => [IActivatedEventAbi];
    IWindowAbi => [IWindowGeometryChangedEventAbi, IWindowCloseRequestedEventAbi, IWindowHiddenEventAbi, IWindowDestroyedEventAbi, IWindowMinimizedEventAbi, IWindowMaximizedEventAbi, IWindowRestoredEventAbi];
    IPopupAbi => [IPopupDismissedEventAbi];
}

mod event_transport;
pub use event_transport::{EventMessage, EventPort, error};

mod event_sources;
pub use event_sources::{EventSourceMetadata, UiEventSource};

/// UI publication macro, exported at the owning facade root.
pub use crate::ui_event_sources;
