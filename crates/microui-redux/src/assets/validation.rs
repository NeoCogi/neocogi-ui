//! Bulk resource-table validation independent of UI state and asset construction.
use super::*;
use std::collections::HashSet;
use nc_com::NC_E_OUT_OF_MEMORY;

impl AssetData {
    /// Copies and validates one immutable resource revision without constructing UI objects.
    pub fn read(source: ComRef<'_, IAssetBundleAbi>) -> Result<Self, NcStatus> {
        // Read allocation bounds first. Each foreign copy receives an initialized local
        // buffer, and no partially read candidate becomes visible to another consumer.
        let info = source.info()?;
        let mut data = Self {
            size: info.size,
            pixels: filled(info.rgba_bytes()?, 0)?.into(),
            fonts: filled(info.fonts as usize, AssetFont::default())?.into(),
            glyphs: filled(info.glyphs as usize, AssetGlyph::default())?.into(),
            icons: filled(info.icons as usize, AssetIcon::default())?.into(),
            names: filled(info.names as usize, 0)?.into(),
            metrics: source.metrics()?,
            chrome: source.chrome()?,
            surfaces: filled(SurfaceRole::COUNT * SurfaceState::COUNT, transparent())?.into(),
            controls: filled(ControlRole::COUNT * ControlState::COUNT, transparent())?.into(),
            menus: filled(MenuRole::COUNT * MenuState::COUNT, transparent())?.into(),
            window_chrome: filled(ChromeRole::COUNT * ChromeState::COUNT, transparent())?.into(),
        };
        source.pixels(NcCopyToBufferOffset::new(0, data.pixels.to_mut()))?;
        source.fonts(NcCopyToBufferOffset::new(0, data.fonts.to_mut()))?;
        source.glyphs(NcCopyToBufferOffset::new(0, data.glyphs.to_mut()))?;
        source.icons(NcCopyToBufferOffset::new(0, data.icons.to_mut()))?;
        source.names(NcCopyToBufferOffset::new(0, data.names.to_mut()))?;
        source.visuals(AppearanceFamily::SURFACE, NcCopyToBufferOffset::new(0, data.surfaces.to_mut()))?;
        source.visuals(AppearanceFamily::CONTROL, NcCopyToBufferOffset::new(0, data.controls.to_mut()))?;
        source.visuals(AppearanceFamily::MENU, NcCopyToBufferOffset::new(0, data.menus.to_mut()))?;
        source.visuals(AppearanceFamily::CHROME, NcCopyToBufferOffset::new(0, data.window_chrome.to_mut()))?;
        data.validate()?;
        Ok(data)
    }

    /// Checks all portable resource invariants before packing, installation or publication.
    pub fn validate(&self) -> Result<(), NcStatus> {
        self.info()?;

        // Names and glyphs form exact partitions. Preserve duplicates until this check;
        // silently collecting into maps would discard malformed records.
        let mut next_name = 0;
        let mut next_glyph = 0;
        let mut font_names = HashSet::new();
        for font in self.fonts.iter() {
            let name = self.partitioned_name(font.name, &mut next_name)?;
            if !font_names.insert(name) || font.glyphs.first != next_glyph
                || font.line_size <= 0 || font.font_size <= 0
                || font.baseline < 0 || font.baseline > font.line_size
            {
                return Err(NC_E_INVALID_ARG);
            }
            next_glyph = next_glyph.checked_add(font.glyphs.count).ok_or(NC_E_INVALID_ARG)?;
            let entries = self.glyphs.get(font.glyphs.first as usize..next_glyph as usize).ok_or(NC_E_INVALID_ARG)?;
            let mut scalars = HashSet::new();
            for glyph in entries {
                if char::from_u32(glyph.scalar).is_none() || !scalars.insert(glyph.scalar) {
                    return Err(NC_E_INVALID_ARG);
                }
                self.validate_rect(glyph.rect, true)?;
            }
            if !scalars.contains(&u32::from('_')) {
                return Err(NC_E_INVALID_ARG);
            }
        }
        if next_glyph as usize != self.glyphs.len() || !font_names.contains("body") {
            return Err(NC_E_INVALID_ARG);
        }
        let mut icon_names = HashSet::new();
        for icon in self.icons.iter() {
            let name = self.partitioned_name(icon.name, &mut next_name)?;
            if !icon_names.insert(name) {
                return Err(NC_E_INVALID_ARG);
            }
            self.validate_rect(icon.rect, false)?;
            if name == "white" {
                // Rectangle validation proved every row is in the exact RGBA buffer.
                for y in icon.rect.y..icon.rect.y + icon.rect.height {
                    let start = (y as usize * self.size.width as usize + icon.rect.x as usize) * 4;
                    let end = start + icon.rect.width as usize * 4;
                    if self.pixels[start..end].iter().any(|channel| *channel != 255) {
                        return Err(NC_E_INVALID_ARG);
                    }
                }
            }
        }
        for name in ["white", "close", "expand", "collapse", "check", "expand_down", "open_folder", "closed_folder", "file"] {
            if !icon_names.contains(name) {
                return Err(NC_E_INVALID_ARG);
            }
        }
        if next_name as usize != self.names.len() {
            return Err(NC_E_INVALID_ARG);
        }
        validate_metrics(self.metrics)?;
        let chrome = self.chrome;
        if !matches!(chrome.title_alignment, TitleAlignment::LEADING | TitleAlignment::CENTERED)
            || [chrome.close_side, chrome.minimize_side, chrome.maximize_side].iter().any(|side| !matches!(*side, CaptionSide::LEADING | CaptionSide::TRAILING))
            || chrome.show_without_activation > 1 || chrome.has_backdrop > 1
            || [chrome.vertical_spacing, chrome.minimum_extent, chrome.inner_spacing, chrome.minimized_content_height, chrome.backdrop_padding].iter().any(|n| *n < 0)
        {
            return Err(NC_E_INVALID_ARG);
        }
        for visual in self.surfaces.iter().chain(self.controls.iter()).chain(self.menus.iter()).chain(self.window_chrome.iter()) {
            validate_insets(visual.insets)?;
            if visual.center_visible > 1 {
                return Err(NC_E_INVALID_ARG);
            }
            if let AssetPatch::Image { icon, source, .. } = AssetPatch::try_from(visual.patch).map_err(|_| NC_E_INVALID_ARG)? {
                validate_insets(source)?;
                let rect = self.icons.get(icon as usize).ok_or(NC_E_INVALID_ARG)?.rect;
                if source.left.checked_add(source.right).ok_or(NC_E_INVALID_ARG)? > rect.width
                    || source.top.checked_add(source.bottom).ok_or(NC_E_INVALID_ARG)? > rect.height
                {
                    return Err(NC_E_INVALID_ARG);
                }
            }
        }
        Ok(())
    }

