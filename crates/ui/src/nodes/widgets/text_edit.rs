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
//! UTF-8-safe text editing primitives shared by textbox and text-area widgets.
//!
//! The helpers in this file keep cursor indices on valid byte boundaries, apply keyboard/text
//! input, and translate pointer positions into cursor locations. Cursor movement and deletion use
//! Unicode scalar-value boundaries, not grapheme-cluster boundaries.
use std::borrow::Cow;

use crate::nodes::math::clamp_i64_to_i32;
use crate::nodes::text_layout::TextLine;
use crate::nodes::{rect, AtlasHandle, FontId, Key, KeyEvent, Modifiers, Recti};

/// Determines what pressing return means for the active editor.
#[derive(Copy, Clone)]
pub(crate) enum ReturnBehavior {
    /// Return submits the edit.
    Submit,
    /// Return inserts a newline, optionally submitting when Ctrl is held.
    Newline {
        /// Whether Ctrl+Return should submit instead of inserting a newline.
        submit_on_ctrl: bool,
    },
}

/// Selects one visual side of a byte position shared by two wrapped lines.
///
/// Word wrapping does not insert a byte between adjacent visual lines: the previous line's `end`
/// is exactly the continuation line's `start`. Retaining this concrete affinity lets vertical and
/// pointer navigation keep the caret on the selected visual line without changing the public byte
/// cursor representation.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum CaretAffinity {
    /// Prefer the visual line ending at the shared byte position.
    #[default]
    Upstream,
    /// Prefer the continuation line starting at the shared byte position.
    Downstream,
}

/// Result of applying one input snapshot to a text buffer.
pub(crate) struct TextEditOutcome {
    /// New cursor byte position.
    pub cursor: usize,
    /// Whether the text buffer changed.
    pub changed: bool,
    /// Whether the cursor moved.
    pub moved: bool,
    /// Whether the edit requested submission.
    pub submit: bool,
}

#[derive(Copy, Clone)]
/// Metrics needed to align text and draw a caret consistently.
pub(crate) struct FontLineMetrics {
    /// Distance between baselines in pixels.
    pub line_height: i32,
    /// Baseline offset from the top of the line.
    pub baseline: i32,
    /// Descent below the baseline.
    pub descent: i32,
}

/// Converts multiline text to the editor's sole stored newline representation.
///
/// CRLF becomes one LF and a lone CR becomes one LF. The original allocation is returned unchanged
/// when it already contains only LF line endings, keeping the common path allocation-free beyond
/// the caller-owned `String` itself.
pub(crate) fn normalize_multiline_text(text: impl Into<String>) -> String {
    let text = text.into();
    if !text.contains('\r') {
        return text;
    }

    let mut normalized = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' {
            // Consume the LF half of CRLF so one logical line ending occupies one stored byte.
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            normalized.push('\n');
        } else {
            normalized.push(ch);
        }
    }
    normalized
}

/// Removes line endings from text entering a single-line editor.
///
/// Both halves of CRLF are discarded rather than replaced with presentation-only whitespace. The
/// resulting stored value therefore always satisfies the textbox's one-line measurement contract.
pub(crate) fn normalize_single_line_text(text: impl Into<String>) -> String {
    let mut text = text.into();
    text.retain(|ch| ch != '\r' && ch != '\n');
    text
}

/// Borrows ordinary text input directly and allocates only when its line endings need rewriting.
fn normalize_input(text: &str, behavior: ReturnBehavior) -> Cow<'_, str> {
    match behavior {
        ReturnBehavior::Submit if text.contains('\r') || text.contains('\n') => Cow::Owned(normalize_single_line_text(text)),
        ReturnBehavior::Newline { .. } if text.contains('\r') => Cow::Owned(normalize_multiline_text(text)),
        ReturnBehavior::Submit | ReturnBehavior::Newline { .. } => Cow::Borrowed(text),
    }
}

