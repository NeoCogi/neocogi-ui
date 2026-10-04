//! Independently declared event interfaces use the common exclusive event protocol.
use neocogi_com::events::{EventPayload, downcast_event};
use neocogi_demo_contracts::{CounterChanged, CounterEvent, ICounterChangedEventAbi};
use neocogi_ui_core_contracts::{Activation, IActivatedEventAbi};

/// A custom control can share an activation contract and add its own typed snapshot.
#[test]
fn custom_counter_events_are_queryable_without_a_custom_queue_interface() {
    let changed = CounterEvent::Changed(CounterChanged { value: u32::MAX }).into_event().unwrap();
    let count = downcast_event::<ICounterChangedEventAbi>(&changed).unwrap().unwrap();
    assert_eq!(count.value(), Ok(u32::MAX));
    assert!(downcast_event::<IActivatedEventAbi>(&changed).unwrap().is_none());
    assert!(matches!(CounterEvent::from_event(&changed).unwrap(), Some(CounterEvent::Changed(value)) if value.value == u32::MAX));
    let activation = CounterEvent::Activated(Activation { label: "Count".into() }).into_event().unwrap();
    assert!(downcast_event::<ICounterChangedEventAbi>(&activation).unwrap().is_none());
    assert!(matches!(CounterEvent::from_event(&activation).unwrap(), Some(CounterEvent::Activated(value)) if value.label == "Count"));
}
