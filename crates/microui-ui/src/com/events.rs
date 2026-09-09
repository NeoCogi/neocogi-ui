//! Typed COM delivery reuses native semantic queues; only the application-owned lease retains its sink.
use super::*;
use crate::event::WidgetEventPortHandle;
use nc_com::{INcStringAbi, NcString};

pub(crate) use microui_redux::events::EventPayload as ComEvent;

/// Labelled controls share transport while their source snapshot types remain concrete inside the plugin.
macro_rules! activation_event {
    ($event:ty) => {
        impl ComEvent for $event {
            type Sink = IActivationSinkAbi;
            /// Copies the label captured during activation, never the producer's current mutable value.
            fn deliver(&self, sink: ComRef<'_, IActivationSinkAbi>) -> Result<(), NcStatus> {
                let label: ComPtr<INcStringAbi> = NcString::from_utf8_str(&self.label);
                sink.submitted(label.as_com_ref())
            }
        }
    };
}
activation_event!(crate::ButtonSubmitted);
activation_event!(crate::ListBoxSubmitted);
activation_event!(crate::ListItemSubmitted);
activation_event!(crate::MenuItemSubmitted);

/// Captured scalar values require no snapshot allocation or live-widget read at delivery time.
macro_rules! scalar_event {
    ($event:ty, $sink:ty, $field:ident) => {
        impl ComEvent for $event {
            type Sink = $sink;
            /// Delivers the actual committed user value through its concrete sink contract.
            fn deliver(&self, sink: ComRef<'_, $sink>) -> Result<(), NcStatus> {
                sink.changed(self.$field)
            }
        }
    };
}
scalar_event!(crate::CheckboxChanged, ICheckboxSinkAbi, checked);
scalar_event!(crate::SliderChanged, INumericSinkAbi, value);
scalar_event!(crate::NumberChanged, INumericSinkAbi, value);
scalar_event!(crate::ScrollbarChanged, IScrollbarSinkAbi, offset);

/// Both editor implementations already produce complete immutable semantic snapshots.
macro_rules! text_events {
    ($changed:ty, $submitted:ty) => {
        impl ComEvent for $changed {
            type Sink = ITextChangedSinkAbi;
            /// Delivers normalized text and its matching cursor from the same captured event.
            fn deliver(&self, sink: ComRef<'_, ITextChangedSinkAbi>) -> Result<(), NcStatus> {
                let text = NcString::from_utf8_str(&self.text);
                sink.changed(text.as_com_ref(), self.cursor as u64)
            }
        }
        impl ComEvent for $submitted {
            type Sink = ITextSubmittedSinkAbi;
            /// Converts one captured submission without consulting or retaining the source widget.
            fn deliver(&self, sink: ComRef<'_, ITextSubmittedSinkAbi>) -> Result<(), NcStatus> {
                let text = NcString::from_utf8_str(&self.text);
                sink.submitted(text.as_com_ref())
            }
        }
    };
}
text_events!(crate::TextboxChanged, crate::TextboxSubmitted);
text_events!(crate::TextAreaChanged, crate::TextAreaSubmitted);

/// Connects the producer's static endpoint to its concrete foreign sink.
pub(crate) fn subscribe<E: ComEvent>(endpoint: WidgetEventPortHandle<E>, sink: ComRef<'_, E::Sink>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
    endpoint.subscribe(sink)
}

impl ComEvent for crate::WindowEvent {
    type Sink = IWindowSinkAbi;
    /// Encodes the already captured manager event, including its explicit terminal destruction fact.
    fn deliver(&self, sink: ComRef<'_, IWindowSinkAbi>) -> Result<(), NcStatus> {
        let event = match *self {
            crate::WindowEvent::GeometryChanged { rect } => WindowEvent::GeometryChanged { rect: copied_rect(rect) },
            crate::WindowEvent::CloseRequested => WindowEvent::CloseRequested,
            crate::WindowEvent::Hidden => WindowEvent::Hidden,
            crate::WindowEvent::Destroyed => WindowEvent::Destroyed,
            crate::WindowEvent::Minimized { rect } => WindowEvent::Minimized { rect: copied_rect(rect) },
            crate::WindowEvent::Maximized { rect } => WindowEvent::Maximized { rect: copied_rect(rect) },
            crate::WindowEvent::Restored { rect } => WindowEvent::Restored { rect: copied_rect(rect) },
        };
        sink.event(event.into())
    }
}
impl ComEvent for crate::PopupEvent {
    type Sink = IPopupSinkAbi;
    /// The current native popup event family contains exactly one ordinary dismissal fact.
    fn deliver(&self, sink: ComRef<'_, IPopupSinkAbi>) -> Result<(), NcStatus> {
        match self {
            crate::PopupEvent::Dismissed => sink.dismissed(),
        }
    }
}
