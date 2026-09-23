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
//! Multiline editable text content composed inside a retained scroll area.
//!
//! [`TextEditor`] owns only text, cursor, editing, and caret behavior. [`TextEditor::create`] mounts
//! that leaf inside [`ScrollArea`], which exclusively owns clipping, translation, wheel input,
//! scroll offsets, and its two reusable scrollbar children. Editing operates on Unicode scalar
//! values rather than grapheme clusters. Stored text uses LF line endings: construction,
//! programmatic replacement, and text input convert CRLF and lone CR at ingress. Rendering uses the
//! selected atlas's glyph coverage and missing-character fallback.

use neocogi_ui_nodes_contracts::TextAreaObserverAbi;
use crate::{ControlRole};

use crate::text_layout::{TextLine, build_text_lines, text_block_size};
use crate::*;

/// Focused editor-test input excludes the removed native scrolling composition.
#[cfg(test)]
struct TextAreaParameters {
    /// Initial text is normalized by the production editor constructor.
    buf: String,
    /// Wrapping controls the production text layout used during measurement and editing.
    wrap: TextWrap,
    /// Semantic font is resolved by the real host resources.
    font: FontRef,
}
#[cfg(test)]
impl TextAreaParameters {
    /// Supplies the same defaults as public multiline editor construction.
    fn new(text: impl Into<String>) -> Self {
        Self {
            buf: text.into(),
            wrap: TextWrap::None,
            font: FontRef::Role(FontRole::Body),
        }
    }
    /// Selects the specific line-wrapping policy under test.
    fn wrap(mut self, wrap: TextWrap) -> Self {
        self.wrap = wrap;
        self
    }
}

use super::text_edit::{
    CaretAffinity, FontLineMetrics, ReturnBehavior, affinity_for_line, apply_text_input, caret_rect, clamp_cursor_boundary, cursor_from_x, cursor_x_in_line,
    font_line_metrics, line_index_for_cursor, normalize_multiline_text,
};

/// Concrete editable text leaf retained as the ordinary content of a [`ScrollArea`].
#[neocogi_com::com_object(interfaces(
    neocogi_ui_core_contracts::IMultilineEditor,
    neocogi_ui_core_contracts::ILeafLayout,
    neocogi_ui_core_contracts::IFontStyle
))]
pub struct TextEditor {
    /// Non-semantic COM lifecycle and host-backed phase resources.
    runtime: crate::widget_object::WidgetRuntime,
    /// Current text buffer.
    buf: String,
    /// Current byte cursor, always positioned at a Unicode scalar-value boundary.
    cursor: usize,
    /// Exclusive selection endpoints and bounded user-edit history.
    editing: super::editing::Editing,
    /// Requests a runtime-only vertical-cursor preference reset.
    reset_preferred_x: bool,
    /// Requests that the next update reveal the current caret through the containing scroll area.
    reveal_caret: bool,
    /// Wrapping mode used by measurement, editing, and paint.
    wrap: TextWrap,
    /// Font used by measurement, editing, and paint.
    font: FontRef,
    /// Runtime-only derived editing state.
    interaction: TextAreaInteraction,
    /// Shared scalar scroll data supports caret reveal without retaining a parent widget or tree.
    scroll_area: Option<std::rc::Rc<crate::ScrollModel>>,
    /// One constructor-supplied destination shared by edits and submissions.
    observer: Option<ComPtr<TextAreaObserverAbi>>,
}

/// Concrete fixed scrolling text-area class with one actual editor child.
#[neocogi_com::com_object(neocogi_ui_nodes_contracts::TextArea)]
pub struct TextArea {
    /// Non-semantic COM lifecycle and host-backed phase resources.
    runtime: crate::widget_object::WidgetRuntime,
    /// Exactly three owned structural children: surface and both scrollbar objects.
    pub(crate) editor: ComPtr<neocogi_ui_core_contracts::IChildrenEditorAbi>,
    /// Multiline editing capability of the sole content child.
    pub(crate) text: ComPtr<neocogi_ui_core_contracts::IMultilineEditorAbi>,
    /// Font capability queried from that same content child.
    pub(crate) font: ComPtr<neocogi_ui_core_contracts::IFontStyleAbi>,
    /// Scalar scrolling state shared with the surface, scrollbars, and editor caret requests.
    pub(crate) model: std::rc::Rc<crate::ScrollModel>,
}

impl TextArea {
    /// Combines the completed fixed topology and its queried editor capabilities in one class value.
    pub(crate) fn from_runtime(
        runtime: crate::widget_object::WidgetRuntime,
        editor: ComPtr<neocogi_ui_core_contracts::IChildrenEditorAbi>,
        text: ComPtr<neocogi_ui_core_contracts::IMultilineEditorAbi>,
        font: ComPtr<neocogi_ui_core_contracts::IFontStyleAbi>,
        model: std::rc::Rc<crate::ScrollModel>,
    ) -> Self {
        Self { runtime, editor, text, font, model }
    }
}

impl Widget for TextArea {
    /// Drains shared caret, scrollbar, and enablement requests even when ordinary update is skipped.
    fn take_shared_change(&self) -> bool {
        crate::ScrollComposition::take_scroll_change(self)
    }

    /// Uses the same panel appearance as an ordinary ScrollArea.
    fn frame_appearance_role(&self) -> FrameRole {
        crate::ScrollComposition::scroll_frame_role(self)
    }

    /// Applies wheel input to the scroll model shared with the actual editor and bars.
    fn update(&mut self, _ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) -> crate::Change {
        crate::ScrollComposition::update_scroll(self, input)
    }

    /// Paints only the outer panel and scrollbar corner; the editor child paints text and caret.
    fn paint(&self, ctx: &mut WidgetPaintCtx<'_>) {
        crate::ScrollComposition::paint_scroll(self, ctx);
    }
}

impl crate::ScrollComposition for TextArea {
    /// Supplies the exact model shared by every member of the fixed text-area composition.
    fn scroll_model(&self) -> &crate::ScrollModel {
        &self.model
    }
}