/// Reads line-height, baseline, and descent from the atlas.
pub(crate) fn font_line_metrics(font: FontId, atlas: &AtlasHandle) -> FontLineMetrics {
    let line_height = atlas.get_font_height(font) as i32;
    let baseline = atlas.get_font_baseline(font);
    let descent = line_height.saturating_sub(baseline).max(0);
    FontLineMetrics { line_height, baseline, descent }
}

/// Centers a single text line inside bounds while keeping it fully clipped to the bounds.
pub(crate) fn centered_line_top(bounds: Recti, line_height: i32) -> i32 {
    let bounds_y = i64::from(bounds.y);
    let candidate = bounds_y + i64::from(bounds.height) / 2 - i64::from(line_height) / 2;
    let max_text_y = (bounds_y + i64::from(bounds.height) - i64::from(line_height)).max(bounds_y);
    // Clamp the complete alignment expression in i64 so a large font and extreme rectangle origin
    // cannot overflow before the final retained coordinate is produced.
    clamp_i64_to_i32(candidate.clamp(bounds_y, max_text_y))
}

/// Builds a one-pixel caret rectangle clipped to the visible text area.
pub(crate) fn caret_rect(x: i32, baseline_y: i32, metrics: FontLineMetrics, clip: Recti) -> Recti {
    let clip_top = i64::from(clip.y);
    let clip_bottom = clip_top + i64::from(clip.height.max(0));
    let caret_top = (i64::from(baseline_y) - i64::from(metrics.baseline) + 2).clamp(clip_top, clip_bottom);
    let caret_bottom = (i64::from(baseline_y) + i64::from(metrics.descent) - 2).clamp(clip_top, clip_bottom);
    let height = (caret_bottom - caret_top).clamp(1, i64::from(i32::MAX)) as i32;
    rect(x, clamp_i64_to_i32(caret_top), 1, height)
}

/// Clamps a byte cursor to the nearest previous Unicode scalar-value boundary.
pub(crate) fn clamp_cursor_boundary(buf: &str, cursor: usize) -> usize {
    let mut cursor = cursor.min(buf.len());
    while cursor > 0 && !buf.is_char_boundary(cursor) {
        cursor -= 1;
    }
    cursor
}

/// Inserts text at a valid cursor boundary and advances the cursor past the inserted text.
fn insert_text(buf: &mut String, cursor: &mut usize, text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    let insert_at = clamp_cursor_boundary(buf, *cursor);
    buf.insert_str(insert_at, text);
    *cursor = insert_at + text.len();
    true
}

/// Deletes the UTF-8 scalar value immediately before the cursor.
fn delete_prev(buf: &mut String, cursor: &mut usize) -> bool {
    if buf.is_empty() {
        return false;
    }
    *cursor = clamp_cursor_boundary(buf, *cursor);
    if *cursor == 0 {
        // Backspace never deletes text at or after the cursor. Delete owns the forward direction.
        return false;
    }
    let mut start = *cursor;
    start -= 1;
    // Walk back to the start byte of the previous UTF-8 scalar.
    while start > 0 && !buf.is_char_boundary(start) {
        start -= 1;
    }
    buf.replace_range(start..*cursor, "");
    *cursor = start;
    true
}

/// Deletes the next UTF-8 scalar value after `cursor`.
fn delete_next(buf: &mut String, cursor: usize) -> bool {
    let cursor = clamp_cursor_boundary(buf, cursor);
    if buf.is_empty() || cursor >= buf.len() {
        return false;
    }
    let mut end = cursor + 1;
    // Walk forward to the first boundary after the deleted scalar.
    while end < buf.len() && !buf.is_char_boundary(end) {
        end += 1;
    }
    buf.replace_range(cursor..end, "");
    true
}

/// Moves the cursor one UTF-8 scalar value to the left.
fn move_left(buf: &str, cursor: usize) -> usize {
    let cursor = clamp_cursor_boundary(buf, cursor);
    if cursor == 0 {
        return 0;
    }
    let mut new_cursor = cursor - 1;
    while new_cursor > 0 && !buf.is_char_boundary(new_cursor) {
        new_cursor -= 1;
    }
    new_cursor
}

