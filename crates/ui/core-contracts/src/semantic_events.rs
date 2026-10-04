//! Immutable exclusive event implementations and optional native payload conversions.
use crate::*;
use neocogi_com::{ComBox, ComPtr, INcStringAbi, INcStringComPtrExt, NcStatus, NcString, WeakComBox, NC_E_INVALID_ARG};
use neocogi_com::events::{EventPayload, IEvent, IEventAbi, downcast_event};

/// One concrete snapshot owns its native data and exposes only immutable IDL getters.
/// The native convenience conversion uses exactly those same public COM methods.
macro_rules! snapshot {
    ($object:ident, $payload:ty, $interface:ident, $abi:ident, {$($method:item)*}, $read:expr, $validate:expr) => {
        #[neocogi_com::com_object(interfaces(crate::$interface), ownership = exclusive)]
        /// Private exclusive event allocation; its source widget is never retained here.
        struct $object {
            /// Complete captured data, moved into the event exactly once.
            value: $payload,
        }
        impl IEvent for $object {}
        impl $interface for $object { $($method)* }
        impl EventPayload for $payload {
            /// Authoritative domain validation shared by construction and native readers.
            fn validate(&self) -> Result<(), NcStatus> { ($validate)(self) }
            /// Allocate only after validating the complete immutable snapshot.
            fn into_event(self) -> Result<ComBox<IEventAbi>, NcStatus> {
                self.validate()?;
                $object::into_box($object { value: self }).query().map_err(|(status, _unpublished)| status)
            }
            /// Use ordinary typed COM queries/getters, including for foreign implementations.
            fn from_event(event: &ComBox<IEventAbi>) -> Result<Option<Self>, NcStatus> {
                let Some(event) = downcast_event::<$abi>(event)? else { return Ok(None) };
                let value: Self = ($read)(event)?;
                value.validate()?;
                Ok(Some(value))
            }
        }
    };
}

snapshot!(
    Activated,
    Activation,
    IActivatedEvent,
    IActivatedEventAbi,
    {
        /// Copy the captured label into an immutable string capability.
        fn label(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
            Ok(NcString::from_utf8_str(&self.value.label))
        }
    },
    |event: WeakComBox<IActivatedEventAbi>| -> Result<Activation, NcStatus> { Ok(Activation { label: event.label()?.to_string()? }) },
    |_: &Activation| Ok(())
);

snapshot!(
    ChangedText,
    TextChanged,
    ITextChangedEvent,
    ITextChangedEventAbi,
    {
        /// Copy the captured text without consulting the editor's current state.
        fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
            Ok(NcString::from_utf8_str(&self.value.text))
        }
        /// Return the cursor belonging to this exact immutable snapshot.
        fn cursor(&self) -> Result<u64, NcStatus> {
            Ok(self.value.cursor)
        }
    },
    |event: WeakComBox<ITextChangedEventAbi>| -> Result<TextChanged, NcStatus> {
        Ok(TextChanged {
            text: event.text()?.to_string()?,
            cursor: event.cursor()?,
        })
    },
    |value: &TextChanged| {
        let cursor = usize::try_from(value.cursor).map_err(|_| NC_E_INVALID_ARG)?;
        if value.text.is_char_boundary(cursor) { Ok(()) } else { Err(NC_E_INVALID_ARG) }
    }
);

snapshot!(
    SubmittedText,
    TextSubmitted,
    ITextSubmittedEvent,
    ITextSubmittedEventAbi,
    {
        /// Copy the text captured at submission rather than the editor's live buffer.
        fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
            Ok(NcString::from_utf8_str(&self.value.text))
        }
    },
    |event: WeakComBox<ITextSubmittedEventAbi>| -> Result<TextSubmitted, NcStatus> { Ok(TextSubmitted { text: event.text()?.to_string()? }) },
    |_: &TextSubmitted| Ok(())
);

snapshot!(
    WindowSnapshot,
    WindowEvent,
    IWindowEvent,
    IWindowEventAbi,
    {
        /// Return the captured copied lifecycle value through its checked IDL codec.
        fn value(&self) -> Result<WindowEvent, NcStatus> {
            Ok(self.value.clone())
        }
    },
    |event: WeakComBox<IWindowEventAbi>| event.value(),
    |value: &WindowEvent| {
        match value {
            WindowEvent::GeometryChanged { rect } | WindowEvent::Minimized { rect } | WindowEvent::Maximized { rect } | WindowEvent::Restored { rect }
                if rect.width < 0 || rect.height < 0 =>
            {
                Err(NC_E_INVALID_ARG)
            }
            _ => Ok(()),
        }
    }
);

