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
//! Shared text measurement, wrapping, and control alignment helpers.
//!
//! Text widgets and simple display widgets use these routines to keep UTF-8 line slicing,
//! baseline alignment, and control text placement consistent. Measurement follows
//! [`AtlasHandle::get_text_size`], including its missing-character fallback.
use crate::atlas::TextRun;
use crate::math::clamp_i64_to_i32;
use crate::{vec2, AtlasHandle, Dimensioni, FontId, FontRef, Recti, Skin, Vec2i, WidgetOption};

/// Controls how text should wrap when rendered inside a container.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TextWrap {
    /// Render text on a single line without wrapping.
    None,
    /// Wrap text at ASCII space boundaries when it exceeds the cell width.
    ///
    /// An individual word is never split, even when it is wider than the available width.
    Word,
    /// Prefer ASCII space boundaries, splitting oversized words at UTF-8 character boundaries.
    ///
    /// A single glyph remains indivisible even when its advance exceeds the available width.
    /// Like the rest of the text pipeline, this does not perform grapheme segmentation.
    WordOrCharacter,
}

#[derive(Clone, Copy)]
/// Byte range and measured width for one display line.
pub struct TextLine {
    /// Start byte offset in the source text.
    pub start: usize,
    /// Exclusive end byte offset in the source text.
    pub end: usize,
    /// Atlas-measured width of the exact source slice, clamped to drawable pixels.
    pub width: i32,
}

/// Pushes one source line into `lines`, splitting it on word boundaries when wrapping is enabled.
fn push_wrapped_line(
    lines: &mut Vec<TextLine>,
    buf: &str,
    line_range: core::ops::Range<usize>,
    wrap: TextWrap,
    max_width: i32,
    font: FontId,
    atlas: &AtlasHandle,
) {
    // One range keeps the UTF-8 source interval together instead of passing independent endpoints.
    let line_start = line_range.start;
    let line_end = line_range.end;
    let line = &buf[line_start..line_end];
    if line.is_empty() {
        // Preserve blank lines as zero-width ranges so cursor and display math keep line count.
        lines.push(TextLine {
            start: line_start,
            end: line_start,
            width: 0,
        });
        return;
    }

    if wrap == TextWrap::None || max_width <= 0 {
        let width = atlas.get_text_size(font, line).width;
        lines.push(TextLine { start: line_start, end: line_end, width });
        return;
    }

    let mut offset = 0;
    let mut seg_start = 0;
    let mut run = TextRun::default();
    for word in line.split_inclusive(' ') {
        let word_len = word.len();
        let mut candidate = run;
        atlas.extend_text_run(font, word, &mut candidate);
        if offset > seg_start && !candidate.fits(max_width) {
            // Trial only the new token with the real pen. Summing isolated token ink widths
            // would count overhang as spacing, causing both false breaks and wrong line widths.
            lines.push(TextLine {
                start: line_start + seg_start,
                end: line_start + offset,
                width: run.width(),
            });
            seg_start = offset;
            run = TextRun::default();
            candidate = run;
            atlas.extend_text_run(font, word, &mut candidate);
        }
        if wrap == TextWrap::WordOrCharacter && !candidate.fits(max_width) {
            // Whole-word trial happens first. Oversized words use the identical accumulator
            // one scalar at a time. Each glyph is visited a bounded number of times, so layout
            // remains linear rather than repeatedly measuring growing prefixes.
            for (byte, character) in word.char_indices() {
                let scalar = &word[byte..byte + character.len_utf8()];
                let position = offset + byte;
                let mut next = run;
                atlas.extend_text_run(font, scalar, &mut next);
                if position > seg_start && !next.fits(max_width) {
                    lines.push(TextLine {
                        start: line_start + seg_start,
                        end: line_start + position,
                        width: run.width(),
                    });
                    seg_start = position;
                    next = TextRun::default();
                    atlas.extend_text_run(font, scalar, &mut next);
                }
                // Retain at least one complete scalar even when it alone cannot fit.
                run = next;
            }
        } else {
            run = candidate;
        }
        offset += word_len;
    }

    lines.push(TextLine {
        start: line_start + seg_start,
        end: line_start + line.len(),
        width: run.width(),
    });
}

