//! Skin integration tests cross the real generated GFX construction and publication interfaces.

use super::ThemeLoaderObject;
use neocogi_com::{ComPtr, INcStringComPtrExt};
use neocogi_gfx_support::abi::*;
use neocogi_gfx_impl::{AssetLoaderObject, AssetPublisherObject};
use neocogi_gfx_support::{AssetData, AssetTables};
use neocogi_appearance::skin::{ISkinDiagnosticsAbi, ThemeLoaderAbi};
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
    let loader: ComPtr<ThemeLoaderAbi> = ThemeLoaderObject::from_value(ThemeLoaderObject::new(assets.as_com_ref(), publisher.as_com_ref()).unwrap())
        .query()
        .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for theme_name in ["windows-3.11", "windows-95", "mac-os-9"] {
        let path = NativePath::new(&root.join("themes").join(theme_name).join("theme.json"));
        let source = loader.load_theme(path.as_interface(), base.as_com_ref()).unwrap();
        let loaded = ImportedAssets::read(source.as_com_ref(), Some(&imported.catalog())).unwrap();
        assert_eq!(loaded.bundle().atlas().font_id("body"), imported.original().font_id("body"));
        assert_ne!(source.as_raw().object, base.as_raw().object);
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

/// A valid original application can use any artwork-like key. The first theme
/// load must preserve those pixels and bind its own placements to different keys.
#[test]
fn original_application_artwork_names_survive_first_and_repeated_theme_loads() {
    use neocogi_com::NcCopyFromBufferOffset;
    let mut original = AssetData::read(test_bundle().as_com_ref()).unwrap();
    original.tables.size = Size { width: 64, height: 64 };
    original.pixels = vec![255; 64 * 64 * 4].into();
    let application_color = [19, 37, 53, 255];
    original.pixels.to_mut()[4..8].copy_from_slice(&application_color);
    for suffix in ["0", "1", "3", "9999", "01", "+2"] {
        let name = format!("@neocogi-theme-image/{suffix}");
        let range = AssetRange {
            first: original.tables.names.len() as u32,
            count: name.len() as u32,
        };
        original.tables.names.to_mut().extend_from_slice(name.as_bytes());
        original.tables.icons.to_mut().push(AssetIcon {
            name: range,
            rect: Rect { x: 1, y: 0, width: 1, height: 1 },
        });
    }
    original.validate().unwrap();
    let base = original.clone().into_bundle().unwrap();
    let assets: ComPtr<AssetLoaderAbi> = AssetLoaderObject::from_value(AssetLoaderObject::default()).query().unwrap();
    let publisher: ComPtr<AssetPublisherAbi> = AssetPublisherObject::from_value(AssetPublisherObject::default()).query().unwrap();
    let loader: ComPtr<ThemeLoaderAbi> = ThemeLoaderObject::from_value(ThemeLoaderObject::new(assets.as_com_ref(), publisher.as_com_ref()).unwrap())
        .query()
        .unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let white = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../assets/WHITE.png");
    for name in ["first.png", "second.png"] {
        std::fs::copy(&white, temporary.path().join(name)).unwrap();
    }
    let path = temporary.path().join("theme.json");
    std::fs::write(
        &path,
        br#"{
        "schema_version": 1, "name": "Artwork namespace",
        "appearances": { "surface": { "panel": {
            "normal": { "patch": { "type": "image", "path": "first.png" } },
            "disabled": { "patch": { "type": "image", "path": "second.png" } }
        } } }
    }"#,
    )
    .unwrap();
    let native_path = NativePath::new(&path);
    let mut first = None;
    for _ in 0..2 {
        let result = loader.load_theme(native_path.as_interface(), base.as_com_ref()).unwrap();
        let actual = AssetData::read(result.as_com_ref()).unwrap();
        let pixel = |rect: Rect| {
            let start = (rect.y as usize * actual.tables.size.width as usize + rect.x as usize) * 4;
            &actual.pixels[start..start + 4]
        };
        for suffix in ["0", "1", "3", "9999", "01", "+2"] {
            let name = format!("@neocogi-theme-image/{suffix}");
            let icon = actual.tables.icons.iter().find(|icon| actual.name(icon.name).unwrap() == name).unwrap();
            assert_eq!(pixel(icon.rect), application_color);
        }
        let panel = &actual.tables.surfaces[SurfaceRole::Panel.raw() as usize];
        for (visual, expected) in [(&panel.normal, "@neocogi-theme-image/2"), (&panel.disabled, "@neocogi-theme-image/4")] {
            let AssetPatch::Image { icon, .. } = visual.patch else {
                panic!("theme must retain its own image placement")
            };
            let icon = actual.tables.icons[icon as usize];
            assert_eq!(actual.name(icon.name).unwrap(), expected);
            assert_eq!(pixel(icon.rect), [255; 4]);
        }
        if let Some(previous) = &first {
            let previous: &AssetData = previous;
            assert_eq!(actual.pixels, previous.pixels);
            assert_eq!(actual.tables.names, previous.tables.names);
            assert_eq!(actual.tables.icons, previous.tables.icons);
        }
        #[cfg(feature = "builder")]
        {
            let offline = super::theme::load(&path, &original).unwrap();
            assert_eq!(actual.pixels, offline.data().pixels);
            assert_eq!(actual.tables.names, offline.data().tables.names);
            assert_eq!(actual.tables.icons, offline.data().tables.icons);
            assert_eq!(actual.tables.surfaces, offline.data().tables.surfaces);
        }
        first = Some(actual);
    }
    // Named image insertion remains available; the allocator has not reserved a prefix.
    let builder = assets
        .derive_builder(
            Size { width: 64, height: 64 },
            base.as_com_ref(),
            AssetDeriveOptions {
                replace_semantic_fonts: false,
                replace_semantic_icons: false,
            },
        )
        .unwrap();
    builder
        .add_icon_bytes(
            NcCopyFromBufferOffset::new(0, b"@neocogi-theme-image/2"),
            NcCopyFromBufferOffset::new(0, include_bytes!("../../../../assets/WHITE.png")),
        )
        .unwrap();
}
