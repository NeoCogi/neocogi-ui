//! Overhanging text traverses production COM layout, input, paint and composition.
use super::*;

/// Distinguishes glyph quads from the window's ordinary background geometry.
const TEXT_COLOR: u32 = 0xff12ab34;

/// Builds a real runtime with visible overhang and no viewport padding masking the width.
fn overhang_fixture() -> Headless {
    let mut data = neocogi_ui_support::test_support::test_asset_data_with_font_sizes(&[("body", 16)]);
    for glyph in data.tables.glyphs.to_mut() {
        glyph.offset_x = 2;
        glyph.advance_x = 4;
    }
    data.tables.metrics.padding = 0;
    data.tables.surfaces.to_mut()[SurfaceRole::GenericFrame.raw() as usize].normal.content_color = TEXT_COLOR;
    let source = data.into_bundle().unwrap();
    Headless::new(source.as_com_ref())
}

/// An exactly fitting text block measures one row and emits all three glyphs on that row.
#[test]
fn overhanging_text_block_measures_and_paints_one_row() {
    for wrapping in [TextWrapping::Word, TextWrapping::WordOrCharacter] {
        let mut fixture = overhang_fixture();
        let block = fixture.ui.widgets().text_block("a a", wrapping).unwrap();
        let owner = window(&fixture.ui, "text", Rect { x: 10, y: 10, width: 14, height: 64 }, block);
        owner.set_options(PLAIN | WindowOptions::AUTO_HEIGHT).unwrap();
        fixture.frame(VIEW).unwrap();
        assert_eq!(owner.snapshot().unwrap().rect.height, 16);
        let frame = fixture.snapshot();
        // Identify the three glyph quads independently of the window background. A second
        // row or fictitious interword spacing cannot pass these actual screen-origin checks.
        let glyphs: Vec<_> = frame.vertices.as_chunks::<6>().0.iter().filter(|quad| quad[0].rgba == TEXT_COLOR).collect();
        assert_eq!(glyphs.len(), 3);
        let origins: Vec<_> = glyphs.iter().map(|quad| (quad[0].x, quad[0].y)).collect();
        assert_eq!(origins.iter().map(|point| point.0).collect::<Vec<_>>(), [12.0, 16.0, 20.0]);
        assert!(origins.iter().all(|point| point.1 == origins[0].1));
    }
}

/// End follows the actual visual row; fallback boundaries remain UTF-8 scalar boundaries.
#[test]
fn overhanging_text_area_navigation_uses_the_same_wrapped_rows() {
    for wrapping in [TextWrapping::Word, TextWrapping::WordOrCharacter] {
        let mut fixture = overhang_fixture();
        let mut area = fixture.ui.widgets().text_area_unobserved("a a", wrapping, false).unwrap();
        // Remove the requested outer frame so its two border pixels do not reduce the
        // explicitly tested 14-pixel editor allocation.
        area.clear_flags(WidgetOptions::FRAME).unwrap();
        let mut editor = area.handle();
        let _owner = window(&fixture.ui, "editor", Rect { x: 10, y: 10, width: 14, height: 64 }, area);
        key(&fixture.ui, KeyCode::Tab, 0, 0);
        fixture.frame(VIEW).unwrap();
        editor.set_cursor(0).unwrap();
        key(&fixture.ui, KeyCode::End, 0, 0);
        fixture.frame(VIEW).unwrap();
        assert_eq!(editor.cursor().unwrap(), 3, "the exactly fitting word must remain on row zero");
        if wrapping == TextWrapping::WordOrCharacter {
            editor.set_text("a aéééé").unwrap();
            editor.set_cursor(0).unwrap();
            fixture.frame(VIEW).unwrap();
            key(&fixture.ui, KeyCode::ArrowDown, 0, 0);
            key(&fixture.ui, KeyCode::End, 0, 0);
            fixture.frame(VIEW).unwrap();
            assert_eq!(editor.cursor().unwrap(), 7, "the next row contains the complete aéé slice");
            assert!(editor.text().unwrap().is_char_boundary(editor.cursor().unwrap() as usize));
        }
    }
}
