//! Typed UI authoring declarations; callable interfaces remain ordinary IDL.
use neocogi_com::{ComInterface, idl_metadata::InterfaceMetadata};

/// Static UI publication policy for one producer, copied into an owned design catalog.
/// Neither these Rust references nor an event list participate in COM ABI identity.
#[derive(Clone, Copy)]
pub struct EventSourceMetadata {
    /// Concrete producer projection and the values reachable from its ordinary methods.
    pub interface: &'static InterfaceMetadata,
    /// Advertised snapshots in authored order. UI catalog publication validates their getters.
    pub events: &'static [&'static InterfaceMetadata],
}

/// Declares a producer's UI snapshots independently of its callable COM contract.
/// Implementations belong to the crate owning the generated concrete projection.
pub trait UiEventSource: ComInterface {
    /// One authoritative static list used by catalog publishers and built-in tooling.
    const EVENT_SOURCE: EventSourceMetadata;
}

/// Publishes UI event lists without repeating interface identities or getter types.
/// Use an empty list for a constructible widget that emits no events.
///
/// ```ignore
/// neocogi_ui::abi::core::ui_event_sources! {
///     ButtonAbi => [IActivatedEventAbi];
///     TextBlockAbi => [];
/// }
/// ```
#[macro_export]
macro_rules! ui_event_sources {
    ($($source:ty => [$($event:ty),* $(,)?]);* $(;)?) => {
        $(impl $crate::abi::core::UiEventSource for $source {
            const EVENT_SOURCE: $crate::abi::core::EventSourceMetadata = $crate::abi::core::EventSourceMetadata {
                interface: &<$source>::IDL_INTERFACE,
                events: &[$(&<$event>::IDL_INTERFACE),*],
            };
        })*
    };
}
