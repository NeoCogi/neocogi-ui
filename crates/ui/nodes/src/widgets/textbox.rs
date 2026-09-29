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
//! Single-line textbox widget and shared textbox update/paint helpers.
//!
//! The textbox stores a UTF-8 byte cursor and uses shared text-edit helpers to keep cursor movement
//! and deletion on valid Unicode scalar-value boundaries. Construction, programmatic replacement,
//! and text input remove CR and LF so the stored value always matches the one-line measurement
//! contract. Other characters absent from the selected atlas render through its fallback.
use crate::{ControlRole};

use crate::*;
use crate::math::clamp_i64_to_i32;

use super::text_edit::{
    ReturnBehavior, apply_text_input, caret_rect, centered_line_top, clamp_cursor_boundary, cursor_from_text_x, font_line_metrics, normalize_single_line_text,
};

/// One-shot construction input for a [`Textbox`].
pub struct TextboxParameters {
    /// Initial buffer edited by the textbox; CR and LF are removed when the widget is mounted.
    buf: String,
    /// Font used for measurement, editing, and paint.
    pub font: FontRef,
    /// Base widget options.
    pub opt: WidgetOption,
}

impl crate::LeafWidget for Textbox {
    fn measure(&self, style: &Skin, atlas: &AtlasHandle, constraints: Constraints) -> Dimensioni {
        self.preferred_size_widget(style, atlas, constraints)
    }
}

impl TextboxParameters {
    /// Creates textbox parameters with default widget options.
    pub fn new(buf: impl Into<String>) -> Self {
        Self {
            buf: buf.into(),
            font: FontRef::Role(FontRole::Body),
            opt: WidgetOption::FRAME,
        }
    }
}

#[neocogi_com::com_class(neocogi_ui_nodes_contracts::Textbox)]
/// Concrete retained textbox and dynamically constructible COM object.
pub struct Textbox {
    /// Non-semantic COM lifecycle and host-backed phase resources.
    runtime: crate::widget_object::WidgetRuntime,
    /// Current text buffer.
    buf: String,
    /// Current byte cursor, always positioned at a Unicode scalar-value boundary.
    cursor: usize,
    /// Exclusive selection endpoints and bounded user-edit history.
    editing: super::editing::Editing,
    /// Initialization-only font.
    font: FontRef,
    /// Single event slot; receives its destination during construction and closes with the widget.
    events: neocogi_ui_nodes_contracts::TextboxEventPort,
}

impl Textbox {
    /// Combines validated semantic parameters with one already bound common runtime.
    pub(crate) fn from_runtime(runtime: crate::widget_object::WidgetRuntime, parameters: TextboxParameters) -> Self {
        // Normalize at concrete storage so every constructor preserves the one-line invariant.
        let buf = normalize_single_line_text(parameters.buf);
        let cursor = buf.len();
        Self {
            runtime,
            buf,
            cursor,
            editing: super::editing::Editing::default(),
            font: parameters.font,
            events: Default::default(),
        }
    }

    /// Returns the current text buffer.
    pub fn text(&self) -> &str {
        self.buf.as_str()
    }

    /// Replaces the current text, removes CR and LF, and moves the cursor to the end.
    pub fn set_text(&mut self, text: impl Into<String>) {
        // Apply the same single-line policy as construction and pasted input. Keeping the stored
        // invariant here prevents measurement, hit testing, and paint from disagreeing about rows.
        self.editing = super::editing::Editing::default();
        self.buf = normalize_single_line_text(text);
        self.cursor = self.buf.len();
    }

    /// Clears the current text and resets the cursor.
    #[cfg(test)]
    pub fn clear(&mut self) {
        self.editing = super::editing::Editing::default();
        self.buf.clear();
        self.cursor = 0;
    }

    /// Returns the current byte cursor at a Unicode scalar-value boundary.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Moves the cursor to the nearest preceding Unicode scalar-value boundary in the current text.
    pub fn set_cursor(&mut self, cursor: usize) {
        self.editing.move_to(&self.buf, &mut self.cursor, cursor, false);
    }

