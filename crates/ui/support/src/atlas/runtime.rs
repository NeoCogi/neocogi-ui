//
// Copyright 2023-Present (c) Raja Lehtihet & Wael El Oraiby
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the conditions in LICENSE are met.
//

//! Runtime lookup indices over canonical NCIDL-backed asset tables.

use super::*;
use neocogi_com::{NcStatus, NC_E_OUT_OF_MEMORY};
use neocogi_ui::abi::core::{AssetRange, Rect as AssetRect};

/// Clamps one mathematically accumulated text coordinate into the renderer rectangle domain.
fn clamp_text_coordinate(value: i128) -> i32 {
    // A UTF-8 string cannot contain enough i32 advances to overflow i128. Retaining the wider
    // value until emission preserves cancellation between positive and negative font metrics.
    value.clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32
}

/// Right edge used by measurement after the drawable glyph origin has been clamped.
fn measured_glyph_right(x: i32, advance: i32, width: i32) -> i32 {
    x.saturating_add(advance.max(width))
}

/// Copyable horizontal run state shared by whole-word fit and character fallback.
///
/// Pen movement is independent of the right extent: overhang never becomes extra spacing.
/// A UTF-8 string cannot contain enough signed i32 advances to overflow i128, so pen
/// cancellation remains exact until a drawable coordinate is emitted. No text or resources
/// are retained here; copying this state permits a bounded trial of the next token or scalar.
#[derive(Clone, Copy, Default)]
pub(crate) struct TextRun {
    /// Mathematical origin of the next glyph, including every signed advance.
    pen: i128,
    /// Nonnegative mathematical right extent, used for fit before coordinate saturation.
    right: i128,
    /// Width in the drawable i32 domain, identical to measurement of the exact run slice.
    width: i32,
}

impl TextRun {
    /// Appends canonical metrics, preserving the atlas's origin-then-edge clamping order.
    fn push(&mut self, glyph: &AssetGlyph) {
        let x = self.pen + i128::from(glyph.offset_x);
        self.right = self.right.max(x + i128::from(glyph.advance_x.max(glyph.rect.width)));
        self.width = self
            .width
            .max(measured_glyph_right(clamp_text_coordinate(x), glyph.advance_x, glyph.rect.width));
        self.pen += i128::from(glyph.advance_x);
    }

    /// Tests the mathematical right extent, so saturation cannot hide overflow at i32::MAX.
    pub(crate) fn fits(self, max_width: i32) -> bool {
        self.right <= i128::from(max_width)
    }

    /// Returns the same bounded width as the atlas measurement of this exact slice.
    pub(crate) fn width(self) -> i32 {
        self.width
    }
}

/// Converts the generated renderer rectangle into the runtime math rectangle at point of use.
fn runtime_rect(value: AssetRect) -> Recti {
    // Both records use identical integer units; this is a transient API-boundary projection rather
    // than retained resource storage.
    Recti::new(value.x, value.y, value.width, value.height)
}

