//! Ordered host transitions and router-owned held input state.
use std::collections::VecDeque;
use crate::{UiInputEvent, Vec2i};
pub use neocogi_ui_support::input::*;

/// Host transitions retained in call order until the next UI update.
enum RawInputEvent {
    /// Revoke a gesture without synthesizing a release or activation.
    Cancel,
    /// Pointer position before held-state normalization.
    MouseMove {
        /// Absolute framebuffer coordinates.
        pos: Vec2i,
    },
    /// Explicit press, retained independently of subsequent releases.
    MouseDown {
        /// Absolute framebuffer coordinates.
        pos: Vec2i,
        /// Buttons entering held state.
        button: MouseButton,
    },
    /// Explicit release, never synthesized by cancellation.
    MouseUp {
        /// Absolute framebuffer coordinates.
        pos: Vec2i,
        /// Buttons leaving held state.
        button: MouseButton,
    },
    /// Wheel distance associated with the latest consumed pointer position.
    Scroll {
        /// Physical pixels, positive right/down.
        delta: Vec2i,
    },
    /// Logical command key with an authoritative modifier snapshot.
    Key {
        /// Complete host-normalized transition.
        event: KeyEvent,
    },
    /// Committed text from the host input method.
    Text {
        /// UTF-8 insertion, independent of logical character keys.
        text: String,
    },
}

/// Held and pointer state after one queued raw event has been applied.
#[derive(Copy, Clone, Debug)]
pub(crate) struct InputSnapshot {
    pub(crate) mouse_pos: Vec2i,
    pub(crate) mouse_buttons: MouseButton,
    /// Modifier state committed by the latest consumed key transition.
    pub(crate) modifiers: Modifiers,
}

/// Ordered raw input queue plus the state committed by events already consumed by the UI.
pub(crate) struct Input {
    /// Pointer position after the most recently consumed input event.
    mouse_pos: Vec2i,
    /// Mouse buttons held after the most recently consumed input event.
    mouse_down: MouseButton,
    /// Modifier snapshot committed by the most recently consumed keyboard transition.
    modifiers: Modifiers,
    /// Raw events waiting to be applied, in API call order.
    pending: VecDeque<RawInputEvent>,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            mouse_pos: Vec2i::default(),
            mouse_down: MouseButton::NONE,
            modifiers: Modifiers::NONE,
            pending: VecDeque::new(),
        }
    }
}

impl Input {
    /// Queues loss of host focus or pointer capture alongside ordinary transitions.
    pub(crate) fn cancel(&mut self) {
        self.pending.push_back(RawInputEvent::Cancel);
    }

    /// Queues a mouse-pointer position update.
    pub(crate) fn mousemove(&mut self, x: i32, y: i32) {
        self.pending.push_back(RawInputEvent::MouseMove { pos: Vec2i::new(x, y) });
    }

    /// Queues a mouse-button press at the supplied pointer position.
    pub(crate) fn mousedown(&mut self, x: i32, y: i32, btn: MouseButton) {
        self.pending.push_back(RawInputEvent::MouseDown { pos: Vec2i::new(x, y), button: btn });
    }

    /// Queues a mouse-button release at the supplied pointer position.
    pub(crate) fn mouseup(&mut self, x: i32, y: i32, btn: MouseButton) {
        self.pending.push_back(RawInputEvent::MouseUp { pos: Vec2i::new(x, y), button: btn });
    }

    /// Queues one scroll-wheel or trackpad transition.
    pub(crate) fn scroll(&mut self, x: i32, y: i32) {
        self.pending.push_back(RawInputEvent::Scroll { delta: Vec2i::new(x, y) });
    }

    /// Queues one complete logical keyboard transition.
    pub(crate) fn key(&mut self, event: KeyEvent) {
        // The event already carries the backend's authoritative modifier snapshot. Retaining one
        // transition rather than parallel modifier/navigation streams preserves exact ordering.
        self.pending.push_back(RawInputEvent::Key { event });
    }

    /// Queues one UTF-8 text input transition.
    pub(crate) fn text(&mut self, text: &str) {
        self.pending.push_back(RawInputEvent::Text { text: text.to_owned() });
    }

    /// Returns whether at least one raw event is waiting for UI update.
    pub(crate) fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Returns state committed by events already popped from the queue.
    pub(crate) fn snapshot(&self) -> InputSnapshot {
        InputSnapshot {
            mouse_pos: self.mouse_pos,
            mouse_buttons: self.mouse_down,
            modifiers: self.modifiers,
        }
    }

