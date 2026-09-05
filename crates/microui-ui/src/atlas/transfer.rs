//! Bulk COM metadata joins the same duplicate-preserving validation path as native sources.
use super::*;
use microui_contracts::{AssetFont, AssetGlyph, AssetIcon, AssetInfo, AssetRange};
use nc_com::{NcStatus, NC_E_INVALID_ARG};

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
    fn name(&mut self, name: &str) -> AssetRange {
        let range = AssetRange {
            first: self.names.len() as u32,
            count: name.len() as u32,
        };
        self.names.extend_from_slice(name.as_bytes());
        range
    }

    /// Reconstructs local identities only after the caller authenticates the catalog namespace.
    /// Original catalog names preserve exact local IDs; revision-private artwork receives fresh IDs.
    pub(crate) fn import(&self, info: AssetInfo, pixels: Vec<u8>, catalog: Option<&AtlasHandle>) -> Result<AtlasHandle, NcStatus> {
        let mut next_name = 0;
        let mut next_glyph = 0;
        let mut fonts = Vec::with_capacity(self.fonts.len());
        for font in &self.fonts {
            let name = self.read_name(font.name, &mut next_name)?;
            if font.glyphs.first != next_glyph || font.line_size <= 0 || font.font_size <= 0 {
                return Err(NC_E_INVALID_ARG);
            }
            next_glyph = next_glyph.checked_add(font.glyphs.count).ok_or(NC_E_INVALID_ARG)?;
            let glyphs = self.glyphs.get(font.glyphs.first as usize..next_glyph as usize).ok_or(NC_E_INVALID_ARG)?;
            let entries = glyphs
                .iter()
                .map(|glyph| {
                    Ok((
                        char::from_u32(glyph.scalar).ok_or(NC_E_INVALID_ARG)?,
                        CharEntry {
                            offset: Vec2i::new(glyph.offset_x, glyph.offset_y),
                            advance: Vec2i::new(glyph.advance_x, glyph.advance_y),
                            rect: crate::rect(glyph.rect.x, glyph.rect.y, glyph.rect.width, glyph.rect.height),
                        },
                    ))
                })
                .collect::<Result<Vec<_>, NcStatus>>()?;
            fonts.push((
                name.to_owned(),
                FontCandidate {
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
            let name = self.read_name(icon.name, &mut next_name)?;
            icons.push((
                name.to_owned(),
                Icon {
                    id: catalog.and_then(|atlas| atlas.icon_id(name)).unwrap_or_else(IconId::allocate),
                    rect: crate::rect(icon.rect.x, icon.rect.y, icon.rect.width, icon.rect.height),
                },
            ));
        }
        if next_name as usize != self.names.len() || next_glyph as usize != self.glyphs.len() {
            return Err(NC_E_INVALID_ARG);
        }
        // Atlas finalization remains the authority for duplicate keys, glyph fallback, rectangle
        // bounds and the opaque white tile. Wire decoding does not duplicate those rules.
        let dimensions = validation::checked_atlas_dimensions(info.size.width as usize, info.size.height as usize).map_err(|_| NC_E_INVALID_ARG)?;
        let pixels = pixels.as_chunks::<4>().0.iter().map(|p| crate::color4b(p[0], p[1], p[2], p[3])).collect();
        AtlasHandle::finish(AtlasCandidate::from_decoded(dimensions, pixels, fonts, icons)).map_err(|_| NC_E_INVALID_ARG)
    }

    /// Rejects gaps, aliases, empty names and invalid UTF-8 before building native maps.
    fn read_name(&self, range: AssetRange, next: &mut u32) -> Result<&str, NcStatus> {
        if range.first != *next || range.count == 0 {
            return Err(NC_E_INVALID_ARG);
        }
        *next = next.checked_add(range.count).ok_or(NC_E_INVALID_ARG)?;
        let bytes = self.names.get(range.first as usize..*next as usize).ok_or(NC_E_INVALID_ARG)?;
        std::str::from_utf8(bytes).map_err(|_| NC_E_INVALID_ARG)
    }
}
