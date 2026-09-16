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
//! Widget runtime contracts and retained event tracking.

use crate::{FrameRole, SurfaceRole};

use std::cmp::max;

use bitflags::bitflags;
use rs_math3d::Dimensioni;

use crate::atlas::AtlasHandle;
use crate::input::{Key, Modifiers};
use crate::theme::Skin;
use crate::Constraints;
use crate::UiInputEvent;
pub use crate::widget_context::{WidgetPaintCtx, WidgetUpdateCtx};

bitflags! {
    #[derive(Copy, Clone)]
    /// Controls which widget states should draw a filled background.
    pub struct WidgetFillOption : u32 {
        /// Fill the background for the idle/normal state.
        const NORMAL = 1;
        /// Fill the background while hovered.
        const HOVER = 2;
        /// Fill the background while the widget owns focus.
        const CLICK = 4;
        /// Fill the background for every interaction state.
        const ALL = Self::NORMAL.bits() | Self::HOVER.bits() | Self::CLICK.bits();
    }
}

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
        /// query `Context::text_input_area` after update to activate and position native input.
        const TEXT_INPUT = 256;
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

/// Common retained runtime phase contract implemented by concrete widgets.
///
/// Widgets participate in the retained update and paint phases. Geometry is deliberately split by
/// node kind: [`LeafWidget`] measures intrinsic content, while a scoped container algorithm measures
/// and places an authoritative child collection. A branch therefore has no meaningless leaf
/// measurement implementation.
///
/// The runtime routes an event before running that event's complete update traversal, then commits
/// layout before considering the next queued event. [`Widget::keyboard_behavior`] is the sole
/// keyboard-eligibility query. Event-kind filtering belongs to the dispatcher for ordinary widgets
/// and to a scoped container algorithm for an overloaded container surface.
pub trait Widget {
    /// Updates retained widget state for exactly one normalized input event.
    ///
    /// Pointer positions in `input` are relative to the widget's derived content rectangle, using
    /// the same origin as [`WidgetUpdateCtx::local_rect`]. Outer frame pixels remain part of the
    /// runtime hit target, so a pointer event on the border may lie just outside the local content
    /// bounds. At most one eligible widget receives `Some(input)` during a traversal; every other
    /// eligible widget receives `None`. Held state is available from [`WidgetUpdateCtx`]. The
    /// update context intentionally cannot record drawing commands. Implementations that retain a
    /// local drag mode must reconcile it from [`WidgetUpdateCtx::active`] on every call; pointer
    /// capture is runtime-owned and has no separate widget lifecycle callback. Delivery of a
    /// consumed or captured event alone does not invalidate measurement. Return [`Change::Changed`]
    /// whenever this call changes inputs read by measurement or child placement, including on a
    /// `None` update (animation, shared-state synchronization, or committing an editor on blur).
    /// The runtime propagates that invalidation upward before the next input is routed.
    ///
    /// [`Change::NoChange`] does not suppress event dispatch, painting, descendant updates, or
    /// invalidation already recorded through typed handles. Resize constraints, skin revision,
    /// topology edits, and handle mutations are tracked independently. Widget authors must report
    /// their own changes truthfully; app authors must synchronize external/shared state through a
    /// typed handle or a widget update that reports `Changed`. Out-of-band interior mutation is
    /// not observable by the runtime. Hidden/clipped widgets are not guaranteed an update, so
    /// shared geometry state should be synchronized through handles before it must affect layout.
    /// The default is suitable for widgets with no update behavior.
    fn update(&mut self, _ctx: &mut WidgetUpdateCtx<'_>, _input: Option<&UiInputEvent>) -> Change {
        // Static widgets retain valid measurement until an independently tracked input changes.
        Change::NoChange
    }
    /// Records paint commands through paint-only capabilities.
    ///
    /// Paint is observational with respect to application-authored semantic state, topology,
    /// interaction, and committed layout. Rendering caches belong to the owning COM object's
    /// phase machinery rather than the semantic widget implementation. Painting may therefore
    /// neither alter the current commit nor publish application-coordination events. Mutating
    /// retained UI through an independently captured typed widget handle is a contract violation
    /// rather than a deferred-next-frame operation.
    fn paint(&self, ctx: &mut WidgetPaintCtx<'_>);
    /// Drains geometry changes from explicit shared semantic models owned by a composite.
    /// This hook only consumes notification flags; it must not edit UI state or topology.
    /// Ordinary widgets report changes through update/setters and keep the default false.
    fn take_shared_change(&self) -> bool {
        false
    }

    /// Returns the semantic appearance used when [`WidgetOption::FRAME`] is effective.
    ///
    /// The runtime uses this one role for measurement, layout, input localization, and paint. A
    /// custom widget therefore changes themed border geometry without duplicating or bypassing the
    /// retained outer/content-box contract.
    fn frame_appearance_role(&self) -> FrameRole {
        // Generic framing remains the neutral default for application widgets that request FRAME
        // without opting into one of the built-in control meanings.
        FrameRole::Surface(SurfaceRole::GenericFrame)
    }
    /// Returns declarative keyboard routing capabilities for this widget surface.
    ///
    /// Keyboard focus is persistent and owned by retained routing rather than by pointer capture.
    /// Override this method for focusable custom widgets; the default keeps structural drawing and
    /// structural containers out of Tab order without another opt-out flag.
    fn keyboard_behavior(&self) -> KeyboardBehavior {
        KeyboardBehavior::NONE
    }
}

/// Intrinsic geometry contract for a retained leaf widget.
///
/// Each constraint axis explicitly distinguishes a finite maximum from an unbounded preferred-size
/// query. Returned components are clamped to zero before the node's frame is applied.
pub trait LeafWidget: Widget {
    /// Returns this leaf's preferred content size.
    fn measure(&self, skin: &Skin, atlas: &AtlasHandle, constraints: Constraints) -> Dimensioni;
}

impl Widget for WidgetOption {
    fn paint(&self, _ctx: &mut WidgetPaintCtx<'_>) {}
}

impl LeafWidget for WidgetOption {
    fn measure(&self, style: &Skin, atlas: &AtlasHandle, _constraints: Constraints) -> Dimensioni {
        // Internal placeholder widgets reserve enough room for text or an expand icon.
        let padding = style.metrics().padding.max(0);
        let vertical_pad = max(1, padding / 2);
        let font_height = atlas.get_font_height(style.resolve_font_role(atlas, crate::FontRole::Body)) as i32;
        let icon_height = atlas.get_icon_size(style.resolve_icon_role(atlas, crate::IconRole::ExpandDown)).height;
        let content = max(font_height, icon_height).max(0);
        // Valid font metrics and application skin values may independently reach i32 limits;
        // preferred geometry clamps rather than wrapping before the parent applies constraints.
        let height = content.saturating_add(vertical_pad.saturating_mul(2));
        let width = padding.saturating_mul(2).saturating_add(content);
        Dimensioni::new(width, height)
    }
}