impl crate::widget_object::WidgetAlgorithm for TextArea {
    /// Adds or removes wheel routing from the class's current common options.
    fn effective_options(&self, options: WidgetOption) -> WidgetOption {
        crate::ScrollComposition::scroll_options(self, options)
    }
}

impl crate::widget_object::HostedWidget for TextArea {
    /// Borrows common phase state stored directly beside the fixed composition capabilities.
    fn runtime(&self) -> &crate::widget_object::WidgetRuntime {
        &self.runtime
    }

    /// Exclusively borrows common phase state for mutation or resource refresh.
    fn runtime_mut(&mut self) -> &mut crate::widget_object::WidgetRuntime {
        &mut self.runtime
    }

    /// Closes the surface, editor, and both scrollbar children through their fixed writer.
    fn close_contents(&mut self) -> Result<(), NcStatus> {
        self.editor.close()?.finish()
    }
}

/// Runtime-only cursor navigation state retained by the editable leaf.
#[derive(Default)]
struct TextAreaInteraction {
    /// Desired caret x position preserved while moving vertically.
    preferred_x: Option<i32>,
    /// Visual side retained when a wrapped line end and continuation start share one byte offset.
    caret_affinity: CaretAffinity,
}

impl TextEditor {
    /// Creates the unmounted editable leaf before its containing scroll area exists.
    #[cfg(test)]
    fn new_editor(parameters: TextAreaParameters) -> crate::test_support::algorithm::Algorithm<Self> {
        // Initialize semantic and event state together so direct unit tests and composed creation
        // observe the same editing behavior.
        crate::test_support::algorithm::Algorithm::new(WidgetOption::NONE, |runtime| {
            Self::from_runtime(runtime, parameters.buf, parameters.wrap, parameters.font, None)
        })
    }

    /// Returns the current text buffer.
    pub fn text(&self) -> &str {
        // Expose a borrowed semantic view without cloning or consulting presentation state.
        self.buf.as_str()
    }

    /// Replaces text, normalizes line endings to LF, moves the cursor to the end, and reveals it.
    pub fn set_text(&mut self, text: impl Into<String>) {
        // Replace buffer and cursor atomically so no phase can observe a cursor outside the new
        // scalar-aligned byte range. Canonical LF storage also keeps layout and atlas rendering on
        // one line-break convention for programmatic replacements.
        self.editing = super::editing::Editing::default();
        self.buf = normalize_multiline_text(text);
        self.cursor = self.buf.len();
        self.interaction.caret_affinity = CaretAffinity::Upstream;
        self.reset_preferred_x = true;
        self.reveal_caret = true;
    }

    /// Returns the current byte cursor at a Unicode scalar-value boundary.
    pub const fn cursor(&self) -> usize {
        // Every mutation path clamps or derives the cursor from valid source boundaries.
        self.cursor
    }

    /// Moves the cursor to the nearest preceding Unicode scalar-value boundary.
    pub fn set_cursor(&mut self, cursor: usize) {
        // Clamp external byte offsets before scheduling navigation-state reset and caret reveal.
        self.editing.move_to(&self.buf, &mut self.cursor, cursor, false);
        // A bare byte offset has no visual-side information, so public positioning deterministically
        // selects the preceding wrapped segment when the offset is shared.
        self.interaction.caret_affinity = CaretAffinity::Upstream;
        self.reset_preferred_x = true;
        self.reveal_caret = true;
    }

    /// Measures intrinsic editable text using the wrapping width supplied by ScrollArea.
    fn preferred_size_widget(&self, style: &Skin, atlas: &AtlasHandle, constraints: Constraints) -> Dimensioni {
        // Resolve word wrapping against a bounded viewport width while leaving unwrapped content
        // intrinsically wide enough to create horizontal overflow.
        let font = style.resolve_font(atlas, &self.font);
        let max_width = match self.wrap {
            TextWrap::None => i32::MAX / 4,
            TextWrap::Word | TextWrap::WordOrCharacter => constraints.width.bound().unwrap_or(i32::MAX / 4).max(1),
        };
        let lines = build_text_lines(self.buf.as_str(), self.wrap, max_width, font, atlas);
        // Editable text retains the final blank row after a trailing newline so the caret always has
        // a measurable line. Word-only wrapping may exceed a bounded width by design.
        text_block_size(&lines, atlas.get_font_height(font) as i32)
    }

    /// Requests that the containing scroll area reveal one content-local caret rectangle.
    fn reveal_rect(&self, rect: Recti) {
        // The shared scalar request marks the containing layout dirty without borrowing a parent
        // widget or relying on whether that parent receives an ordinary update this pass.
        if let Some(scroll_area) = &self.scroll_area {
            scroll_area.reveal(rect);
        }
    }

    /// Applies one routed editing event and publishes resulting semantic events.
    fn update_widget(&mut self, ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) -> crate::Change {
        // Runtime-only navigation preference is reset lazily so typed cursor setters need no phase
        // context and remain safe between retained updates.
        if self.reset_preferred_x {
            self.interaction.preferred_x = None;
            self.reset_preferred_x = false;
        }
        let font = ctx.skin().resolve_font(ctx.atlas(), &self.font);
        let outcome = textarea_update(ctx, input, self, font);

        // Emit complete immutable snapshots after the semantic update has committed cursor and text.
        if outcome.changed {
            ctx.posted(
                neocogi_ui_nodes_contracts::TextAreaEvent::Changed(crate::TextChanged {
                    text: self.buf.clone(),
                    cursor: self.cursor as u64,
                })
                .post_to(&mut self.observer),
            );
        }
        if outcome.submitted {
            ctx.posted(neocogi_ui_nodes_contracts::TextAreaEvent::Submitted(crate::TextSubmitted { text: self.buf.clone() }).post_to(&mut self.observer));
        }
        // Only committed value/text changes affect this widget's preferred dimensions.
        crate::Change::from(outcome.changed)
    }