/// Moves the cursor one UTF-8 scalar value to the right.
fn move_right(buf: &str, cursor: usize) -> usize {
    let cursor = clamp_cursor_boundary(buf, cursor);
    if cursor >= buf.len() {
        return buf.len();
    }
    let mut new_cursor = cursor + 1;
    while new_cursor < buf.len() && !buf.is_char_boundary(new_cursor) {
        new_cursor += 1;
    }
    new_cursor
}

/// Applies text, editing keys, cursor keys, and return behavior to a UTF-8 buffer.
pub(crate) fn apply_text_input(
    buf: &mut String,
    cursor: usize,
    editing: &mut super::editing::Editing,
    text_input: &str,
    key_event: Option<KeyEvent>,
    return_behavior: ReturnBehavior,
) -> TextEditOutcome {
    let mut cursor_pos = clamp_cursor_boundary(buf, cursor);
    let before_selection = editing.selection(buf, cursor_pos);
    let pressed = key_event.filter(|event| event.is_pressed());
    let command = pressed.filter(|event| event.modifiers.intersects(Modifiers::CTRL | Modifiers::SUPER) && !event.modifiers.intersects(Modifiers::ALT));
    let mut changed = false;
    let mut submit = false;
    if let Some(event) = command {
        match event.key {
            Key::Character('a' | 'A') => editing.select_all(buf, &mut cursor_pos),
            Key::Character('z' | 'Z') => changed = editing.restore(buf, &mut cursor_pos, event.modifiers.intersects(Modifiers::SHIFT)),
            Key::Character('y' | 'Y') => changed = editing.restore(buf, &mut cursor_pos, true),
            _ => {}
        }
        if matches!(event.key, Key::Character('a' | 'A' | 'z' | 'Z' | 'y' | 'Y')) {
            return TextEditOutcome {
                cursor: cursor_pos,
                changed,
                moved: true,
                submit: false,
            };
        }
    }

    // Copy history only for a potentially mutating transition, never a caret move or idle update.
    let text_input = normalize_input(text_input, return_behavior);
    let mutating = !text_input.is_empty() || pressed.is_some_and(|event| matches!(event.key, Key::Backspace | Key::Delete | Key::Enter));
    let previous = if mutating { editing.snapshot(buf, cursor_pos) } else { None };
    if !text_input.is_empty() {
        editing.delete_selection(buf, &mut cursor_pos);
        changed = insert_text(buf, &mut cursor_pos, &text_input);
    }
    if let Some(event) = pressed {
        let extend = event.modifiers.intersects(Modifiers::SHIFT);
        match event.key {
            Key::Backspace => changed |= editing.delete_selection(buf, &mut cursor_pos) || delete_prev(buf, &mut cursor_pos),
            Key::Delete => changed |= editing.delete_selection(buf, &mut cursor_pos) || delete_next(buf, cursor_pos),
            Key::ArrowLeft => {
                let target = match (extend, editing.selection(buf, cursor_pos)) {
                    (false, Some(range)) => range.start,
                    _ => move_left(buf, cursor_pos),
                };
                editing.move_to(buf, &mut cursor_pos, target, extend);
            }
            Key::ArrowRight => {
                let target = match (extend, editing.selection(buf, cursor_pos)) {
                    (false, Some(range)) => range.end,
                    _ => move_right(buf, cursor_pos),
                };
                editing.move_to(buf, &mut cursor_pos, target, extend);
            }
            Key::Enter => match return_behavior {
                ReturnBehavior::Submit => submit = true,
                ReturnBehavior::Newline { submit_on_ctrl } if submit_on_ctrl && event.modifiers.intersects(Modifiers::CTRL) => submit = true,
                ReturnBehavior::Newline { .. } => {
                    editing.delete_selection(buf, &mut cursor_pos);
                    changed |= insert_text(buf, &mut cursor_pos, "\n");
                }
            },
            _ => {}
        }
    }
    if changed {
        editing.record(previous);
    }
    TextEditOutcome {
        cursor: cursor_pos,
        changed,
        moved: cursor_pos != cursor || before_selection != editing.selection(buf, cursor_pos),
        submit,
    }
}

