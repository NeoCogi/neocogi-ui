//! Application-relative names for adjacent authored theme resources.
use std::path::{Component, Path, PathBuf};

/// Preserve the path the copied JSON will resolve, without following source symlinks.
/// Source canonicalization is a separate confinement check performed before copying.
pub fn destination(definition: &Path, resource: &Path) -> Result<PathBuf, String> {
    if definition.is_absolute() || resource.is_absolute() || resource.to_string_lossy().contains(':') {
        return Err("adjacent theme resources must use relative paths".into());
    }
    let mut result = PathBuf::new();
    for component in definition.parent().ok_or("theme has no parent")?.join(resource).components() {
        match component {
            Component::Normal(part) => result.push(part),
            Component::CurDir => (),
            Component::ParentDir if result.pop() => (),
            _ => return Err("theme resource path escapes the application directory".into()),
        }
    }
    if result.as_os_str().is_empty() {
        return Err("theme resource path names no file".into());
    }
    Ok(result)
}

/// Normalize a JSON reference without requiring discarded directory prefixes to exist.
pub fn reference(resource: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in resource.components() {
        match component {
            Component::CurDir => (),
            Component::ParentDir if result.file_name().is_some() => {
                result.pop();
            }
            _ => result.push(component),
        }
    }
    result
}

/// Resolve only regular source components beneath the checkout. Rejecting symlink
/// inputs makes cache invalidation depend on literal files, without a second watcher.
pub fn source(root: &Path, definition: &Path, resource: &Path) -> Result<PathBuf, String> {
    destination(definition, resource)?;
    let mut path = root.to_path_buf();
    for component in definition.parent().ok_or("theme has no parent")?.join(resource).components() {
        match component {
            Component::Normal(part) => {
                path.push(part);
                let metadata = std::fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
                if metadata.is_symlink() {
                    return Err("adjacent theme source paths must not contain symlinks".into());
                }
            }
            Component::ParentDir => {
                path.pop();
            }
            Component::CurDir => (),
            _ => return Err("adjacent theme source paths must be relative".into()),
        }
    }
    let path = path.canonicalize().map_err(|error| error.to_string())?;
    path.strip_prefix(root).map_err(|error| error.to_string())?;
    if !path.is_file() {
        return Err("adjacent recipe inputs must be regular files".into());
    }
    Ok(path)
}
