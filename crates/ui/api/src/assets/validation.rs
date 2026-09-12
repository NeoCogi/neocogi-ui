//! Bulk resource-table validation independent of UI state and asset construction.
use super::*;
use std::collections::HashSet;
use neocogi_com::NC_E_OUT_OF_MEMORY;

impl AssetData {
    /// Copies and validates one complete immutable resource revision.
    pub fn read(source: ComRef<'_, IAssetBundleAbi>) -> Result<Self, NcStatus> {
        // Construction consumers need both representations. Allocate each from the same checked
        // header, then publish neither until metadata, pixel length and white-tile contents agree.
        let info = source.info()?;
        let tables = AssetTables::read_from(source, info)?;
        let mut pixels = filled(info.rgba_bytes()?, 0)?;
        source.pixels(NcCopyToBufferOffset::new(0, &mut pixels))?;
        let data = Self { pixels: pixels.into(), tables };
        data.validate()?;
        Ok(data)
    }

    /// Checks table semantics, exact pixel storage and the required opaque-white tile.
    pub fn validate(&self) -> Result<(), NcStatus> {
        self.info()?;
        self.tables.validate_contents()?;
        self.tables.validate_white_pixels(&self.pixels)
    }

    /// Resolves a bounded UTF-8 name from this revision's metadata.
    pub fn name(&self, range: AssetRange) -> Result<&str, NcStatus> {
        self.tables.name(range)
    }
}

impl AssetTables {
    /// Copies and validates metadata without allocating or retaining the complete RGBA image.
    pub fn read(source: ComRef<'_, IAssetBundleAbi>) -> Result<Self, NcStatus> {
        // Validate every table before using its white rectangle to address the foreign pixel
        // method. Only those rows are copied, so UI import cost does not scale with empty texture
        // space or with artwork that only the platform uploads.
        let info = source.info()?;
        let tables = Self::read_from(source, info)?;
        tables.validate()?;
        tables.validate_white_source(source)?;
        Ok(tables)
    }

    /// Copies bounded metadata after the common header has fixed every allocation size.
    fn read_from(source: ComRef<'_, IAssetBundleAbi>, info: AssetInfo) -> Result<Self, NcStatus> {
        // Checking the complete header first rejects schema, dimension and count errors before any
        // allocation. Each foreign copy writes an initialized local buffer which remains private
        // until every requested table has been returned successfully.
        info.rgba_bytes()?;
        let mut tables = Self {
            size: info.size,
            fonts: filled(info.fonts as usize, AssetFont::default())?.into(),
            glyphs: filled(info.glyphs as usize, AssetGlyph::default())?.into(),
            icons: filled(info.icons as usize, AssetIcon::default())?.into(),
            names: filled(info.names as usize, 0)?.into(),
            metrics: source.metrics()?,
            chrome: source.chrome()?,
            surfaces: filled(SurfaceRole::COUNT, transparent_surfaces())?.into(),
            controls: filled(ControlRole::COUNT, transparent_controls())?.into(),
            menus: filled(MenuRole::COUNT, transparent_menus())?.into(),
            window_chrome: filled(ChromeRole::COUNT, transparent_chrome())?.into(),
        };
        source.fonts(NcCopyToBufferOffset::new(0, tables.fonts.to_mut()))?;
        source.glyphs(NcCopyToBufferOffset::new(0, tables.glyphs.to_mut()))?;
        source.icons(NcCopyToBufferOffset::new(0, tables.icons.to_mut()))?;
        source.names(NcCopyToBufferOffset::new(0, tables.names.to_mut()))?;
        source.surface_visuals(NcCopyToBufferOffset::new(0, tables.surfaces.to_mut()))?;
        source.control_visuals(NcCopyToBufferOffset::new(0, tables.controls.to_mut()))?;
        source.menu_visuals(NcCopyToBufferOffset::new(0, tables.menus.to_mut()))?;
        source.chrome_visuals(NcCopyToBufferOffset::new(0, tables.window_chrome.to_mut()))?;
        Ok(tables)
    }

    /// Checks every metadata and resolved-appearance invariant without inspecting image bytes.
    pub fn validate(&self) -> Result<(), NcStatus> {
        self.info()?;
        self.validate_contents()
    }

