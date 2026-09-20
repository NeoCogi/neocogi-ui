//
// Copyright 2022-Present (c) Raja Lehtihet & Wael El Oraiby
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
//
// -----------------------------------------------------------------------------
// Ported to rust from https://github.com/rxi/microui/ and the original license
//
// Copyright (c) 2020 rxi
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to
// deal in the Software without restriction, including without limitation the
// rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
// sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
// IN THE SOFTWARE.
//
//! Copied interaction policy and keyboard action mapping shared by UI providers.
use bitflags::bitflags;
use crate::input::{Key, Modifiers};
use crate::UiInputEvent;

bitflags! {
    #[derive(Copy, Clone)]
    /// Widget-specific options that influence layout and interactivity.
    pub struct WidgetOption : u32 {
        /// Gives the widget a Skin-owned outer border and inset content rectangle.
        const FRAME = 512;
        /// Consumes scroll input while the widget is hovered.
        const GRAB_SCROLL = 32;
        /// Disables interaction with this widget's own surface; eligible descendants remain interactive.
        const NO_INTERACT = 4;
        /// Aligns the widget to the right side of the cell.
        const ALIGN_RIGHT = 2;
        /// Centers the widget inside the cell.
        const ALIGN_CENTER = 1;
        /// No special options.
        const NONE = 0;
    }
}

bitflags! {
    #[derive(Copy, Clone, Debug, Default, Eq, Hash, PartialEq)]
    /// Declarative keyboard capabilities exposed by one retained widget surface.
    ///
    /// Focus and traversal flags describe routing eligibility; action flags map normalized key
    /// presses through [`KeyboardBehavior::action`]. Pointer capture remains independent, and
    /// concrete widgets continue to own their semantic values and typed event ports. Built-in
    /// controls opt into the smallest applicable set; custom widgets are inert by default.
    pub struct KeyboardBehavior: u16 {
        /// The surface can own persistent keyboard focus after an eligible pointer press.
        const FOCUSABLE = 1;
        /// Sequential Tab traversal includes this surface.
        ///
        /// Every Tab stop is also treated as focusable; callers need not combine both flags.
        const TAB_STOP = 2;
        /// Enter requests the control's primary action.
        const ACTIVATE_ENTER = 4;
        /// Space requests the control's primary action.
        const ACTIVATE_SPACE = 8;
        /// Left and right request a one-step decrease and increase respectively.
        const ADJUST_HORIZONTAL = 16;
        /// Down and up request a one-step decrease and increase respectively.
        const ADJUST_VERTICAL = 32;
        /// Left collapses and right expands a hierarchical control.
        const EXPAND_COLLAPSE = 64;
        /// F4, Alt+Down, Alt+Up, and Escape control a popup-bearing surface.
        const POPUP = 128;
        /// The focused control currently accepts committed text from a host input method.
        ///
        /// Editors with a separate adjustment mode return this flag only while editing. Hosts
        /// query `UiRuntime::text_input_area` after update to activate and position native input.
        const TEXT_INPUT = 256;
        /// Focused embedded UI owns Tab, menu, and window-navigation keys.
        /// Ctrl+Tab (optionally Shift) leaves this scope through outer traversal.
        const KEYBOARD_SCOPE = 512;
        /// No keyboard focus or traversal behavior.
        const NONE = 0;
    }
}

/// Widget-level meaning derived from a normalized keyboard transition.
///
/// This small semantic vocabulary prevents each built-in control from independently interpreting
/// platform keys. It also lets custom widgets reuse the same Windows-style mapping while retaining
/// their own state and typed application events.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum KeyboardAction {
    /// Invoke the control's primary action.
    Activate,
    /// Move one logical step toward the lower value.
    Decrease,
    /// Move one logical step toward the higher value.
    Increase,
    /// Reveal a hierarchical branch or popup.
    Expand,
    /// Hide a hierarchical branch or popup.
    Collapse,
}