impl AtlasHandle {
    /// Builds UI-specific indices for tables already accepted by the canonical GFX validator.
    ///
    /// `previous` is the original catalog snapshot for a derived revision. Matching names inherit
    /// its opaque process identities, while revision-private resources receive new identities.
    pub(crate) fn from_validated_tables(tables: AssetTables, previous: Option<&Self>) -> Result<Self, NcStatus> {
        let mut font_ids = Vec::new();
        font_ids.try_reserve_exact(tables.fonts.len()).map_err(|_| NC_E_OUT_OF_MEMORY)?;
        for font in tables.fonts.iter() {
            let name = tables.name(font.name).expect("validated font range must name UTF-8 text");
            font_ids.push(previous.and_then(|atlas| atlas.font_id(name)).unwrap_or_else(FontId::allocate));
        }

        let mut icon_ids = Vec::new();
        icon_ids.try_reserve_exact(tables.icons.len()).map_err(|_| NC_E_OUT_OF_MEMORY)?;
        for icon in tables.icons.iter() {
            let name = tables.name(icon.name).expect("validated icon range must name UTF-8 text");
            icon_ids.push(previous.and_then(|atlas| atlas.icon_id(name)).unwrap_or_else(IconId::allocate));
        }

        let mut font_name_order = index_slots(tables.fonts.len())?;
        font_name_order.sort_unstable_by(|left, right| {
            let left = tables.name(tables.fonts[*left].name).expect("validated font name");
            let right = tables.name(tables.fonts[*right].name).expect("validated font name");
            left.cmp(right)
        });
        let mut icon_name_order = index_slots(tables.icons.len())?;
        icon_name_order.sort_unstable_by(|left, right| {
            let left = tables.name(tables.icons[*left].name).expect("validated icon name");
            let right = tables.name(tables.icons[*right].name).expect("validated icon name");
            left.cmp(right)
        });

        let glyph_ascii = neocogi_ui::assets::AsciiGlyphIndex::build(&tables)?;
        let mut glyph_order = index_slots(tables.glyphs.len())?;
        for font in tables.fonts.iter() {
            glyph_order[table_range(font.glyphs)].sort_unstable_by_key(|slot| tables.glyphs[*slot].scalar);
        }

        let body = find_font_slot(&tables, &font_name_order, "body")
            .map(|slot| font_ids[slot])
            .expect("AssetTables::validate requires the body font");
        let font_roles = FontRole::ALL.map(|role| {
            let name = role.canonical_name().expect("FontRole::ALL contains only declared values");
            find_font_slot(&tables, &font_name_order, name).map_or(body, |slot| font_ids[slot])
        });
        let icon_roles = IconRole::ALL.map(|role| {
            let name = role.canonical_name().expect("IconRole::ALL contains only declared values");
            let slot = find_icon_slot(&tables, &icon_name_order, name).expect("AssetTables::validate requires every semantic icon");
            icon_ids[slot]
        });
        let white_slot = find_icon_slot(&tables, &icon_name_order, "white").expect("AssetTables::validate requires the opaque white icon");
        let white_icon = icon_ids[white_slot];

        Ok(Self(Rc::new(Atlas {
            tables,
            font_ids,
            font_name_order,
            icon_ids,
            icon_name_order,
            glyph_order,
            glyph_ascii,
            white_icon,
            font_roles,
            icon_roles,
            #[cfg(any(test, feature = "test-support"))]
            pixels: Vec::new(),
        })))
    }

    /// Installs exact fixture pixels after canonical validation and index construction.
    #[cfg(any(test, feature = "test-support"))]
    pub fn from_validated_data(data: neocogi_ui::assets::AssetData, previous: Option<&Self>) -> Result<Self, NcStatus> {
        // Tests deliberately cross the same canonical validator as a published bundle. The direct
        // constructor avoids an export/import cycle while retaining pixels needed by test sources.
        data.validate()?;
        let pixels = data.pixels.into_owned();
        let mut atlas = Self::from_validated_tables(data.tables, previous)?;
        Rc::get_mut(&mut atlas.0).expect("new atlas handle has one owner").pixels = pixels;
        Ok(atlas)
    }

    /// Returns the atlas texture width in pixels.
    pub fn width(&self) -> usize {
        self.0.tables.size.width as usize
    }

    /// Returns the atlas texture height in pixels.
    pub fn height(&self) -> usize {
        self.0.tables.size.height as usize
    }

    /// Borrows fixture RGBA bytes for direct publication by test infrastructure.
    #[cfg(any(test, feature = "test-support"))]
    pub fn pixels(&self) -> &[u8] {
        &self.0.pixels
    }

    /// Borrows the one canonical resource and appearance snapshot.
    pub fn tables(&self) -> &AssetTables {
        &self.0.tables
    }

    /// Mutably borrows canonical fixture tables after detaching a shared test snapshot.
    #[cfg(any(test, feature = "test-support"))]
    pub fn tables_mut(&mut self) -> &mut AssetTables {
        // Copy-on-write is confined to explicit test style editing. Production revisions arrive as
        // new immutable bundles and never require interior mutability or parallel native records.
        &mut Rc::make_mut(&mut self.0).tables
    }

    /// Returns a mapping from icon names to their catalog-scoped identifiers.
    pub fn clone_icon_table(&self) -> Vec<(String, IconId)> {
        self.0
            .tables
            .icons
            .iter()
            .zip(self.0.icon_ids.iter().copied())
            .map(|(icon, id)| (self.name(icon.name).to_owned(), id))
            .collect()
    }