    /// Checks metadata contents after their storage lengths and dimensions are known to be valid.
    fn validate_contents(&self) -> Result<(), NcStatus> {
        // Names and glyphs form exact partitions. Preserve duplicates until this check;
        // silently collecting into maps would discard malformed records.
        let mut next_name = 0;
        let mut next_glyph = 0;
        let mut font_names = HashSet::new();
        for font in self.fonts.iter() {
            let name = self.partitioned_name(font.name, &mut next_name)?;
            if !font_names.insert(name)
                || font.glyphs.first != next_glyph
                || font.line_size <= 0
                || font.font_size <= 0
                || font.baseline < 0
                || font.baseline > font.line_size
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
        }
        let required_icon_names = IconRole::ALL.map(|role| role.canonical_name().expect("IconRole::ALL contains only declared IDL values"));
        for name in core::iter::once("white").chain(required_icon_names) {
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
            || [chrome.close_side, chrome.minimize_side, chrome.maximize_side]
                .iter()
                .any(|side| !matches!(*side, CaptionSide::LEADING | CaptionSide::TRAILING))
            || chrome.show_without_activation > 1
            || chrome.has_backdrop > 1
            || [
                chrome.vertical_spacing,
                chrome.minimum_extent,
                chrome.inner_spacing,
                chrome.minimized_content_height,
                chrome.backdrop_padding,
            ]
            .iter()
            .any(|n| *n < 0)
        {
            return Err(NC_E_INVALID_ARG);
        }
        let surfaces = self.surfaces.iter().flat_map(|values| [&values.normal, &values.disabled]);
        let controls = self.controls.iter().flat_map(|values| {
            [
                &values.disabled,
                &values.enabled.normal,
                &values.enabled.hovered,
                &values.enabled.pressed,
                &values.focused.normal,
                &values.focused.hovered,
                &values.focused.pressed,
            ]
        });
        let menus = self.menus.iter().flat_map(|values| {
            [
                &values.normal,
                &values.hovered,
                &values.pressed,
                &values.focused,
                &values.open,
                &values.disabled,
            ]
        });
        let chrome = self.window_chrome.iter().flat_map(|values| [&values.base, &values.active, &values.disabled]);
        for visual in surfaces.chain(controls).chain(menus).chain(chrome) {
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
        if rect.x < 0
            || rect.y < 0
            || rect.width < minimum
            || rect.height < minimum
            || rect.x.checked_add(rect.width).ok_or(NC_E_INVALID_ARG)? > self.size.width
            || rect.y.checked_add(rect.height).ok_or(NC_E_INVALID_ARG)? > self.size.height
        {
            return Err(NC_E_INVALID_ARG);
        }
        Ok(())
    }

    /// Returns the required white-tile rectangle after metadata validation has proved it exists.
    fn white_rect(&self) -> Rect {
        self.icons
            .iter()
            .find(|icon| self.name(icon.name) == Ok("white"))
            .expect("AssetTables::validate requires the white icon")
            .rect
    }

    /// Checks white-tile channels in a complete, exact-size RGBA allocation.
    fn validate_white_pixels(&self, pixels: &[u8]) -> Result<(), NcStatus> {
        let rect = self.white_rect();
        for y in rect.y..rect.y + rect.height {
            let first = (y as usize * self.size.width as usize + rect.x as usize) * 4;
            let last = first + rect.width as usize * 4;
            if pixels[first..last].iter().any(|channel| *channel != 255) {
                return Err(NC_E_INVALID_ARG);
            }
        }
        Ok(())
    }

    /// Reads only the white tile rows required to authenticate solid-color rendering.
    fn validate_white_source(&self, source: ComRef<'_, IAssetBundleAbi>) -> Result<(), NcStatus> {
        let rect = self.white_rect();
        let row_bytes = rect.width as usize * 4;
        let mut row = filled(row_bytes, 0)?;
        for y in rect.y..rect.y + rect.height {
            let first = (y as usize * self.size.width as usize + rect.x as usize) * 4;
            source.pixels(NcCopyToBufferOffset::new(first as u64, &mut row))?;
            if row.iter().any(|channel| *channel != 255) {
                return Err(NC_E_INVALID_ARG);
            }
        }
        Ok(())
    }
}

/// Rejects negative scalar layout values before a producer creates a derived style.
pub fn validate_metrics(metrics: AssetMetrics) -> Result<(), NcStatus> {
    validate_insets(metrics.window_content_insets)?;
    validate_insets(metrics.window_border)?;
    if [
        metrics.default_cell_width,
        metrics.padding,
        metrics.spacing,
        metrics.indent,
        metrics.title_height,
        metrics.scrollbar_size,
        metrics.thumb_size,
    ]
    .iter()
    .any(|n| *n < 0)
    {
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
        insets: Insets::default(),
        center_visible: 0,
        content_color: 0,
        patch: AssetPatch::Flat {
            top_left: 0,
            top: 0,
            top_right: 0,
            left: 0,
            center: 0,
            right: 0,
            bottom_left: 0,
            bottom: 0,
            bottom_right: 0,
        }
        .into(),
    }
}

/// Initializes both explicit surface branches with a valid transparent visual.
fn transparent_surfaces() -> AssetSurfaceVisuals {
    AssetSurfaceVisuals {
        normal: transparent(),
        disabled: transparent(),
    }
}

/// Initializes the three pointer branches used by enabled and focused controls.
fn transparent_pointer() -> AssetPointerVisuals {
    AssetPointerVisuals {
        normal: transparent(),
        hovered: transparent(),
        pressed: transparent(),
    }
}

/// Initializes the complete nested control state record without flattening it.
fn transparent_controls() -> AssetControlVisuals {
    AssetControlVisuals {
        disabled: transparent(),
        enabled: transparent_pointer(),
        focused: transparent_pointer(),
    }
}

/// Initializes every named menu branch with a valid transparent visual.
fn transparent_menus() -> AssetMenuVisuals {
    AssetMenuVisuals {
        normal: transparent(),
        hovered: transparent(),
        pressed: transparent(),
        focused: transparent(),
        open: transparent(),
        disabled: transparent(),
    }
}

/// Initializes every named chrome branch with a valid transparent visual.
fn transparent_chrome() -> AssetChromeVisuals {
    AssetChromeVisuals {
        base: transparent(),
        active: transparent(),
        disabled: transparent(),
    }
}