    /// Moves the cursor to the end of the current text.
    #[cfg(test)]
    pub fn move_cursor_to_end(&mut self) {
        self.cursor = self.buf.len();
    }
}

impl Textbox {
    /// Measures a single-line editor, bounded by available width when supplied.
    fn preferred_size_widget(&self, style: &Skin, atlas: &AtlasHandle, constraints: Constraints) -> Dimensioni {
        let padding = style.metrics().padding.max(0);
        let vertical_pad = (padding / 2).max(1);
        let font = style.resolve_font(atlas, &self.font);
        let font_height = atlas.get_font_height(font) as i32;
        let text_w = if self.buf.is_empty() {
            0
        } else {
            atlas.get_text_size(font, self.buf.as_str()).width
        };
        // Text width and style padding are independent; clamp their preferred sum before applying
        // an optional finite parent constraint.
        let mut width = text_w.max(0).saturating_add(padding.saturating_mul(2)).saturating_add(1);
        if let Some(max_width) = constraints.width.bound() {
            width = width.min(max_width);
        }
        let height = font_height.max(0).saturating_add(vertical_pad.saturating_mul(2));
        Dimensioni::new(width, height)
    }

    /// Applies input and cursor movement for this textbox.
    fn update_widget(&mut self, ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) -> crate::Change {
        let font = ctx.skin().resolve_font(ctx.atlas(), &self.font);
        let outcome = textbox_update(ctx, input, &mut self.buf, &mut self.cursor, &mut self.editing, font, self.runtime.state.host());
        let changed = outcome.changed.then(|| crate::TextChanged {
            text: self.buf.clone(),
            cursor: self.cursor as u64,
        });
        let submitted = outcome.submitted.then(|| crate::TextSubmitted { text: self.buf.clone() });
        if let Some(event) = changed {
            ctx.posted(self.events.post(&neocogi_ui_nodes_contracts::TextboxEvent::Changed(event)));
        }
        if let Some(event) = submitted {
            ctx.posted(self.events.post(&neocogi_ui_nodes_contracts::TextboxEvent::Submitted(event)));
        }
        // Only committed value/text changes affect this widget's preferred dimensions.
        crate::Change::from(outcome.changed)
    }

    /// Paints the textbox frame, text, and caret.
    fn paint_widget(&self, ctx: &mut WidgetPaintCtx<'_>) {
        let font = ctx.skin().resolve_font(ctx.atlas(), &self.font);
        textbox_paint(ctx, self.buf.as_str(), self.cursor, &self.editing, ctx.options(), font);
    }
}