impl KeyboardBehavior {
    /// Returns whether pointer or programmatic routing may assign keyboard focus to this surface.
    pub const fn is_focusable(self) -> bool {
        self.intersects(Self::FOCUSABLE.union(Self::TAB_STOP))
    }

    /// Returns whether sequential traversal should visit this surface.
    pub const fn is_tab_stop(self) -> bool {
        self.intersects(Self::TAB_STOP)
    }

    /// Maps one routed key press to the action declared by these capabilities.
    pub fn action(self, input: Option<&UiInputEvent>) -> Option<KeyboardAction> {
        let UiInputEvent::Key { event } = input? else {
            return None;
        };
        if !event.is_pressed() {
            // Releases affect neither semantic values nor one-shot submission ports.
            return None;
        }

        let system_modifiers = Modifiers::ALT | Modifiers::CTRL | Modifiers::SUPER;
        let plain_command = !event.modifiers.intersects(system_modifiers);

        // Popup chords are intentionally resolved before plain directional behavior. Alt is part
        // of the command itself for Windows-style combo opening and closing.
        if self.intersects(Self::POPUP) {
            if event.key == Key::ArrowDown && event.modifiers.intersects(Modifiers::ALT) {
                return Some(KeyboardAction::Expand);
            }
            if event.key == Key::ArrowUp && event.modifiers.intersects(Modifiers::ALT) {
                return Some(KeyboardAction::Collapse);
            }
            if event.key == Key::Escape && !event.modifiers.intersects(Modifiers::CTRL | Modifiers::SUPER) {
                return Some(KeyboardAction::Collapse);
            }
            if event.key == Key::Function(4) && plain_command && !event.repeat {
                return Some(KeyboardAction::Activate);
            }
        }

        if !plain_command {
            // Ctrl, Alt, and Super chords remain available to application shortcuts. Shift alone
            // does not suppress ordinary control activation or directional adjustment.
            return None;
        }
        match event.key {
            // Activation is one-shot for a held key; directional actions intentionally retain
            // repeat so sliders, spin controls, and expanded trees respond continuously.
            Key::Enter if self.intersects(Self::ACTIVATE_ENTER) && !event.repeat => Some(KeyboardAction::Activate),
            Key::Space if self.intersects(Self::ACTIVATE_SPACE) && !event.repeat => Some(KeyboardAction::Activate),
            Key::ArrowLeft if self.intersects(Self::EXPAND_COLLAPSE) => Some(KeyboardAction::Collapse),
            Key::ArrowRight if self.intersects(Self::EXPAND_COLLAPSE) => Some(KeyboardAction::Expand),
            Key::ArrowLeft if self.intersects(Self::ADJUST_HORIZONTAL) => Some(KeyboardAction::Decrease),
            Key::ArrowRight if self.intersects(Self::ADJUST_HORIZONTAL) => Some(KeyboardAction::Increase),
            Key::ArrowDown if self.intersects(Self::ADJUST_VERTICAL) => Some(KeyboardAction::Decrease),
            Key::ArrowUp if self.intersects(Self::ADJUST_VERTICAL) => Some(KeyboardAction::Increase),
            _ => None,
        }
    }
}

/// Whether one widget update changed inputs to retained measurement or child placement.
///
/// This is a geometry contract, independent of event emission and painting. A paint-only change
/// such as moving a caret can return `NoChange`; changing text, intrinsic size, participation, or
/// child transforms must return `Changed`. When uncertain, returning `Changed` is conservative.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Change {
    /// Geometry inputs changed or may have changed; invalidate this node and its ancestors.
    Changed,
    /// This update preserved geometry inputs; independently recorded invalidation still applies.
    NoChange,
}

impl From<bool> for Change {
    /// Converts a concrete before/after comparison into the update contract.
    fn from(changed: bool) -> Self {
        if changed { Self::Changed } else { Self::NoChange }
    }
}
