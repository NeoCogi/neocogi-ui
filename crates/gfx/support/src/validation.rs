//! Bulk GFX resource validation independent of UI state and resource construction.
use super::*;
use neocogi_com::NC_E_OUT_OF_MEMORY;

impl AssetData {
    /// Copies and validates one complete immutable resource revision.
    pub fn read(source: ComRef<'_, IAssetBundleAbi>) -> Result<Self, NcStatus> {
        // Construction consumers need both representations. Allocate each from the same checked
        // header, then publish neither until metadata, pixel length and white-tile contents agree.
        let info = source.info()?;
        let tables = AssetTables::read_from(source, info)?;
        let count = info.rgba_bytes()?;
        let image = source.image()?;
        if image.size()? != info.size {
            return Err(NC_E_INVALID_ARG);
        }
        let pixels = read_output(count, |output| image.pixels(output))?;
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
        tables.validate_white_source(source.image()?.as_com_ref())?;
        Ok(tables)
    }

    /// Copies bounded metadata after the common header has fixed every allocation size.
    fn read_from(source: ComRef<'_, IAssetBundleAbi>, info: AssetInfo) -> Result<Self, NcStatus> {
        // Checking the complete header first rejects schema, dimension and count errors before any
        // allocation. Each foreign copy produces a fresh checked prefix which remains private
        // until every requested table has been returned successfully.
        info.rgba_bytes()?;
        let tables = Self {
            size: info.size,
            fonts: read_output(info.fonts as usize, |output| source.fonts(output))?.into(),
            glyphs: read_output(info.glyphs as usize, |output| source.glyphs(output))?.into(),
            icons: read_output(info.icons as usize, |output| source.icons(output))?.into(),
            names: read_output(info.names as usize, |output| source.names(output))?.into(),
            metrics: source.metrics()?,
            chrome: source.chrome()?,
            surfaces: read_output(SurfaceRole::COUNT, |output| source.surface_visuals(output))?.into(),
            controls: read_output(ControlRole::COUNT, |output| source.control_visuals(output))?.into(),
            menus: read_output(MenuRole::COUNT, |output| source.menu_visuals(output))?.into(),
            window_chrome: read_output(ChromeRole::COUNT, |output| source.chrome_visuals(output))?.into(),
        };
        Ok(tables)
    }

    /// Checks every metadata and resolved-appearance invariant without inspecting image bytes.
    pub fn validate(&self) -> Result<(), NcStatus> {
        self.info()?;
        self.validate_contents()
    }

