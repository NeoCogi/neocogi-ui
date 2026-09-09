//! Native clients forward the shared recipe to COM; parsing and packing remain in the asset plugin.
use super::recipe_data as data;
use neocogi_ui::{AssetLoaderAbi, IAssetBundleAbi, NativePath, Size};
use neocogi_com::{ComPtr, NcCopyFromBufferOffset, NcStatus};

/// Resolves a package-relative resource independently of the process working directory.
pub(super) fn asset_path(relative: &str) -> std::io::Result<std::path::PathBuf> {
    let executable = std::env::current_exe()?;
    let directory = executable.parent().ok_or_else(|| std::io::Error::other("executable has no parent"))?;
    Ok(directory.join(relative))
}

/// Constructs the default bundle after the host registers its adjacent plugins.
/// The provider owns every decoder, font rasterizer and packing operation; no UI implementation is linked here.
pub fn load_bundle() -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
    let directory = asset_path("").map_err(|_| neocogi_com::NC_E_LOAD_FAILED)?;
    // SAFETY: The trusted asset plugin declares a no-argument constructor for this class.
    let loader: ComPtr<AssetLoaderAbi> = neocogi_com::Runtime::global().construct(())?;
    let builder = loader.create_builder(
        Size {
            width: data::SIZE.0 as i32,
            height: data::SIZE.1 as i32,
        },
        NativePath::new(&directory.join(data::WHITE)).as_interface(),
    )?;
    for &(name, path) in data::ICONS {
        builder.add_icon(
            NcCopyFromBufferOffset::new(0, name.as_bytes()),
            NativePath::new(&directory.join(path)).as_interface(),
        )?;
    }
    for &(name, path, pixels) in data::FONTS {
        builder.add_font(
            NcCopyFromBufferOffset::new(0, name.as_bytes()),
            NativePath::new(&directory.join(path)).as_interface(),
            pixels,
        )?;
    }
    builder.finish()
}