/// Finds the display line containing `cursor`, preserving a wrapped-boundary preference.
pub(crate) fn line_index_for_cursor(lines: &[TextLine], cursor: usize, affinity: CaretAffinity) -> usize {
    for (idx, line) in lines.iter().enumerate() {
        if cursor < line.end {
            return idx;
        }
        if cursor == line.end {
            // Only wrapped neighbors share a boundary. Logical newline-separated lines leave the
            // delimiter byte between their ranges and therefore remain unambiguous.
            if affinity == CaretAffinity::Downstream && lines.get(idx + 1).is_some_and(|next| next.start == cursor) {
                return idx + 1;
            }
            return idx;
        }
    }
    // last_line_index = line_count - 1, remaining at zero for an empty list.
    lines.len().saturating_sub(1)
}

/// Chooses the affinity that resolves `cursor` back to `selected_line`.
pub(crate) fn affinity_for_line(lines: &[TextLine], selected_line: usize, cursor: usize) -> CaretAffinity {
    if selected_line > 0
        && lines.get(selected_line).is_some_and(|line| line.start == cursor)
        && lines.get(selected_line - 1).is_some_and(|line| line.end == cursor)
    {
        CaretAffinity::Downstream
    } else {
        CaretAffinity::Upstream
    }
}

/// Returns the x offset of `cursor` measured from the start of `line`.
pub(crate) fn cursor_x_in_line(line: &TextLine, buf: &str, cursor: usize, font: FontId, atlas: &AtlasHandle) -> i32 {
    let end = cursor.min(line.end).max(line.start);
    if end <= line.start {
        0
    } else {
        atlas.get_text_size(font, &buf[line.start..end]).width
    }
}

/// Converts a target x coordinate inside one line into the nearest UTF-8 cursor position.
pub(crate) fn cursor_from_x(line: &TextLine, buf: &str, target_x: i32, font: FontId, atlas: &AtlasHandle) -> usize {
    if target_x <= 0 {
        return line.start;
    }
    line.start + atlas.text_cursor_at_x(font, &buf[line.start..line.end], target_x)
}

/// Converts a target x coordinate in single-line text into the nearest UTF-8 cursor position.
pub(crate) fn cursor_from_text_x(buf: &str, target_x: i32, font: FontId, atlas: &AtlasHandle) -> usize {
    atlas.text_cursor_at_x(font, buf, target_x)
}

