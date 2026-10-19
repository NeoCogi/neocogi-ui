//! Custom controls share activation while publishing their own concrete event interface.
use neocogi_com::{NC_E_NOINTERFACE};
use neocogi_demo_contracts::{events::counter_changed, ICounterChangedEventAbi};
use neocogi_ui_core_contracts::{events::activated, IActivatedEventAbi};

#[test]
fn custom_counter_events_are_queryable_without_a_custom_queue_interface() {
    let changed = counter_changed(u32::MAX).unwrap();
    assert_eq!(changed.query_weak::<ICounterChangedEventAbi>().unwrap().value(), Ok(u32::MAX));
    assert_eq!(changed.query_weak::<IActivatedEventAbi>().unwrap_err(), NC_E_NOINTERFACE);
    let activation = activated().unwrap();
    assert_eq!(activation.query_weak::<ICounterChangedEventAbi>().unwrap_err(), NC_E_NOINTERFACE);
    assert!(activation.query_ref::<IActivatedEventAbi>().is_ok());
}
