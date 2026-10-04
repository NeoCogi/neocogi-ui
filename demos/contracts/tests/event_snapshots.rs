//! Custom controls share activation while publishing their own concrete event interface.
use neocogi_com::{INcStringComPtrExt, events::downcast_event};
use neocogi_demo_contracts::{events::counter_changed, ICounterChangedEventAbi};
use neocogi_ui_core_contracts::{events::activated, IActivatedEventAbi};

#[test]
fn custom_counter_events_are_queryable_without_a_custom_queue_interface() {
    let changed = counter_changed(u32::MAX).unwrap();
    assert_eq!(downcast_event::<ICounterChangedEventAbi>(&changed).unwrap().unwrap().value(), Ok(u32::MAX));
    assert!(downcast_event::<IActivatedEventAbi>(&changed).unwrap().is_none());
    let activation = activated("Count").unwrap();
    assert!(downcast_event::<ICounterChangedEventAbi>(&activation).unwrap().is_none());
    assert_eq!(downcast_event::<IActivatedEventAbi>(&activation).unwrap().unwrap().label().unwrap().to_string().unwrap(), "Count");
}