    /// Paints editable content and its caret in stable content-local coordinates.
    fn paint_widget(&self, ctx: &mut WidgetPaintCtx<'_>) {
        // ScrollArea has already applied translation and clipping before this child paint phase, so
        // the editor records no offset arithmetic or scrollbar operations.
        let font = ctx.skin().resolve_font(ctx.atlas(), &self.font);
        textarea_paint(ctx, self, font);
    }
}

/// Converts a text-line index into a saturated content-local y coordinate.
fn text_line_y(index: usize, line_height: i32) -> i32 {
    // Retained documents can contain more rows than i32 and valid fonts can use the complete
    // positive metric range. Clamp both conversion and multiplication at the geometry boundary.
    i32::try_from(index).unwrap_or(i32::MAX).saturating_mul(line_height)
}

/// Complete derived text layout in stable content-local coordinates.
struct TextAreaLayout {
    /// Complete allocation supplied by the containing scroll surface.
    bounds: Recti,
    /// Wrapped editable lines, including a trailing blank caret row.
    lines: Vec<TextLine>,
    /// Font metrics shared by navigation, caret geometry, and paint.
    metrics: FontLineMetrics,
}

/// Builds editable lines for the exact content allocation used by update or paint.
fn textarea_layout(bounds: Recti, atlas: &AtlasHandle, state: &TextEditor, font: FontId) -> TextAreaLayout {
    // Word wrapping uses the allocated content width selected by ScrollArea convergence. Unwrapped
    // content retains intrinsic line widths and is translated horizontally by the parent viewport.
    let max_width = match state.wrap {
        TextWrap::None => i32::MAX / 4,
        TextWrap::Word | TextWrap::WordOrCharacter => bounds.width.max(1),
    };
    let lines = build_text_lines(state.buf.as_str(), state.wrap, max_width, font, atlas);
    TextAreaLayout {
        bounds,
        lines,
        metrics: font_line_metrics(font, atlas),
    }
}