    /// Returns a mapping from font names to their catalog-scoped identifiers.
    pub fn clone_font_table(&self) -> Vec<(String, FontId)> {
        self.0
            .tables
            .fonts
            .iter()
            .zip(self.0.font_ids.iter().copied())
            .map(|(font, id)| (self.name(font.name).to_owned(), id))
            .collect()
    }

    /// Looks up a font by its canonical table name.
    pub fn font_id(&self, name: &str) -> Option<FontId> {
        find_font_slot(&self.0.tables, &self.0.font_name_order, name).map(|slot| self.0.font_ids[slot])
    }

    /// Looks up an icon by its canonical table name.
    pub fn icon_id(&self, name: &str) -> Option<IconId> {
        find_icon_slot(&self.0.tables, &self.0.icon_name_order, name).map(|slot| self.0.icon_ids[slot])
    }

    /// Returns the named opaque-white tile used for solid rendering.
    pub fn white_icon(&self) -> IconId {
        self.0.white_icon
    }

    /// Returns the pre-resolved font identity for one semantic role.
    pub(crate) fn font_role(&self, role: FontRole) -> FontId {
        self.0.font_roles[role.raw() as usize]
    }

    /// Returns the pre-resolved icon identity for one semantic role.
    pub(crate) fn icon_role(&self, role: IconRole) -> IconId {
        self.0.icon_roles[role.raw() as usize]
    }

    /// Reports whether this atlas contains the logical font resource.
    pub(crate) fn contains_font(&self, font: FontId) -> bool {
        self.0.font_ids.contains(&font)
    }

    /// Reports whether this atlas contains the logical icon resource.
    pub(crate) fn contains_icon(&self, icon: IconId) -> bool {
        self.0.icon_ids.contains(&icon)
    }

    /// Returns the exact bundle-local slot for an authenticated font identity.
    pub fn font_index(&self, font: FontId) -> u32 {
        let slot = self.font_slot(font);
        u32::try_from(slot).expect("validated asset font count fits u32")
    }

    /// Returns the exact bundle-local slot for an authenticated icon identity.
    pub fn icon_index(&self, icon: IconId) -> u32 {
        let slot = self.icon_slot(icon);
        u32::try_from(slot).expect("validated asset icon count fits u32")
    }

    /// Resolves one checked bundle-local font slot for scoped semantic painting.
    pub fn font_at_index(&self, index: u32) -> Option<FontId> {
        self.0.font_ids.get(index as usize).copied()
    }

    /// Resolves one checked bundle-local icon slot for scoped semantic painting.
    pub fn icon_at_index(&self, index: u32) -> Option<IconId> {
        self.0.icon_ids.get(index as usize).copied()
    }

    /// Returns exact canonical glyph metrics for a character without applying fallback policy.
    ///
    /// # Panics
    ///
    /// Panics when `font` belongs to another catalog.
    pub fn get_char_entry(&self, font: FontId, character: char) -> Option<AssetGlyph> {
        let slot = self.font_slot(font);
        self.glyph(slot, character).copied()
    }

    /// Returns the canonical line height for the specified font.
    pub fn get_font_height(&self, font: FontId) -> usize {
        self.font(font).line_size as usize
    }

    /// Returns the canonical baseline offset for the specified font.
    pub fn get_font_baseline(&self, font: FontId) -> i32 {
        self.font(font).baseline
    }

    /// Returns the canonical requested pixel size for the specified font.
    pub fn get_font_size(&self, font: FontId) -> usize {
        self.font(font).font_size as usize
    }

    /// Returns the dimensions of an icon from its canonical record.
    pub fn get_icon_size(&self, icon: IconId) -> Dimensioni {
        let rect = self.icon(icon).rect;
        Dimensioni::new(rect.width, rect.height)
    }

    /// Returns the atlas rectangle storing an icon.
    pub fn get_icon_rect(&self, icon: IconId) -> Recti {
        runtime_rect(self.icon(icon).rect)
    }

    /// Returns the atlas texture dimensions.
    pub fn get_texture_dimension(&self) -> Dimensioni {
        Dimension::new(self.0.tables.size.width, self.0.tables.size.height)
    }

    /// Resolves a stable font identity to the canonical generated record.
    fn font(&self, font: FontId) -> &AssetFont {
        &self.0.tables.fonts[self.font_slot(font)]
    }

