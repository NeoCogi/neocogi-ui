//! Generated application-specific contracts for the independent widget extension.
//!
//! Mesh rendering belongs to the separate `neocogi.render.mesh` module so a
//! renderer plugin never imports the UI widget contract graph.

#![deny(missing_docs)]

include!(env!("IDL_NEOCOGI_DEMO"));

mod helpers;

pub mod events;

// Event publication is UI authoring policy, separate from callable IDL ABI.
neocogi_ui::abi::core::ui_event_sources! {
    CounterAbi => [neocogi_ui::abi::core::IActivatedEventAbi, ICounterChangedEventAbi];
}