/// Applies multiline editing and requests parent-owned scrolling when the caret moves.
fn textarea_update(ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>, state: &mut TextEditor, font: FontId) -> TextAreaUpdateOutcome {
    // External cursor and text setters schedule reveal without fabricating an input event.
    let mut outcome = TextAreaUpdateOutcome::default();
    let mut ensure_visible = std::mem::take(&mut state.reveal_caret);
    let mut reset_preferred = false;
    let mut vertical_moved = false;
    let mut preferred_x = state.interaction.preferred_x;
    let mut caret_affinity = state.interaction.caret_affinity;
    let mut cursor_pos = clamp_cursor_boundary(&state.buf, state.cursor);

    // Normalize the one routed input into the scalar and navigation components used by the shared
    // text-edit core. Eventless updates still process pending programmatic caret reveal.
    let text_input = match input {
        Some(UiInputEvent::Text { text }) => text.as_str(),
        _ => "",
    };
    let key_event = match input {
        Some(UiInputEvent::Key { event }) => Some(*event),
        _ => None,
    };

    match super::clipboard::advance(
        &mut state.editing,
        state.runtime.state.host(),
        &mut state.buf,
        &mut cursor_pos,
        key_event,
        ReturnBehavior::Newline { submit_on_ctrl: true },
        ctx.focused(),
    ) {
        Ok(true) => {
            outcome.changed = true;
            ensure_visible = true;
            reset_preferred = true;
            caret_affinity = CaretAffinity::Upstream;
        }
        Ok(false) => {}
        // A failed request cannot erase a completed receipt's change result or abort the frame.
        Err(_) => {}
    }
    if ctx.focused() {
        // Apply scalar editing and horizontal cursor movement before building layout because inserted
        // text can immediately change wrapping, line indices, and intrinsic content height.
        let cursor_before_edit = cursor_pos;
        let edit = apply_text_input(
            &mut state.buf,
            cursor_pos,
            &mut state.editing,
            text_input,
            key_event,
            ReturnBehavior::Newline { submit_on_ctrl: true },
        );
        cursor_pos = edit.cursor;
        if edit.changed {
            outcome.changed = true;
            ensure_visible = true;
            reset_preferred = true;
            if edit.cursor != cursor_before_edit {
                // Insertion and backspace produce a new byte position with no retained visual-side
                // meaning. Forward Delete leaves the cursor fixed and therefore preserves its
                // selected continuation side when the wrap boundary survives reflow.
                caret_affinity = CaretAffinity::Upstream;
            }
        }
        if edit.moved {
            ensure_visible = true;
            reset_preferred = true;
            // Horizontal movement can land on the byte shared by two wrapped lines. Left arrives
            // from inside the continuation and must keep that downstream visual side; Right
            // arrives from the preceding line and keeps the upstream side. At unshared positions
            // the affinity is inert but retaining the same rule makes the next layout deterministic.
            caret_affinity = if key_event.is_some_and(|event| event.is_pressed() && event.key == Key::ArrowLeft) {
                CaretAffinity::Downstream
            } else {
                CaretAffinity::Upstream
            };
        }
        outcome.submitted = edit.submit;
    }

    // Resolve line geometry from the updated buffer in content-local coordinates. ScrollArea's
    // translation already localizes pointer positions into this same coordinate system.
    let layout = textarea_layout(ctx.local_rect(), ctx.atlas(), state, font);
    let mut cursor_line = line_index_for_cursor(&layout.lines, cursor_pos, caret_affinity);
    let mut caret_x = cursor_x_in_line(&layout.lines[cursor_line], state.buf.as_str(), cursor_pos, font, ctx.atlas());

    let before_navigation = cursor_pos;
    if ctx.focused() {
        if key_event.is_some_and(|event| event.is_pressed() && event.key == Key::Home) {
            // Home targets the current visual line. On a wrapped continuation its start shares a
            // byte with the previous line's end, so preserve the selected downstream side.
            cursor_pos = layout.lines[cursor_line].start;
            caret_affinity = affinity_for_line(&layout.lines, cursor_line, cursor_pos);
            caret_x = cursor_x_in_line(&layout.lines[cursor_line], state.buf.as_str(), cursor_pos, font, ctx.atlas());
            ensure_visible = true;
            reset_preferred = true;
        } else if key_event.is_some_and(|event| event.is_pressed() && event.key == Key::End) {
            // End targets the current visual line, including wrapped segments.
            cursor_pos = layout.lines[cursor_line].end;
            caret_affinity = affinity_for_line(&layout.lines, cursor_line, cursor_pos);
            caret_x = cursor_x_in_line(&layout.lines[cursor_line], state.buf.as_str(), cursor_pos, font, ctx.atlas());
            ensure_visible = true;
            reset_preferred = true;
        }

        if key_event.is_some_and(|event| event.is_pressed() && event.key == Key::ArrowUp) {
            // Preserve the desired visual x while moving to the nearest scalar boundary above.
            let target_x = preferred_x.unwrap_or(caret_x);
            if cursor_line > 0 {
                cursor_line -= 1;
                cursor_pos = cursor_from_x(&layout.lines[cursor_line], state.buf.as_str(), target_x, font, ctx.atlas());
                caret_affinity = affinity_for_line(&layout.lines, cursor_line, cursor_pos);
            }
            preferred_x = Some(target_x);
            ensure_visible = true;
            vertical_moved = true;
        }

        if key_event.is_some_and(|event| event.is_pressed() && event.key == Key::ArrowDown) {
            // Preserve the desired visual x while moving to the nearest scalar boundary below.
            let target_x = preferred_x.unwrap_or(caret_x);
            if cursor_line + 1 < layout.lines.len() {
                cursor_line += 1;
                cursor_pos = cursor_from_x(&layout.lines[cursor_line], state.buf.as_str(), target_x, font, ctx.atlas());
                caret_affinity = affinity_for_line(&layout.lines, cursor_line, cursor_pos);
            }
            preferred_x = Some(target_x);
            ensure_visible = true;
            vertical_moved = true;
        }
    }

    if ctx.focused() && key_event.is_some_and(|event| event.is_pressed() && matches!(event.key, Key::Home | Key::End | Key::ArrowUp | Key::ArrowDown)) {
        let target = cursor_pos;
        cursor_pos = before_navigation;
        state
            .editing
            .move_to(&state.buf, &mut cursor_pos, target, ctx.modifiers().intersects(Modifiers::SHIFT));
    }
    let pointer = match input {
        Some(UiInputEvent::MouseDown { pos, button }) if button.intersects(MouseButton::LEFT) && ctx.mouse_over(layout.bounds, *pos) => Some((*pos, false)),
        Some(UiInputEvent::MouseDrag { pos, buttons, .. }) if buttons.intersects(MouseButton::LEFT) => Some((*pos, true)),
        _ => None,
    };
    if ctx.focused()
        && let Some((pos, dragging)) = pointer
    {
        // Pointer positions already include the parent's scroll translation. Convert directly from
        // content-local pixels into a visual line and nearest scalar boundary.
        let last_line = i32::try_from(layout.lines.len().saturating_sub(1)).unwrap_or(i32::MAX);
        let line_idx = (pos.y / layout.metrics.line_height.max(1)).clamp(0, last_line) as usize;
        let target = cursor_from_x(&layout.lines[line_idx], state.buf.as_str(), pos.x, font, ctx.atlas());
        state
            .editing
            .move_to(&state.buf, &mut cursor_pos, target, dragging || ctx.modifiers().intersects(Modifiers::SHIFT));
        caret_affinity = affinity_for_line(&layout.lines, line_idx, cursor_pos);
        ensure_visible = true;
        reset_preferred = true;
    }

    // Re-resolve final caret geometry after keyboard and pointer navigation have both completed.
    cursor_pos = clamp_cursor_boundary(&state.buf, cursor_pos);
    cursor_line = line_index_for_cursor(&layout.lines, cursor_pos, caret_affinity);
    caret_x = cursor_x_in_line(&layout.lines[cursor_line], state.buf.as_str(), cursor_pos, font, ctx.atlas());

    if reset_preferred && !vertical_moved {
        // Horizontal edits and direct pointer placement start a fresh preferred visual column.
        preferred_x = None;
    }
    if preferred_x.is_none() {
        // Seed vertical navigation from the final caret x when no earlier column is retained.
        preferred_x = Some(caret_x);
    }

    state.cursor = cursor_pos;
    state.interaction.preferred_x = preferred_x;
    state.interaction.caret_affinity = caret_affinity;
    if ensure_visible {
        // Reveal the complete caret line rather than a single pixel so vertical navigation keeps its
        // baseline and descent inside the parent viewport.
        state.reveal_rect(Recti::new(
            caret_x,
            text_line_y(cursor_line, layout.metrics.line_height),
            1,
            layout.metrics.line_height,
        ));
    }
    outcome
}

/// Outcome flags consumed by the retained text-area event publisher.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
struct TextAreaUpdateOutcome {
    /// Whether user input changed the complete text buffer.
    changed: bool,
    /// Whether user input requested submission without inserting a newline.
    submitted: bool,
}

