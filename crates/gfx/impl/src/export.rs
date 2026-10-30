//! Build-time exporter for the ordinary immutable COM bundle implementation.
//! Generated code uses GFX support and COM; font, PNG and JSON construction stay outside consumers.
use neocogi_gfx_support::abi::*;
use neocogi_gfx_support::AssetData;
use std::{
    io::{self, Write as _},
    path::Path,
};

/// Atomically writes self-contained Rust `load_bundle` and `bundle_data` functions.
/// Generated code requires GFX support `compiled-assets`: metadata stays static and pixels expand
/// once into bounded owned storage. Fonts, PNG files and JSON are processed only by the exporter.
/// A sibling temporary file keeps the previous output intact until the complete replacement is flushed.
pub fn write_rust(data: &AssetData, destination: &Path) -> io::Result<()> {
    // Reject invalid tables before touching the destination or encoding tagged records.
    data.validate().map_err(|status| io::Error::other(status.to_string()))?;
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut rust = io::BufWriter::new(tempfile::NamedTempFile::new_in(parent)?);
    rust.write_all(b"// Generated immutable GFX bundle; regenerate from the owning asset recipe.\nuse neocogi_gfx_support::abi::*;\nuse neocogi_gfx_support::{AssetData, AssetTables};\nuse std::borrow::Cow;\n\n/// Publishes one compiled resource catalog without parsing fonts, PNG or JSON at runtime.\npub fn load_bundle() -> Result<neocogi_com::ComPtr<IAssetBundleAbi>, neocogi_com::NcStatus> {\n    bundle_data()?.into_bundle()\n}\n\n/// Returns compiled tables for a fresh bundle or a revision of an existing catalog.\npub fn bundle_data() -> Result<AssetData, neocogi_com::NcStatus> {\n    Ok(AssetData {\n")?;
    // Program data is emitted structurally: target profiles may omit diagnostic Debug.
    let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&data.pixels, 9);
    rust.write_all(b"        pixels: Cow::Owned(neocogi_gfx_support::decode_pixels(&")?;
    write_bytes(&mut rust, &compressed)?;
    writeln!(rust, ", {})?),", data.pixels.len())?;
    rust.write_all(b"        tables: AssetTables { size: ")?;
    write_size(&mut rust, data.tables.size)?;
    rust.write_all(b", names: Cow::Borrowed(&")?;
    write_bytes(&mut rust, &data.tables.names)?;
    rust.write_all(b"), fonts: Cow::Borrowed(&[")?;
    for value in data.tables.fonts.iter() {
        write_font(&mut rust, *value)?;
        rust.write_all(b",")?;
    }
    rust.write_all(b"]), glyphs: Cow::Borrowed(&[")?;
    for value in data.tables.glyphs.iter() {
        write_glyph(&mut rust, *value)?;
        rust.write_all(b",")?;
    }
    rust.write_all(b"]), icons: Cow::Borrowed(&[")?;
    for value in data.tables.icons.iter() {
        write_icon(&mut rust, *value)?;
        rust.write_all(b",")?;
    }
    rust.write_all(b"]), metrics: ")?;
    write_metrics(&mut rust, data.tables.metrics)?;
    rust.write_all(b",\n")?;
    let c = data.tables.chrome;
    // Open integer enums have explicit constructors; formatting their Debug output would
    // accidentally couple generated source to private wrapper fields.
    writeln!(
        rust,
        "            chrome: AssetChrome {{ title_alignment: TitleAlignment::{}, close_side: CaptionSide::{}, minimize_side: CaptionSide::{}, maximize_side: CaptionSide::{}, vertical_spacing: {}, minimum_extent: {}, inner_spacing: {}, show_without_activation: {}, minimized_content_height: {}, has_backdrop: {}, backdrop_rgba: {}, backdrop_padding: {} }},",
        if c.title_alignment == TitleAlignment::Leading {
            "Leading"
        } else {
            "Centered"
        },
        side(c.close_side),
        side(c.minimize_side),
        side(c.maximize_side),
        c.vertical_spacing,
        c.minimum_extent,
        c.inner_spacing,
        c.show_without_activation,
        c.minimized_content_height,
        c.has_backdrop,
        c.backdrop_rgba,
        c.backdrop_padding
    )?;
    rust.write_all(b"            surfaces: Cow::Owned(vec![")?;
    for value in data.tables.surfaces.iter() {
        write_surface(&mut rust, *value)?;
        rust.write_all(b",")?;
    }
    rust.write_all(b"]),\n")?;
    rust.write_all(b"            controls: Cow::Owned(vec![")?;
    for value in data.tables.controls.iter() {
        write_control(&mut rust, *value)?;
        rust.write_all(b",")?;
    }
    rust.write_all(b"]),\n")?;
    rust.write_all(b"            menus: Cow::Owned(vec![")?;
    for value in data.tables.menus.iter() {
        write_menu(&mut rust, *value)?;
        rust.write_all(b",")?;
    }
    rust.write_all(b"]),\n")?;
    rust.write_all(b"            window_chrome: Cow::Owned(vec![")?;
    for value in data.tables.window_chrome.iter() {
        write_chrome_visuals(&mut rust, *value)?;
        rust.write_all(b",")?;
    }
    rust.write_all(b"]),\n")?;
    rust.write_all(b"        },\n    })\n}\n")?;
    // Atomic replacement publishes exactly one self-contained source artifact. Failed writes
    // leave both the previous file and every future build's visible input unchanged.
    rust.flush()?;
    rust.get_ref().as_file().sync_all()?;
    let temporary = rust.into_inner().map_err(|error| error.into_error())?;
    temporary.persist(destination).map(|_| ()).map_err(|error| error.error)
}