    /// Resolves a stable icon identity to the canonical generated record.
    fn icon(&self, icon: IconId) -> &neocogi_ui::abi::core::AssetIcon {
        &self.0.tables.icons[self.icon_slot(icon)]
    }

    /// Resolves a stable font identity to its canonical table slot.
    fn font_slot(&self, font: FontId) -> usize {
        self.0
            .font_ids
            .iter()
            .position(|candidate| *candidate == font)
            .unwrap_or_else(|| panic!("font ID does not belong to this atlas: {font:?}"))
    }

    /// Resolves a stable icon identity to its canonical table slot.
    fn icon_slot(&self, icon: IconId) -> usize {
        self.0
            .icon_ids
            .iter()
            .position(|candidate| *candidate == icon)
            .unwrap_or_else(|| panic!("icon ID does not belong to this atlas: {icon:?}"))
    }

    /// Searches the pre-sorted glyph index inside one canonical font partition.
    fn glyph(&self, font_slot: usize, character: char) -> Option<&AssetGlyph> {
        if u32::from(character) < 128 {
            return self.0.glyph_ascii[font_slot].get(u32::from(character)).map(|slot| &self.0.tables.glyphs[slot]);
        }
        let font = self.0.tables.fonts[font_slot];
        let order = &self.0.glyph_order[table_range(font.glyphs)];
        order
            .binary_search_by_key(&u32::from(character), |slot| self.0.tables.glyphs[*slot].scalar)
            .ok()
            .map(|index| &self.0.tables.glyphs[order[index]])
    }

    /// Resolves a validated name range without retaining a second string table.
    fn name(&self, range: AssetRange) -> &str {
        self.0.tables.name(range).expect("validated asset name remains valid")
    }

    /// Resolves the same underscore fallback for painting, measurement and wrapping.
    fn glyph_or_fallback(&self, font_slot: usize, character: char) -> &AssetGlyph {
        self.glyph(font_slot, character)
            .or_else(|| self.glyph(font_slot, '_'))
            .expect("validated asset font contains the underscore fallback")
    }

    /// Extends a horizontal trial without allocating or remeasuring its existing prefix.
    ///
    /// LF/CR reset the pen, preserving the atlas's multiline width policy. The line builder
    /// splits on LF itself; widget storage normalizes CRLF and lone CR before layout.
    pub(crate) fn extend_text_run(&self, font: FontId, text: &str, run: &mut TextRun) {
        let slot = self.font_slot(font);
        for character in text.chars() {
            match character {
                '\r' | '\n' => run.pen = 0,
                _ => run.push(self.glyph_or_fallback(slot, character)),
            }
        }
    }

    /// Walks glyphs with canonical metrics, baseline placement, and one run origin.
    fn walk_glyphs<F>(&self, font: FontId, text: &str, origin: Vec2i, mut draw: F)
    where
        F: FnMut(char, Vec2i, Recti, Recti, i32),
    {
        self.walk_glyphs_until(font, text, origin, |_, character, advance, destination, source, baseline_y| {
            draw(character, advance, destination, source, baseline_y);
            std::ops::ControlFlow::Continue(())
        });
    }

