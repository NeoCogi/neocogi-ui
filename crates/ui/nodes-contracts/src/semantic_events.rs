//! Node-specific exclusive snapshots and native enum conveniences.
use crate::*;
use neocogi_ui_core_contracts::{Activation, TextChanged, TextSubmitted};
use neocogi_com::{ComBox, NcStatus, NC_E_INVALID_ARG};
use neocogi_com::events::{EventPayload, IEvent, IEventAbi, downcast_event};

/// One scalar event interface has one authoritative native value and validator.
macro_rules! scalar_snapshot {
    ($object:ident, $payload:ident, $interface:ident, $abi:ident, $field:ident, $ty:ty, $valid:expr) => {
        #[neocogi_com::com_object(interfaces(crate::$interface), ownership = exclusive)]
        /// Immutable captured scalar, exclusively owned by the receiving message.
        struct $object {
            /// User-committed value, independent of later widget edits.
            value: $payload,
        }
        impl IEvent for $object {}
        impl $interface for $object {
            /// Read the validated captured field through its typed C ABI method.
            fn $field(&self) -> Result<$ty, NcStatus> {
                Ok(self.value.$field)
            }
        }
        impl EventPayload for $payload {
            /// Check the semantic domain before allocation or accepting foreign data.
            fn validate(&self) -> Result<(), NcStatus> {
                if ($valid)(self.$field) { Ok(()) } else { Err(NC_E_INVALID_ARG) }
            }
            /// Move the scalar into exactly one event allocation.
            fn into_event(self) -> Result<ComBox<IEventAbi>, NcStatus> {
                self.validate()?;
                $object::into_box($object { value: self })
                    .query()
                    .map_err(|(status, _unpublished)| status)
            }
            /// The native convenience reads the same getter available to non-Rust hosts.
            fn from_event(event: &ComBox<IEventAbi>) -> Result<Option<Self>, NcStatus> {
                let Some(event) = downcast_event::<$abi>(event)? else { return Ok(None) };
                let value = Self { $field: event.$field()? };
                value.validate()?;
                Ok(Some(value))
            }
        }
    };
}
scalar_snapshot!(
    CheckboxSnapshot,
    CheckboxChanged,
    ICheckboxChangedEvent,
    ICheckboxChangedEventAbi,
    checked,
    bool,
    |_: bool| true
);
scalar_snapshot!(
    NumericSnapshot,
    NumericChanged,
    INumericChangedEvent,
    INumericChangedEventAbi,
    value,
    f32,
    |value: f32| value.is_finite()
);
scalar_snapshot!(
    ScrollbarSnapshot,
    ScrollbarChanged,
    IScrollbarChangedEvent,
    IScrollbarChangedEventAbi,
    offset,
    i32,
    |value: i32| value >= 0
);
scalar_snapshot!(
    SplitSnapshot,
    SplitChanged,
    ISplitChangedEvent,
    ISplitChangedEventAbi,
    position,
    i32,
    |value: i32| value >= 0
);

/// A producer's optional native enum maps directly to its semantic event interface.
macro_rules! single_variant {
    ($payload:ident, $variant:ident, $value:ty) => {
        impl EventPayload for $payload {
            /// One semantic validator is shared by every producer of this value.
            fn validate(&self) -> Result<(), NcStatus> {
                match self {
                    Self::$variant(value) => value.validate(),
                }
            }
            /// The event allocation contains the semantic snapshot, without another wrapper.
            fn into_event(self) -> Result<ComBox<IEventAbi>, NcStatus> {
                match self {
                    Self::$variant(value) => value.into_event(),
                }
            }
            /// Construct the Rust enum only at a native consumer's request.
            fn from_event(event: &ComBox<IEventAbi>) -> Result<Option<Self>, NcStatus> {
                <$value>::from_event(event).map(|value| value.map(Self::$variant))
            }
        }
    };
}
single_variant!(ButtonEvent, Activated, Activation);
single_variant!(ListItemEvent, Activated, Activation);
single_variant!(ListBoxEvent, Activated, Activation);
single_variant!(CheckboxEvent, Changed, CheckboxChanged);
single_variant!(SliderEvent, Changed, NumericChanged);
single_variant!(NumberEvent, Changed, NumericChanged);
single_variant!(ScrollbarEvent, Changed, ScrollbarChanged);
single_variant!(SplitViewEvent, Changed, SplitChanged);

/// Editors share concrete text events; the origin identifies the specific editor kind.
macro_rules! text_variants {
    ($payload:ident) => {
        impl EventPayload for $payload {
            /// Validate the exact snapshot selected by the native enum.
            fn validate(&self) -> Result<(), NcStatus> {
                match self {
                    Self::Changed(value) => value.validate(),
                    Self::Submitted(value) => value.validate(),
                }
            }
            /// Capture change or submission as the corresponding concrete COM event.
            fn into_event(self) -> Result<ComBox<IEventAbi>, NcStatus> {
                match self {
                    Self::Changed(value) => value.into_event(),
                    Self::Submitted(value) => value.into_event(),
                }
            }
            /// Failed type queries are ordinary absence; getter/ABI errors remain errors.
            fn from_event(event: &ComBox<IEventAbi>) -> Result<Option<Self>, NcStatus> {
                if let Some(value) = TextChanged::from_event(event)? {
                    Ok(Some(Self::Changed(value)))
                } else {
                    TextSubmitted::from_event(event).map(|value| value.map(Self::Submitted))
                }
            }
        }
    };
}
text_variants!(TextboxEvent);
text_variants!(TextAreaEvent);
