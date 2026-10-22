//! Generated concrete built-in widget and container COM contracts.
//!
//! Shared node, layout, popup, and host protocols remain in `neocogi.ui.core`.
//! This module owns concrete class and factory identities only.

#![no_std]
#![deny(missing_docs)]

include!(concat!(env!("OUT_DIR"), "/neocogi_ui_nodes.include.rs"));

mod helpers;

pub mod events;

// Event publication is UI authoring policy, separate from callable IDL ABI.
neocogi_ui_core_contracts::ui_event_sources! {
    TextBlockAbi => [];
    ButtonAbi => [neocogi_ui_core_contracts::IActivatedEventAbi];
    SplitViewAbi => [ISplitChangedEventAbi];
    TextboxAbi => [neocogi_ui_core_contracts::ITextChangedEventAbi, neocogi_ui_core_contracts::ITextSubmittedEventAbi];
    CheckboxAbi => [ICheckboxChangedEventAbi];
    SliderAbi => [INumericChangedEventAbi];
    NumberAbi => [INumericChangedEventAbi];
    ListItemAbi => [neocogi_ui_core_contracts::IActivatedEventAbi];
    ListBoxAbi => [neocogi_ui_core_contracts::IActivatedEventAbi];
    ScrollbarAbi => [IScrollbarChangedEventAbi];
    TextAreaAbi => [neocogi_ui_core_contracts::ITextChangedEventAbi, neocogi_ui_core_contracts::ITextSubmittedEventAbi];
    ComboAbi => [neocogi_ui_core_contracts::IChoiceChangedEventAbi];
}
