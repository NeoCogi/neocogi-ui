use super::*;
use neocogi_ui::assets::AssetTables;

/// Consumer adapter keeping runtime rendering assertions outside asset production code.
struct TestTheme {
    /// Authored display name checked by theme selection tests.
    name: String,
    /// Native view imported through the real COM consumer boundary.
    bundle: neocogi_ui_runtime::SkinBundle,
}
impl TestTheme {
    /// Publishes one compiled result and imports it through the UI exactly once.
    fn new(theme: LoadedTheme) -> Self {
        let source = theme.data.into_bundle().unwrap();
        let imported = neocogi_ui_runtime::assets::ImportedAssets::read(source.as_com_ref(), None).unwrap();
        Self {
            name: theme.name,
            bundle: imported.bundle().clone(),
        }
    }
    /// Borrows the resolved runtime pair for measurement and patch assertions.
    fn bundle(&self) -> &neocogi_ui_runtime::SkinBundle {
        &self.bundle
    }
    /// Borrows the authored name retained independently of compilation inputs.
    fn name(&self) -> &str {
        &self.name
    }
}

/// Reproduces the tiny baseline fixture without loading external resource files.
fn test_atlas() -> AssetData {
    let mut names = String::from("body");
    let mut icons = Vec::new();
    for name in core::iter::once("white").chain(IconRole::ALL.map(IconRole::atlas_name)) {
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
            chrome: crate::style::compile_chrome(WindowChromeSkin::default()),
            surfaces: vec![].into(),
            controls: vec![].into(),
            menus: vec![].into(),
            window_chrome: vec![].into(),
        },
    };
    // The same flat compiler supplies all omitted appearances in real theme construction.
    apply_flat(&mut data, Insets::uniform(1), &FlatPalette::default());
    data.validate().unwrap();
    data
}
use neocogi_ui_contracts::appearance::{ChromeRole, ControlRole, MenuRole};
use std::fs;

/// Installs one repository-bundled theme through the same filesystem and PNG path as Context.
fn install_bundled_theme(relative_path: &str) -> (TestTheme, usize) {
    // Resolve from the Cargo manifest so tests remain independent of the process working dir.
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(relative_path);
    let loaded = load(path.as_path(), &test_atlas()).expect("bundled theme must load through the complete entry point");
    let loaded = TestTheme::new(loaded);
    let image_count = loaded
        .bundle()
        .atlas()
        .clone_icon_table()
        .iter()
        .filter(|(name, _)| name.starts_with("@neocogi-theme-image/"))
        .count();
    (loaded, image_count)
}

/// Verifies an embedded host can compile the same complete theme without filesystem access in
/// the loader itself.
#[test]
fn in_memory_theme_uses_the_supplied_asset_provider() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes/windows-95/theme.json");
    let definition = fs::read(&path).expect("read bundled theme definition for test");
    let loaded = load_bytes(path.as_path(), definition.as_slice(), &test_atlas(), &mut FileAssets)
        .expect("in-memory theme must resolve every asset through the supplied provider");
    let loaded = TestTheme::new(loaded);

    assert_eq!(loaded.name(), "Windows 95");
    assert!(
        loaded
            .bundle()
            .atlas()
            .clone_icon_table()
            .iter()
            .any(|(name, _)| name.starts_with("@neocogi-theme-image/"))
    );
}

/// Verifies missing state patches remain state-specific flat fallbacks after palette replacement.
#[test]
fn omitted_patch_states_use_theme_flat_colors() {
    let document = ThemeDefinition::parse_for_test(
        r#"{
                "schema_version": 1,
                "name": "Flat only",
                "skin": {
                    "palette": {
                        "button": [1, 2, 3, 255],
                        "button_hovered": [4, 5, 6, 255],
                        "disabled_background": [7, 8, 9, 255]
                    }
                },
                "appearances": {
                    "control": {
                        "button": { "insets": { "left": 2, "top": 2, "right": 2, "bottom": 2 } }
                    }
                }
            }"#,
    )
    .expect("test definition must match the strict schema");
    let atlas = test_atlas();
    let theme_atlas = document.build_atlas(&atlas).expect("flat theme must retain the base atlas");
    let loaded = document.install(theme_atlas).expect("flat-only theme must install");
    let loaded = TestTheme::new(loaded);

    let normal = loaded
        .bundle()
        .skin()
        .control(ControlRole::Button, ControlState::Enabled(PointerState::Normal))
        .patch;
    let hovered = loaded
        .bundle()
        .skin()
        .control(ControlRole::Button, ControlState::Enabled(PointerState::Hovered))
        .patch;
    let disabled = loaded.bundle().skin().control(ControlRole::Button, ControlState::Disabled).patch;
    assert_eq!(normal.insets.left, 2);
    assert!(matches!(
        normal.content,
        neocogi_ui_runtime::NinePatchContent::Flat { cells }
            if matches!(cells.center, neocogi_ui_runtime::NinePatchCell::Color { color } if color.r == 1)
    ));
    assert!(matches!(
        hovered.content,
        neocogi_ui_runtime::NinePatchContent::Flat { cells }
            if matches!(cells.center, neocogi_ui_runtime::NinePatchCell::Color { color } if color.r == 4)
    ));
    assert!(matches!(
        disabled.content,
        neocogi_ui_runtime::NinePatchContent::Flat { cells }
            if matches!(cells.center, neocogi_ui_runtime::NinePatchCell::Color { color } if (color.r, color.g, color.b) == (7, 8, 9))
    ));
}