/// Shared single-line text editing update used by textbox and numeric inline editors.
pub(crate) fn textbox_update(
    ctx: &mut WidgetUpdateCtx<'_>,
    input: Option<&UiInputEvent>,
    buf: &mut String,
    cursor: &mut usize,
    editing: &mut super::editing::Editing,
    font: FontId,
    host: neocogi_com::ComRef<'_, neocogi_ui_core_contracts::IWidgetHostAbi>,
) -> TextboxUpdateOutcome {
    let mut outcome = TextboxUpdateOutcome::default();
    let r = ctx.local_rect();
    if !ctx.focused() {
        // Reset to end when blurred so refocusing starts from a predictable position.
        editing.move_to(buf, cursor, buf.len(), false);
    }
    let mut cursor_pos = clamp_cursor_boundary(buf, *cursor);

    let mouse_pressed = match input {
        Some(UiInputEvent::MouseDown { button, .. }) => *button,
        _ => MouseButton::NONE,
    };
    let mouse_pos = match input {
        Some(
            UiInputEvent::MouseMove { pos, .. }
            | UiInputEvent::MouseDrag { pos, .. }
            | UiInputEvent::MouseDown { pos, .. }
            | UiInputEvent::MouseUp { pos, .. }
            | UiInputEvent::Scroll { pos, .. },
        ) => *pos,
        _ => Vec2i::default(),
    };
    let key_event = match input {
        Some(UiInputEvent::Key { event }) => Some(*event),
        _ => None,
    };
    let text_input = match input {
        Some(UiInputEvent::Text { text }) => text.as_str(),
        _ => "",
    };
    // Clipboard failures do not abort the frame; any earlier completed receipt still reports its edit.
    if let Ok(changed) = super::clipboard::advance(editing, host, buf, &mut cursor_pos, key_event, ReturnBehavior::Submit, ctx.focused()) {
        outcome.changed |= changed;
    }
    let home_pressed = key_event.is_some_and(|event| event.is_pressed() && event.key == Key::Home);
    let end_pressed = key_event.is_some_and(|event| event.is_pressed() && event.key == Key::End);
    let edit = if ctx.focused() {
        apply_text_input(buf, cursor_pos, editing, text_input, key_event, ReturnBehavior::Submit)
    } else {
        // Without focus, the textbox ignores key/text input but keeps a consistent outcome.
        super::text_edit::TextEditOutcome {
            cursor: cursor_pos,
            changed: false,
            moved: false,
            submit: false,
        }
    };
    if ctx.focused() {
        cursor_pos = edit.cursor;
        if edit.changed {
            outcome.changed = true;
        }
        if edit.submit {
            outcome.submitted = true;
        }
        if home_pressed {
            // A single-line editor has one visual start, so Home always selects byte zero.
            editing.move_to(buf, &mut cursor_pos, 0, ctx.modifiers().intersects(Modifiers::SHIFT));
        } else if end_pressed {
            // End is the symmetric single-line boundary and selects the complete buffer length.
            editing.move_to(buf, &mut cursor_pos, buf.len(), ctx.modifiers().intersects(Modifiers::SHIFT));
        }
    }
    // Submission records an event but does not cooperatively mutate focus. The runtime dispatcher
    // remains authoritative and continues routing keyboard/text input to its focused node.

    let text_metrics = ctx.atlas().get_text_size(font, buf.as_str());
    let padding = ctx.skin().metrics().padding;
    let caret_width = ctx.atlas().get_text_size(font, &buf[..cursor_pos]).width;
    let textx = textbox_text_x(r, padding, text_metrics.width, caret_width);

    let dragging = matches!(input, Some(UiInputEvent::MouseDrag { buttons, .. }) if buttons.intersects(MouseButton::LEFT));
    if ctx.focused() && (dragging || mouse_pressed.intersects(MouseButton::LEFT) && ctx.mouse_over(r, mouse_pos)) {
        // Convert local click x into a UTF-8 boundary cursor position.
        let click_x = clamp_i64_to_i32(i64::from(mouse_pos.x) - (i64::from(textx) - i64::from(r.x)));
        let target = cursor_from_text_x(buf, click_x, font, ctx.atlas());
        editing.move_to(buf, &mut cursor_pos, target, dragging || ctx.modifiers().intersects(Modifiers::SHIFT));
    }

    cursor_pos = clamp_cursor_boundary(buf, cursor_pos);
    *cursor = cursor_pos;
    outcome
}

/// Semantic editing events produced by the shared single-line editor.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct TextboxUpdateOutcome {
    /// Whether user input changed the retained text buffer.
    pub(crate) changed: bool,
    /// Whether user input requested submission without changing focus.
    pub(crate) submitted: bool,
}

/// Chooses the same origin for hit testing and painting while keeping the active caret visible.
fn textbox_text_x(bounds: Recti, padding: i32, text_width: i32, caret_width: i32) -> i32 {
    // Evaluate the complete signed expression in i64. Negative application padding and saturated
    // measured text can then cancel correctly before the retained coordinate is clamped once.
    let trailing_origin = clamp_i64_to_i32(i64::from(bounds.width) - i64::from(padding) - i64::from(text_width) - 1);
    let caret_origin = clamp_i64_to_i32(i64::from(bounds.width) - i64::from(padding) - i64::from(caret_width) - 1);
    let origin = caret_origin.min(padding).max(trailing_origin.min(padding));
    clamp_i64_to_i32(i64::from(bounds.x) + i64::from(origin))
}