    /// Walks the same canonical geometry, stopping as soon as the visitor selects a boundary.
    /// The byte offset always belongs to the originally requested scalar, including fallback.
    /// Line endings reset the pen without invoking the visitor, just as the full walk does.
    fn walk_glyphs_until<F>(&self, font: FontId, text: &str, origin: Vec2i, mut draw: F)
    where
        F: FnMut(usize, char, Vec2i, Recti, Recti, i32) -> std::ops::ControlFlow<()>,
    {
        let font_slot = self.font_slot(font);
        let font = self.0.tables.fonts[font_slot];
        let mut destination = Recti::default();
        let line_height = i128::from(font.line_size);
        let baseline = i128::from(font.baseline);
        let mut baseline_y = baseline;
        let mut pen_x = 0_i128;
        let mut characters = text.chars().peekable();
        let mut source_offset = 0;

        while let Some(character) = characters.next() {
            // Offset tracking is used only by cursor visitors. Ordinary drawing/measurement
            // visitors ignore it, so their monomorphized walk retains the compact Chars iterator.
            let offset = source_offset;
            source_offset += character.len_utf8();
            match character {
                '\r' => {
                    if characters.peek() == Some(&'\n') {
                        characters.next();
                        source_offset += 1;
                    }
                    pen_x = 0;
                    baseline_y += line_height;
                }
                '\n' => {
                    pen_x = 0;
                    baseline_y += line_height;
                }
                _ => {
                    let glyph = self.glyph_or_fallback(font_slot, character);
                    let source = runtime_rect(glyph.rect);
                    destination.width = source.width;
                    destination.height = source.height;
                    destination.x = clamp_text_coordinate(i128::from(origin.x) + pen_x + i128::from(glyph.offset_x));
                    destination.y = clamp_text_coordinate(i128::from(origin.y) + baseline_y - i128::from(glyph.offset_y) - i128::from(glyph.rect.height));
                    let advance = Vec2i::new(glyph.advance_x, glyph.advance_y);
                    if draw(
                        offset,
                        character,
                        advance,
                        destination,
                        source,
                        clamp_text_coordinate(i128::from(origin.y) + baseline_y),
                    )
                    .is_break()
                    {
                        return;
                    }
                    pen_x += i128::from(glyph.advance_x);
                }
            }
        }
    }

    /// Walks each drawable scalar and invokes `draw` with its resolved geometry.
    ///
    /// LF, lone CR, and CRLF advance one line. Missing characters use the canonical underscore
    /// record, while `draw` still receives the originally requested character.
    pub fn draw_string<DrawFunction: FnMut(char, Vec2i, Recti, Recti)>(&self, font: FontId, text: &str, origin: Vec2i, mut draw: DrawFunction) {
        self.walk_glyphs(font, text, origin, |character, advance, destination, source, _| {
            draw(character, advance, destination, source);
        });
    }

    /// Chooses the nearest scalar byte boundary using the same prefix extent as text measurement.
    ///
    /// Signed advances, glyph overhang and coordinate saturation contribute to a running maximum
    /// exactly as they do in `get_text_size`. A hit stops the canonical walk immediately; no
    /// prefix is remeasured, no widths are cached, and unread trailing scalars are not visited.
    /// Midpoint ties choose the following byte boundary. Nonpositive x and empty text return
    /// zero before resolving the font. Multiline input retains the maximum preceding extent;
    /// text widgets normally call this on a single display-line slice.
    pub fn text_cursor_at_x(&self, font: FontId, text: &str, target_x: i32) -> usize {
        if target_x <= 0 || text.is_empty() {
            return 0;
        }
        let mut result = text.len();
        let mut previous_width = 0;
        self.walk_glyphs_until(font, text, Vec2i::default(), |offset, character, advance, destination, _, _| {
            let width = max(previous_width, measured_glyph_right(destination.x, advance.x, destination.width));
            if target_x < width {
                let midpoint = (i64::from(previous_width) + i64::from(width)) / 2;
                result = if i64::from(target_x) < midpoint {
                    offset
                } else {
                    offset + character.len_utf8()
                };
                return std::ops::ControlFlow::Break(());
            }
            previous_width = width;
            std::ops::ControlFlow::Continue(())
        });
        result
    }

    /// Measures text with the exact glyph walk used for drawing.
    pub fn get_text_size(&self, font: FontId, text: &str) -> Dimensioni {
        let mut result = Dimensioni::new(0, 0);
        let line_height = self.get_font_height(font) as i32;
        let baseline = self.get_font_baseline(font);
        let descent = line_height.saturating_sub(baseline).max(0);
        let mut max_line_bottom = 0;
        let mut saw_glyph = false;

        self.walk_glyphs(font, text, Vec2i::default(), |_, advance, destination, _, baseline_y| {
            saw_glyph = true;
            result.width = max(result.width, measured_glyph_right(destination.x, advance.x, destination.width));
            result.height = max(result.height, destination.y.saturating_add(destination.height));
            max_line_bottom = max(max_line_bottom, baseline_y.saturating_add(descent));
        });
        if saw_glyph {
            result.height = max(result.height, max_line_bottom);
        }
        result
    }
}

/// Converts one validated generated range to a native slice range.
fn table_range(range: AssetRange) -> std::ops::Range<usize> {
    let first = range.first as usize;
    first..first + range.count as usize
}

