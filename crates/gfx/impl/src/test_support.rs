//! Small immutable resources exercise production consumers without font rasterization or files.
use neocogi_com::ComPtr;
use neocogi_gfx_support::abi::*;
use neocogi_gfx_support::{AssetData, AssetTables};

/// Copies the canonical resource representation for assertions about publication and rendering.
pub(crate) fn data(source: &ComPtr<IAssetBundleAbi>) -> AssetData {
    AssetData::read(source.as_com_ref()).expect("fixture publishes readable immutable tables")
}

/// Appends one exact name; ranges partition the shared UTF-8 table in declaration order.
fn name(names: &mut Vec<u8>, value: &str) -> AssetRange {
    let range = AssetRange {
        first: names.len() as u32,
        count: value.len() as u32,
    };
    names.extend_from_slice(value.as_bytes());
    range
}

/// Publishes a fresh catalog whose small deterministic geometry supports clipping and text tests.
pub(crate) fn test_bundle() -> ComPtr<IAssetBundleAbi> {
    let mut names = Vec::new();
    let body = name(&mut names, "body");
    // Space advances without painting. All other printable glyphs have the same bounded cell,
    // making placement assertions independent of the installed fonts and rasterizer version.
    let glyphs = (32..127)
        .map(|scalar| AssetGlyph {
            scalar,
            offset_x: 0,
            offset_y: 0,
            advance_x: 8,
            advance_y: 0,
            rect: match scalar {
                32 => Rect::default(),
                _ => Rect { x: 0, y: 0, width: 4, height: 4 },
            },
        })
        .collect::<Vec<_>>();
    let icons = [
        "white",
        "close",
        "expand",
        "collapse",
        "check",
        "expand_down",
        "open_folder",
        "closed_folder",
        "file",
    ]
    .into_iter()
    .map(|value| AssetIcon {
        name: name(&mut names, value),
        rect: Rect { x: 0, y: 0, width: 4, height: 4 },
    })
    .collect::<Vec<_>>();
    let appearance = neocogi_appearance::AppearanceTables::flat(Insets::uniform(1), &FlatPalette::default());
    AssetData {
        pixels: vec![255; 4 * 4 * 4].into(),
        tables: AssetTables {
            size: Size { width: 4, height: 4 },
            fonts: vec![AssetFont {
                name: body,
                glyphs: AssetRange { first: 0, count: 95 },
                line_size: 10,
                baseline: 8,
                font_size: 10,
            }]
            .into(),
            glyphs: glyphs.into(),
            icons: icons.into(),
            names: names.into(),
            metrics: AssetMetrics::default(),
            chrome: neocogi_appearance::default_chrome(),
            surfaces: Vec::from(appearance.surfaces).into(),
            controls: Vec::from(appearance.controls).into(),
            menus: Vec::from(appearance.menus).into(),
            window_chrome: Vec::from(appearance.window_chrome).into(),
        },
    }
    .into_bundle()
    .expect("fixture satisfies the canonical asset contract")
}