/// Shared single-line textbox painting used by textbox and numeric inline editors.
pub(crate) fn textbox_paint(ctx: &mut WidgetPaintCtx<'_>, buf: &str, cursor: usize, editing: &super::editing::Editing, opt: WidgetOption, font: FontId) {
    let r = ctx.local_rect();
    if !opt.intersects(WidgetOption::FRAME) {
        // Runtime-owned framed editors already painted the complete input patch below this content.
        ctx.draw_control_center(ControlRole::TextInput, r);
    }

    let metrics = font_line_metrics(font, ctx.atlas());
    let texty = centered_line_top(r, metrics.line_height);
    let baseline_y = clamp_i64_to_i32(i64::from(texty) + i64::from(metrics.baseline));

    let text_metrics = ctx.atlas().get_text_size(font, buf);
    let padding = ctx.skin().metrics().padding;
    let cursor_pos = clamp_cursor_boundary(buf, cursor);
    let caret_offset = if cursor_pos == 0 {
        0
    } else {
        ctx.atlas().get_text_size(font, &buf[..cursor_pos]).width
    };

    let textx = textbox_text_x(r, padding, text_metrics.width, caret_offset);
    if ctx.focused() {
        // Focused editing path clips text/caret to the textbox bounds.
        let color = ctx.control_content_color(ControlRole::TextInput);
        let caret = caret_rect(clamp_i64_to_i32(i64::from(textx) + i64::from(caret_offset)), baseline_y, metrics, r);
        ctx.with_clip(r, |ctx| {
            ctx.draw_text(font, buf, vec2(textx, texty), color);
            if let Some(range) = editing.selection(buf, cursor) {
                super::text_edit::paint_selection(ctx, font, buf, range, vec2(textx, texty), metrics.line_height);
            }
            ctx.draw_rect(caret, color);
        });
    } else {
        ctx.draw_control_text_with_font(font, buf, r, ControlRole::TextInput, opt);
    }
}

impl Widget for Textbox {
    fn frame_appearance_role(&self) -> FrameRole {
        // Single-line textboxes use the semantic input frame for every interaction state.
        FrameRole::Control(ControlRole::TextInput)
    }

    fn update(&mut self, ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) -> crate::Change {
        self.update_widget(ctx, input)
    }

    fn paint(&self, ctx: &mut WidgetPaintCtx<'_>) {
        self.paint_widget(ctx);
    }

    fn keyboard_behavior(&self) -> KeyboardBehavior {
        KeyboardBehavior::TAB_STOP | KeyboardBehavior::TEXT_INPUT
    }
}

#[cfg(test)]
impl From<TextboxParameters> for crate::test_support::algorithm::Algorithm<Textbox> {
    /// Transfers one-shot parameters into the concrete Textbox runtime.
    fn from(parameters: TextboxParameters) -> Self {
        Self::new(parameters.opt, |runtime| Textbox::from_runtime(runtime, parameters))
    }
}

impl crate::widget_object::WidgetAlgorithm for Textbox {
    /// Ends ordinary native events on logical COM closure, before retained semantic storage is finally released.
    fn close_events(&mut self) {
        self.events.close();
    }
}

impl crate::widget_object::HostedWidget for Textbox {
    /// Releases queues, revokes outstanding clipboard receipts, and drops local history.
    fn close_contents(&mut self) {
        crate::widget_object::WidgetAlgorithm::close_events(self);
        self.editing = super::editing::Editing::default();
    }

    /// Borrows common lifecycle and resource state without a second textbox representation.
    fn runtime(&self) -> &crate::widget_object::WidgetRuntime {
        &self.runtime
    }

    /// Exclusively borrows common lifecycle and resource state for a COM mutation or phase.
    fn runtime_mut(&mut self) -> &mut crate::widget_object::WidgetRuntime {
        &mut self.runtime
    }
}

