//! Immutable COM event snapshots and deferred application dispatch.
//! One common receiver serves every widget kind. The inbox owns each event as
//! `ComBox<IEventAbi>` until its handler queries derived event interfaces.
//!
//! The walkthrough below uses an initialized, otherwise empty UI. Its keyboard
//! input exercises actual activation, then removal with an application event queued.
//! The runtime regression compiles this exact source file and executes it against
//! the production core and node providers.
#![doc = concat!("```no_run\n", include_str!("../docs/button_events.rs"), "\n```")]

/// Owned application fixtures for integration tests, without borrowed callback adapters.
#[cfg(feature = "test-support")]
pub mod testing;

mod dispatcher;
pub use dispatcher::{ApplyOutcome, Dispatcher, MessageProcessor};
mod origin;
pub use origin::EventOriginOwner;

/// COM event interfaces, owned messages and common channel construction.
pub use neocogi_com::events::{
    EventBinding, EventMessage, EventOrigin, IErrorEvent, IErrorEventAbi, IEvent, IEventAbi, IEventReceiver, IEventReceiverAbi, channel, error,
};
/// Built-in immutable snapshot constructors return the sole COM event owner.
pub use neocogi_ui_core_contracts::events::*;
/// Concrete control snapshot constructors use the same IEvent root.
pub use neocogi_ui_nodes_contracts::events::*;
/// Selection is widget state shared with event getters, rather than an event wrapper.
pub use neocogi_ui_core_contracts::{ChoiceSelection, EVENT_DISPATCH_LIMIT};