/// Paints text and the focused caret while ScrollArea supplies translation and clipping.
fn textarea_paint(ctx: &mut WidgetPaintCtx<'_>, state: &TextEditor, font: FontId) {
    // Derive the same wrapped lines used by update, then limit recording to lines intersecting the
    // traversal-owned local clip so large documents do not emit invisible text operations.
    let layout = textarea_layout(ctx.local_rect(), ctx.atlas(), state, font);
    let local_clip = ctx.local_clip();
    let line_height = layout.metrics.line_height.max(1);
    let first_line = (local_clip.y.max(0) / line_height) as usize;
    let visible_bottom = local_clip.y.saturating_add(local_clip.height).max(0);
    let last_line = ((visible_bottom / line_height) as usize + 1).min(layout.lines.len());

    // Fill the translated content surface with the editable base color; Painter clips the large
    // semantic rectangle to the effective ScrollArea viewport.
    ctx.draw_control_center(ControlRole::TextInput, layout.bounds);
    let color = ctx.control_content_color(ControlRole::TextInput);
    let cursor_pos = clamp_cursor_boundary(&state.buf, state.cursor);
    let cursor_line = line_index_for_cursor(&layout.lines, cursor_pos, state.interaction.caret_affinity);
    let caret_x = cursor_x_in_line(&layout.lines[cursor_line], state.buf.as_str(), cursor_pos, font, ctx.atlas());
    let caret = if ctx.focused() {
        // Align the caret with the same baseline metrics as the corresponding text line.
        let line_top = text_line_y(cursor_line, line_height);
        Some(caret_rect(
            caret_x,
            line_top.saturating_add(layout.metrics.baseline),
            layout.metrics,
            layout.bounds,
        ))
    } else {
        None
    };

    ctx.with_clip(layout.bounds, |ctx| {
        // Record only the line interval intersecting the inherited viewport clip.
        for (idx, line) in layout.lines.iter().enumerate().take(last_line).skip(first_line) {
            let text = &state.buf[line.start..line.end];
            let origin = Vec2i::new(0, text_line_y(idx, line_height));
            if !text.is_empty() {
                ctx.draw_text(font, text, origin, color);
            }
            if ctx.focused()
                && let Some(range) = state.editing.selection(&state.buf, cursor_pos)
            {
                // Wrapped boundaries have no source byte; only an actual newline receives a cell.
                let newline = usize::from(state.buf.as_bytes().get(line.end) == Some(&b'\n'));
                let start = range.start.max(line.start);
                let end = range.end.min(line.end + newline);
                if start < end {
                    super::text_edit::paint_selection(ctx, font, text, start - line.start..end - line.start, origin, line_height);
                }
            }
        }

        if let Some(caret) = caret {
            // Painter's inherited clip rejects an offscreen caret without editor-owned scroll math.
            ctx.draw_rect(caret, color);
        }
    });
}

impl Widget for TextEditor {
    /// Applies one routed event to retained text and cursor state.
    fn update(&mut self, ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) -> crate::Change {
        // Keep the trait boundary thin so unit tests and retained traversal share one implementation.
        self.update_widget(ctx, input)
    }

    /// Records text and caret paint operations in content-local coordinates.
    fn paint(&self, ctx: &mut WidgetPaintCtx<'_>) {
        // Keep the trait boundary thin so direct paint tests and traversal share one implementation.
        self.paint_widget(ctx);
    }

    fn keyboard_behavior(&self) -> KeyboardBehavior {
        // The inner editor is the composed control's one Tab stop. Its containing ScrollArea and
        // structural scrollbars remain non-focusable pointer surfaces and therefore preserve this focus.
        KeyboardBehavior::TAB_STOP | KeyboardBehavior::TEXT_INPUT
    }
}

impl LeafWidget for TextEditor {
    /// Measures intrinsic editable text content for the containing scroll surface.
    fn measure(&self, style: &Skin, atlas: &AtlasHandle, constraints: Constraints) -> Dimensioni {
        // Delegate to the phase-independent measurement helper used by composition and tests.
        self.preferred_size_widget(style, atlas, constraints)
    }
}

impl TextEditor {
    /// Constructs the actual editor fields beside one already validated host runtime.
    pub(crate) fn from_runtime(
        runtime: crate::widget_object::WidgetRuntime,
        text: impl Into<String>,
        wrap: TextWrap,
        font: FontRef,
        model: Option<std::rc::Rc<crate::ScrollModel>>,
    ) -> Self {
        let buf = normalize_multiline_text(text);
        let cursor = buf.len();
        Self {
            runtime,
            buf,
            cursor,
            editing: super::editing::Editing::default(),
            reset_preferred_x: false,
            reveal_caret: false,
            wrap,
            font,
            interaction: TextAreaInteraction::default(),
            scroll_area: model,
            observer: None,
        }
    }
    /// Reads the one wrapping choice consumed by both measurement and caret geometry.
    pub(crate) fn com_wrap(&self) -> TextWrap {
        self.wrap
    }
    /// Invalidates visual-column preference and reveals the caret after a wrapping change.
    pub(crate) fn com_set_wrap(&mut self, wrap: TextWrap) {
        self.wrap = wrap;
        self.reset_preferred_x = true;
        self.reveal_caret = true;
    }
}
impl crate::widget_object::WidgetAlgorithm for TextEditor {
    /// Ends ordinary native events on logical COM closure, before retained semantic storage is finally released.
    fn close_events(&mut self) {
        self.observer = None;
    }
}

impl crate::widget_object::HostedWidget for TextEditor {
    /// Releases observers, revokes outstanding clipboard receipts, and drops local history.
    fn close_contents(&mut self) -> Result<(), neocogi_com::NcStatus> {
        crate::widget_object::WidgetAlgorithm::close_events(self);
        self.editing = super::editing::Editing::default();
        Ok(())
    }

    /// Borrows common phase state stored directly beside the editable text fields.
    fn runtime(&self) -> &crate::widget_object::WidgetRuntime {
        &self.runtime
    }

    /// Exclusively borrows common phase state for mutation or resource refresh.
    fn runtime_mut(&mut self) -> &mut crate::widget_object::WidgetRuntime {
        &mut self.runtime
    }
}

impl crate::widget_object::FontState for TextEditor {
    /// Borrows the actual font selection used by text measurement and paint.
    fn font(&self) -> &crate::FontRef {
        &self.font
    }
    /// Recomputes visual navigation and caret reveal after the shared resource helper validates the font.
    fn set_font(&mut self, font: crate::FontRef) {
        self.font = font;
        self.reset_preferred_x = true;
        self.reveal_caret = true;
    }
}

use crate::widget_object::{
    HostedWidget, direct_container_layout_impl, direct_leaf_layout_impl, direct_scrolling_impl, direct_widget_phase_impl, set_direct_font_role,
    set_direct_named_font,
};
use neocogi_ui_core_contracts::{
    GeometryChange, IFontStyle, IMultilineEditor, IMultilineEditorAbi, ITextContent, ITextEditor, IWidgetAbi, IWidgetHostAbi, FontRole, TextWrapping,
};
use neocogi_ui_nodes_contracts::{TextAreaAbi};
use neocogi_com::{ComPtr, INcStringAbi, NcCopyFromBufferOffset, NcStatus, NcString};