impl crate::widget_object::FontState for Textbox {
    /// Borrows the semantic font reference already used by layout and painting.
    fn font(&self) -> &crate::FontRef {
        &self.font
    }
    /// Replaces the same field after the COM resource owner validates the selection.
    fn set_font(&mut self, font: crate::FontRef) {
        self.font = font;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{test_atlas, test_skin};

    #[derive(Debug, Eq, PartialEq)]
    enum RecordedEvent {
        Changed(String, u64),
        Submitted(String),
    }

    /// Maps both enum variants into one ordinary consumer FIFO, preserving their actual post order.
    fn text_dispatcher(
        textbox: crate::test_support::algorithm::Algorithm<Textbox>,
    ) -> (
        crate::test_support::algorithm::Algorithm<neocogi_com::LocalCom<Textbox>>,
        impl FnMut(&mut Vec<RecordedEvent>) -> bool,
    ) {
        let (events_tx, events) = std::sync::mpsc::channel();
        let queue = neocogi_ui::textbox::queue(events_tx, |event| match event {
            neocogi_ui::textbox::Event::Changed(value) => RecordedEvent::Changed(value.text, value.cursor),
            neocogi_ui::textbox::Event::Submitted(value) => RecordedEvent::Submitted(value.text),
        })
        .unwrap();
        let textbox = textbox.map(|mut value| {
            Textbox::into_local_cyclic::<TextboxAbi>(move |origin| {
                value.events = neocogi_com::events::EventPort::with_queue(Some(queue), origin);
                value
            })
            .unwrap()
        });
        (textbox, move |values: &mut Vec<RecordedEvent>| {
            let before = values.len();
            values.extend(events.try_iter());
            values.len() != before
        })
    }

    fn update_textbox(textbox: &mut Textbox, focused: bool, input: Vec<UiInputEvent>) {
        let atlas = test_atlas();
        let style = test_skin(&atlas);
        let bounds = rect(0, 0, 120, 20);
        let mut modifiers = Modifiers::NONE;
        for event in &input {
            if let UiInputEvent::Key { event } = event {
                modifiers = event.modifiers;
            }
            let mut ctx =
                WidgetUpdateCtx::new_with_interaction(bounds, bounds, &style, &atlas, true, false, focused, false, false, MouseButton::NONE, modifiers);
            textbox.update(&mut ctx, Some(event));
        }
    }

    /// Home and intermediate positions remain visible when the text is wider than its control.
    #[test]
    fn horizontal_origin_reveals_the_active_caret() {
        let bounds = rect(10, 0, 100, 20);
        for caret in [0, 40, 150, 300] {
            let x = textbox_text_x(bounds, 4, 300, caret) + caret;
            assert!((14..=105).contains(&x));
        }
        assert_eq!(textbox_text_x(bounds, 4, 300, 0), 14);
    }

    /// Extended line navigation replaces the selection and local undo restores its exact endpoints.
    #[test]
    fn selection_replacement_undo_and_programmatic_reset() {
        let mut textbox = crate::test_support::algorithm::Algorithm::<Textbox>::from(TextboxParameters::new("abcd"));
        textbox.set_cursor(1);
        update_textbox(
            &mut textbox,
            true,
            vec![UiInputEvent::Key {
                event: KeyEvent::pressed(Key::End, Modifiers::SHIFT),
            }],
        );
        assert_eq!(textbox.editing.selection(&textbox.buf, textbox.cursor), Some(1..4));
        update_textbox(&mut textbox, true, vec![UiInputEvent::Text { text: "X".into() }]);
        assert_eq!(textbox.text(), "aX");
        update_textbox(
            &mut textbox,
            true,
            vec![UiInputEvent::Key {
                event: KeyEvent::pressed(Key::Character('z'), Modifiers::CTRL),
            }],
        );
        assert_eq!(textbox.text(), "abcd");
        assert_eq!(textbox.editing.selection(&textbox.buf, textbox.cursor), Some(1..4));
        textbox.set_text("application value");
        update_textbox(
            &mut textbox,
            true,
            vec![UiInputEvent::Key {
                event: KeyEvent::pressed(Key::Character('z'), Modifiers::CTRL),
            }],
        );
        assert_eq!(textbox.text(), "application value");
        assert_eq!(textbox.editing.selection(&textbox.buf, textbox.cursor), None);
    }

    /// A captured drag extends beyond the control bounds instead of discarding its last segment.
    #[test]
    fn pointer_drag_selects_to_the_buffer_edge() {
        let mut textbox = crate::test_support::algorithm::Algorithm::<Textbox>::from(TextboxParameters::new("abcd"));
        update_textbox(
            &mut textbox,
            true,
            vec![
                UiInputEvent::MouseDown {
                    pos: vec2(0, 10),
                    button: MouseButton::LEFT,
                },
                UiInputEvent::MouseDrag {
                    pos: vec2(500, 10),
                    delta: vec2(500, 0),
                    buttons: MouseButton::LEFT,
                },
            ],
        );
        assert_eq!(textbox.editing.selection(&textbox.buf, textbox.cursor), Some(0..4));
    }

    #[test]
    fn text_events_preserve_complete_snapshots_and_consumer_fifo() {
        let textbox = crate::test_support::algorithm::Algorithm::<Textbox>::from(TextboxParameters::new(""));
        let (textbox, mut dispatcher) = text_dispatcher(textbox);
        let mut textbox = textbox.try_borrow_mut().unwrap();

        update_textbox(
            &mut textbox,
            true,
            vec![
                UiInputEvent::Text { text: "ab".into() },
                UiInputEvent::Text { text: "cd".into() },
                UiInputEvent::Key {
                    event: KeyEvent::pressed(Key::Enter, Modifiers::NONE),
                },
            ],
        );
        update_textbox(&mut textbox, true, vec![UiInputEvent::Text { text: "e".into() }]);

        assert_eq!(textbox.text(), "abcde");
        let mut events = Vec::new();
        assert!(dispatcher(&mut events));
        assert_eq!(
            events,
            [
                RecordedEvent::Changed("ab".to_owned(), 2),
                RecordedEvent::Changed("abcd".to_owned(), 4),
                RecordedEvent::Submitted("abcd".to_owned()),
                RecordedEvent::Changed("abcde".to_owned(), 5),
            ]
        );
    }

    #[test]
    fn programmatic_text_and_cursor_setters_are_silent() {
        let textbox = crate::test_support::algorithm::Algorithm::<Textbox>::from(TextboxParameters::new("initial"));
        let (textbox, mut dispatcher) = text_dispatcher(textbox);
        let mut textbox = textbox.try_borrow_mut().unwrap();
        textbox.set_text("replacement");
        textbox.set_cursor(3);
        textbox.move_cursor_to_end();
        textbox.clear();
        assert!(!dispatcher(&mut Vec::new()));
    }

    /// Verifies the public Home and End keys select the two single-line cursor boundaries.
    #[test]
    fn home_and_end_keys_select_single_line_boundaries() {
        let mut textbox = crate::test_support::algorithm::Algorithm::<Textbox>::from(TextboxParameters::new("abcd"));
        textbox.set_cursor(2);

        update_textbox(
            &mut textbox,
            true,
            vec![UiInputEvent::Key {
                event: KeyEvent::pressed(Key::Home, Modifiers::NONE),
            }],
        );
        assert_eq!(textbox.cursor(), 0);

        update_textbox(
            &mut textbox,
            true,
            vec![UiInputEvent::Key {
                event: KeyEvent::pressed(Key::End, Modifiers::NONE),
            }],
        );
        assert_eq!(textbox.cursor(), textbox.text().len());
    }

    /// Verifies construction, programmatic replacement, and pasted input all remain single-line.
    #[test]
    fn every_text_ingress_removes_line_endings() {
        let mut textbox = crate::test_support::algorithm::Algorithm::<Textbox>::from(TextboxParameters::new("a\r\nb\rc\n"));
        assert_eq!(textbox.text(), "abc");
        assert_eq!(textbox.cursor(), 3);

        textbox.set_text("d\r\ne\rf\n");
        assert_eq!(textbox.text(), "def");
        assert_eq!(textbox.cursor(), 3);

        update_textbox(&mut textbox, true, vec![UiInputEvent::Text { text: "g\r\nh\ri\n".into() }]);
        assert_eq!(textbox.text(), "defghi");
        assert_eq!(textbox.cursor(), 6);
    }
}

use crate::widget_object::{HostedWidget, direct_leaf_layout_impl, direct_widget_phase_impl, set_direct_font_role, set_direct_named_font};
use neocogi_ui_core_contracts::{GeometryChange, IWidgetHostAbi, FontRole};
use neocogi_ui_nodes_contracts::{TextboxAbi};
use neocogi_com::*;

direct_widget_phase_impl!(Textbox);
direct_leaf_layout_impl!(Textbox);

impl Textbox {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(crate) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: crate::widget_object::SharedAssets,
        text: NcCopyFromBufferOffset<'_, u8>,
        queue: Option<ComPtr<neocogi_ui_nodes_contracts::TextboxEventQueueAbi>>,
    ) -> Result<ComPtr<TextboxAbi>, NcStatus> {
        let mut value = Self::build(host, text, Some(&resources))?;
        // Fix the sole destination before publishing the completed widget.
        Self::from_value_cyclic::<TextboxAbi>(move |origin| {
            value.events = neocogi_com::events::EventPort::with_queue(queue, origin);
            value
        })?
        .query()
    }