snapshot!(
    PopupSnapshot,
    PopupDismissed,
    IPopupDismissedEvent,
    IPopupDismissedEventAbi,
    {
        /// Return the captured dismissal without owning the popup.
        fn value(&self) -> Result<PopupDismissed, NcStatus> {
            Ok(self.value.clone())
        }
    },
    |event: WeakComBox<IPopupDismissedEventAbi>| event.value(),
    |_: &PopupDismissed| Ok(())
);

snapshot!(
    ChoiceSnapshot,
    ChoiceSelection,
    IChoiceChangedEvent,
    IChoiceChangedEventAbi,
    {
        /// Return the committed identity independently of later menu replacement.
        fn selection(&self) -> Result<ChoiceSelection, NcStatus> {
            Ok(self.value.clone())
        }
    },
    |event: WeakComBox<IChoiceChangedEventAbi>| event.selection(),
    |_: &ChoiceSelection| Ok(())
);

impl EventPayload for IChoiceMenuEvent {
    /// Choice events contain the same authoritative selection snapshot as combo events.
    fn into_event(self) -> Result<ComBox<IEventAbi>, NcStatus> {
        match self {
            Self::Changed(value) => value.into_event(),
        }
    }
    /// Preserve the native enum only as an optional Rust decoding convenience.
    fn from_event(event: &ComBox<IEventAbi>) -> Result<Option<Self>, NcStatus> {
        ChoiceSelection::from_event(event).map(|value| value.map(Self::Changed))
    }
}

#[neocogi_com::com_object(interfaces(crate::IFileAcceptedEvent), ownership = exclusive)]
/// One successful immutable file selection, with no reference to its picker.
struct AcceptedFile {
    /// Retains exact native identity; display text cannot substitute for this capability.
    path: ComPtr<INativePathAbi>,
}
impl IEvent for AcceptedFile {}
impl IFileAcceptedEvent for AcceptedFile {
    /// Share only the immutable path capability, not the event's exclusive owner.
    fn path(&self) -> Result<ComPtr<INativePathAbi>, NcStatus> {
        Ok(self.path.clone())
    }
}

#[neocogi_com::com_object(interfaces(crate::IFileCancelledEvent), ownership = exclusive)]
/// Cancellation has no path and cannot be queried as an accepted file event.
struct CancelledFile;
impl IEvent for CancelledFile {}
impl IFileCancelledEvent for CancelledFile {}

impl EventPayload for FileDialogCompletion {
    /// Native conversion rejects a foreign platform's path representation.
    fn validate(&self) -> Result<(), NcStatus> {
        match self {
            Self::Accepted { path } => neocogi_platform_contracts::path::decode(path.as_com_ref()).map(|_| ()),
            Self::Cancelled => Ok(()),
        }
    }
    /// Exactly one concrete interface represents the committed terminal outcome.
    fn into_event(self) -> Result<ComBox<IEventAbi>, NcStatus> {
        self.validate()?;
        match self {
            Self::Accepted { path } => AcceptedFile::into_box(AcceptedFile { path }).query(),
            Self::Cancelled => CancelledFile::into_box(CancelledFile).query(),
        }
        .map_err(|(status, _unpublished)| status)
    }
    /// Cancellation is a typed fact, never an absent or empty path sentinel.
    fn from_event(event: &ComBox<IEventAbi>) -> Result<Option<Self>, NcStatus> {
        if let Some(accepted) = downcast_event::<IFileAcceptedEventAbi>(event)? {
            let value = Self::Accepted { path: accepted.path()? };
            value.validate()?;
            Ok(Some(value))
        } else {
            Ok(downcast_event::<IFileCancelledEventAbi>(event)?.map(|_| Self::Cancelled))
        }
    }
}
impl EventPayload for IFileDialogEvent {
    /// Delegate the native wrapper to the terminal outcome's authoritative validator.
    fn validate(&self) -> Result<(), NcStatus> {
        match self {
            Self::Completed(value) => value.validate(),
        }
    }
    /// The transported object represents acceptance or cancellation directly.
    fn into_event(self) -> Result<ComBox<IEventAbi>, NcStatus> {
        match self {
            Self::Completed(value) => value.into_event(),
        }
    }
    /// Reconstruct a native wrapper only when requested by a Rust consumer.
    fn from_event(event: &ComBox<IEventAbi>) -> Result<Option<Self>, NcStatus> {
        FileDialogCompletion::from_event(event).map(|value| value.map(Self::Completed))
    }
}