direct_widget_phase_impl!(TextEditor);
direct_leaf_layout_impl!(TextEditor);

impl TextEditor {
    /// Constructs the private editor child directly on its semantic allocation.
    fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        text: &str,
        wrap: crate::TextWrap,
        model: std::rc::Rc<crate::ScrollModel>,
        resources: Option<&crate::widget_object::SharedAssets>,
        observer: Option<neocogi_com::ComRef<'_, TextAreaObserverAbi>>,
    ) -> Result<ComPtr<IMultilineEditorAbi>, NcStatus> {
        let runtime = crate::widget_object::WidgetRuntime::for_resources(host, crate::WidgetOption::NONE, resources)?;
        let mut value = Self::from_runtime(runtime, text, wrap, crate::FontRef::Role(crate::FontRole::Body), Some(model));
        // The leaf retains the sole destination before any parent can publish it.
        value.observer = observer.map(|destination| destination.add_ref());
        Ok(Self::from_value(value)
            .query()
            .expect("TextEditor declares its editing, font, event, and leaf capabilities"))
    }
}

impl ITextContent for TextEditor {
    /// Copies normalized text from the only editor buffer into an immutable snapshot.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        Ok(NcString::from_utf8_str(self.text()))
    }

    /// Replaces text and requests caret reveal through the same concrete editor state.
    fn set_text(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        self.set_text(crate::boundary::text(input)?);
        self.runtime_mut().state.record_change(GeometryChange::Changed);
        Ok(())
    }
}

impl ITextEditor for TextEditor {
    /// Reads the scalar-aligned byte offset from the same buffer used by routed input.
    fn cursor(&self) -> Result<u64, NcStatus> {
        self.runtime().state.readable()?;
        Ok(self.cursor() as u64)
    }

    /// Clamps the byte offset and retains caret reveal until ordinary update resolves its geometry.
    fn set_cursor(&mut self, cursor: u64) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        self.set_cursor(usize::try_from(cursor).unwrap_or(usize::MAX));
        self.runtime_mut().state.record_change(GeometryChange::Changed);
        Ok(())
    }
}

impl IMultilineEditor for TextEditor {
    /// Encodes the exact wrapping policy consumed by the multiline layout algorithm.
    fn wrapping(&self) -> Result<TextWrapping, NcStatus> {
        self.runtime().state.readable()?;
        Ok(match self.com_wrap() {
            crate::TextWrap::None => TextWrapping::None,
            crate::TextWrap::Word => TextWrapping::Word,
            crate::TextWrap::WordOrCharacter => TextWrapping::WordOrCharacter,
        })
    }

    /// Validates before changing wrapping and invalidating text and caret geometry.
    fn set_wrapping(&mut self, wrapping: TextWrapping) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let wrap = crate::widget_object::local_wrap(wrapping)?;
        if self.com_wrap() != wrap {
            self.com_set_wrap(wrap);
            self.runtime_mut().state.record_change(GeometryChange::Changed);
        }
        Ok(())
    }
}

impl IFontStyle for TextEditor {
    /// Validates and commits one semantic font role on the actual editor.
    fn set_font_role(&mut self, role: FontRole) -> Result<(), NcStatus> {
        set_direct_font_role(self, role)
    }

    /// Resolves and commits one named font on the actual editor.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_font(self, name)
    }
}

direct_widget_phase_impl!(TextArea);
direct_container_layout_impl!(TextArea);
direct_scrolling_impl!(TextArea);

impl TextArea {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(crate) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: crate::widget_object::SharedAssets,
        text: NcCopyFromBufferOffset<'_, u8>,
        wrapping: TextWrapping,
        scrolling: bool,
        observer: Option<neocogi_com::ComRef<'_, TextAreaObserverAbi>>,
    ) -> Result<ComPtr<TextAreaAbi>, NcStatus> {
        let value = Self::build(host, text, wrapping, scrolling, Some(&resources), observer)?;
        Ok(Self::from_value(value).query().expect("TextArea declares its callable class contract"))
    }

    /// Builds the fixed editor tree with either direct or runtime-shared resources.
    fn build(
        host: ComPtr<IWidgetHostAbi>,
        input: NcCopyFromBufferOffset<'_, u8>,
        wrapping: TextWrapping,
        scrolling: bool,
        resources: Option<&crate::widget_object::SharedAssets>,
        observer: Option<neocogi_com::ComRef<'_, TextAreaObserverAbi>>,
    ) -> Result<Self, NcStatus> {
        let text_value = crate::boundary::text(input)?;
        let wrap = crate::widget_object::local_wrap(wrapping)?;
        let parts = crate::ScrollParts::prepare(host.clone(), scrolling, resources)?;
        let text = TextEditor::create_com(host.clone(), text_value, wrap, parts.model.clone(), resources, observer)?;
        let font = text.query()?;
        parts
            .content
            .insert(
                0,
                neocogi_ui_core_contracts::Mount::widget(text.query::<IWidgetAbi>()?.as_com_ref()).as_message(),
            )?
            .finish()?;
        let runtime = crate::widget_object::WidgetRuntime::for_resources(host, crate::WidgetOption::FRAME, resources)?;
        Ok(Self::from_runtime(runtime, parts.editor, text, font, parts.model))
    }
}