    /// Builds one textbox with either directly imported or runtime-shared resources.
    fn build(
        host: ComPtr<IWidgetHostAbi>,
        input: NcCopyFromBufferOffset<'_, u8>,
        resources: Option<&crate::widget_object::SharedAssets>,
    ) -> Result<Self, NcStatus> {
        let parameters = crate::TextboxParameters::new(crate::boundary::text(input)?);
        let runtime = crate::widget_object::WidgetRuntime::for_resources(host, parameters.opt, resources)?;
        Ok(Self::from_runtime(runtime, parameters))
    }
}

impl neocogi_ui_nodes_contracts::Textbox for Textbox {
    /// Returns an immutable snapshot without exposing the semantic String allocation.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        Ok(NcString::from_utf8_str(self.text()))
    }
    /// Applies single-line normalization before recording an actual geometry change.
    fn set_text(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let input = crate::boundary::text(input)?;
        let previous = self.text().to_owned();
        self.set_text(input);
        if self.text() != previous {
            self.runtime_mut().state.record_change(GeometryChange::Changed);
        }
        Ok(())
    }
    /// Reports a byte cursor rather than a glyph or grapheme index.
    fn cursor(&self) -> Result<u64, NcStatus> {
        self.runtime().state.readable()?;
        Ok(self.cursor() as u64)
    }
    /// Clamps oversize input before applying the shared Unicode-boundary algorithm.
    fn set_cursor(&mut self, cursor: u64) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        self.set_cursor(usize::try_from(cursor).unwrap_or(usize::MAX));
        Ok(())
    }
    /// Validates a semantic font role before changing the Textbox's measurement input.
    fn set_font_role(&mut self, role: FontRole) -> Result<(), NcStatus> {
        set_direct_font_role(self, role)
    }
    /// Resolves one exact named font before replacing the Textbox's selection.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_font(self, name)
    }
}

/// Ordinary leaves reserve no duplicate ancestors or per-instance descriptor tables.
#[cfg(all(test, target_pointer_width = "64"))]
#[test]
fn com_object_control_is_independent_of_interface_count() {
    // The call gate occupies one word in the payload. Arc owns its private
    // strong/weak header outside this storage; this is not total allocation size.
    // No per-interface fields remain; handles carry their selected tables.
    // The constructor receiver is not an interface implemented by the widget.
    assert_eq!(Textbox::COM_INTERFACE_COUNT, 4);
    println!(
        "Textbox: state={}, payload={}",
        core::mem::size_of::<Textbox>(),
        core::mem::size_of::<TextboxComObject>()
    );
    assert_eq!(core::mem::offset_of!(TextboxComObject, value), 8);
    assert_eq!(core::mem::size_of::<TextboxComObject>(), core::mem::size_of::<Textbox>() + 8);
}
