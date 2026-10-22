//! Generated application-specific contracts for the independent widget extension.
//!
//! Mesh rendering belongs to the separate `neocogi.render.mesh` module so a
//! renderer plugin never imports the UI widget contract graph.

#![no_std]
#![deny(missing_docs)]

include!(concat!(env!("OUT_DIR"), "/neocogi_demo.include.rs"));

mod helpers;

pub mod events;

// Event publication is UI authoring policy, separate from callable IDL ABI.
neocogi_ui_core_contracts::ui_event_sources! {
    CounterAbi => [neocogi_ui_core_contracts::IActivatedEventAbi, ICounterChangedEventAbi];
}