/// Allocates a fallible identity ordering for one bounded canonical table.
fn index_slots(count: usize) -> Result<Vec<usize>, NcStatus> {
    // Asset validation bounds every count, but allocation failure still crosses the COM-facing
    // import path as a status instead of becoming an unwinding implementation detail.
    let mut slots = Vec::new();
    slots.try_reserve_exact(count).map_err(|_| NC_E_OUT_OF_MEMORY)?;
    slots.extend(0..count);
    Ok(slots)
}

/// Finds one font slot through the name-sorted derived index.
fn find_font_slot(tables: &AssetTables, order: &[usize], name: &str) -> Option<usize> {
    order
        .binary_search_by(|slot| tables.name(tables.fonts[*slot].name).expect("validated font name").cmp(name))
        .ok()
        .map(|index| order[index])
}

/// Finds one icon slot through the name-sorted derived index.
fn find_icon_slot(tables: &AssetTables, order: &[usize], name: &str) -> Option<usize> {
    order
        .binary_search_by(|slot| tables.name(tables.icons[*slot].name).expect("validated icon name").cmp(name))
        .ok()
        .map(|index| order[index])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates a separately owned canonical fixture on every call.
    fn make_atlas() -> AtlasHandle {
        crate::test_support::test_atlas()
    }

    /// Verifies independently loaded resources remain distinct while handle clones share IDs.
    #[test]
    fn resource_ids_identify_logical_resources_instead_of_equal_table_positions() {
        let first = make_atlas();
        let first_clone = first.clone();
        let second = make_atlas();
        let first_font = first.font_id("body").unwrap();
        let second_font = second.font_id("body").unwrap();
        let first_icon = first.white_icon();
        let second_icon = second.white_icon();

        assert_ne!(first_font, second_font);
        assert_ne!(first_icon, second_icon);
        assert_eq!(first_clone.font_id("body"), Some(first_font));
        assert_eq!(first_clone.white_icon(), first_icon);
        assert!(first_clone.contains_font(first_font));
        assert!(first_clone.contains_icon(first_icon));
        assert!(!second.contains_font(first_font));
        assert!(!second.contains_icon(first_icon));
    }

    /// Exact ASCII and Unicode metrics stay distinct from fallback across immutable revision import.
    #[test]
    fn glyph_indices_preserve_unicode_missing_ascii_and_derived_revisions() {
        let mut data = crate::test_support::test_asset_data_with_font_sizes(&[("body", 10)]);
        let glyphs = data.tables.glyphs.to_mut();
        glyphs[0].advance_x = 5;
        glyphs[1].advance_x = 3;
        glyphs[2].scalar = u32::from('Ω');
        glyphs[2].advance_x = 7;
        let atlas = AtlasHandle::from_validated_data(data.clone(), None).unwrap();
        let font = atlas.font_id("body").unwrap();
        assert_eq!(atlas.get_char_entry(font, 'a').unwrap().advance_x, 3);
        assert!(atlas.get_char_entry(font, 'b').is_none());
        assert_eq!(atlas.get_char_entry(font, 'Ω').unwrap().advance_x, 7);
        assert!(atlas.get_char_entry(font, '€').is_none());
        assert_eq!(atlas.get_text_size(font, "abΩ€").width, 20);
        data.tables.glyphs.to_mut()[1].advance_x = 11;
        let changed = AtlasHandle::from_validated_data(data, Some(&atlas)).unwrap();
        assert_eq!(changed.font_id("body"), Some(font));
        assert_eq!(changed.get_text_size(font, "abΩ€").width, 28);
        assert_eq!(atlas.get_text_size(font, "abΩ€").width, 20);
    }

    /// Verifies direct lookup rejects a same-position identity from another catalog.
    #[test]
    fn foreign_resource_lookup_panics_before_indexing_a_same_slot_entry() {
        let first = make_atlas();
        let second = make_atlas();
        let font = first.font_id("body").unwrap();
        let icon = first.white_icon();

        assert!(std::panic::catch_unwind(|| second.get_font_size(font)).is_err());
        assert!(std::panic::catch_unwind(|| second.get_icon_rect(icon)).is_err());
    }

    /// Verifies a platform CRLF sequence advances exactly one visual row.
    #[test]
    fn text_runtime_treats_crlf_as_one_line_ending() {
        let atlas = make_atlas();
        let font = atlas.font_id("body").unwrap();
        let crlf = atlas.get_text_size(font, "_\r\n_");
        let lf = atlas.get_text_size(font, "_\n_");
        assert_eq!((crlf.width, crlf.height), (lf.width, lf.height));
    }
}

