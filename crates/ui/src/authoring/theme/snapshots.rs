//! Storage-sharing and persistence regressions for immutable authoring versions.
use super::*;

/// Palette editing copies the skin section and leaves all other recipes shared.
#[test]
fn candidate_sections_preserve_old_versions_and_json_values() {
    let original = ThemeDocument::from_bytes(
        Path::new("theme.json"),
        br#"{
    "schema_version": 2,
    "name": "Shared",
    "fonts": {
        "texture_width": 512,
        "texture_height": 512,
        "body": {
            "path": "font.ttf",
            "size": 12
        },
        "small": {
            "path": "font.ttf",
            "size": 12
        },
        "title": {
            "path": "font.ttf",
            "size": 12
        },
        "heading": {
            "path": "font.ttf",
            "size": 12
        },
        "mono": {
            "path": "font.ttf",
            "size": 12
        }
    },
    "icons": {
        "close": {
            "path": "close.png"
        },
        "expand": {
            "path": "close.png"
        },
        "collapse": {
            "path": "close.png"
        },
        "check": {
            "path": "close.png"
        },
        "expand_down": {
            "path": "close.png"
        },
        "open_folder": {
            "path": "close.png"
        },
        "closed_folder": {
            "path": "close.png"
        },
        "file": {
            "path": "close.png"
        }
    }
}"#,
    )
    .unwrap();
    let bytes = original.to_bytes().unwrap();
    let mut next = original.clone();
    assert!(Arc::ptr_eq(&original.skin, &next.skin));
    next.skin_mut().palette.button.r ^= 255;
    assert!(!Arc::ptr_eq(&original.skin, &next.skin));
    assert!(Arc::ptr_eq(&original.appearances, &next.appearances));
    assert!(Arc::ptr_eq(original.fonts.as_ref().unwrap(), next.fonts.as_ref().unwrap()));
    assert!(Arc::ptr_eq(original.icons.as_ref().unwrap(), next.icons.as_ref().unwrap()));
    assert_eq!(original.to_bytes().unwrap(), bytes);
    assert_ne!(original, next);
    assert_eq!(ThemeDocument::from_bytes(Path::new("theme.json"), &next.to_bytes().unwrap()).unwrap(), next);
    next.fonts_mut().unwrap().body.size += 1;
    next.icons_mut().unwrap().close.path = PathBuf::from("new.png");
    next.appearances_mut().control.insert(ControlRole::Button, Default::default());
    assert!(!Arc::ptr_eq(original.fonts.as_ref().unwrap(), next.fonts.as_ref().unwrap()));
    assert!(!Arc::ptr_eq(original.icons.as_ref().unwrap(), next.icons.as_ref().unwrap()));
    assert!(!Arc::ptr_eq(&original.appearances, &next.appearances));
    assert_eq!(original.to_bytes().unwrap(), bytes);
}

/// Shared documents can be read on workers while one owner builds a new candidate.
#[test]
fn immutable_documents_are_send_sync_and_optional_recipes_stay_absent() {
    fn shared<T: Send + Sync>() {}
    shared::<ThemeDocument>();
    let original = Arc::new(ThemeDocument::new("Absent"));
    let mut next = (*original).clone();
    assert!(next.fonts_mut().is_none());
    assert!(next.icons_mut().is_none());
    next.name = "Candidate".into();
    assert!(Arc::ptr_eq(&original.skin, &next.skin));
    assert!(Arc::ptr_eq(&original.appearances, &next.appearances));
    assert_eq!(original.name, "Absent");
}