/// Paints a selected source span using the existing focused-item appearance and text contrast.
pub(crate) fn paint_selection(
    ctx: &mut crate::nodes::WidgetPaintCtx<'_>,
    font: FontId,
    text: &str,
    range: std::ops::Range<usize>,
    origin: crate::nodes::Vec2i,
    height: i32,
) {
    let start = ctx.atlas().get_text_size(font, &text[..range.start]).width;
    let mut end = ctx.atlas().get_text_size(font, &text[..range.end.min(text.len())]).width;
    if range.end > text.len() {
        // An explicitly selected newline occupies one visible space, including on empty lines.
        end = end.saturating_add(ctx.atlas().get_text_size(font, " ").width.max(1));
    }
    let bounds = crate::nodes::Recti::new(origin.x.saturating_add(start), origin.y, end.saturating_sub(start), height);
    let state = crate::nodes::ControlState::Focused(crate::nodes::PointerState::Normal);
    let color = ctx.skin().control(crate::nodes::ControlRole::Item, state).content_color;
    ctx.with_clip(bounds, |ctx| {
        // Repaint the same measured line origin under the selection clip so glyph positions do not shift.
        ctx.draw_control_center_state(crate::nodes::ControlRole::Item, state, bounds);
        ctx.draw_text(font, text, origin, color);
    });
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    //! Tests for UTF-8 safe text editing primitives.

    use super::*;
    use crate::nodes::{Key, KeyEvent, Modifiers};

    /// Display-line hits keep the original source offset; single-line hits keep UTF-8 ties.
    #[test]
    fn cursor_hits_preserve_line_offsets_and_unicode_midpoints() {
        let atlas = neocogi_ui::support::test_support::test_atlas();
        let font = atlas.font_id("body").unwrap();
        let text = "éaΩb☃";
        let line = TextLine { start: 2, end: 6, width: 24 };
        for (target, expected) in [(-1, 0), (0, 0), (3, 0), (4, 1), (8, 1), (12, 3), (16, 3), (20, 4), (24, 4), (i32::MAX, 4)] {
            assert_eq!(cursor_from_text_x("aΩb", target, font, &atlas), expected);
            assert_eq!(cursor_from_x(&line, text, target, font, &atlas), 2 + expected);
        }
        let empty = TextLine { start: 2, end: 2, width: 0 };
        assert_eq!(cursor_from_x(&empty, text, 1, font, &atlas), 2);
    }

    /// Matched CPU-only workloads for cursor cost and the ordinary glyph-walk consumers.
    ///
    /// Run in release mode with `--ignored --nocapture`; timings are evidence, not assertions.
    /// Fixtures and allocation stay outside timing for cursor/measurement/drawing. Wrapping
    /// intentionally includes its normal output allocation. Each row reports median ns/call
    /// from seven batches, after warmup, with results consumed to prevent dead-code removal.
    #[test]
    #[ignore = "manual matched performance measurement"]
    fn cursor_hit_test_measurement() {
        use std::{hint::black_box, time::Instant};

        fn median_ns(mut operation: impl FnMut(), iterations: u32) -> u128 {
            for _ in 0..iterations {
                operation();
            }
            let mut samples = [0; 7];
            for sample in &mut samples {
                let start = Instant::now();
                for _ in 0..iterations {
                    operation();
                }
                *sample = start.elapsed().as_nanos() / u128::from(iterations);
            }
            samples.sort_unstable();
            samples[3]
        }

        let atlas = neocogi_ui::support::test_support::test_atlas();
        let font = atlas.font_id("body").unwrap();
        for length in [64, 512, 2048] {
            let text: String = "aΩb ".chars().cycle().take(length).collect();
            let line = TextLine {
                start: 0,
                end: text.len(),
                width: atlas.get_text_size(font, &text).width,
            };
            assert_eq!(cursor_from_text_x(&text, i32::MAX, font, &atlas), text.len());
            assert_eq!(cursor_from_x(&line, &text, i32::MAX, font, &atlas), text.len());
            let end_text = median_ns(
                || {
                    black_box(cursor_from_text_x(black_box(&text), i32::MAX, font, &atlas));
                },
                16,
            );
            let end_line = median_ns(
                || {
                    black_box(cursor_from_x(&line, black_box(&text), i32::MAX, font, &atlas));
                },
                16,
            );
            let leading = median_ns(
                || {
                    black_box(cursor_from_text_x(black_box(&text), 1, font, &atlas));
                },
                4096,
            );
            let measure = median_ns(
                || {
                    black_box(atlas.get_text_size(font, black_box(&text)));
                },
                256,
            );
            let draw = median_ns(
                || {
                    atlas.draw_string(font, black_box(&text), crate::nodes::Vec2i::default(), |ch, advance, destination, source| {
                        black_box((ch, advance, destination, source));
                    });
                },
                256,
            );
            let wrap = median_ns(
                || {
                    black_box(crate::nodes::text_layout::build_text_lines(
                        black_box(&text),
                        crate::nodes::TextWrap::WordOrCharacter,
                        160,
                        font,
                        &atlas,
                    ));
                },
                256,
            );
            println!(
                "scalars={length} bytes={} end_text={end_text} end_line={end_line} leading={leading} measure={measure} draw={draw} wrap={wrap} ns/call",
                text.len()
            );
        }
    }

    #[test]
    fn text_input_clamps_external_cursor_to_utf8_boundary() {
        let mut buf = String::from("éa");
        let outcome = apply_text_input(&mut buf, 1, &mut super::super::editing::Editing::default(), "x", None, ReturnBehavior::Submit);

        assert_eq!(buf, "xéa");
        assert_eq!(outcome.cursor, 1);
        assert!(outcome.changed);
    }

    #[test]
    fn delete_next_clamps_external_cursor_to_utf8_boundary() {
        let mut buf = String::from("éa");
        let outcome = apply_text_input(
            &mut buf,
            1,
            &mut super::super::editing::Editing::default(),
            "",
            Some(KeyEvent::pressed(Key::Delete, Modifiers::NONE)),
            ReturnBehavior::Submit,
        );

        assert_eq!(buf, "a");
        assert_eq!(outcome.cursor, 0);
        assert!(outcome.changed);
    }

    /// Verifies platform and legacy line endings collapse to the one multiline representation.
    #[test]
    fn multiline_normalization_collapses_crlf_and_lone_cr() {
        assert_eq!(normalize_multiline_text("a\r\nb\rc\n"), "a\nb\nc\n");
    }

    /// Verifies single-line storage cannot retain either newline scalar.
    #[test]
    fn single_line_normalization_removes_all_line_endings() {
        assert_eq!(normalize_single_line_text("a\r\nb\rc\n"), "abc");
    }

    /// Verifies pasted text follows the same newline policy as constructors and setters.
    #[test]
    fn text_input_normalizes_for_each_editor_kind() {
        let mut single = String::new();
        let single_outcome = apply_text_input(
            &mut single,
            0,
            &mut super::super::editing::Editing::default(),
            "a\r\nb\rc\n",
            None,
            ReturnBehavior::Submit,
        );
        assert_eq!(single, "abc");
        assert_eq!(single_outcome.cursor, 3);

        let mut multiline = String::new();
        let multiline_outcome = apply_text_input(
            &mut multiline,
            0,
            &mut super::super::editing::Editing::default(),
            "a\r\nb\rc\n",
            None,
            ReturnBehavior::Newline { submit_on_ctrl: true },
        );
        assert_eq!(multiline, "a\nb\nc\n");
        assert_eq!(multiline_outcome.cursor, multiline.len());
    }

    /// Verifies backspace at the beginning never removes content after the cursor.
    #[test]
    fn backspace_at_zero_does_not_delete_a_leading_newline() {
        let mut buf = String::from("\ntext");
        let outcome = apply_text_input(
            &mut buf,
            0,
            &mut super::super::editing::Editing::default(),
            "",
            Some(KeyEvent::pressed(Key::Backspace, Modifiers::NONE)),
            ReturnBehavior::Newline { submit_on_ctrl: true },
        );

        assert_eq!(buf, "\ntext");
        assert_eq!(outcome.cursor, 0);
        assert!(!outcome.changed);
    }

    /// Verifies one shared byte position can resolve to either wrapped visual line.
    #[test]
    fn wrapped_boundary_resolution_respects_caret_affinity() {
        let lines = [TextLine { start: 0, end: 2, width: 16 }, TextLine { start: 2, end: 3, width: 8 }];

        assert_eq!(line_index_for_cursor(&lines, 2, CaretAffinity::Upstream), 0);
        assert_eq!(line_index_for_cursor(&lines, 2, CaretAffinity::Downstream), 1);
        assert_eq!(affinity_for_line(&lines, 0, 2), CaretAffinity::Upstream);
        assert_eq!(affinity_for_line(&lines, 1, 2), CaretAffinity::Downstream);
    }
}
