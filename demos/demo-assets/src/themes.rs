//! Complete compiled theme choices; JSON, fonts and PNGs are processed only by the build host.
use microui_redux::assets::AssetData;

/// Windows 3.11 compiled data also remains usable as an independent source bundle.
pub mod windows_311 {
    include!(concat!(env!("OUT_DIR"), "/windows_311.rs"));
}
/// Windows 95 compiled data uses the same generated provider as the default atlas.
pub mod windows_95 {
    include!(concat!(env!("OUT_DIR"), "/windows_95.rs"));
}
/// Classic Macintosh compiled data needs no special target-side image path.
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
    /// Returns immutable tables suitable for into_revision(base), preserving the application's catalog.
    pub fn data(self) -> Result<AssetData, nc_com::NcStatus> {
        match self {
            Self::Default => super::bundle_data(),
            Self::Windows311 => windows_311::bundle_data(),
            Self::Windows95 => windows_95::bundle_data(),
            Self::MacOs9 => mac_os_9::bundle_data(),
        }
    }
}
