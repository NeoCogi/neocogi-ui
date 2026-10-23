//! Canonical resources for provider tests; no fixture substitutes engine behavior.
use crate::{AtlasHandle, ChromeRole, ChromeState, ControlRole, ControlState, NinePatch, Skin, SurfaceRole, SurfaceState, Visual};
use neocogi_ui::assets::{AssetData, AssetTables};
use neocogi_ui_core_contracts::{AssetFont, AssetGlyph, AssetIcon, AssetRange, Insets, Rect, Size};

/// Activates the production GFX service for fixtures that construct independent UI runtimes.
///
/// Only the opt-in test-support feature links the implementation. Repeated registration of
/// this same static description is idempotent, so each test can own an ordinary typed factory
/// reference without an extra singleton, mock compositor, or mutable fixture registry.
#[cfg(feature = "test-support")]
pub fn gfx_factory(registry: neocogi_com::Runtime) -> neocogi_com::ComPtr<neocogi_ui::GfxPluginFactoryAbi> {
    // SAFETY: The description and all referenced code/data remain in the test executable.
    unsafe { registry.register_plugin_descriptor(nc_gfx::plugin_descriptor()) }.expect("linked GFX plugin must register");
    registry
        .acquire_plugin_root(neocogi_com::guid_from_qualified_name("neocogi.gfx.runtime"))
        .expect("linked GFX factory acquisition must succeed")
}

/// Required semantic icon names make the small fixture a valid complete asset catalog.
const ICON_NAMES: [&str; 9] = [
    "white",
    "close",
    "expand",
    "collapse",
    "check",
    "expand_down",
    "open_folder",
    "closed_folder",
    "file",
];

/// Constructs the standard immutable atlas shared within one test's ownership domain.
pub fn test_atlas() -> AtlasHandle {
    // Use the required semantic body name so runtime and skin validation see a complete catalog.
    test_atlas_with_font_sizes(&[("body", 10)])
}

/// Constructs a standard icon set plus caller-selected named font metrics.
pub fn test_atlas_with_font_sizes(fonts: &[(&str, usize)]) -> AtlasHandle {
    AtlasHandle::from_validated_data(test_asset_data_with_font_sizes(fonts), None).expect("shared fixture must satisfy the canonical asset contract")
}

/// Constructs complete canonical asset data for caller-selected named font metrics.
pub fn test_asset_data_with_font_sizes(fonts: &[(&str, usize)]) -> AssetData {
    let mut names = Vec::new();
    let mut font_records = Vec::with_capacity(fonts.len());
    let mut glyphs = Vec::with_capacity(fonts.len() * 3);
    for (name, size) in fonts {
        let name = append_name(&mut names, name);
        let first = glyphs.len() as u32;
        glyphs.extend(['_', 'a', 'b'].map(|character| AssetGlyph {
            scalar: u32::from(character),
            offset_x: 0,
            offset_y: 0,
            advance_x: 8,
            advance_y: 0,
            rect: Rect { x: 0, y: 0, width: 1, height: 1 },
        }));
        font_records.push(AssetFont {
            name,
            glyphs: AssetRange { first, count: 3 },
            line_size: *size as i32,
            baseline: (*size as i32 * 4) / 5,
            font_size: *size as i32,
        });
    }
    let icons = ICON_NAMES
        .iter()
        .map(|name| AssetIcon {
            name: append_name(&mut names, name),
            rect: Rect { x: 0, y: 0, width: 1, height: 1 },
        })
        .collect::<Vec<_>>();
    let appearance = neocogi_appearance::AppearanceTables::flat(Insets::uniform(1), &crate::FlatPalette::default());
    AssetData {
        pixels: vec![0xFF; 4].into(),
        tables: AssetTables {
            size: Size { width: 1, height: 1 },
            fonts: font_records.into(),
            glyphs: glyphs.into(),
            icons: icons.into(),
            names: names.into(),
            metrics: crate::AssetMetrics::default(),
            chrome: neocogi_appearance::default_chrome(),
            surfaces: Vec::from(appearance.surfaces).into(),
            controls: Vec::from(appearance.controls).into(),
            menus: Vec::from(appearance.menus).into(),
            window_chrome: Vec::from(appearance.window_chrome).into(),
        },
    }
}