impl neocogi_ui_nodes_contracts::TextArea for TextArea {
    /// Forwards snapshots after checking the outer object's logical lifetime.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        self.text.text()
    }

    /// Forwards normalized replacement; the actual child invalidates the owning subtree.
    fn set_text(&mut self, text: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        self.text.set_text(text)
    }

    /// Reads the actual content editor's cursor without retaining a local copy.
    fn cursor(&self) -> Result<u64, NcStatus> {
        self.runtime().state.readable()?;
        self.text.cursor()
    }

    /// Requests scalar-aligned cursor positioning and caret reveal on the actual child.
    fn set_cursor(&mut self, cursor: u64) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        self.text.set_cursor(cursor)
    }

    /// Returns the actual editor's wrapping policy.
    fn wrapping(&self) -> Result<TextWrapping, NcStatus> {
        self.runtime().state.readable()?;
        self.text.wrapping()
    }

    /// Changes only the policy used by text measurement and input.
    fn set_wrapping(&mut self, wrapping: TextWrapping) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        self.text.set_wrapping(wrapping)
    }

    /// Resolves a semantic font on the actual editor and invalidates its layout cache.
    fn set_font_role(&mut self, role: FontRole) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        self.font.set_font_role(role)
    }

    /// Resolves one stable named font through the actual editor's resource owner.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        self.font.set_named_font(name)
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

    /// Maps both outputs into one ordinary consumer FIFO, preserving their actual post order.
    fn text_dispatcher(text_area: &mut TextEditor) -> impl FnMut(&mut Vec<RecordedEvent>) -> bool + use<> {
        let (events_tx, events) = std::sync::mpsc::channel();
        text_area.observer = Some(
            neocogi_ui::text_area::observer(events_tx, |event| match event {
                neocogi_ui::text_area::Event::Changed(value) => RecordedEvent::Changed(value.text, value.cursor),
                neocogi_ui::text_area::Event::Submitted(value) => RecordedEvent::Submitted(value.text),
            })
            .unwrap(),
        );
        move |values| {
            let before = values.len();
            values.extend(events.try_iter());
            values.len() != before
        }
    }

    /// Runs direct focused editor updates at one explicit content allocation.
    fn update_text_area_in_bounds(text_area: &mut TextEditor, bounds: Recti, input: Vec<UiInputEvent>) {
        // Reproduce held key state across the supplied FIFO events while keeping one stable content
        // allocation and focused interaction snapshot. Supplying bounds lets wrapping regressions
        // exercise the same update path without mounting the surrounding scroll area.
        let atlas = test_atlas();
        let style = test_skin(&atlas);
        let mut modifiers = Modifiers::NONE;
        for event in &input {
            if let UiInputEvent::Key { event } = event {
                modifiers = event.modifiers;
            }
            let mut ctx = WidgetUpdateCtx::new_with_interaction(bounds, bounds, &style, &atlas, true, true, true, false, false, MouseButton::NONE, modifiers);
            text_area.update(&mut ctx, Some(event));
        }
    }

    /// Runs direct focused editor updates at the ordinary non-wrapping test allocation.
    fn update_text_area(text_area: &mut TextEditor, input: Vec<UiInputEvent>) {
        update_text_area_in_bounds(text_area, Recti::new(0, 0, 160, 80), input);
    }

    /// Multiline navigation extends one selection across the newline and restores it through undo.
    #[test]
    fn shift_vertical_and_home_navigation_share_the_edit_selection() {
        let mut editor = TextEditor::new_editor(TextAreaParameters::new("abc\ndef"));
        editor.set_cursor(1);
        update_text_area(
            &mut editor,
            vec![UiInputEvent::Key {
                event: KeyEvent::pressed(Key::ArrowDown, Modifiers::SHIFT),
            }],
        );
        assert_eq!(editor.editing.selection(&editor.buf, editor.cursor), Some(1..5));
        update_text_area(
            &mut editor,
            vec![UiInputEvent::Key {
                event: KeyEvent::pressed(Key::Home, Modifiers::SHIFT),
            }],
        );
        assert_eq!(editor.editing.selection(&editor.buf, editor.cursor), Some(1..4));
        update_text_area(&mut editor, vec![UiInputEvent::Text { text: "X".into() }]);
        assert_eq!(editor.text(), "aXdef");
        update_text_area(
            &mut editor,
            vec![UiInputEvent::Key {
                event: KeyEvent::pressed(Key::Character('z'), Modifiers::CTRL),
            }],
        );
        assert_eq!(editor.text(), "abc\ndef");
        assert_eq!(editor.editing.selection(&editor.buf, editor.cursor), Some(1..4));
        editor.set_text("new value");
        update_text_area(
            &mut editor,
            vec![UiInputEvent::Key {
                event: KeyEvent::pressed(Key::Character('z'), Modifiers::CTRL),
            }],
        );
        assert_eq!(editor.text(), "new value");
    }

    /// Verifies independent changed and submitted snapshots from the editable content leaf.
    #[test]
    fn text_area_dispatches_independent_change_and_submission_events() {
        // Directly exercise the inner leaf because scrolling composition is orthogonal to event
        // payload order and the constructor uses this exact semantic initialization path.
        let mut text_area = TextEditor::new_editor(TextAreaParameters::new(""));
        let mut dispatcher = text_dispatcher(&mut text_area);
        update_text_area(
            &mut text_area,
            vec![
                UiInputEvent::Text { text: "line".into() },
                UiInputEvent::Key {
                    event: KeyEvent::pressed(Key::Control, Modifiers::CTRL),
                },
                UiInputEvent::Key {
                    event: KeyEvent::pressed(Key::Enter, Modifiers::CTRL),
                },
            ],
        );
        let mut events = Vec::new();
        assert!(dispatcher(&mut events));
        assert_eq!(
            events,
            [RecordedEvent::Changed("line".to_owned(), 4), RecordedEvent::Submitted("line".to_owned())]
        );
    }

    /// Verifies construction, replacement, and pasted text share canonical LF storage.
    #[test]
    fn every_multiline_text_ingress_normalizes_line_endings() {
        let mut text_area = TextEditor::new_editor(TextAreaParameters::new("a\r\nb\rc\n"));
        assert_eq!(text_area.text(), "a\nb\nc\n");
        assert_eq!(text_area.cursor(), text_area.text().len());

        text_area.set_text("d\r\ne\rf\n");
        assert_eq!(text_area.text(), "d\ne\nf\n");

        update_text_area(&mut text_area, vec![UiInputEvent::Text { text: "g\r\nh\ri\n".into() }]);
        assert_eq!(text_area.text(), "d\ne\nf\ng\nh\ni\n");
        assert_eq!(text_area.cursor(), text_area.text().len());
    }

    /// Verifies vertical navigation retains the continuation side of a wrapped byte boundary.
    #[test]
    fn wrapped_vertical_navigation_retains_visual_caret_affinity() {
        // With the test font's eight-pixel advances, this width produces ranges 0..2 and 2..4.
        // Byte 2 is therefore both the first line's end and the continuation line's start.
        let mut text_area = TextEditor::new_editor(TextAreaParameters::new("_ __").wrap(TextWrap::Word));
        text_area.set_cursor(0);
        let atlas = test_atlas();
        let style = test_skin(&atlas);
        let bounds = Recti::new(0, 0, 16, 30);
        let down = UiInputEvent::Key {
            event: KeyEvent::pressed(Key::ArrowDown, Modifiers::NONE),
        };
        let mut ctx = WidgetUpdateCtx::new_with_interaction(
            bounds,
            bounds,
            &style,
            &atlas,
            true,
            true,
            true,
            false,
            false,
            MouseButton::NONE,
            Modifiers::NONE,
        );
        text_area.update(&mut ctx, Some(&down));

        let font = style.resolve_font(&atlas, &text_area.font);
        let layout = textarea_layout(bounds, &atlas, &text_area, font);
        assert_eq!(text_area.cursor(), 2);
        assert_eq!(text_area.interaction.caret_affinity, CaretAffinity::Downstream);
        assert_eq!(
            line_index_for_cursor(&layout.lines, text_area.cursor(), text_area.interaction.caret_affinity),
            1
        );

        // Forward Delete does not move the byte cursor. The shorter replacement still wraps at the
        // same boundary, so the caret must remain on the continuation side after reflow.
        let delete = UiInputEvent::Key {
            event: KeyEvent::pressed(Key::Delete, Modifiers::NONE),
        };
        let mut ctx = WidgetUpdateCtx::new_with_interaction(
            bounds,
            bounds,
            &style,
            &atlas,
            true,
            true,
            true,
            false,
            false,
            MouseButton::NONE,
            Modifiers::NONE,
        );
        text_area.update(&mut ctx, Some(&delete));
        assert_eq!(text_area.text(), "_ _");
        assert_eq!(text_area.cursor(), 2);
        assert_eq!(text_area.interaction.caret_affinity, CaretAffinity::Downstream);

        // End must now operate on the selected continuation rather than jumping back to line zero.
        let end = UiInputEvent::Key {
            event: KeyEvent::pressed(Key::End, Modifiers::NONE),
        };
        let mut ctx = WidgetUpdateCtx::new_with_interaction(
            bounds,
            bounds,
            &style,
            &atlas,
            true,
            true,
            true,
            false,
            false,
            MouseButton::NONE,
            Modifiers::NONE,
        );
        text_area.update(&mut ctx, Some(&end));
        assert_eq!(text_area.cursor(), 3);
    }

    /// Verifies horizontal arrows retain the visual side from which they enter a wrap boundary.
    #[test]
    fn wrapped_horizontal_navigation_selects_the_arrival_side_of_a_shared_boundary() {
        // With the test font's eight-pixel advances, this allocation produces ranges 0..2 and
        // 2..4. Byte 2 therefore has distinct upstream and downstream visual positions.
        let bounds = Recti::new(0, 0, 16, 30);
        let parameters = || TextAreaParameters::new("_ __").wrap(TextWrap::Word);

        let mut from_continuation = TextEditor::new_editor(parameters());
        from_continuation.set_cursor(3);
        update_text_area_in_bounds(
            &mut from_continuation,
            bounds,
            vec![UiInputEvent::Key {
                event: KeyEvent::pressed(Key::ArrowLeft, Modifiers::NONE),
            }],
        );
        assert_eq!(from_continuation.cursor(), 2);
        assert_eq!(from_continuation.interaction.caret_affinity, CaretAffinity::Downstream);

        let mut from_first_line = TextEditor::new_editor(parameters());
        from_first_line.set_cursor(1);
        update_text_area_in_bounds(
            &mut from_first_line,
            bounds,
            vec![UiInputEvent::Key {
                event: KeyEvent::pressed(Key::ArrowRight, Modifiers::NONE),
            }],
        );
        assert_eq!(from_first_line.cursor(), 2);
        assert_eq!(from_first_line.interaction.caret_affinity, CaretAffinity::Upstream);

        // Resolve both stored affinities against the actual wrapped layout, proving that equal
        // public byte cursors paint and reveal on the two intended visual lines.
        let atlas = test_atlas();
        let style = test_skin(&atlas);
        let font = style.resolve_font(&atlas, &from_continuation.font);
        let layout = textarea_layout(bounds, &atlas, &from_continuation, font);
        assert_eq!(line_index_for_cursor(&layout.lines, 2, from_continuation.interaction.caret_affinity), 1);
        assert_eq!(line_index_for_cursor(&layout.lines, 2, from_first_line.interaction.caret_affinity), 0);
    }

    /// Verifies Home selects the start of the current wrapped visual line, not the whole buffer.
    #[test]
    fn wrapped_home_selects_the_continuation_start() {
        let bounds = Recti::new(0, 0, 16, 30);
        let mut text_area = TextEditor::new_editor(TextAreaParameters::new("_ __").wrap(TextWrap::Word));
        text_area.set_cursor(3);

        update_text_area_in_bounds(
            &mut text_area,
            bounds,
            vec![UiInputEvent::Key {
                event: KeyEvent::pressed(Key::Home, Modifiers::NONE),
            }],
        );

        assert_eq!(text_area.cursor(), 2);
        assert_eq!(text_area.interaction.caret_affinity, CaretAffinity::Downstream);
    }

    /// Verifies multiline backspace at byte zero cannot delete the following newline.
    #[test]
    fn backspace_at_start_preserves_leading_newline() {
        let mut text_area = TextEditor::new_editor(TextAreaParameters::new("\ntext"));
        text_area.set_cursor(0);
        update_text_area(
            &mut text_area,
            vec![UiInputEvent::Key {
                event: KeyEvent::pressed(Key::Backspace, Modifiers::NONE),
            }],
        );

        assert_eq!(text_area.text(), "\ntext");
        assert_eq!(text_area.cursor(), 0);
    }
}
