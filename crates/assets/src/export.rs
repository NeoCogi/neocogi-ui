//! Build-time exporter for the ordinary immutable COM bundle implementation.
//! Generated runtime code depends only on neocogi-ui and neocogi-com, with no UI/parser dependency.
use neocogi_ui_contracts::*;
use neocogi_ui::assets::AssetData;
use std::{
    io::{self, Write as _},
    path::Path,
};

/// Atomically writes self-contained Rust `load_bundle` and `bundle_data` functions.
/// Generated code requires facade `compiled-assets`: metadata stays static and pixels expand
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
    rust.write_all(b"// Generated immutable COM bundle; regenerate from the owning asset recipe.\nuse neocogi_ui::*;\nuse std::borrow::Cow;\n\n/// Publishes one compiled resource catalog without parsing fonts, PNG or JSON at runtime.\npub fn load_bundle() -> Result<neocogi_com::ComPtr<IAssetBundleAbi>, neocogi_com::NcStatus> {\n    bundle_data()?.into_bundle()\n}\n\n/// Returns compiled tables for a fresh bundle or a revision of an existing catalog.\npub fn bundle_data() -> Result<neocogi_ui::assets::AssetData, neocogi_com::NcStatus> {\n    Ok(neocogi_ui::assets::AssetData {\n")?;
    // Debug formatting is Rust source for these plain named scalar records. Tagged union
    // storage is never dumped; resolved artwork is reconstructed through its checked enum.
    let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&data.pixels, 9);
    writeln!(
        rust,
        "        pixels: Cow::Owned(neocogi_ui::assets::decode_pixels(&{:?}, {})?),",
        compressed,
        data.pixels.len()
    )?;
    writeln!(rust, "        tables: neocogi_ui::assets::AssetTables {{")?;
    writeln!(rust, "            size: {:?},", data.tables.size)?;
    writeln!(rust, "            names: Cow::Borrowed(&{:?}),", data.tables.names)?;
    writeln!(rust, "            fonts: Cow::Borrowed(&{:?}),", data.tables.fonts)?;
    writeln!(rust, "            glyphs: Cow::Borrowed(&{:?}),", data.tables.glyphs)?;
    writeln!(rust, "            icons: Cow::Borrowed(&{:?}),", data.tables.icons)?;
    writeln!(rust, "            metrics: {:?},", data.tables.metrics)?;
    let c = data.tables.chrome;
    // Open integer enums have explicit constructors; formatting their Debug output would
    // accidentally couple generated source to private wrapper fields.
    writeln!(
        rust,
        "            chrome: AssetChrome {{ title_alignment: TitleAlignment::{}, close_side: CaptionSide::{}, minimize_side: CaptionSide::{}, maximize_side: CaptionSide::{}, vertical_spacing: {}, minimum_extent: {}, inner_spacing: {}, show_without_activation: {}, minimized_content_height: {}, has_backdrop: {}, backdrop_rgba: {}, backdrop_padding: {} }},",
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
    writeln!(rust, "            surfaces: Cow::Owned(vec![")?;
    for values in data.tables.surfaces.iter() {
        writeln!(
            rust,
            "                AssetSurfaceVisuals {{ normal: {}, disabled: {} }},",
            visual_source(values.normal)?,
            visual_source(values.disabled)?
        )?;
    }
    writeln!(rust, "            ]),")?;
    writeln!(rust, "            controls: Cow::Owned(vec![")?;
    for values in data.tables.controls.iter() {
        writeln!(
            rust,
            "                AssetControlVisuals {{ disabled: {}, enabled: {}, focused: {} }},",
            visual_source(values.disabled)?,
            pointer_source(values.enabled)?,
            pointer_source(values.focused)?
        )?;
    }
    writeln!(rust, "            ]),")?;
    writeln!(rust, "            menus: Cow::Owned(vec![")?;
    for values in data.tables.menus.iter() {
        writeln!(
            rust,
            "                AssetMenuVisuals {{ normal: {}, hovered: {}, pressed: {}, focused: {}, open: {}, disabled: {} }},",
            visual_source(values.normal)?,
            visual_source(values.hovered)?,
            visual_source(values.pressed)?,
            visual_source(values.focused)?,
            visual_source(values.open)?,
            visual_source(values.disabled)?
        )?;
    }
    writeln!(rust, "            ]),")?;
    writeln!(rust, "            window_chrome: Cow::Owned(vec![")?;
    for values in data.tables.window_chrome.iter() {
        writeln!(
            rust,
            "                AssetChromeVisuals {{ base: {}, active: {}, disabled: {} }},",
            visual_source(values.base)?,
            visual_source(values.active)?,
            visual_source(values.disabled)?
        )?;
    }
    writeln!(rust, "            ]),")?;
    rust.write_all(b"        },\n    })\n}\n")?;
    // Atomic replacement publishes exactly one self-contained source artifact. Failed writes
    // leave both the previous file and every future build's visible input unchanged.
    rust.flush()?;
    rust.get_ref().as_file().sync_all()?;
    let temporary = rust.into_inner().map_err(|error| error.into_error())?;
    temporary.persist(destination).map(|_| ()).map_err(|error| error.error)
}

/// Formats one validated visual as generated Rust source with an explicit tagged patch.
fn visual_source(value: AssetVisual) -> io::Result<String> {
    let patch = AssetPatch::try_from(value.patch).map_err(|status| io::Error::other(status.to_string()))?;
    Ok(format!(
        "AssetVisual {{ insets: {:?}, center_visible: {}, content_color: {}, patch: AssetPatch::{patch:?}.into() }}",
        value.insets, value.center_visible, value.content_color
    ))
}

/// Formats the three named pointer fields shared by enabled and focused control branches.
fn pointer_source(values: AssetPointerVisuals) -> io::Result<String> {
    Ok(format!(
        "AssetPointerVisuals {{ normal: {}, hovered: {}, pressed: {} }}",
        visual_source(values.normal)?,
        visual_source(values.hovered)?,
        visual_source(values.pressed)?
    ))
}

/// Returns a source-level constant name for one previously validated caption edge.
fn side(value: CaptionSide) -> &'static str {
    if value == CaptionSide::LEADING { "LEADING" } else { "TRAILING" }
}