/// Builds logical text lines, preserving the trailing blank line after a final newline.
pub fn build_text_lines(buf: &str, wrap: TextWrap, max_width: i32, font: FontId, atlas: &AtlasHandle) -> Vec<TextLine> {
    let mut lines = Vec::new();
    if buf.is_empty() {
        lines.push(TextLine { start: 0, end: 0, width: 0 });
        return lines;
    }

    let mut line_start = 0;
    for (idx, ch) in buf.char_indices() {
        if ch == '\n' {
            // `idx` is a UTF-8 boundary from `char_indices`, so slicing remains valid.
            push_wrapped_line(&mut lines, buf, line_start..idx, wrap, max_width, font, atlas);
            line_start = idx + ch.len_utf8();
        }
    }

    if line_start <= buf.len() {
        push_wrapped_line(&mut lines, buf, line_start..buf.len(), wrap, max_width, font, atlas);
    }

    if lines.is_empty() {
        lines.push(TextLine { start: 0, end: 0, width: 0 });
    }
    lines
}

/// Builds display lines for read-only text, suppressing the extra visual row after a final newline.
pub fn build_display_text_lines(buf: &str, wrap: TextWrap, max_width: i32, font: FontId, atlas: &AtlasHandle) -> Vec<TextLine> {
    let mut lines = build_text_lines(buf, wrap, max_width, font, atlas);
    if buf.ends_with('\n') && lines.last().is_some_and(|last| last.start == buf.len() && last.end == buf.len()) {
        lines.pop();
    }
    lines
}

/// Returns the y coordinate that aligns a font baseline inside a control rectangle.
pub fn baseline_aligned_top(rect: Recti, line_height: i32, baseline: i32) -> i32 {
    let rect_y = i64::from(rect.y);
    let rect_height = i64::from(rect.height);
    let line_height = i64::from(line_height);
    if rect_height >= line_height {
        return clamp_i64_to_i32(rect_y + (rect_height - line_height) / 2);
    }

    let baseline_center = rect_y + rect_height / 2;
    let min_top = rect_y + rect_height - line_height;
    let max_top = rect_y;
    // text_top = clamp(centered_baseline - font_baseline, minimum_top, maximum_top).
    clamp_i64_to_i32((baseline_center - i64::from(baseline)).clamp(min_top, max_top))
}

/// Computes the bounding size for a block of measured lines.
pub fn text_block_size(lines: &[TextLine], line_height: i32) -> Dimensioni {
    let width = lines.iter().map(|line| line.width).max().unwrap_or(0).max(0);
    // block_height = line_height * max(line_count, 1).
    let line_count = i32::try_from(lines.len()).unwrap_or(i32::MAX).max(1);
    let height = line_height.saturating_mul(line_count).max(0);
    Dimensioni::new(width, height)
}

/// Computes the top-left text position for a single-line control.
pub fn control_text_position_with_font(style: &Skin, atlas: &AtlasHandle, font: FontId, text: &str, rect: Recti, opt: WidgetOption) -> Vec2i {
    let tsize = atlas.get_text_size(font, text);
    let padding = style.metrics().padding;
    let line_height = atlas.get_font_height(font) as i32;
    let baseline = atlas.get_font_baseline(font);
    let y = baseline_aligned_top(rect, line_height, baseline);
    let x = if opt.intersects(WidgetOption::ALIGN_CENTER) {
        i64::from(rect.x) + (i64::from(rect.width) - i64::from(tsize.width)) / 2
    } else if opt.intersects(WidgetOption::ALIGN_RIGHT) {
        i64::from(rect.x) + i64::from(rect.width) - i64::from(tsize.width) - i64::from(padding)
    } else {
        i64::from(rect.x) + i64::from(padding)
    };
    vec2(clamp_i64_to_i32(x), y)
}

/// Measures text with the widget's resolved font choice.
pub fn text_size(style: &Skin, atlas: &AtlasHandle, font: &crate::FontRef, text: &str) -> Dimensioni {
    // Resolve the stable reference only while the active atlas is available for measurement.
    atlas.get_text_size(style.resolve_font(atlas, font), text)
}

/// Computes a control height that can fit both text and an optional visual element.
pub fn content_height(style: &Skin, atlas: &AtlasHandle, font: &FontRef, visual_height: i32) -> i32 {
    // Fit whichever visual is taller, then apply the theme's compact vertical breathing room.
    let font_height = atlas.get_font_height(style.resolve_font(atlas, font)) as i32;
    let vertical_pad = (style.metrics().padding / 2).max(1);
    font_height.max(visual_height).max(0).saturating_add(vertical_pad.saturating_mul(2))
}

/// Computes preferred size for a single-line label plus an optional icon or texture.
pub fn inline_content_size(style: &Skin, atlas: &AtlasHandle, font: &FontRef, label: &str, visual_size: Option<Dimensioni>) -> Dimensioni {
    let padding = style.metrics().padding.max(0);
    let text_size = if label.is_empty() {
        Dimensioni::default()
    } else {
        text_size(style, atlas, font, label)
    };
    let visual_size = visual_size.unwrap_or_default();
    let has_text = !label.is_empty();
    let has_visual = visual_size.width > 0 && visual_size.height > 0;

    // Text gets horizontal padding on both sides; visuals add one extra text/visual gap.
    let mut width = padding.saturating_mul(2).saturating_add(text_size.width.max(0));
    if has_visual {
        width = width.saturating_add(visual_size.width.max(0));
        if has_text {
            width = width.saturating_add(padding);
        }
    }

    let height = content_height(style, atlas, font, visual_size.height);
    Dimensioni::new(width.max(0), height)
}