/// Returns a source-level constant name for one previously validated caption edge.
fn side(value: CaptionSide) -> &'static str {
    if value == CaptionSide::Leading { "Leading" } else { "Trailing" }
}

/// Encodes initialized bytes numerically, independent of diagnostic formatting.
fn write_bytes(out: &mut impl io::Write, bytes: &[u8]) -> io::Result<()> {
    out.write_all(b"[")?;
    for byte in bytes {
        write!(out, "{},", byte)?;
    }
    out.write_all(b"]")
}

/// Writes every field of a validated Size as executable Rust syntax.
fn write_size(out: &mut impl io::Write, value: Size) -> io::Result<()> {
    out.write_all(b"Size { ")?;
    write!(out, "width: {},", value.width)?;
    write!(out, "height: {},", value.height)?;
    out.write_all(b"}")
}

/// Writes every field of a validated Rect as executable Rust syntax.
fn write_rect(out: &mut impl io::Write, value: Rect) -> io::Result<()> {
    out.write_all(b"Rect { ")?;
    write!(out, "x: {},", value.x)?;
    write!(out, "y: {},", value.y)?;
    write!(out, "width: {},", value.width)?;
    write!(out, "height: {},", value.height)?;
    out.write_all(b"}")
}

/// Writes every field of a validated AssetRange as executable Rust syntax.
fn write_range(out: &mut impl io::Write, value: AssetRange) -> io::Result<()> {
    out.write_all(b"AssetRange { ")?;
    write!(out, "first: {},", value.first)?;
    write!(out, "count: {},", value.count)?;
    out.write_all(b"}")
}

/// Writes every field of a validated Insets as executable Rust syntax.
fn write_insets(out: &mut impl io::Write, value: Insets) -> io::Result<()> {
    out.write_all(b"Insets { ")?;
    write!(out, "left: {},", value.left)?;
    write!(out, "top: {},", value.top)?;
    write!(out, "right: {},", value.right)?;
    write!(out, "bottom: {},", value.bottom)?;
    out.write_all(b"}")
}

/// Writes every field of a validated AssetFont as executable Rust syntax.
fn write_font(out: &mut impl io::Write, value: AssetFont) -> io::Result<()> {
    out.write_all(b"AssetFont { ")?;
    out.write_all(b"name: ")?;
    write_range(out, value.name)?;
    out.write_all(b",")?;
    out.write_all(b"glyphs: ")?;
    write_range(out, value.glyphs)?;
    out.write_all(b",")?;
    write!(out, "line_size: {},", value.line_size)?;
    write!(out, "baseline: {},", value.baseline)?;
    write!(out, "font_size: {},", value.font_size)?;
    out.write_all(b"}")
}

