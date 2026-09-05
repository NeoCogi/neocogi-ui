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