#[cfg(test)]
mod tests {
    //! Boundary tests for shared text measurement and wrapping arithmetic.

    use super::*;

    /// Builds canonical signed/overhanging metrics without changing glyph source bounds.
    fn metric_atlas(metrics: impl Fn(char) -> (i32, i32)) -> AtlasHandle {
        let mut data = crate::test_support::test_asset_data_with_font_sizes(&[("body", 16)]);
        for glyph in data.tables.glyphs.to_mut() {
            let (offset, advance) = metrics(char::from_u32(glyph.scalar).unwrap());
            glyph.offset_x = offset;
            glyph.advance_x = advance;
        }
        AtlasHandle::from_validated_data(data, None).unwrap()
    }

    /// Both word policies must concatenate advances rather than the isolated token ink widths.
    #[test]
    fn interword_overhang_fits_and_reports_the_exact_slice_width() {
        let atlas = metric_atlas(|_| (2, 4));
        let font = atlas.font_id("body").unwrap();
        assert_eq!(atlas.get_text_size(font, "a a").width, 14);
        for wrap in [TextWrap::Word, TextWrap::WordOrCharacter] {
            for width in [14, 16] {
                let lines = build_text_lines("a a", wrap, width, font, &atlas);
                assert_eq!(lines.len(), 1);
                assert_eq!((lines[0].start, lines[0].end, lines[0].width), (0, 3, 14));
            }
            let lines = build_text_lines("a a", wrap, 13, font, &atlas);
            assert_eq!(lines.iter().map(|l| (l.start, l.end, l.width)).collect::<Vec<_>>(), [(0, 2, 10), (2, 3, 6)]);
        }
    }

    /// Signed advances, source overhang and coordinate cancellation follow atlas measurement.
    #[test]
    fn every_wrapped_slice_matches_atlas_measurement_for_signed_metrics() {
        let metrics: [fn(char) -> (i32, i32); 5] = [
            |_| (2, 4),
            |ch| if ch == 'a' { (-3, -4) } else { (7, 9) },
            |ch| if ch == 'a' { (i32::MIN, i32::MAX) } else { (i32::MAX, i32::MIN) },
            |ch| if ch == 'b' { (i32::MAX, i32::MAX) } else { (i32::MIN, i32::MIN) },
            |_| (-8, 0),
        ];
        for metric in metrics {
            let atlas = metric_atlas(metric);
            let font = atlas.font_id("body").unwrap();
            for text in ["a a", "ab ba a", "a aaaaa b", "  aé 日本  b ", "a b\n\nété a\n", "a\rb a"] {
                for wrap in [TextWrap::None, TextWrap::Word, TextWrap::WordOrCharacter] {
                    for width in [0, 1, 10, 14, 40, i32::MAX] {
                        let lines = build_text_lines(text, wrap, width, font, &atlas);
                        let mut restored = String::new();
                        for (index, line) in lines.iter().enumerate() {
                            let slice = &text[line.start..line.end];
                            assert_eq!(line.width, atlas.get_text_size(font, slice).width, "{text:?}, {wrap:?}, {width}");
                            if index > 0 && text.as_bytes().get(lines[index - 1].end) == Some(&b'\n') {
                                restored.push('\n');
                            }
                            restored.push_str(slice);
                        }
                        assert_eq!(restored, text, "wrapping must retain every original byte");
                    }
                }
            }
        }
    }

    /// A normal word followed by character fallback must use the same pen/extent policy.
    #[test]
    fn overhanging_words_transition_into_unicode_character_fallback() {
        let atlas = metric_atlas(|_| (2, 4));
        let font = atlas.font_id("body").unwrap();
        let text = "a aéééé";
        let lines = build_text_lines(text, TextWrap::WordOrCharacter, 14, font, &atlas);
        let slices: Vec<_> = lines.iter().map(|l| (&text[l.start..l.end], l.width)).collect();
        assert_eq!(slices, [("a ", 10), ("aéé", 14), ("éé", 10)]);
    }

