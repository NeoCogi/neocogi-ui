//
// Copyright 2023-Present (c) Raja Lehtihet & Wael El Oraiby
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
// this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
// this list of conditions and the following disclaimer in the documentation
// and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
// may be used to endorse or promote products derived from this software without
// specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
// LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
// CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
// SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
// CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGE.

#![doc = include_str!("../docs/EVENTS.md")]

use crate::Widget;
use microui_contracts::{IActivationSinkAbi, IEventSubscriptionAbi, ITextChangedSinkAbi, ITextSubmittedSinkAbi};
use neocogi_com::{ComPtr, ComRef, INcStringAbi, NcStatus, NcString};

/// Static event-to-sink projection implemented beside each concrete semantic event type.
pub(crate) use microui_redux::events::EventPayload as ComEvent;

/// Connects one producer endpoint to its statically selected foreign sink contract.
pub(crate) fn subscribe<E: ComEvent>(endpoint: WidgetEventPortHandle<E>, sink: ComRef<'_, E::Sink>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
    endpoint.subscribe(sink)
}

/// Delivers a captured labelled activation without consulting mutable widget state.
pub(crate) fn deliver_activation(label: &str, sink: ComRef<'_, IActivationSinkAbi>) -> Result<(), NcStatus> {
    let label: ComPtr<INcStringAbi> = NcString::from_utf8_str(label);
    sink.submitted(label.as_com_ref())
}

/// Delivers one immutable text-edit snapshot through the shared changed sink protocol.
pub(crate) fn deliver_text_changed(text: &str, cursor: usize, sink: ComRef<'_, ITextChangedSinkAbi>) -> Result<(), NcStatus> {
    let text = NcString::from_utf8_str(text);
    sink.changed(text.as_com_ref(), cursor as u64)
}

/// Delivers one immutable text submission through the shared submitted sink protocol.
pub(crate) fn deliver_text_submitted(text: &str, sink: ComRef<'_, ITextSubmittedSinkAbi>) -> Result<(), NcStatus> {
    let text = NcString::from_utf8_str(text);
    sink.submitted(text.as_com_ref())
}

/// A concrete retained widget that owns one typed native event source.
///
/// A widget may implement this trait more than once with different `E` types, such as separate
/// `Changed` and `Submitted` ports. The implementation returns a weak capability rather than the
/// port itself: callers can subscribe and inspect liveness, but cannot emit or drain widget events.
pub trait TypedWidget<E: 'static>: Widget {
    /// Returns a weak capability for this widget's `E` event source.
    ///
    /// Repeated calls identify the same widget-owned port. Each returned handle is weak and does
    /// not affect the lifetime of either the widget or port.
    fn event(&self) -> WidgetEventPortHandle<E>;
}

pub use microui_redux::events::{EventSource, EventEndpoint as WidgetEventPortHandle};
#[cfg(test)]
pub use microui_redux::events::test_support::SubscribeError;
#[cfg(test)]
pub(crate) use crate::test_support::events::WidgetEventDispatcher;
#[cfg(test)]
impl<W: Widget + 'static> crate::TypedWidgetHandle<W> {
    /// Projects a concrete widget-owned event port through this weak typed widget handle.
    ///
    /// Access is intentionally brief: [`TypedWidget::event`] clones only the weak port capability,
    /// and the concrete widget borrow is released before the caller can subscribe. If the widget
    /// cell is currently unavailable or already dead, the result behaves as an expired event
    /// handle.
    pub fn widget_event<E: 'static>(&self) -> WidgetEventPortHandle<E>
    where
        W: TypedWidget<E>,
    {
        self.try_read(TypedWidget::event).unwrap_or_else(WidgetEventPortHandle::expired)
    }
}
