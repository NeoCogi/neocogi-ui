//! Generated application-specific contracts for the independent widget extension.
//!
//! Custom graphics use the common renderer's resource and scoped-command contracts.

#![deny(missing_docs)]

include!(env!("IDL_NEOCOGI_DEMO"));

mod helpers;

pub mod events;

// Event publication is UI authoring policy, separate from callable IDL ABI.
neocogi_ui_contracts::abi::core::ui_event_sources! {
    CounterAbi => [neocogi_ui_contracts::abi::core::IActivatedEventAbi, ICounterChangedEventAbi];
}