    /// Constructs a valid atlas whose fallback advance occupies the complete positive i32 range.
    fn extreme_advance_atlas() -> AtlasHandle {
        let mut data = crate::test_support::test_asset_data_with_font_sizes(&[("body", 1)]);
        for glyph in data.tables.glyphs.to_mut() {
            // Canonical glyph records permit every signed advance. Wrapping owns the wider
            // accumulation policy and therefore receives the extreme value without translation.
            glyph.advance_x = i32::MAX;
        }
        AtlasHandle::from_validated_data(data, None).expect("the extreme-advance fixture must satisfy the canonical contract")
    }

    /// Verifies two individually representable words wrap when their mathematical sum is wider.
    #[test]
    fn word_wrap_compares_widths_before_coordinate_saturation() {
        let atlas = extreme_advance_atlas();
        let font = atlas.font_id("body").expect("the fixture font must exist");
        let lines = build_text_lines("_ _", TextWrap::Word, i32::MAX, font, &atlas);

        assert_eq!(lines.len(), 2);
        assert_eq!((lines[0].start, lines[0].end, lines[0].width), (0, 2, i32::MAX));
        assert_eq!((lines[1].start, lines[1].end, lines[1].width), (2, 3, i32::MAX));
    }

    /// Paths without spaces must fit while retaining every original UTF-8 byte.
    #[test]
    fn character_fallback_preserves_unicode_paths_and_measured_widths() {
        let data = crate::test_support::test_asset_data_with_font_sizes(&[("body", 16)]);
        let atlas = AtlasHandle::from_validated_data(data, None).unwrap();
        let font = atlas.font_id("body").unwrap();
        let path = "/themes/été/日本/theme.json";
        let lines = build_display_text_lines(path, TextWrap::WordOrCharacter, 40, font, &atlas);
        assert!(lines.len() > 1);
        let mut restored = String::new();
        for line in &lines {
            // Slicing itself verifies that no visual break bisects a multibyte scalar.
            let text = &path[line.start..line.end];
            assert!(!text.is_empty());
            assert!(line.width <= 40);
            assert_eq!(line.width, atlas.get_text_size(font, text).width);
            restored.push_str(text);
        }
        assert_eq!(restored, path);
        // Word-only callers retain their explicit oversized-token policy.
        let unbroken = build_display_text_lines(path, TextWrap::Word, 40, font, &atlas);
        assert_eq!(unbroken.len(), 1);
        assert!(unbroken[0].width > 40);
    }

    /// Fallback applies only to oversized tokens, preserving normal word and newline breaks.
    #[test]
    fn character_fallback_prefers_words_and_keeps_editable_blank_lines() {
        let data = crate::test_support::test_asset_data_with_font_sizes(&[("body", 16)]);
        let atlas = AtlasHandle::from_validated_data(data, None).unwrap();
        let font = atlas.font_id("body").unwrap();
        let text = "ab ab aaaaaa\n";
        let lines = build_text_lines(text, TextWrap::WordOrCharacter, 40, font, &atlas);
        let slices: Vec<_> = lines.iter().map(|line| &text[line.start..line.end]).collect();
        assert_eq!(slices, ["ab ", "ab ", "aaaaa", "a", ""]);
        assert_eq!(build_display_text_lines(text, TextWrap::WordOrCharacter, 40, font, &atlas).len(), 4);
    }

    /// An oversized glyph must make progress and adjacent advances must not hide overflow.
    #[test]
    fn character_fallback_handles_indivisible_glyphs_and_saturated_widths() {
        let atlas = extreme_advance_atlas();
        let font = atlas.font_id("body").unwrap();
        for width in [1, i32::MAX] {
            let lines = build_text_lines("é_", TextWrap::WordOrCharacter, width, font, &atlas);
            assert_eq!(lines.len(), 2);
            assert_eq!((lines[0].start, lines[0].end, lines[0].width), (0, 2, i32::MAX));
            assert_eq!((lines[1].start, lines[1].end, lines[1].width), (2, 3, i32::MAX));
        }
    }

    /// Glyph overhang belongs to the line's ink extent, not every pen advance.
    #[test]
    fn character_fallback_matches_painted_overhangs() {
        let mut data = crate::test_support::test_asset_data_with_font_sizes(&[("body", 16)]);
        for glyph in data.tables.glyphs.to_mut() {
            glyph.offset_x = 2;
            glyph.advance_x = 4;
        }
        let atlas = AtlasHandle::from_validated_data(data, None).unwrap();
        let font = atlas.font_id("body").unwrap();
        let text = "aaaa";
        let lines = build_display_text_lines(text, TextWrap::WordOrCharacter, 10, font, &atlas);
        assert_eq!(lines.len(), 2);
        for line in lines {
            assert_eq!(&text[line.start..line.end], "aa");
            assert_eq!(line.width, 10);
            assert_eq!(line.width, atlas.get_text_size(font, &text[line.start..line.end]).width);
        }
    }
}