    /// Checks metadata contents after their storage lengths and dimensions are known to be valid.
    fn validate_contents(&self) -> Result<(), NcStatus> {
        // Names and glyphs form exact partitions. Temporary sorted vectors make duplicate
        // detection deterministic, avoid randomized hashing, and reuse sorting machinery already
        // needed by bundle consumers.
        let mut next_name = 0;
        let mut next_glyph = 0;
        let mut font_names = Vec::new();
        font_names.try_reserve_exact(self.fonts.len()).map_err(|_| NC_E_OUT_OF_MEMORY)?;
        for font in self.fonts.iter() {
            let name = self.partitioned_name(font.name, &mut next_name)?;
            font_names.push(name);
            if font.glyphs.first != next_glyph || font.line_size <= 0 || font.font_size <= 0 || font.baseline < 0 || font.baseline > font.line_size {
                return Err(NC_E_INVALID_ARG);
            }
            next_glyph = next_glyph.checked_add(font.glyphs.count).ok_or(NC_E_INVALID_ARG)?;
            let entries = self.glyphs.get(font.glyphs.first as usize..next_glyph as usize).ok_or(NC_E_INVALID_ARG)?;
            let mut scalars = Vec::new();
            scalars.try_reserve_exact(entries.len()).map_err(|_| NC_E_OUT_OF_MEMORY)?;
            for glyph in entries {
                if char::from_u32(glyph.scalar).is_none() {
                    return Err(NC_E_INVALID_ARG);
                }
                scalars.push(glyph.scalar);
                self.validate_rect(glyph.rect, true)?;
            }
            scalars.sort_unstable();
            if has_adjacent_duplicates(&scalars) || scalars.binary_search(&u32::from('_')).is_err() {
                return Err(NC_E_INVALID_ARG);
            }
        }
        font_names.sort_unstable();
        if next_glyph as usize != self.glyphs.len() || has_adjacent_duplicates(&font_names) || font_names.binary_search(&"body").is_err() {
            return Err(NC_E_INVALID_ARG);
        }
        let mut icon_names = Vec::new();
        icon_names.try_reserve_exact(self.icons.len()).map_err(|_| NC_E_OUT_OF_MEMORY)?;
        for icon in self.icons.iter() {
            let name = self.partitioned_name(icon.name, &mut next_name)?;
            icon_names.push(name);
            self.validate_rect(icon.rect, false)?;
        }
        icon_names.sort_unstable();
        if has_adjacent_duplicates(&icon_names) {
            return Err(NC_E_INVALID_ARG);
        }
        let required_icon_names = IconRole::ALL.map(|role| role.canonical_name().expect("IconRole::ALL contains only declared IDL values"));
        for name in core::iter::once("white").chain(required_icon_names) {
            if icon_names.binary_search(&name).is_err() {
                return Err(NC_E_INVALID_ARG);
            }
        }
        if next_name as usize != self.names.len() {
            return Err(NC_E_INVALID_ARG);
        }
        validate_metrics(self.metrics)?;
        let chrome = self.chrome;
        if !matches!(chrome.title_alignment, TitleAlignment::Leading | TitleAlignment::Centered)
            || [chrome.close_side, chrome.minimize_side, chrome.maximize_side]
                .iter()
                .any(|side| !matches!(*side, CaptionSide::Leading | CaptionSide::Trailing))
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
            if let AssetPatch::Image { icon, source, .. } = visual.patch {
                validate_insets(source)?;
                self.icons.get(icon as usize).ok_or(NC_E_INVALID_ARG)?;
                // Retain oversized slices as authored. GFX shrinks them proportionally and
                // overlays pink; stored data follows the same rule as direct drawing input.
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
    ///
    /// Compositors use the center of this authenticated source for solid-color geometry. Exposing
    /// the copied rectangle keeps that policy next to bundle validation without exposing pixel
    /// storage or requiring a second name index in each consumer.
    pub fn white_rect(&self) -> Rect {
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
    pub(super) fn validate_white_source(&self, image: ComRef<'_, IRgbaImageAbi>) -> Result<(), NcStatus> {
        if image.size()? != self.size {
            return Err(NC_E_INVALID_ARG);
        }
        let rect = self.white_rect();
        let row_bytes = rect.width as usize * 4;
        let mut row = neocogi_com::NcOutputBuffer::<u8>::new();
        for y in rect.y..rect.y + rect.height {
            let first = (y as usize * self.size.width as usize + rect.x as usize) * 4;
            image.pixels(row.output(first as u64, row_bytes))?;
            if row.len() != row_bytes || row.as_slice().iter().any(|channel| *channel != 255) {
                return Err(NC_E_INVALID_ARG);
            }
        }
        Ok(())
    }
}

/// Reports duplicate neighbors after a caller establishes deterministic sorted order.
fn has_adjacent_duplicates<T: PartialEq>(values: &[T]) -> bool {
    values.windows(2).any(|pair| pair[0] == pair[1])
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



/// Acquires an exact fresh table after its header bounded the requested capacity.
/// A short successful foreign result is a protocol error, never default-filled data.
fn read_output<T: neocogi_com::ComInlineValue>(count: usize, copy: impl FnOnce(neocogi_com::NcOutput<'_, T>) -> Result<(), NcStatus>) -> Result<Vec<T>, NcStatus> {
    let mut output = neocogi_com::NcOutputBuffer::new();
    copy(output.output(0, count))?;
    if output.len() != count { return Err(NC_E_INVALID_ARG); }
    Ok(output.into_vec())
}