/// Verifies one state can replace text and glyph color without supplying background artwork.
#[test]
fn state_content_color_override_does_not_require_a_patch() {
    let document = ThemeDefinition::parse_for_test(
        r#"{
                "schema_version": 1,
                "name": "Content-color states",
                "skin": { "palette": { "menu_foreground": [1, 2, 3, 255] } },
                "appearances": {
                    "menu": {
                        "item": {
                            "hovered": { "content_color": [250, 251, 252, 255] },
                            "disabled": { "content_color": [90, 91, 92, 255] }
                        }
                    }
                }
            }"#,
    )
    .expect("content-color-only states must match the strict schema");
    let atlas = test_atlas();
    let theme_atlas = document.build_atlas(&atlas).expect("content-color-only theme must retain the base atlas");
    let loaded = document.install(theme_atlas).expect("content-color-only theme must install");
    let loaded = TestTheme::new(loaded);

    let normal = loaded.bundle().skin().menu(MenuRole::Item, MenuState::Normal).content_color;
    let hovered = loaded.bundle().skin().menu(MenuRole::Item, MenuState::Hovered).content_color;
    let disabled = loaded.bundle().skin().menu(MenuRole::Item, MenuState::Disabled).content_color;
    assert_eq!((normal.r, normal.g, normal.b, normal.a), (1, 2, 3, 255));
    assert_eq!((hovered.r, hovered.g, hovered.b, hovered.a), (250, 251, 252, 255));
    assert_eq!((disabled.r, disabled.g, disabled.b, disabled.a), (90, 91, 92, 255));
}

/// Verifies a solid patch is resource-free and covers every cell while retaining role geometry.
#[test]
fn solid_patch_compiles_without_rebuilding_the_atlas() {
    let document = ThemeDefinition::parse_for_test(
        r#"{
                "schema_version": 1,
                "name": "Solid panel",
                "appearances": {
                    "surface": {
                        "panel": {
                            "insets": { "left": 2, "top": 3, "right": 4, "bottom": 5 },
                            "normal": { "patch": { "type": "solid", "color": [11, 22, 33, 255] } }
                        }
                    }
                }
            }"#,
    )
    .expect("solid patch must match the strict tagged schema");
    let base = test_atlas();
    let theme_atlas = document.build_atlas(&base).expect("solid patch must not require atlas work");
    assert_eq!(theme_atlas.atlas.pixels, base.pixels);
    assert_eq!(theme_atlas.atlas.tables.icons, base.tables.icons);
    assert!(theme_atlas.images.is_empty());
    let loaded = document.install(theme_atlas).expect("solid patch must install");
    let loaded = TestTheme::new(loaded);
    let patch = loaded.bundle().skin().surface(SurfaceRole::Panel, SurfaceState::Normal).patch;

    assert_eq!((patch.insets.left, patch.insets.top, patch.insets.right, patch.insets.bottom), (2, 3, 4, 5));
    assert!(matches!(
        patch.content,
        neocogi_ui_runtime::NinePatchContent::Flat { cells }
            if matches!(cells.top_left, neocogi_ui_runtime::NinePatchCell::Color { color } if (color.r, color.g, color.b, color.a) == (11, 22, 33, 255))
                && matches!(cells.center, neocogi_ui_runtime::NinePatchCell::Color { color } if (color.r, color.g, color.b, color.a) == (11, 22, 33, 255))
    ));
}