    /// Resolves a bounded UTF-8 name without inventing a second resource namespace.
    pub fn name(&self, range: AssetRange) -> Result<&str, NcStatus> {
        let end = range.first.checked_add(range.count).ok_or(NC_E_INVALID_ARG)?;
        let bytes = self.names.get(range.first as usize..end as usize).ok_or(NC_E_INVALID_ARG)?;
        if bytes.is_empty() {
            return Err(NC_E_INVALID_ARG);
        }
        std::str::from_utf8(bytes).map_err(|_| NC_E_INVALID_ARG)
    }

    /// Requires the next resource name to start exactly where its predecessor ended.
    fn partitioned_name(&self, range: AssetRange, next: &mut u32) -> Result<&str, NcStatus> {
        if range.first != *next {
            return Err(NC_E_INVALID_ARG);
        }
        let name = self.name(range)?;
        *next = range.first.checked_add(range.count).ok_or(NC_E_INVALID_ARG)?;
        Ok(name)
    }

    /// Checks source rectangles with empty glyphs allowed and positive icons required.
    fn validate_rect(&self, rect: Rect, allow_empty: bool) -> Result<(), NcStatus> {
        let minimum = i32::from(!allow_empty);
        if rect.x < 0 || rect.y < 0 || rect.width < minimum || rect.height < minimum
            || rect.x.checked_add(rect.width).ok_or(NC_E_INVALID_ARG)? > self.size.width
            || rect.y.checked_add(rect.height).ok_or(NC_E_INVALID_ARG)? > self.size.height
        {
            return Err(NC_E_INVALID_ARG);
        }
        Ok(())
    }
}

/// Rejects negative scalar layout values before a producer creates a derived style.
pub fn validate_metrics(metrics: AssetMetrics) -> Result<(), NcStatus> {
    validate_insets(metrics.window_content_insets)?;
    validate_insets(metrics.window_border)?;
    if [metrics.default_cell_width, metrics.padding, metrics.spacing, metrics.indent, metrics.title_height, metrics.scrollbar_size, metrics.thumb_size].iter().any(|n| *n < 0) {
        return Err(NC_E_INVALID_ARG);
    }
    Ok(())
}

/// Applies the same nonnegative slicing domain to layout and image-source insets.
fn validate_insets(insets: Insets) -> Result<(), NcStatus> {
    if [insets.left, insets.top, insets.right, insets.bottom].iter().any(|n| *n < 0) {
        return Err(NC_E_INVALID_ARG);
    }
    Ok(())
}

/// Allocates initialized output while reporting allocation failure through the COM status.
fn filled<T: Clone>(count: usize, value: T) -> Result<Vec<T>, NcStatus> {
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|_| NC_E_OUT_OF_MEMORY)?;
    values.resize(count, value);
    Ok(values)
}

/// Initializes a tagged output slot with a valid, fully transparent visual.
fn transparent() -> AssetVisual {
    AssetVisual {
        insets: Insets::default(), center_visible: 0, content_color: 0,
        patch: AssetPatch::Flat { top_left: 0, top: 0, top_right: 0, left: 0, center: 0, right: 0, bottom_left: 0, bottom: 0, bottom_right: 0 }.into(),
    }
}