    /// Applies and normalizes exactly one queued raw event.
    pub(crate) fn pop_event(&mut self) -> Option<UiInputEvent> {
        let event = self.pending.pop_front()?;
        Some(match event {
            RawInputEvent::Cancel => {
                // Reset held state only when this boundary is consumed, preserving earlier events.
                self.mouse_down = MouseButton::NONE;
                self.modifiers = Modifiers::NONE;
                UiInputEvent::Cancel
            }
            RawInputEvent::MouseMove { pos } => {
                // Raw coordinates come from the application and may jump across the complete i32
                // domain. Preserve the direction and clamp only the unrepresentable magnitude so
                // one hostile or synthetic input event cannot panic before retained routing.
                let delta = Vec2i::new(pos.x.saturating_sub(self.mouse_pos.x), pos.y.saturating_sub(self.mouse_pos.y));
                self.mouse_pos = pos;
                if self.mouse_down.is_empty() {
                    UiInputEvent::MouseMove { pos, delta }
                } else {
                    UiInputEvent::MouseDrag { pos, delta, buttons: self.mouse_down }
                }
            }
            RawInputEvent::MouseDown { pos, button } => {
                self.mouse_pos = pos;
                self.mouse_down |= button;
                UiInputEvent::MouseDown { pos, button }
            }
            RawInputEvent::MouseUp { pos, button } => {
                self.mouse_pos = pos;
                self.mouse_down &= !button;
                UiInputEvent::MouseUp { pos, button }
            }
            RawInputEvent::Scroll { delta } => UiInputEvent::Scroll { pos: self.mouse_pos, delta },
            RawInputEvent::Key { event } => {
                self.modifiers = event.modifiers;
                UiInputEvent::Key { event }
            }
            RawInputEvent::Text { text } => UiInputEvent::Text { text },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_calls_are_normalized_in_fifo_order_with_per_event_held_state() {
        let mut input = Input::default();
        input.mousemove(4, 5);
        input.mousedown(4, 5, MouseButton::LEFT);
        input.mousemove(9, 12);
        input.key(KeyEvent::pressed(Key::Shift, Modifiers::SHIFT));
        input.text("x");
        input.mouseup(9, 12, MouseButton::LEFT);

        assert!(matches!(
            input.pop_event(),
            Some(UiInputEvent::MouseMove { pos, delta })
                if (pos.x, pos.y, delta.x, delta.y) == (4, 5, 4, 5)
        ));
        assert!(matches!(
            input.pop_event(),
            Some(UiInputEvent::MouseDown { button, .. }) if button.bits() == MouseButton::LEFT.bits()
        ));
        assert_eq!(input.snapshot().mouse_buttons.bits(), MouseButton::LEFT.bits());
        assert!(matches!(
            input.pop_event(),
            Some(UiInputEvent::MouseDrag { pos, delta, buttons })
                if (pos.x, pos.y, delta.x, delta.y) == (9, 12, 5, 7) && buttons.bits() == MouseButton::LEFT.bits()
        ));
        assert!(matches!(
            input.pop_event(),
            Some(UiInputEvent::Key { event })
                if event == KeyEvent::pressed(Key::Shift, Modifiers::SHIFT)
        ));
        assert_eq!(input.snapshot().modifiers, Modifiers::SHIFT);
        assert!(matches!(input.pop_event(), Some(UiInputEvent::Text { text }) if text == "x"));
        assert!(matches!(
            input.pop_event(),
            Some(UiInputEvent::MouseUp { button, .. }) if button.bits() == MouseButton::LEFT.bits()
        ));
        assert_eq!(input.snapshot().mouse_buttons.bits(), MouseButton::NONE.bits());
        assert!(!input.has_pending());
    }

    #[test]
    fn repeated_and_zero_valued_calls_are_not_coalesced() {
        let mut input = Input::default();
        input.scroll(0, 0);
        input.scroll(0, 0);
        input.text("");

        assert!(matches!(input.pop_event(), Some(UiInputEvent::Scroll { .. })));
        assert!(matches!(input.pop_event(), Some(UiInputEvent::Scroll { .. })));
        assert!(matches!(input.pop_event(), Some(UiInputEvent::Text { text }) if text.is_empty()));
        assert!(input.pop_event().is_none());
    }

    /// Verifies public mouse input remains total when successive coordinates span the i32 domain.
    #[test]
    fn extreme_mouse_jump_saturates_each_delta_axis() {
        let mut input = Input::default();
        input.mousemove(i32::MIN, i32::MAX);
        input.mousemove(i32::MAX, i32::MIN);

        assert!(matches!(
            input.pop_event(),
            Some(UiInputEvent::MouseMove { pos, delta })
                if (pos.x, pos.y, delta.x, delta.y)
                    == (i32::MIN, i32::MAX, i32::MIN, i32::MAX)
        ));
        assert!(matches!(
            input.pop_event(),
            Some(UiInputEvent::MouseMove { pos, delta })
                if (pos.x, pos.y, delta.x, delta.y)
                    == (i32::MAX, i32::MIN, i32::MAX, i32::MIN)
        ));
    }
}
