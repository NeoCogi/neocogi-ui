//! Generated core COM contracts plus Rust-only conveniences for UI consumers.
//!
//! The `.ncidl` source owns every ABI declaration and its stable identity. This
//! crate adds only ordinary Rust validation, defaults, and semantic helpers;
//! none of those conveniences are stored in the native contract description.

#![no_std]
#![deny(missing_docs)]

extern crate alloc;
#[cfg(test)]
extern crate std;

include!(concat!(env!("OUT_DIR"), "/neocogi_ui_core.include.rs"));

// Preserve the facade's established Rust import surface while declaration
// ownership and native metadata live exclusively in `neocogi.renderer`.
pub use neocogi_renderer_contracts::*;
pub use neocogi_platform_contracts::*;
pub use neocogi_gfx_contracts::*;

mod helpers;
pub use helpers::*;

pub mod events;

// Event publication is UI authoring policy, separate from callable IDL ABI.
crate::ui_event_sources! {
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
