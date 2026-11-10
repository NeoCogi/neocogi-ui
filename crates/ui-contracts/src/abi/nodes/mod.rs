//! Generated concrete built-in widget and container COM contracts.
//!
//! Shared node, layout, popup, and host protocols remain in `neocogi.ui.core`.
//! This module owns concrete class and factory identities only.

#![deny(missing_docs)]

include!(env!("IDL_NEOCOGI_UI_NODES"));

mod helpers;

pub mod events;

// Event publication is UI authoring policy, separate from callable IDL ABI.
neocogi_ui_contracts::abi::core::ui_event_sources! {
    TextBlockAbi => [];
    ButtonAbi => [neocogi_ui_contracts::abi::core::IActivatedEventAbi];
    SplitViewAbi => [ISplitChangedEventAbi];
    TextboxAbi => [neocogi_ui_contracts::abi::core::ITextChangedEventAbi, neocogi_ui_contracts::abi::core::ITextSubmittedEventAbi];
    CheckboxAbi => [ICheckboxChangedEventAbi];
    SliderAbi => [INumericChangedEventAbi];
    NumberAbi => [INumericChangedEventAbi];
    ListItemAbi => [neocogi_ui_contracts::abi::core::IActivatedEventAbi];
    ListBoxAbi => [neocogi_ui_contracts::abi::core::IActivatedEventAbi];
    ScrollbarAbi => [IScrollbarChangedEventAbi];
    TextAreaAbi => [neocogi_ui_contracts::abi::core::ITextChangedEventAbi, neocogi_ui_contracts::abi::core::ITextSubmittedEventAbi];
    ComboAbi => [neocogi_ui_contracts::abi::core::IChoiceChangedEventAbi];
}
