//! Complete theme choices from the skin plugin, or explicitly embedded build-time tables.
use neocogi_ui::IAssetBundleAbi;
use neocogi_com::{ComPtr, ComRef, NcStatus};

/// Windows 3.11 compiled data also remains usable as an independent source bundle.
#[cfg(feature = "embed-assets")]
pub mod windows_311 {
    include!(concat!(env!("OUT_DIR"), "/windows_311.rs"));
}
/// Windows 95 compiled data uses the same generated provider as the default atlas.
#[cfg(feature = "embed-assets")]
pub mod windows_95 {
    include!(concat!(env!("OUT_DIR"), "/windows_95.rs"));
}
/// Classic Macintosh compiled data needs no special target-side image path.
#[cfg(feature = "embed-assets")]
pub mod mac_os_9 {
    include!(concat!(env!("OUT_DIR"), "/mac_os_9.rs"));
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Exhaustive full-demo choices keep menu selection independent of source paths and array slots.
pub enum Theme {
    /// Programmatic flat visuals with the editable named palette.
    Default,
    /// Windows for Workgroups 3.11 artwork.
    Windows311,
    /// Windows 95 bevel artwork.
    Windows95,
    /// Classic Macintosh Platinum artwork.
    MacOs9,
}
impl Theme {
    /// Stable presentation order used by theme menus and smoke validation.
    pub const ALL: [Self; 4] = [Self::Default, Self::Windows311, Self::Windows95, Self::MacOs9];
    /// User-visible label remains exhaustive when a new theme is added.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Default => "Default Skin",
            Self::Windows311 => "Windows 3.11",
            Self::Windows95 => "Windows 95",
            Self::MacOs9 => "Mac OS 9",
        }
    }
    /// Loads a pristine theme into the original catalog, keeping existing widget resource handles valid.
    /// Native callers must register the skin plugin first; its JSON loader owns all theme compilation.
    pub fn load(self, base: ComRef<'_, IAssetBundleAbi>) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        if self == Self::Default {
            return Ok(base.add_ref());
        }
        #[cfg(feature = "embed-assets")]
        {
            let data = match self {
                Self::Windows311 => windows_311::bundle_data()?,
                Self::Windows95 => windows_95::bundle_data()?,
                Self::MacOs9 => mac_os_9::bundle_data()?,
                Self::Default => unreachable!(),
            };
            data.into_revision(base)
        }
        #[cfg(not(feature = "embed-assets"))]
        {
            use neocogi_ui::{GfxPluginFactoryAbi, NativePath, SkinPluginFactoryAbi};
            let path = match self {
                Self::Windows311 => "themes/windows-3.11/theme.json",
                Self::Windows95 => "themes/windows-95/theme.json",
                Self::MacOs9 => "themes/mac-os-9/theme.json",
                Self::Default => unreachable!(),
            };
            let path = super::files::asset_path(path).map_err(|_| neocogi_com::NC_E_LOAD_FAILED)?;
            let runtime = neocogi_com::Runtime::global();
            let gfx = runtime.activate_plugin_factory::<GfxPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.gfx.runtime"))?;
            let assets = gfx.asset_loader()?;
            let publisher = gfx.asset_publisher()?;
            let factory = runtime.activate_plugin_factory::<SkinPluginFactoryAbi>(neocogi_com::guid_from_qualified_name("neocogi.skin.runtime"))?;
            let loader = factory.theme_loader(assets.as_com_ref(), publisher.as_com_ref())?;
            loader.load_theme(NativePath::new(&path).as_interface(), base)
        }
    }
}
