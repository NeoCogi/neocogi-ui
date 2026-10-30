//! Exercise the exact build helper without changing authored application assets.
#[path = "../build_paths.rs"]
mod paths;
use std::path::{Path, PathBuf};

#[test]
fn adjacent_json_paths_preserve_aliases_and_reject_nonportable_or_escaping_names() {
    let definition = Path::new("themes/example/theme.json");
    for (resource, expected) in [
        ("./font.ttf", "themes/example/font.ttf"),
        ("../../assets/font.ttf", "assets/font.ttf"),
        ("alias/image.png", "themes/example/alias/image.png"),
    ] {
        assert_eq!(paths::destination(definition, Path::new(resource)).unwrap(), PathBuf::from(expected));
    }
    for resource in ["../../../outside", "/absolute/font.ttf", "C:/absolute/font.ttf"] {
        assert!(paths::destination(definition, Path::new(resource)).is_err());
    }
    assert_eq!(paths::reference(Path::new("unused/../font.ttf")), PathBuf::from("font.ttf"));
    assert_eq!(paths::reference(Path::new("../../assets/font.ttf")), PathBuf::from("../../assets/font.ttf"));
}

/// Source confinement and JSON path normalization use the same production helpers.
#[test]
fn regular_sources_resolve_and_symlink_sources_reject_before_staging() {
    struct Directory(PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let temporary = Directory(std::env::temp_dir().join(format!("neocogi-theme-paths-{}", std::process::id())));
    std::fs::create_dir_all(temporary.0.join("themes/example/unused")).unwrap();
    let root = temporary.0.canonicalize().unwrap();
    let definition = Path::new("themes/example/theme.json");
    std::fs::write(root.join("themes/example/font.ttf"), b"font fixture").unwrap();
    assert!(paths::source(&root, definition, Path::new("unused")).is_err());
    let source = paths::source(&root, definition, Path::new("unused/../font.ttf")).unwrap();
    assert_eq!(std::fs::read(source).unwrap(), b"font fixture");
    assert_eq!(paths::reference(Path::new("unused/../font.ttf")), PathBuf::from("font.ttf"));
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("font.ttf", root.join("themes/example/alias.ttf")).unwrap();
        assert!(paths::source(&root, definition, Path::new("alias.ttf")).is_err());
        std::os::unix::fs::symlink("unused", root.join("themes/example/linked-dir")).unwrap();
        assert!(paths::source(&root, definition, Path::new("linked-dir/../font.ttf")).is_err());
    }
}
