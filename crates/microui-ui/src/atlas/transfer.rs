//! Conversion from validated portable tables to UI-local lookup storage.
use super::*;
use microui_contracts::{AssetFont, AssetGlyph, AssetIcon, AssetInfo, AssetRange};
use nc_com::NcStatus;
#[cfg(test)]
use nc_com::NC_E_INVALID_ARG;

/// Owned wire tables prepared once by an exporter or copied once by an importer.
#[derive(Default)]
pub(crate) struct Tables {
    /// UTF-8 bytes partitioned by font names followed by icon names.
    pub names: Vec<u8>,
    /// Fonts partition the glyph array in declaration order.
    pub fonts: Vec<AssetFont>,
    /// Unicode scalar records, sorted within exported fonts for deterministic output.
    pub glyphs: Vec<AssetGlyph>,
    /// Named image rectangles in atlas table order.
    pub icons: Vec<AssetIcon>,
}

/// Copies one local rectangle into the shared pixel record.
#[cfg(test)]
fn rect(value: Recti) -> microui_contracts::Rect {
    microui_contracts::Rect {
        x: value.x,
        y: value.y,
        width: value.width,
        height: value.height,
    }
}

impl Tables {
    /// Writes deterministic typed metadata without exposing hash maps or Rust references through COM.
    #[cfg(test)]
    pub(crate) fn export(atlas: &AtlasHandle) -> Result<Self, NcStatus> {
        // Check before narrowing any offset or allocating the wire tables.
        let glyphs = atlas
            .0
            .fonts
            .iter()
            .try_fold(0usize, |sum, (_, font)| sum.checked_add(font.entries.len()))
            .ok_or(NC_E_INVALID_ARG)?;
        let names = atlas
            .0
            .fonts
            .iter()
            .map(|(name, _)| name.len())
            .chain(atlas.0.icons.iter().map(|(name, _)| name.len()))
            .try_fold(0usize, |sum, len| sum.checked_add(len))
            .ok_or(NC_E_INVALID_ARG)?;
        if atlas.0.fonts.len() > microui_contracts::MAX_ASSET_FONTS as usize
            || atlas.0.icons.len() > microui_contracts::MAX_ASSET_ICONS as usize
            || glyphs > microui_contracts::MAX_ASSET_GLYPHS as usize
            || names > microui_contracts::MAX_ASSET_NAMES as usize
        {
            return Err(NC_E_INVALID_ARG);
        }
        let mut tables = Self::default();
        for (name, font) in &atlas.0.fonts {
            let name = tables.name(name);
            let first = tables.glyphs.len() as u32;
            let mut glyphs: Vec<_> = font.entries.iter().collect();
            glyphs.sort_unstable_by_key(|(scalar, _)| **scalar);
            tables.glyphs.extend(glyphs.into_iter().map(|(scalar, entry)| AssetGlyph {
                scalar: *scalar as u32,
                offset_x: entry.offset.x,
                offset_y: entry.offset.y,
                advance_x: entry.advance.x,
                advance_y: entry.advance.y,
                rect: rect(entry.rect),
            }));
            tables.fonts.push(AssetFont {
                name,
                glyphs: AssetRange { first, count: font.entries.len() as u32 },
                line_size: font.line_size as i32,
                baseline: font.baseline,
                font_size: font.font_size as i32,
            });
        }
        for (name, icon) in &atlas.0.icons {
            let name = tables.name(name);
            tables.icons.push(AssetIcon { name, rect: rect(icon.rect) });
        }
        Ok(tables)
    }

    /// Appends one name while deriving its byte range from the actual storage.
    #[cfg(test)]
    fn name(&mut self, name: &str) -> AssetRange {
        let range = AssetRange {
            first: self.names.len() as u32,
            count: name.len() as u32,
        };
        self.names.extend_from_slice(name.as_bytes());
        range
    }

    /// Reconstructs local identities from tables already checked by `AssetData::read`.
    ///
    /// Original catalog names preserve exact local IDs; revision-private artwork receives fresh
    /// IDs. Portable validation remains singular: this conversion cannot be called before the
    /// facade has checked name partitions, scalar values, metrics, rectangles and the white tile.
    pub(crate) fn import(&self, info: AssetInfo, pixels: Vec<u8>, catalog: Option<&AtlasHandle>) -> Result<AtlasHandle, NcStatus> {
        let mut fonts = Vec::with_capacity(self.fonts.len());
        for font in &self.fonts {
            let name = self.validated_name(font.name);
            let last = font.glyphs.first + font.glyphs.count;
            let glyphs = &self.glyphs[font.glyphs.first as usize..last as usize];
            let entries = glyphs
                .iter()
                .map(|glyph| {
                    (
                        char::from_u32(glyph.scalar).expect("AssetData::read validated every glyph scalar"),
                        CharEntry {
                            offset: Vec2i::new(glyph.offset_x, glyph.offset_y),
                            advance: Vec2i::new(glyph.advance_x, glyph.advance_y),
                            rect: crate::rect(glyph.rect.x, glyph.rect.y, glyph.rect.width, glyph.rect.height),
                        },
                    )
                })
                .collect();
            fonts.push((
                name.to_owned(),
                Font {
                    id: catalog.and_then(|atlas| atlas.font_id(name)).unwrap_or_else(FontId::allocate),
                    line_size: font.line_size as usize,
                    baseline: font.baseline,
                    font_size: font.font_size as usize,
                    entries,
                },
            ));
        }
        let mut icons = Vec::with_capacity(self.icons.len());
        for icon in &self.icons {
            let name = self.validated_name(icon.name);
            icons.push((
                name.to_owned(),
                Icon {
                    id: catalog.and_then(|atlas| atlas.icon_id(name)).unwrap_or_else(IconId::allocate),
                    rect: crate::rect(icon.rect.x, icon.rect.y, icon.rect.width, icon.rect.height),
                },
            ));
        }
        let pixels = pixels.as_chunks::<4>().0.iter().map(|pixel| crate::color4b(pixel[0], pixel[1], pixel[2], pixel[3])).collect();
        let font_slots = fonts.iter().enumerate().map(|(slot, (_, font))| (font.id, slot)).collect::<HashMap<_, _>>();
        let icon_slots = icons.iter().enumerate().map(|(slot, (_, icon))| (icon.id, slot)).collect::<HashMap<_, _>>();
        let white_icon = icons
            .iter()
            .find_map(|(name, icon)| (name == "white").then_some(icon.id))
            .expect("AssetData::read validated the required white icon");
        Ok(AtlasHandle(Rc::new(Atlas {
            width: info.size.width as usize,
            height: info.size.height as usize,
            pixels,
            fonts,
            icons,
            white_icon,
            font_slots,
            icon_slots,
        })))
    }

    /// Borrows one name whose range and UTF-8 encoding were checked before local conversion.
    fn validated_name(&self, range: AssetRange) -> &str {
        let last = range.first + range.count;
        std::str::from_utf8(&self.names[range.first as usize..last as usize])
            .expect("AssetData::read validated every resource name")
    }
}