/// Writes every field of a validated AssetGlyph as executable Rust syntax.
fn write_glyph(out: &mut impl io::Write, value: AssetGlyph) -> io::Result<()> {
    out.write_all(b"AssetGlyph { ")?;
    write!(out, "scalar: {},", value.scalar)?;
    write!(out, "offset_x: {},", value.offset_x)?;
    write!(out, "offset_y: {},", value.offset_y)?;
    write!(out, "advance_x: {},", value.advance_x)?;
    write!(out, "advance_y: {},", value.advance_y)?;
    out.write_all(b"rect: ")?;
    write_rect(out, value.rect)?;
    out.write_all(b",")?;
    out.write_all(b"}")
}

/// Writes every field of a validated AssetIcon as executable Rust syntax.
fn write_icon(out: &mut impl io::Write, value: AssetIcon) -> io::Result<()> {
    out.write_all(b"AssetIcon { ")?;
    out.write_all(b"name: ")?;
    write_range(out, value.name)?;
    out.write_all(b",")?;
    out.write_all(b"rect: ")?;
    write_rect(out, value.rect)?;
    out.write_all(b",")?;
    out.write_all(b"}")
}

/// Writes every field of a validated AssetMetrics as executable Rust syntax.
fn write_metrics(out: &mut impl io::Write, value: AssetMetrics) -> io::Result<()> {
    out.write_all(b"AssetMetrics { ")?;
    write!(out, "default_cell_width: {},", value.default_cell_width)?;
    write!(out, "padding: {},", value.padding)?;
    out.write_all(b"window_content_insets: ")?;
    write_insets(out, value.window_content_insets)?;
    out.write_all(b",")?;
    write!(out, "spacing: {},", value.spacing)?;
    write!(out, "indent: {},", value.indent)?;
    write!(out, "title_height: {},", value.title_height)?;
    out.write_all(b"window_border: ")?;
    write_insets(out, value.window_border)?;
    out.write_all(b",")?;
    write!(out, "scrollbar_size: {},", value.scrollbar_size)?;
    write!(out, "thumb_size: {},", value.thumb_size)?;
    out.write_all(b"}")
}

/// Writes every field of a validated AssetVisual as executable Rust syntax.
fn write_visual(out: &mut impl io::Write, value: AssetVisual) -> io::Result<()> {
    out.write_all(b"AssetVisual { ")?;
    out.write_all(b"insets: ")?;
    write_insets(out, value.insets)?;
    out.write_all(b",")?;
    write!(out, "center_visible: {},", value.center_visible)?;
    write!(out, "content_color: {},", value.content_color)?;
    out.write_all(b"patch: ")?;
    write_patch(out, value.patch)?;
    out.write_all(b",")?;
    out.write_all(b"}")
}

/// Writes every field of a validated AssetPointerVisuals as executable Rust syntax.
fn write_pointer(out: &mut impl io::Write, value: AssetPointerVisuals) -> io::Result<()> {
    out.write_all(b"AssetPointerVisuals { ")?;
    out.write_all(b"normal: ")?;
    write_visual(out, value.normal)?;
    out.write_all(b",")?;
    out.write_all(b"hovered: ")?;
    write_visual(out, value.hovered)?;
    out.write_all(b",")?;
    out.write_all(b"pressed: ")?;
    write_visual(out, value.pressed)?;
    out.write_all(b",")?;
    out.write_all(b"}")
}

/// Writes every field of a validated AssetSurfaceVisuals as executable Rust syntax.
fn write_surface(out: &mut impl io::Write, value: AssetSurfaceVisuals) -> io::Result<()> {
    out.write_all(b"AssetSurfaceVisuals { ")?;
    out.write_all(b"normal: ")?;
    write_visual(out, value.normal)?;
    out.write_all(b",")?;
    out.write_all(b"disabled: ")?;
    write_visual(out, value.disabled)?;
    out.write_all(b",")?;
    out.write_all(b"}")
}

/// Writes every field of a validated AssetControlVisuals as executable Rust syntax.
fn write_control(out: &mut impl io::Write, value: AssetControlVisuals) -> io::Result<()> {
    out.write_all(b"AssetControlVisuals { ")?;
    out.write_all(b"disabled: ")?;
    write_visual(out, value.disabled)?;
    out.write_all(b",")?;
    out.write_all(b"enabled: ")?;
    write_pointer(out, value.enabled)?;
    out.write_all(b",")?;
    out.write_all(b"focused: ")?;
    write_pointer(out, value.focused)?;
    out.write_all(b",")?;
    out.write_all(b"}")
}

