//! Build-time exporter for the ordinary immutable COM bundle implementation.
//! Generated runtime code depends only on microui-redux and nc-com, with no UI/parser dependency.
use super::*;
use std::{
    io::{self, Write as _},
    path::Path,
};

impl SkinBundle {
    /// Atomically writes self-contained Rust `load_bundle` and `bundle_data` functions.
    /// Generated code requires facade `compiled-assets`: metadata stays static and pixels expand
    /// once into bounded owned storage. Fonts, PNG files and JSON are processed only by the exporter.
    /// A sibling temporary file keeps the previous output intact until the complete replacement is flushed.
    pub fn to_com_rust_files(&self, destination: impl AsRef<Path>) -> io::Result<()> {
        let destination = destination.as_ref();
        let data = export_bundle_data(self).map_err(|status| io::Error::other(status.to_string()))?;
        let parent = destination
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut rust = io::BufWriter::new(tempfile::NamedTempFile::new_in(parent)?);
        rust.write_all(b"// Generated immutable COM bundle; regenerate from the owning asset recipe.\nuse microui_redux::*;\nuse std::borrow::Cow;\n\n/// Publishes one compiled resource catalog without parsing fonts, PNG or JSON at runtime.\npub fn load_bundle() -> Result<nc_com::ComPtr<IAssetBundleAbi>, nc_com::NcStatus> {\n    bundle_data()?.into_bundle()\n}\n\n/// Returns compiled tables for a fresh bundle or a revision of an existing catalog.\npub fn bundle_data() -> Result<microui_redux::assets::AssetData, nc_com::NcStatus> {\n    Ok(microui_redux::assets::AssetData {\n")?;
        // Debug formatting is Rust source for these plain named scalar records. Tagged union
        // storage is never dumped; resolved artwork is reconstructed through its checked enum.
        writeln!(rust, "        size: {:?},", data.size)?;
        let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&data.pixels, 9);
        writeln!(
            rust,
            "        pixels: Cow::Owned(microui_redux::assets::decode_pixels(&{:?}, {})?),",
            compressed,
            data.pixels.len()
        )?;
        writeln!(rust, "        names: Cow::Borrowed(&{:?}),", data.names)?;
        writeln!(rust, "        fonts: Cow::Borrowed(&{:?}),", data.fonts)?;
        writeln!(rust, "        glyphs: Cow::Borrowed(&{:?}),", data.glyphs)?;
        writeln!(rust, "        icons: Cow::Borrowed(&{:?}),", data.icons)?;
        writeln!(rust, "        metrics: {:?},", data.metrics)?;
        let c = data.chrome;
        // Open integer enums have explicit constructors; formatting their Debug output would
        // accidentally couple generated source to private wrapper fields.
        writeln!(
            rust,
            "        chrome: AssetChrome {{ title_alignment: TitleAlignment::{}, close_side: CaptionSide::{}, minimize_side: CaptionSide::{}, maximize_side: CaptionSide::{}, vertical_spacing: {}, minimum_extent: {}, inner_spacing: {}, show_without_activation: {}, minimized_content_height: {}, has_backdrop: {}, backdrop_rgba: {}, backdrop_padding: {} }},",
            if c.title_alignment == TitleAlignment::LEADING {
                "LEADING"
            } else {
                "CENTERED"
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
        for (name, values) in [
            ("surfaces", &data.surfaces),
            ("controls", &data.controls),
            ("menus", &data.menus),
            ("window_chrome", &data.window_chrome),
        ] {
            writeln!(rust, "        {name}: Cow::Owned(vec![")?;
            for value in values.iter() {
                let patch = AssetPatch::try_from(value.patch).map_err(|status| io::Error::other(status.to_string()))?;
                writeln!(
                    rust,
                    "            AssetVisual {{ insets: {:?}, center_visible: {}, content_color: {}, patch: AssetPatch::{patch:?}.into() }},",
                    value.insets, value.center_visible, value.content_color
                )?;
            }
            writeln!(rust, "        ]),")?;
        }
        rust.write_all(b"    })\n}\n")?;
        // Atomic replacement publishes exactly one self-contained source artifact. Failed writes
        // leave both the previous file and every future build's visible input unchanged.
        rust.flush()?;
        rust.get_ref().as_file().sync_all()?;
        let temporary = rust.into_inner().map_err(|error| error.into_error())?;
        temporary.persist(destination).map(|_| ()).map_err(|error| error.error)
    }
}

/// Returns a source-level constant name for one previously validated caption edge.
fn side(value: CaptionSide) -> &'static str {
    if value == CaptionSide::LEADING { "LEADING" } else { "TRAILING" }
}