/// Appends one UTF-8 resource name and returns its canonical byte range.
fn append_name(names: &mut Vec<u8>, name: &str) -> AssetRange {
    let range = AssetRange {
        first: names.len() as u32,
        count: name.len() as u32,
    };
    names.extend_from_slice(name.as_bytes());
    range
}

/// Returns the canonical centered-title chrome recipe used by classic-theme tests.
pub fn classic_mac_chrome(backdrop: crate::Color) -> crate::AssetChrome {
    // Start from the shared canonical default and edit only the authored policy fields. No runtime
    // mirror type or preset-specific branch is introduced by this fixture convenience function.
    let mut chrome = neocogi_appearance::default_chrome();
    chrome.title_alignment = crate::TitleAlignment::Centered;
    chrome.close_side = crate::CaptionSide::Leading;
    chrome.minimize_side = crate::CaptionSide::Trailing;
    chrome.maximize_side = crate::CaptionSide::Trailing;
    chrome.vertical_spacing = 3;
    chrome.minimum_extent = 1;
    chrome.inner_spacing = 4;
    chrome.show_without_activation = 0;
    chrome.has_backdrop = 1;
    chrome.backdrop_rgba = u32::from_le_bytes([backdrop.r, backdrop.g, backdrop.b, backdrop.a]);
    chrome.backdrop_padding = 4;
    chrome
}

/// Constructs a resolved skin whose resource capabilities belong to `atlas`.
pub fn test_skin(atlas: &AtlasHandle) -> Skin {
    // Tests must retain and pass this same handle allocation; reconstructing identical metadata is
    // intentionally a different ownership domain after atlas IDs become scoped.
    Skin::from_atlas(atlas.clone())
}

/// Replaces every structural-surface patch while retaining fixture content colors.
pub fn replace_surface_patches(skin: &mut Skin, role: SurfaceRole, mut patch: impl FnMut(SurfaceState) -> NinePatch) {
    // Iterate the exact surface state domain and publish complete visuals one slot at a time.
    for state in SurfaceState::ALL {
        skin.set_surface(role, state, Visual::new(patch(state), skin.surface(role, state).content_color));
    }
}

/// Replaces every control patch while retaining fixture content colors.
pub fn replace_control_patches(skin: &mut Skin, role: ControlRole, mut patch: impl FnMut(ControlState) -> NinePatch) {
    // Tests express state differences through a typed closure instead of rebuilding a generic table.
    for state in ControlState::ALL {
        skin.set_control(role, state, Visual::new(patch(state), skin.control(role, state).content_color));
    }
}

/// Replaces one chrome patch while retaining that state's content color.
pub fn replace_chrome_patch(skin: &mut Skin, role: ChromeRole, state: ChromeState, patch: NinePatch) {
    // Resolve both halves before publishing one complete chrome visual.
    skin.set_chrome(role, state, Visual::new(patch, skin.chrome(role, state).content_color));
}

#[neocogi_com::com_object(interfaces(neocogi_ui_core_contracts::ITexture))]
/// CPU-only immutable texture supplies dimensions without any native GPU handle.
struct TestImage {
    /// Positive test dimensions used by the real owning facade image wrapper.
    size: neocogi_ui_core_contracts::Size,
}
impl neocogi_ui_core_contracts::ITexture for TestImage {
    /// A real COM projection obeys the production immutable-dimension contract.
    fn size(&self) -> Result<neocogi_ui_core_contracts::Size, neocogi_com::NcStatus> {
        Ok(self.size)
    }
}
/// Constructs an ordinary owned COM image for algorithm measurement fixtures.
pub fn test_image(width: i32, height: i32) -> neocogi_ui::Image {
    neocogi_ui::Image::from_interface(
        TestImage::from_value(TestImage {
            size: neocogi_ui_core_contracts::Size { width, height },
        })
        .query()
        .unwrap(),
    )
    .unwrap()
}

/// Canonical source writers are compiled only for explicit fixture consumers.
pub use crate::assets::fixtures as assets;

/// Persistent typed applications used by runtime integration fixtures.
#[cfg(feature = "test-support")]
pub mod events;
