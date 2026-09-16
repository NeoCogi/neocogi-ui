//! Skin integration tests cross the real generated GFX construction and publication interfaces.

use super::ThemeLoaderObject;
use neocogi_com::{ComPtr, INcStringComPtrExt};
use neocogi_gfx_contracts::*;
use neocogi_gfx_impl::{AssetLoaderObject, AssetPublisherObject};
use neocogi_gfx_support::{AssetData, AssetTables};
use neocogi_skin_contracts::{ISkinDiagnosticsAbi, ThemeLoaderAbi};
use neocogi_ui::NativePath;
use neocogi_ui_support::assets::ImportedAssets;

/// Builds one small but complete semantic base without invoking the external theme loader.
fn test_bundle() -> ComPtr<IAssetBundleAbi> {
    let mut names = String::from("body");
    let mut icons = Vec::new();
    let icon_names = IconRole::ALL.map(|role| role.canonical_name().expect("declared icon roles have canonical names"));
    for name in core::iter::once("white").chain(icon_names) {
        let first = names.len() as u32;
        names.push_str(name);
        icons.push(AssetIcon {
            name: AssetRange { first, count: name.len() as u32 },
            rect: Rect { x: 0, y: 0, width: 1, height: 1 },
        });
    }
    let glyphs = ['_', 'a', 'b']
        .map(|character| AssetGlyph {
            scalar: character as u32,
            offset_x: 0,
            offset_y: 0,
            advance_x: 8,
            advance_y: 0,
            rect: Rect { x: 0, y: 0, width: 1, height: 1 },
        })
        .to_vec();
    let mut data = AssetData {
        pixels: vec![255; 4].into(),
        tables: AssetTables {
            size: Size { width: 1, height: 1 },
            names: names.into_bytes().into(),
            fonts: vec![AssetFont {
                name: AssetRange { first: 0, count: 4 },
                glyphs: AssetRange { first: 0, count: 3 },
                line_size: 10,
                baseline: 8,
                font_size: 10,
            }]
            .into(),
            glyphs: glyphs.into(),
            icons: icons.into(),
            metrics: AssetMetrics::default(),
            chrome: neocogi_appearance::default_chrome(),
            surfaces: Vec::new().into(),
            controls: Vec::new().into(),
            menus: Vec::new().into(),
            window_chrome: Vec::new().into(),
        },
    };
    neocogi_appearance::apply_flat(&mut data, Insets::uniform(1), &FlatPalette::default());
    data.validate().unwrap();
    data.into_bundle().unwrap()
}

/// External themes keep their catalog while all decoding and publication execute in GFX.
#[test]
fn bundled_themes_cross_only_typed_gfx_capabilities() {
    let base = test_bundle();
    let imported = ImportedAssets::read(base.as_com_ref(), None).unwrap();
    let assets: ComPtr<AssetLoaderAbi> = AssetLoaderObject::from_value(AssetLoaderObject::default()).query().unwrap();
    let publisher: ComPtr<AssetPublisherAbi> = AssetPublisherObject::from_value(AssetPublisherObject::default()).query().unwrap();
    let loader: ComPtr<ThemeLoaderAbi> = ThemeLoaderObject::from_value(ThemeLoaderObject::new(assets.as_com_ref(), publisher.as_com_ref()))
        .query()
        .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for theme_name in ["windows-3.11", "windows-95", "mac-os-9"] {
        let path = NativePath::new(&root.join("themes").join(theme_name).join("theme.json"));
        let source = loader.load_theme(path.as_interface(), base.as_com_ref()).unwrap();
        let loaded = ImportedAssets::read(source.as_com_ref(), Some(&imported)).unwrap();
        assert_eq!(loaded.bundle().atlas().font_id("body"), imported.original().font_id("body"));
        assert_ne!(source.as_ptr(), base.as_ptr());
    }

    // Diagnostic storage belongs to Skin even when a filesystem operation fails before GFX.
    let missing = NativePath::new(&root.join("missing-theme.json"));
    assert_eq!(
        loader.load_theme(missing.as_interface(), base.as_com_ref()).unwrap_err(),
        neocogi_com::NC_E_INVALID_ARG
    );
    let detail = loader.query::<ISkinDiagnosticsAbi>().unwrap().diagnostic().unwrap().to_string().unwrap();
    assert!(detail.contains("missing-theme.json"));
}