/// Writes every field of a validated AssetMenuVisuals as executable Rust syntax.
fn write_menu(out: &mut impl io::Write, value: AssetMenuVisuals) -> io::Result<()> {
    out.write_all(b"AssetMenuVisuals { ")?;
    out.write_all(b"normal: ")?;
    write_visual(out, value.normal)?;
    out.write_all(b",")?;
    out.write_all(b"hovered: ")?;
    write_visual(out, value.hovered)?;
    out.write_all(b",")?;
    out.write_all(b"pressed: ")?;
    write_visual(out, value.pressed)?;
    out.write_all(b",")?;
    out.write_all(b"focused: ")?;
    write_visual(out, value.focused)?;
    out.write_all(b",")?;
    out.write_all(b"open: ")?;
    write_visual(out, value.open)?;
    out.write_all(b",")?;
    out.write_all(b"disabled: ")?;
    write_visual(out, value.disabled)?;
    out.write_all(b",")?;
    out.write_all(b"}")
}

/// Writes every field of a validated AssetChromeVisuals as executable Rust syntax.
fn write_chrome_visuals(out: &mut impl io::Write, value: AssetChromeVisuals) -> io::Result<()> {
    out.write_all(b"AssetChromeVisuals { ")?;
    out.write_all(b"base: ")?;
    write_visual(out, value.base)?;
    out.write_all(b",")?;
    out.write_all(b"active: ")?;
    write_visual(out, value.active)?;
    out.write_all(b",")?;
    out.write_all(b"disabled: ")?;
    write_visual(out, value.disabled)?;
    out.write_all(b",")?;
    out.write_all(b"}")
}

/// Tagged payload syntax is explicit; no Debug or opaque storage representation participates.
fn write_patch(out: &mut impl io::Write, value: AssetPatch) -> io::Result<()> {
    match value {
        AssetPatch::Flat {
            top_left,
            top,
            top_right,
            left,
            center,
            right,
            bottom_left,
            bottom,
            bottom_right,
        } => {
            write!(
                out,
                "AssetPatch::Flat {{ top_left: {}, top: {}, top_right: {}, left: {}, center: {}, right: {}, bottom_left: {}, bottom: {}, bottom_right: {} }}",
                top_left, top, top_right, left, center, right, bottom_left, bottom, bottom_right
            )
        }
        AssetPatch::Image { icon, source, tint } => {
            write!(out, "AssetPatch::Image {{ icon: {}, source: ", icon)?;
            write_insets(out, source)?;
            write!(out, ", tint: {} }}", tint)
        }
    }
}

#[cfg(test)]
#[path = "../tests/support/export_data.rs"]
mod fixture;

#[cfg(test)]
mod tests {
    use super::*;

    /// A failed admission leaves the existing source and directory publication unchanged.
    #[test]
    fn rejected_data_preserves_the_previous_atomic_export() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("bundle.rs");
        let data = fixture::data();
        data.validate().unwrap();
        write_rust(&data, &path).unwrap();
        let before = std::fs::read(&path).unwrap();
        let mut rejected = data.clone();
        rejected.tables.metrics.padding = -1;
        assert!(write_rust(&rejected, &path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
        let mut changed = data.clone();
        changed.tables.glyphs.to_mut()[0].advance_y -= 1;
        changed.validate().unwrap();
        assert_ne!(
            fixture::snapshot(&data),
            fixture::snapshot(&changed),
            "comparison detects a valid but different glyph field"
        );
    }

    /// Partial write failures propagate through nested image/flat writers without success.
    struct FailingWriter {
        /// Successful prefix bytes before the deliberate I/O error.
        remaining: usize,
    }
    impl io::Write for FailingWriter {
        /// A successful prefix forces write_all to retry before observing the injected error.
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.remaining == 0 {
                return Err(io::Error::other("injected write failure"));
            }
            let count = self.remaining.min(bytes.len());
            self.remaining -= count;
            Ok(count)
        }
        /// No separate buffer exists; the selected failure belongs to the write path.
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    /// A serializer must not hide an I/O error at any small emitted prefix.
    #[test]
    fn nested_writers_propagate_prefix_failures() {
        let data = fixture::data();
        for value in [data.tables.surfaces[0].normal, data.tables.surfaces[0].disabled] {
            for remaining in 0..64 {
                assert!(write_visual(&mut FailingWriter { remaining }, value).is_err());
            }
        }
    }
}
