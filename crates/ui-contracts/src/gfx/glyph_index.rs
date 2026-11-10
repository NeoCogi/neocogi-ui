//! Bounded immutable ASCII acceleration; canonical tables own all Unicode glyph semantics.
use crate::gfx::AssetTables;
use neocogi_com::{NcStatus, NC_E_INVALID_ARG, NC_E_OUT_OF_MEMORY};
use neocogi_ui_contracts::gfx::abi::MAX_ASSETS;

/// One 512-byte index per font revision, with no global cache or mutable lookup state.
/// Non-ASCII scalars retain canonical binary search; absence remains distinct from fallback.
#[derive(Clone)]
pub struct AsciiGlyphIndex {
    /// Absolute canonical glyph slots; u32::MAX is outside the bounded glyph-table domain.
    slots: [u32; 128],
}
impl AsciiGlyphIndex {
    /// Builds bounded indices from already validated, uniquely partitioned canonical tables.
    /// Checks range arithmetic and allocation admission before publishing the complete vector.
    pub fn build(tables: &AssetTables) -> Result<Vec<Self>, NcStatus> {
        tables
            .fonts
            .len()
            .checked_add(tables.icons.len())
            .filter(|&count| count <= MAX_ASSETS as usize)
            .ok_or(NC_E_INVALID_ARG)?;
        let mut indices = Vec::new();
        indices.try_reserve_exact(tables.fonts.len()).map_err(|_| NC_E_OUT_OF_MEMORY)?;
        for font in tables.fonts.iter() {
            let end = font.glyphs.first.checked_add(font.glyphs.count).ok_or(NC_E_INVALID_ARG)?;
            let glyphs = tables.glyphs.get(font.glyphs.first as usize..end as usize).ok_or(NC_E_INVALID_ARG)?;
            let mut index = Self { slots: [u32::MAX; 128] };
            for (offset, glyph) in glyphs.iter().enumerate() {
                if glyph.scalar < 128 {
                    index.slots[glyph.scalar as usize] = font.glyphs.first + offset as u32;
                }
            }
            indices.push(index);
        }
        Ok(indices)
    }
    /// Returns an exact ASCII slot or absence, without applying a font's fallback policy.
    pub fn get(&self, scalar: u32) -> Option<usize> {
        self.slots
            .get(scalar as usize)
            .copied()
            .filter(|slot| *slot != u32::MAX)
            .map(|slot| slot as usize)
    }
}