#[cfg(test)]
mod cursor_tests {
    use super::*;

    /// Independent reference retains the previous complete-prefix public measurement behavior.
    fn reference_cursor(atlas: &AtlasHandle, font: FontId, text: &str, target: i32) -> usize {
        if target <= 0 {
            return 0;
        }
        let mut previous = 0;
        for (offset, ch) in text.char_indices() {
            let end = offset + ch.len_utf8();
            let width = atlas.get_text_size(font, &text[..end]).width;
            if target < width {
                let midpoint = (i64::from(previous) + i64::from(width)) / 2;
                return if i64::from(target) < midpoint { offset } else { end };
            }
            previous = width;
        }
        text.len()
    }

    /// Crosses the canonical validator and compares UTF-8, fallback and extreme signed metrics.
    #[test]
    fn cursor_matches_prefix_measurement_with_signed_overhang_and_saturation() {
        let values = [i32::MIN, -19, -1, 0, 1, 9, 23, i32::MAX];
        let texts = ["", "a", "ab", "aba_b", "aΩb☃🦀", "\r\n", "a\nb", "a\r\nb", "ba\r_☃\naba"];
        for (i, advance) in values.iter().enumerate() {
            for (j, offset) in values.iter().enumerate() {
                let mut data = crate::test_support::test_asset_data_with_font_sizes(&[("body", 16)]);
                for (k, glyph) in data.tables.glyphs.to_mut().iter_mut().enumerate() {
                    glyph.advance_x = if k == 1 { *advance } else { values[(i + k) % values.len()] };
                    glyph.offset_x = if k == 1 { *offset } else { values[(j + k) % values.len()] };
                    if (i + j + k) % 2 == 0 {
                        glyph.rect.width = 0;
                        glyph.rect.height = 0;
                    }
                }
                let atlas = AtlasHandle::from_validated_data(data, None).unwrap();
                let font = atlas.font_id("body").unwrap();
                for text in texts {
                    for target in (-2..=128).chain([i32::MAX / 2, i32::MAX - 1, i32::MAX]) {
                        let actual = atlas.text_cursor_at_x(font, text, target);
                        assert_eq!(
                            actual,
                            reference_cursor(&atlas, font, text, target),
                            "metrics=({advance},{offset}), text={text:?}, target={target}"
                        );
                        assert!(text.is_char_boundary(actual));
                    }
                }
            }
        }
    }

    /// Concrete midpoint and equality examples retain source byte positions for missing Unicode.
    #[test]
    fn cursor_keeps_midpoint_ties_and_unicode_boundaries() {
        let atlas = crate::test_support::test_atlas();
        let font = atlas.font_id("body").unwrap();
        for (target, expected) in [(0, 0), (3, 0), (4, 1), (8, 1), (12, 3), (16, 3), (20, 4), (24, 4)] {
            assert_eq!(atlas.text_cursor_at_x(font, "aΩb", target), expected);
        }
    }

    /// Empty/leading hits return before resolving a resource, matching the original helpers.
    #[test]
    fn empty_or_leading_cursor_hit_does_not_resolve_font() {
        let atlas = crate::test_support::test_atlas();
        let other = crate::test_support::test_atlas();
        let foreign_font = other.font_id("body").unwrap();
        assert_eq!(atlas.text_cursor_at_x(foreign_font, "", 1), 0);
        assert_eq!(atlas.text_cursor_at_x(foreign_font, "abc", 0), 0);
    }

    /// A selected leading glyph must not require visiting an arbitrarily long unread suffix.
    #[test]
    fn canonical_walk_stops_after_selected_boundary() {
        let atlas = crate::test_support::test_atlas();
        let font = atlas.font_id("body").unwrap();
        let mut visited = 0;
        atlas.walk_glyphs_until(font, &"a".repeat(8192), Vec2i::default(), |_, _, _, _, _, _| {
            visited += 1;
            std::ops::ControlFlow::Break(())
        });
        assert_eq!(visited, 1);
    }
}
