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
//! Raw input state, key/button identifiers, and input-related state flags.

use bitflags::bitflags;

bitflags! {
    #[derive(Copy, Clone, Debug)]
    /// Mouse button state as reported by the input system.
    pub struct MouseButton : u32 {
        /// Middle mouse button.
        const MIDDLE = 4;
        /// Right mouse button.
        const RIGHT = 2;
        /// Left mouse button.
        const LEFT = 1;
        /// No buttons pressed.
        const NONE = 0;
    }
}

bitflags! {
    #[derive(Copy, Clone, Debug, Default, Eq, Hash, PartialEq)]
    /// Modifier state accompanying one logical keyboard transition.
    pub struct Modifiers: u8 {
        /// Either Alt/Option key is held.
        const ALT = 1;
        /// Either Control key is held.
        const CTRL = 2;
        /// Either Shift key is held.
        const SHIFT = 4;
        /// Either Windows/Command/Super key is held.
        const SUPER = 8;
        /// No modifiers are held.
        const NONE = 0;
    }
}

/// Backend-independent logical key identity.
///
/// Character keys describe the key's logical printable value and are deliberately separate from
/// [`crate::UiInputEvent::Text`]. A `Character('a')` transition can therefore participate in a
/// shortcut while composed UTF-8 text continues through the text-input channel exactly once.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Key {
    /// Logical printable key used by accelerators and custom controls.
    Character(char),
    /// Backspace editing key.
    Backspace,
    /// Forward-delete editing key.
    Delete,
    /// Return or keypad-enter key.
    Enter,
    /// Escape or cancellation key.
    Escape,
    /// Space-bar key.
    Space,
    /// Tab traversal key.
    Tab,
    /// Insert editing key.
    Insert,
    /// Home navigation key.
    Home,
    /// End navigation key.
    End,
    /// Page-up navigation key.
    PageUp,
    /// Page-down navigation key.
    PageDown,
    /// Up-arrow navigation key.
    ArrowUp,
    /// Down-arrow navigation key.
    ArrowDown,
    /// Left-arrow navigation key.
    ArrowLeft,
    /// Right-arrow navigation key.
    ArrowRight,
    /// One numbered function key, conventionally in the inclusive range `1..=24`.
    Function(u8),
    /// Alt/Option modifier key itself, needed to distinguish an Alt tap from an Alt chord.
    Alt,
    /// Control modifier key itself.
    Control,
    /// Shift modifier key itself.
    Shift,
    /// Windows/Command/Super modifier key itself.
    Super,
}

/// Direction of one queued logical key transition.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum KeyState {
    /// Key became held.
    Pressed,
    /// Key ceased being held.
    Released,
}

/// Complete logical keyboard transition accepted from a platform backend.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct KeyEvent {
    /// Logical key whose state changed.
    pub key: Key,
    /// Whether this transition pressed or released the key.
    pub state: KeyState,
    /// Complete modifier snapshot after applying this transition.
    pub modifiers: Modifiers,
    /// Whether the platform generated this press through key-repeat.
    ///
    /// Release transitions should set this to `false`.
    pub repeat: bool,
}

impl KeyEvent {
    /// Constructs one non-repeated press with its resulting modifier snapshot.
    pub const fn pressed(key: Key, modifiers: Modifiers) -> Self {
        Self {
            key,
            state: KeyState::Pressed,
            modifiers,
            repeat: false,
        }
    }

    /// Constructs one release with its resulting modifier snapshot.
    pub const fn released(key: Key, modifiers: Modifiers) -> Self {
        Self {
            key,
            state: KeyState::Released,
            modifiers,
            repeat: false,
        }
    }

    /// Marks a press as platform-generated key repeat.
    pub const fn repeated(mut self) -> Self {
        self.repeat = true;
        self
    }

    /// Returns whether this transition presses its key.
    pub const fn is_pressed(self) -> bool {
        matches!(self.state, KeyState::Pressed)
    }
}