/// Verifies image-only data cannot exist beside or inside the wrong patch alternative.
#[test]
fn patch_variants_reject_meaningless_field_combinations() {
    let invalid_states = [
        (r#"{ "source_insets": { "left": 1, "top": 1, "right": 1, "bottom": 1 } }"#, "source_insets"),
        (r#"{ "patch": { "type": "solid", "color": [1, 2, 3, 255], "path": "unused.png" } }"#, "path"),
        (r#"{ "patch": { "type": "image", "tint": [255, 255, 255, 255] } }"#, "path"),
    ];

    for (state, rejected_field) in invalid_states {
        let json = format!(
            r#"{{
                    "schema_version": 1,
                    "name": "Invalid patch",
                    "appearances": {{ "surface": {{ "panel": {{ "normal": {state} }} }} }}
                }}"#
        );
        let error = match ThemeDefinition::parse_for_test(&json) {
            Ok(_) => panic!("meaningless `{rejected_field}` combination must be rejected"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains(rejected_field),
            "diagnostic must identify `{rejected_field}`: {error}"
        );
    }
}

/// Verifies the strict schema rejects misspelled fields instead of silently ignoring them.
#[test]
fn unknown_json_fields_are_rejected() {
    let error = match ThemeDefinition::parse_for_test(r#"{ "schema_version": 1, "name": "Broken", "appearences": {} }"#) {
        Ok(_) => panic!("misspelled root field must be rejected"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("unknown field `appearences`"));
}

/// Verifies the loader accepts exactly schema v1 without maintaining compatibility branches.
#[test]
fn a_different_theme_schema_is_not_accepted_as_a_compatibility_format() {
    let error = match ThemeDefinition::parse_for_test(r#"{ "schema_version": 2, "name": "Future" }"#) {
        Ok(_) => panic!("schema version two must not enter the version-one compiler"),
        Err(error) => error,
    };

    // An exact version check keeps parsing deterministic without aliases or migration code.
    assert!(matches!(
        error,
        ThemeLoadError::UnsupportedSchema {
            expected: THEME_SCHEMA_VERSION,
            actual: 2,
        }
    ));
}

/// Verifies the removed composition feature cannot silently return through a permissive root.
#[test]
fn extends_is_rejected_as_an_unknown_schema_field() {
    let error = match ThemeDefinition::parse_for_test(r#"{ "schema_version": 1, "name": "Child", "extends": "parent.json" }"#) {
        Ok(_) => panic!("a parent link must not be accepted by the single-file schema"),
        Err(error) => error,
    };

    // Explicit rejection distinguishes removal from accidentally ignoring the obsolete field.
    assert!(error.to_string().contains("unknown field `extends`"));
}

/// Verifies misspelled appearance keys are rejected directly by the concrete role enum.
#[test]
fn unknown_appearance_role_key_is_rejected_during_decode() {
    let error = match ThemeDefinition::parse_for_test(
        r#"{
                "schema_version": 1,
                "name": "Broken role",
                "appearances": { "control": { "buton": {} } }
            }"#,
    ) {
        Ok(_) => panic!("an unknown control role must not enter theme compilation"),
        Err(error) => error,
    };

    assert!(matches!(error, ThemeLoadError::DefinitionJson { source, .. } if source.to_string().contains("buton")));
}

/// Verifies a declared font catalog is complete rather than silently borrowing a missing role
/// from whichever atlas happened to be active while the theme was loaded.
#[test]
fn font_catalog_requires_every_semantic_role() {
    let error = match ThemeDefinition::parse_for_test(
        r#"{
                "schema_version": 1,
                "name": "Incomplete fonts",
                "fonts": {
                    "texture_width": 512,
                    "texture_height": 256,
                    "body": { "path": "body.ttf", "size": 12 },
                    "small": { "path": "body.ttf", "size": 10 },
                    "title": { "path": "title.ttf", "size": 12 },
                    "heading": { "path": "body.ttf", "size": 18 }
                }
            }"#,
    ) {
        Ok(_) => panic!("missing mono role must violate the strict font schema"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("missing field `mono`"));
}

/// Verifies a theme-owned semantic icon set is complete rather than mixing unrelated base
/// artwork into whichever roles the document forgot to declare.
#[test]
fn icon_catalog_requires_every_semantic_role() {
    let error = match ThemeDefinition::parse_for_test(
        r#"{
                "schema_version": 1,
                "name": "Incomplete icons",
                "icons": {
                    "close": { "path": "close.png" },
                    "expand": { "path": "expand.png" },
                    "collapse": { "path": "collapse.png" },
                    "check": { "path": "check.png" },
                    "expand_down": { "path": "expand-down.png" },
                    "open_folder": { "path": "open-folder.png" },
                    "closed_folder": { "path": "closed-folder.png" }
                }
            }"#,
    ) {
        Ok(_) => panic!("missing file icon role must violate the strict icon schema"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("missing field `file`"));
}

/// Verifies the bundled Windows theme and every original PNG install through the public schema.
#[test]
fn bundled_windows_95_theme_reuses_shared_png_regions() {
    let (loaded, images) = install_bundled_theme("themes/windows-95/theme.json");
    assert_eq!(loaded.name(), "Windows 95");
    assert_eq!(images, 11, "each shared PNG path must be baked exactly once");
    assert_eq!(loaded.bundle().skin().window_chrome.minimized_content_height, 2);
    assert_eq!(loaded.bundle().skin().window_chrome.captions.vertical_spacing, 2);
    let atlas = loaded.bundle().atlas();
    assert_eq!((atlas.width(), atlas.height()), (512, 256));
    assert_eq!(
        atlas.clone_font_table().len(),
        5,
        "the theme atlas must contain exactly its five semantic font roles"
    );
    assert_eq!(atlas.get_font_size(atlas.font_id("heading").unwrap()), 18);
    let insets = loaded.bundle().skin().chrome(ChromeRole::WindowFrame, ChromeState::Base).patch.insets;
    assert_eq!((insets.left, insets.top, insets.right, insets.bottom), (4, 4, 4, 4));
    assert!(matches!(
        loaded.bundle().skin().control(ControlRole::Button, ControlState::Disabled).patch.content,
        neocogi_ui_runtime::NinePatchContent::Image { .. }
    ));
    assert!(matches!(
        loaded.bundle().skin().chrome(ChromeRole::DialogFrame, ChromeState::Active).patch.content,
        neocogi_ui_runtime::NinePatchContent::Image { .. }
    ));
    for state in [ControlState::Focused(PointerState::Normal), ControlState::Focused(PointerState::Hovered)] {
        let disclosure = loaded.bundle().skin().control(ControlRole::Item, state).patch;
        assert!(matches!(
            disclosure.content,
            neocogi_ui_runtime::NinePatchContent::Flat { cells }
                if matches!(cells.center, neocogi_ui_runtime::NinePatchCell::Color { color } if (color.r, color.g, color.b, color.a) == (0, 0, 128, 255))
        ));
        let content_color = loaded.bundle().skin().control(ControlRole::Item, state).content_color;
        assert_eq!((content_color.r, content_color.g, content_color.b, content_color.a), (255, 255, 255, 255));
    }
}

/// Verifies the earlier Windows theme keeps its period title colors inside black frames.
#[test]
fn bundled_windows_311_theme_draws_black_framed_white_and_blue_titles() {
    let (loaded, images) = install_bundled_theme("themes/windows-3.11/theme.json");
    assert_eq!(loaded.name(), "Windows 3.11 for Workgroups");
    assert_eq!(images, 12, "only referenced role-state artwork is baked into the theme atlas");
    assert_eq!(loaded.bundle().skin().window_chrome.minimized_content_height, 2);
    assert_eq!(loaded.bundle().skin().window_chrome.captions.vertical_spacing, 0);
    let insets = loaded.bundle().skin().chrome(ChromeRole::WindowFrame, ChromeState::Base).patch.insets;
    assert_eq!((insets.left, insets.top, insets.right, insets.bottom), (23, 23, 23, 23));
    let border = loaded.bundle().skin().metrics.window_border;
    assert_eq!((border.left, border.top, border.right, border.bottom), (4, 4, 4, 4));
    let content = loaded.bundle().skin().metrics.window_content_insets;
    assert_eq!(
        (content.left, content.top, content.right, content.bottom),
        (2, 2, 2, 2),
        "the period theme leaves a narrow application margin inside root chrome"
    );
    let window_surface = loaded.bundle().skin().surface(SurfaceRole::Window, SurfaceState::Normal).patch;
    assert!(matches!(
        window_surface.content,
        neocogi_ui_runtime::NinePatchContent::Flat { cells }
            if matches!(cells.center, neocogi_ui_runtime::NinePatchCell::Color { color } if (color.r, color.g, color.b, color.a) == (255, 255, 255, 255))
    ));
    // Ordinary windows retain their long L-corner bitmap. Only modal dialogs use the uniform
    // four-pixel blue focus frame visible around period Windows 3.11 dialog boxes.
    assert!(matches!(
        loaded.bundle().skin().chrome(ChromeRole::WindowFrame, ChromeState::Active).patch.content,
        neocogi_ui_runtime::NinePatchContent::Image { .. }
    ));
    let dialog_frame = loaded.bundle().skin().chrome(ChromeRole::DialogFrame, ChromeState::Active).patch;
    assert_eq!(
        (
            dialog_frame.insets.left,
            dialog_frame.insets.top,
            dialog_frame.insets.right,
            dialog_frame.insets.bottom,
        ),
        (4, 4, 4, 4)
    );
    assert!(matches!(
        dialog_frame.content,
        neocogi_ui_runtime::NinePatchContent::Flat { cells }
            if matches!(cells.top, neocogi_ui_runtime::NinePatchCell::Color { color } if (color.r, color.g, color.b, color.a) == (0, 0, 170, 255))
                && matches!(cells.center, neocogi_ui_runtime::NinePatchCell::Empty)
    ));
    let menu_popup = loaded.bundle().skin().menu(MenuRole::Popup, MenuState::Normal).patch;
    assert_eq!(
        (menu_popup.insets.left, menu_popup.insets.top, menu_popup.insets.right, menu_popup.insets.bottom),
        (1, 1, 1, 1)
    );
    assert!(matches!(
        menu_popup.content,
        neocogi_ui_runtime::NinePatchContent::Flat { cells }
            if matches!(cells.top, neocogi_ui_runtime::NinePatchCell::Color { color } if (color.r, color.g, color.b, color.a) == (0, 0, 0, 255))
                && matches!(cells.center, neocogi_ui_runtime::NinePatchCell::Color { color } if (color.r, color.g, color.b, color.a) == (255, 255, 255, 255))
    ));
    // The outer Windows frame already supplies the title's top, left, and right black edges.
    // The title patch contributes only the missing one-pixel bottom edge, avoiding a doubled
    // line where the two patches meet while leaving themes without framed titles unchanged.
    for (state, expected_center) in [
        (ChromeState::Base, (255, 255, 255, 255)),
        (ChromeState::Active, (0, 0, 170, 255)),
        (ChromeState::Disabled, (255, 255, 255, 255)),
    ] {
        let patch = loaded.bundle().skin().chrome(ChromeRole::Title, state).patch;
        assert_eq!((patch.insets.left, patch.insets.top, patch.insets.right, patch.insets.bottom), (0, 0, 0, 1));
        assert!(matches!(
            patch.content,
            neocogi_ui_runtime::NinePatchContent::Flat { cells }
                if matches!(cells.top, neocogi_ui_runtime::NinePatchCell::Color { color } if (color.r, color.g, color.b, color.a) == (0, 0, 0, 255))
                    && matches!(cells.center, neocogi_ui_runtime::NinePatchCell::Color { color } if (color.r, color.g, color.b, color.a) == expected_center)
        ));
    }
    // Base Windows 3.11 titles are white and require black text, while active blue titles use
    // white text. Pointer interaction is not a representable chrome state.
    let base = loaded.bundle().skin().chrome(ChromeRole::Title, ChromeState::Base).content_color;
    let active = loaded.bundle().skin().chrome(ChromeRole::Title, ChromeState::Active).content_color;
    assert_eq!((base.r, base.g, base.b, base.a), (0, 0, 0, 255));
    assert_eq!((active.r, active.g, active.b, active.a), (255, 255, 255, 255));
    let disabled_title = loaded.bundle().skin().chrome(ChromeRole::Title, ChromeState::Disabled).content_color;
    assert_eq!((disabled_title.r, disabled_title.g, disabled_title.b, disabled_title.a), (125, 125, 125, 255));
    let minimize = loaded
        .bundle()
        .skin()
        .control(ControlRole::MinimizeButton, ControlState::Enabled(PointerState::Normal));
    assert_eq!(
        (
            minimize.content_color.r,
            minimize.content_color.g,
            minimize.content_color.b,
            minimize.content_color.a
        ),
        (0, 0, 0, 255),
        "the caption control's content color drives its manager-owned semantic symbol"
    );
    // Disabling must preserve the raised caption face authored by the theme. Letting these
    // roles fall back to their flat palette appearance would combine a one-pixel black frame
    // with the role's three-pixel visual insets and produce an incorrect heavy black square.
    for role in [
        ControlRole::CloseButton,
        ControlRole::MinimizeButton,
        ControlRole::MaximizeButton,
        ControlRole::RestoreButton,
    ] {
        let normal = loaded.bundle().skin().control(role, ControlState::Enabled(PointerState::Normal)).patch;
        let disabled = loaded.bundle().skin().control(role, ControlState::Disabled).patch;
        assert!(matches!(
            (normal.content, disabled.content),
            (neocogi_ui_runtime::NinePatchContent::Image { image: normal }, neocogi_ui_runtime::NinePatchContent::Image { image: disabled })
                if normal.icon == disabled.icon
        ));
    }
    let selected_text = loaded.bundle().skin().menu(MenuRole::Item, MenuState::Hovered).content_color;
    assert_eq!((selected_text.r, selected_text.g, selected_text.b, selected_text.a), (255, 255, 255, 255));
    let slider_normal = loaded
        .bundle()
        .skin()
        .control(ControlRole::SliderTrack, ControlState::Enabled(PointerState::Normal))
        .patch;
    let slider_focused = loaded
        .bundle()
        .skin()
        .control(ControlRole::SliderTrack, ControlState::Focused(PointerState::Normal))
        .patch;
    assert!(matches!(
        (slider_normal.content, slider_focused.content),
        (neocogi_ui_runtime::NinePatchContent::Image { image: normal }, neocogi_ui_runtime::NinePatchContent::Image { image: focused })
            if normal.icon == focused.icon
    ));
    for state in [
        ControlState::Enabled(PointerState::Hovered),
        ControlState::Enabled(PointerState::Pressed),
        ControlState::Focused(PointerState::Normal),
        ControlState::Focused(PointerState::Hovered),
        ControlState::Focused(PointerState::Pressed),
    ] {
        // Combo popup choices and tree rows share the semantic Item visual. Its complete
        // interaction ladder carries both blue selection art and contrasting text.
        assert!(matches!(
            loaded.bundle().skin().control(ControlRole::Item, state).patch.content,
            neocogi_ui_runtime::NinePatchContent::Image { .. }
        ));
        let content_color = loaded.bundle().skin().control(ControlRole::Item, state).content_color;
        assert_eq!((content_color.r, content_color.g, content_color.b, content_color.a), (255, 255, 255, 255));
    }
}

/// Verifies the bundled Mac theme installs its semantic resources and asymmetric Platinum
/// chrome geometry together with the generated state artwork.
#[test]
fn bundled_mac_os_9_theme_reuses_shared_png_regions() {
    let (loaded, images) = install_bundled_theme("themes/mac-os-9/theme.json");
    assert_eq!(loaded.name(), "Mac OS 9");
    assert_eq!(images, 29, "each shared PNG path must be baked exactly once");
    let chrome = loaded.bundle().skin().window_chrome;
    assert_eq!(chrome.title_alignment, neocogi_ui_contracts::WindowTitleAlignment::Centered);
    assert_eq!(chrome.captions.close_side, neocogi_ui_contracts::CaptionButtonSide::Leading);
    assert_eq!(chrome.captions.vertical_spacing, 3);
    assert_eq!(chrome.captions.inner_spacing, 4);
    assert_eq!(chrome.minimized_content_height, 2);
    assert!(!chrome.captions.show_without_activation);
    assert_eq!(loaded.bundle().skin().metrics.title_height, 20);
    for (role, expected_size) in [
        (FontRole::Body, 12),
        (FontRole::Small, 10),
        (FontRole::Title, 12),
        (FontRole::Heading, 18),
        (FontRole::Mono, 13),
    ] {
        let font = loaded.bundle().skin().resolve_font_role(loaded.bundle().atlas(), role);
        assert_eq!(loaded.bundle().atlas().get_font_size(font), expected_size);
    }
    for (role, expected_size) in [
        (IconRole::Close, (9, 9)),
        (IconRole::Expand, (9, 9)),
        (IconRole::Collapse, (9, 9)),
        (IconRole::Check, (11, 11)),
        (IconRole::ExpandDown, (9, 9)),
        (IconRole::OpenFolder, (16, 16)),
        (IconRole::ClosedFolder, (16, 16)),
        (IconRole::File, (16, 16)),
    ] {
        let icon = loaded.bundle().skin().resolve_icon_role(loaded.bundle().atlas(), role);
        let size = loaded.bundle().atlas().get_icon_size(icon);
        assert_eq!((size.width, size.height), expected_size);
    }
    let content = loaded.bundle().skin().metrics.window_content_insets;
    assert_eq!(
        (content.left, content.top, content.right, content.bottom),
        (0, 0, 0, 0),
        "Platinum windows expose their complete application body below root chrome"
    );
    let insets = loaded.bundle().skin().chrome(ChromeRole::WindowFrame, ChromeState::Base).patch.insets;
    assert_eq!((insets.left, insets.top, insets.right, insets.bottom), (6, 22, 6, 6));
    let border = loaded.bundle().skin().metrics.window_border;
    assert_eq!(
        (border.left, border.top, border.right, border.bottom),
        (6, 2, 6, 6),
        "the title begins below the two-pixel top edge while the remaining edges retain the six-layer frame"
    );
    let active_title = loaded.bundle().skin().chrome(ChromeRole::Title, ChromeState::Active).patch;
    let neocogi_ui_runtime::NinePatchContent::Image { image: active_title_image } = active_title.content else {
        panic!("Mac OS 9 active title must use baked artwork");
    };
    let active_title_size = loaded.bundle().atlas().get_icon_size(active_title_image.icon);
    assert_eq!((active_title_size.width, active_title_size.height), (8, 20));
    assert_eq!(
        (
            active_title.insets.left,
            active_title.insets.top,
            active_title.insets.right,
            active_title.insets.bottom,
        ),
        (0, 0, 0, 2),
        "the centered label must not overwrite the title's lower shadow and black separator"
    );
    assert_eq!(
        (
            active_title_image.source_insets.left,
            active_title_image.source_insets.top,
            active_title_image.source_insets.right,
            active_title_image.source_insets.bottom,
        ),
        (0, 0, 0, 2)
    );
    let active_frame = loaded.bundle().skin().chrome(ChromeRole::WindowFrame, ChromeState::Active).patch;
    let neocogi_ui_runtime::NinePatchContent::Image { image: active_frame_image } = active_frame.content else {
        panic!("Mac OS 9 active frame must use baked title-and-body artwork");
    };
    let active_frame_size = loaded.bundle().atlas().get_icon_size(active_frame_image.icon);
    assert_eq!((active_frame_size.width, active_frame_size.height), (13, 29));
    // Push buttons and checkboxes deliberately own different source geometry. Verifying their
    // baked sizes prevents later theme edits from collapsing both controls back onto the same
    // generic frame merely because both expose the same typed interaction states.
    let button = loaded.bundle().skin().control(ControlRole::Button, ControlState::Enabled(PointerState::Normal));
    let neocogi_ui_runtime::NinePatchContent::Image { image: button_image } = button.patch.content else {
        panic!("Mac OS 9 buttons must use baked artwork");
    };
    let button_size = loaded.bundle().atlas().get_icon_size(button_image.icon);
    assert_eq!((button_size.width, button_size.height), (17, 17));
    let checkbox = loaded
        .bundle()
        .skin()
        .control(ControlRole::Checkbox, ControlState::Enabled(PointerState::Normal));
    let pressed_checkbox = loaded
        .bundle()
        .skin()
        .control(ControlRole::Checkbox, ControlState::Enabled(PointerState::Pressed));
    let focused_checkbox = loaded
        .bundle()
        .skin()
        .control(ControlRole::Checkbox, ControlState::Focused(PointerState::Normal));
    let disabled_checkbox = loaded.bundle().skin().control(ControlRole::Checkbox, ControlState::Disabled);
    let [checkbox_image, pressed_checkbox_image, focused_checkbox_image, disabled_checkbox_image] =
        [checkbox, pressed_checkbox, focused_checkbox, disabled_checkbox].map(|visual| {
            let neocogi_ui_runtime::NinePatchContent::Image { image } = visual.patch.content else {
                panic!("Mac OS 9 checkbox states must use baked artwork");
            };
            image
        });
    let checkbox_size = loaded.bundle().atlas().get_icon_size(checkbox_image.icon);
    assert_eq!((checkbox_size.width, checkbox_size.height), (13, 13));
    assert_ne!(checkbox_image.icon, pressed_checkbox_image.icon);
    assert_ne!(checkbox_image.icon, focused_checkbox_image.icon);
    assert_ne!(checkbox_image.icon, disabled_checkbox_image.icon);
    assert!(matches!(
        loaded.bundle().skin().chrome(ChromeRole::DialogFrame, ChromeState::Active).patch.content,
        neocogi_ui_runtime::NinePatchContent::Image { .. }
    ));

    // Base and active chrome retain their independently authored frame bitmaps without a
    // pointer-state path that could select one from the other.
    let normal_frame = loaded.bundle().skin().chrome(ChromeRole::WindowFrame, ChromeState::Base).patch;
    let active_frame = loaded.bundle().skin().chrome(ChromeRole::WindowFrame, ChromeState::Active).patch;
    assert!(matches!(
        (normal_frame.content, active_frame.content),
        (neocogi_ui_runtime::NinePatchContent::Image { image: normal }, neocogi_ui_runtime::NinePatchContent::Image { image: active })
            if normal.icon != active.icon
    ));

    // Complete Mac caption faces suppress the manager-owned semantic symbol through their
    // ordinary transparent content color. Pressed artwork remains a distinct image for feedback.
    let close_visual = loaded
        .bundle()
        .skin()
        .control(ControlRole::CloseButton, ControlState::Enabled(PointerState::Normal));
    let close_pressed_visual = loaded
        .bundle()
        .skin()
        .control(ControlRole::CloseButton, ControlState::Enabled(PointerState::Pressed));
    assert_eq!(close_visual.content_color.a, 0);
    assert_eq!(close_pressed_visual.content_color.a, 0);
    let (neocogi_ui_runtime::NinePatchContent::Image { image: close_image }, neocogi_ui_runtime::NinePatchContent::Image { image: pressed_image }) =
        (close_visual.patch.content, close_pressed_visual.patch.content)
    else {
        panic!("Mac OS 9 close states must use baked artwork");
    };
    let close_size = loaded.bundle().atlas().get_icon_size(close_image.icon);
    assert_eq!((close_size.width, close_size.height), (13, 14));
    assert_ne!(close_image.icon, pressed_image.icon);

    // Platinum selection uses black rows with white content text, while the popup itself retains
    // its authored one-pixel black and beveled frame.
    let menu_popup = loaded.bundle().skin().menu(MenuRole::Popup, MenuState::Normal).patch;
    let neocogi_ui_runtime::NinePatchContent::Image { image: menu_popup_image } = menu_popup.content else {
        panic!("Mac OS 9 popup must use baked artwork");
    };
    let menu_popup_size = loaded.bundle().atlas().get_icon_size(menu_popup_image.icon);
    assert_eq!((menu_popup_size.width, menu_popup_size.height), (7, 7));
    let selected_text = loaded.bundle().skin().menu(MenuRole::Item, MenuState::Hovered).content_color;
    assert_eq!((selected_text.r, selected_text.g, selected_text.b, selected_text.a), (255, 255, 255, 255));
    for state in [ControlState::Enabled(PointerState::Hovered), ControlState::Enabled(PointerState::Pressed)] {
        // Enabled list items and tree disclosures inherit the palette's flat black selection
        // background. Their foreground must remain readable before the control gains focus.
        let visual = loaded.bundle().skin().control(ControlRole::Item, state);
        assert!(matches!(
            visual.patch.content,
            neocogi_ui_runtime::NinePatchContent::Flat { cells }
                if matches!(cells.center, neocogi_ui_runtime::NinePatchCell::Color { color }
                    if (color.r, color.g, color.b, color.a) == (0, 0, 0, 255))
        ));
        assert_eq!(
            (visual.content_color.r, visual.content_color.g, visual.content_color.b, visual.content_color.a),
            (255, 255, 255, 255)
        );
    }
    for state in [
        ControlState::Focused(PointerState::Normal),
        ControlState::Focused(PointerState::Hovered),
        ControlState::Focused(PointerState::Pressed),
    ] {
        // Focused items use the theme's image-backed selection face and the same contrasting
        // foreground as the inherited enabled selection states above.
        assert!(matches!(
            loaded.bundle().skin().control(ControlRole::Item, state).patch.content,
            neocogi_ui_runtime::NinePatchContent::Image { .. }
        ));
        let content_color = loaded.bundle().skin().control(ControlRole::Item, state).content_color;
        assert_eq!((content_color.r, content_color.g, content_color.b, content_color.a), (255, 255, 255, 255));
    }
}
