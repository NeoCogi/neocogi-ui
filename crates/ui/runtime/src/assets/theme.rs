//! Bulk resolved appearance transfer; parsing and per-frame lookup stay outside this module.
use super::*;
use crate::{Color, SliceInsets, NinePatch, NinePatchCell, NinePatchCells, NinePatchImage, Visual};
use crate::theme::catalog::{ChromeVisuals, ControlVisuals, MenuVisuals, PointerVisuals, SurfaceVisuals};
#[cfg(test)]
use crate::{ChromeState, ControlState, MenuState, PointerState, SurfaceState};

/// Complete exported geometry and role-major structured appearance records.
#[cfg(test)]
pub(super) struct ExportedTheme {
    /// Scalar layout geometry copied together with artwork.
    pub metrics: AssetMetrics,
    /// Independent manager-owned title and caption recipe.
    pub chrome: AssetChrome,
    /// One complete named-state record per structural surface role.
    pub(super) surfaces: Vec<AssetSurfaceVisuals>,
    /// One complete nested named-state record per interactive control role.
    pub(super) controls: Vec<AssetControlVisuals>,
    /// One complete named-state record per menu role.
    pub(super) menus: Vec<AssetMenuVisuals>,
    /// One complete named-state record per window chrome role.
    pub(super) window_chrome: Vec<AssetChromeVisuals>,
}

#[cfg(test)]
impl ExportedTheme {
    /// Expands already-resolved values once; no fallback interpretation crosses the ABI.
    pub(super) fn new(bundle: &SkinBundle) -> Result<Self, NcStatus> {
        let skin = bundle.skin();
        let metrics = skin.metrics;
        let chrome = skin.window_chrome;
        let captions = chrome.captions;
        let atlas = bundle.atlas();
        let icons = atlas.clone_icon_table();
        let result = Self {
            metrics: AssetMetrics {
                default_cell_width: metrics.default_cell_width,
                padding: metrics.padding,
                window_content_insets: out_insets(metrics.window_content_insets),
                spacing: metrics.spacing,
                indent: metrics.indent,
                title_height: metrics.title_height,
                window_border: out_insets(metrics.window_border),
                scrollbar_size: metrics.scrollbar_size,
                thumb_size: metrics.thumb_size,
            },
            chrome: AssetChrome {
                title_alignment: match chrome.title_alignment {
                    crate::WindowTitleAlignment::Leading => TitleAlignment::Leading,
                    crate::WindowTitleAlignment::Centered => TitleAlignment::Centered,
                },
                close_side: out_side(captions.close_side),
                minimize_side: out_side(captions.minimize_side),
                maximize_side: out_side(captions.maximize_side),
                vertical_spacing: captions.vertical_spacing,
                minimum_extent: captions.minimum_extent,
                inner_spacing: captions.inner_spacing,
                show_without_activation: u8::from(captions.show_without_activation),
                minimized_content_height: chrome.minimized_content_height,
                has_backdrop: u8::from(chrome.active_title_backdrop.is_some()),
                backdrop_rgba: chrome.active_title_backdrop.map_or(0, |value| rgba(value.color)),
                backdrop_padding: chrome.active_title_backdrop.map_or(0, |value| value.horizontal_padding),
            },
            surfaces: SurfaceRole::ALL
                .into_iter()
                .map(|role| out_surfaces(skin, role, atlas, &icons))
                .collect::<Result<_, _>>()?,
            controls: ControlRole::ALL
                .into_iter()
                .map(|role| out_controls(skin, role, atlas, &icons))
                .collect::<Result<_, _>>()?,
            menus: MenuRole::ALL
                .into_iter()
                .map(|role| out_menus(skin, role, atlas, &icons))
                .collect::<Result<_, _>>()?,
            window_chrome: ChromeRole::ALL
                .into_iter()
                .map(|role| out_chrome(skin, role, atlas, &icons))
                .collect::<Result<_, _>>()?,
        };
        // Native programmatic skins can contain negative author input normalized on use. The
        // portable resolved form has stricter geometry, so validate it before publishing too.
        read_geometry(result.metrics, result.chrome)?;
        Ok(result)
    }
}

/// Imports complete appearance arrays and geometry before binding local semantic IDs.
pub(super) fn read(data: &neocogi_ui::assets::AssetTables, atlas: &AtlasHandle) -> Result<Skin, NcStatus> {
    let (metrics, chrome) = read_geometry(data.metrics, data.chrome)?;
    let icons = atlas.clone_icon_table();
    let appearances = crate::theme::catalog::AppearanceCatalog::from_values(
        read_surfaces(&data.surfaces, atlas, &icons)?,
        read_controls(&data.controls, atlas, &icons)?,
        read_menus(&data.menus, atlas, &icons)?,
        read_chrome(&data.window_chrome, atlas, &icons)?,
    );
    // Each appearance is already resolved. Skin construction performs only semantic name binding.
    Ok(Skin::from_resolved(atlas, metrics, chrome, appearances))
}

/// Resolves one bounded surface-role table into native named-state records.
fn read_surfaces(
    values: &[AssetSurfaceVisuals],
    atlas: &AtlasHandle,
    icons: &[(String, crate::IconId)],
) -> Result<[SurfaceVisuals; SurfaceRole::COUNT], NcStatus> {
    if values.len() != SurfaceRole::COUNT {
        return Err(NC_E_INVALID_ARG);
    }
    let mut result = [empty_surfaces(); SurfaceRole::COUNT];
    for (destination, source) in result.iter_mut().zip(values) {
        *destination = SurfaceVisuals {
            normal: in_visual(source.normal, atlas, icons)?,
            disabled: in_visual(source.disabled, atlas, icons)?,
        };
    }
    Ok(result)
}

/// Resolves one bounded control-role table without flattening its nested state branches.
fn read_controls(
    values: &[AssetControlVisuals],
    atlas: &AtlasHandle,
    icons: &[(String, crate::IconId)],
) -> Result<[ControlVisuals; ControlRole::COUNT], NcStatus> {
    if values.len() != ControlRole::COUNT {
        return Err(NC_E_INVALID_ARG);
    }
    let mut result = [empty_controls(); ControlRole::COUNT];
    for (destination, source) in result.iter_mut().zip(values) {
        *destination = ControlVisuals {
            disabled: in_visual(source.disabled, atlas, icons)?,
            enabled: in_pointer(source.enabled, atlas, icons)?,
            focused: in_pointer(source.focused, atlas, icons)?,
        };
    }
    Ok(result)
}

/// Resolves one pointer-state transfer record into its native branch.
fn in_pointer(value: AssetPointerVisuals, atlas: &AtlasHandle, icons: &[(String, crate::IconId)]) -> Result<PointerVisuals, NcStatus> {
    Ok(PointerVisuals {
        normal: in_visual(value.normal, atlas, icons)?,
        hovered: in_visual(value.hovered, atlas, icons)?,
        pressed: in_visual(value.pressed, atlas, icons)?,
    })
}

/// Resolves one bounded menu-role table into native named-state records.
fn read_menus(values: &[AssetMenuVisuals], atlas: &AtlasHandle, icons: &[(String, crate::IconId)]) -> Result<[MenuVisuals; MenuRole::COUNT], NcStatus> {
    if values.len() != MenuRole::COUNT {
        return Err(NC_E_INVALID_ARG);
    }
    let mut result = [empty_menus(); MenuRole::COUNT];
    for (destination, source) in result.iter_mut().zip(values) {
        *destination = MenuVisuals {
            normal: in_visual(source.normal, atlas, icons)?,
            hovered: in_visual(source.hovered, atlas, icons)?,
            pressed: in_visual(source.pressed, atlas, icons)?,
            focused: in_visual(source.focused, atlas, icons)?,
            open: in_visual(source.open, atlas, icons)?,
            disabled: in_visual(source.disabled, atlas, icons)?,
        };
    }
    Ok(result)
}

/// Resolves one bounded chrome-role table into native named-state records.
fn read_chrome(values: &[AssetChromeVisuals], atlas: &AtlasHandle, icons: &[(String, crate::IconId)]) -> Result<[ChromeVisuals; ChromeRole::COUNT], NcStatus> {
    if values.len() != ChromeRole::COUNT {
        return Err(NC_E_INVALID_ARG);
    }
    let mut result = [empty_chrome(); ChromeRole::COUNT];
    for (destination, source) in result.iter_mut().zip(values) {
        *destination = ChromeVisuals {
            base: in_visual(source.base, atlas, icons)?,
            active: in_visual(source.active, atlas, icons)?,
            disabled: in_visual(source.disabled, atlas, icons)?,
        };
    }
    Ok(result)
}

/// Returns one harmless initialized native visual used only for fallible import scratch.
fn empty_visual() -> Visual {
    Visual::new(NinePatch::solid(Color::default()), Color::default())
}

/// Initializes a complete surface scratch record before fallible conversion.
fn empty_surfaces() -> SurfaceVisuals {
    SurfaceVisuals {
        normal: empty_visual(),
        disabled: empty_visual(),
    }
}

/// Initializes a complete pointer scratch record before fallible conversion.
fn empty_pointer() -> PointerVisuals {
    PointerVisuals {
        normal: empty_visual(),
        hovered: empty_visual(),
        pressed: empty_visual(),
    }
}

/// Initializes a complete control scratch record before fallible conversion.
fn empty_controls() -> ControlVisuals {
    ControlVisuals {
        disabled: empty_visual(),
        enabled: empty_pointer(),
        focused: empty_pointer(),
    }
}

/// Initializes a complete menu scratch record before fallible conversion.
fn empty_menus() -> MenuVisuals {
    MenuVisuals {
        normal: empty_visual(),
        hovered: empty_visual(),
        pressed: empty_visual(),
        focused: empty_visual(),
        open: empty_visual(),
        disabled: empty_visual(),
    }
}

/// Initializes a complete chrome scratch record before fallible conversion.
fn empty_chrome() -> ChromeVisuals {
    ChromeVisuals {
        base: empty_visual(),
        active: empty_visual(),
        disabled: empty_visual(),
    }
}

/// Validates and converts the one complete metric record for import and live style derivation alike.
pub(super) fn read_metrics(metrics: AssetMetrics) -> Result<crate::SkinMetrics, NcStatus> {
    if [
        metrics.default_cell_width,
        metrics.padding,
        metrics.spacing,
        metrics.indent,
        metrics.title_height,
        metrics.scrollbar_size,
        metrics.thumb_size,
    ]
    .iter()
    .any(|value| *value < 0)
    {
        return Err(NC_E_INVALID_ARG);
    }
    Ok(crate::SkinMetrics {
        default_cell_width: metrics.default_cell_width,
        padding: metrics.padding,
        window_content_insets: in_insets(metrics.window_content_insets)?,
        spacing: metrics.spacing,
        indent: metrics.indent,
        title_height: metrics.title_height,
        window_border: in_insets(metrics.window_border)?,
        scrollbar_size: metrics.scrollbar_size,
        thumb_size: metrics.thumb_size,
    })
}

/// Installs complete geometry into an unpublished candidate after validating scalar and enum domains.
fn read_geometry(metrics: AssetMetrics, chrome: AssetChrome) -> Result<(crate::SkinMetrics, crate::WindowChromeSkin), NcStatus> {
    let metrics = read_metrics(metrics)?;
    if [
        chrome.vertical_spacing,
        chrome.minimum_extent,
        chrome.inner_spacing,
        chrome.minimized_content_height,
        chrome.backdrop_padding,
    ]
    .iter()
    .any(|value| *value < 0)
    {
        return Err(NC_E_INVALID_ARG);
    }
    let window_chrome = crate::WindowChromeSkin {
        title_alignment: match chrome.title_alignment {
            TitleAlignment::Leading => crate::WindowTitleAlignment::Leading,
            TitleAlignment::Centered => crate::WindowTitleAlignment::Centered,
            _ => return Err(NC_E_INVALID_ARG),
        },
        captions: crate::CaptionButtonsSkin {
            close_side: in_side(chrome.close_side)?,
            minimize_side: in_side(chrome.minimize_side)?,
            maximize_side: in_side(chrome.maximize_side)?,
            vertical_spacing: chrome.vertical_spacing,
            minimum_extent: chrome.minimum_extent,
            inner_spacing: chrome.inner_spacing,
            show_without_activation: boolean(chrome.show_without_activation)?,
        },
        minimized_content_height: chrome.minimized_content_height,
        active_title_backdrop: if boolean(chrome.has_backdrop)? {
            Some(crate::TitleBackdropSkin {
                color: color(chrome.backdrop_rgba),
                horizontal_padding: chrome.backdrop_padding,
            })
        } else {
            None
        },
    };
    Ok((metrics, window_chrome))
}

/// Copies both named surface appearances for one role.
#[cfg(test)]
fn out_surfaces(skin: &Skin, role: SurfaceRole, atlas: &AtlasHandle, icons: &[(String, crate::IconId)]) -> Result<AssetSurfaceVisuals, NcStatus> {
    Ok(AssetSurfaceVisuals {
        normal: out_visual(skin.surface(role, SurfaceState::Normal), atlas, icons)?,
        disabled: out_visual(skin.surface(role, SurfaceState::Disabled), atlas, icons)?,
    })
}

/// Copies the three pointer appearances beneath one native control-state branch.
#[cfg(test)]
fn out_pointer(
    skin: &Skin,
    role: ControlRole,
    state: fn(PointerState) -> ControlState,
    atlas: &AtlasHandle,
    icons: &[(String, crate::IconId)],
) -> Result<AssetPointerVisuals, NcStatus> {
    Ok(AssetPointerVisuals {
        normal: out_visual(skin.control(role, state(PointerState::Normal)), atlas, icons)?,
        hovered: out_visual(skin.control(role, state(PointerState::Hovered)), atlas, icons)?,
        pressed: out_visual(skin.control(role, state(PointerState::Pressed)), atlas, icons)?,
    })
}

/// Copies the explicit disabled, enabled, and focused appearances for one control role.
#[cfg(test)]
fn out_controls(skin: &Skin, role: ControlRole, atlas: &AtlasHandle, icons: &[(String, crate::IconId)]) -> Result<AssetControlVisuals, NcStatus> {
    Ok(AssetControlVisuals {
        disabled: out_visual(skin.control(role, ControlState::Disabled), atlas, icons)?,
        enabled: out_pointer(skin, role, ControlState::Enabled, atlas, icons)?,
        focused: out_pointer(skin, role, ControlState::Focused, atlas, icons)?,
    })
}

/// Copies every named menu appearance for one role.
#[cfg(test)]
fn out_menus(skin: &Skin, role: MenuRole, atlas: &AtlasHandle, icons: &[(String, crate::IconId)]) -> Result<AssetMenuVisuals, NcStatus> {
    Ok(AssetMenuVisuals {
        normal: out_visual(skin.menu(role, MenuState::Normal), atlas, icons)?,
        hovered: out_visual(skin.menu(role, MenuState::Hovered), atlas, icons)?,
        pressed: out_visual(skin.menu(role, MenuState::Pressed), atlas, icons)?,
        focused: out_visual(skin.menu(role, MenuState::Focused), atlas, icons)?,
        open: out_visual(skin.menu(role, MenuState::Open), atlas, icons)?,
        disabled: out_visual(skin.menu(role, MenuState::Disabled), atlas, icons)?,
    })
}

/// Copies every named chrome appearance for one role.
#[cfg(test)]
fn out_chrome(skin: &Skin, role: ChromeRole, atlas: &AtlasHandle, icons: &[(String, crate::IconId)]) -> Result<AssetChromeVisuals, NcStatus> {
    Ok(AssetChromeVisuals {
        base: out_visual(skin.chrome(role, ChromeState::Base), atlas, icons)?,
        active: out_visual(skin.chrome(role, ChromeState::Active), atlas, icons)?,
        disabled: out_visual(skin.chrome(role, ChromeState::Disabled), atlas, icons)?,
    })
}

/// Copies one native visual and validates its complete portable representation.
#[cfg(test)]
fn out_visual(visual: Visual, atlas: &AtlasHandle, icons: &[(String, crate::IconId)]) -> Result<AssetVisual, NcStatus> {
    let patch = match visual.patch.content {
        NinePatchContent::Flat { cells } => AssetPatch::Flat {
            top_left: cell(cells.top_left),
            top: cell(cells.top),
            top_right: cell(cells.top_right),
            left: cell(cells.left),
            center: cell(cells.center),
            right: cell(cells.right),
            bottom_left: cell(cells.bottom_left),
            bottom: cell(cells.bottom),
            bottom_right: cell(cells.bottom_right),
        },
        NinePatchContent::Image { image } => AssetPatch::Image {
            icon: icons.iter().position(|(_, id)| *id == image.icon).ok_or(NC_E_INVALID_ARG)? as u32,
            source: out_insets(image.source_insets),
            tint: rgba(image.tint),
        },
    };
    let value = AssetVisual {
        insets: out_insets(visual.patch.insets),
        center_visible: u8::from(visual.patch.center_visible),
        content_color: rgba(visual.content_color),
        patch: patch.into(),
    };
    // Exported native fixtures cross the same geometry and image checks as foreign bundles.
    in_visual(value, atlas, icons)?;
    Ok(value)
}

/// Validates tagged artwork and source slicing before creating a renderer-visible visual.
fn in_visual(value: AssetVisual, atlas: &AtlasHandle, icons: &[(String, crate::IconId)]) -> Result<Visual, NcStatus> {
    let insets = in_insets(value.insets)?;
    let mut patch = match AssetPatch::try_from(value.patch).map_err(|_| NC_E_INVALID_ARG)? {
        AssetPatch::Flat {
            top_left,
            top,
            top_right,
            left,
            center,
            right,
            bottom_left,
            bottom,
            bottom_right,
        } => NinePatch::new(
            insets,
            NinePatchCells {
                top_left: NinePatchCell::color(color(top_left)),
                top: NinePatchCell::color(color(top)),
                top_right: NinePatchCell::color(color(top_right)),
                left: NinePatchCell::color(color(left)),
                center: NinePatchCell::color(color(center)),
                right: NinePatchCell::color(color(right)),
                bottom_left: NinePatchCell::color(color(bottom_left)),
                bottom: NinePatchCell::color(color(bottom)),
                bottom_right: NinePatchCell::color(color(bottom_right)),
            },
        ),
        AssetPatch::Image { icon, source, tint } => {
            let source = in_insets(source)?;
            let icon = icons.get(icon as usize).ok_or(NC_E_INVALID_ARG)?.1;
            let rect = atlas.get_icon_rect(icon);
            if source.left.checked_add(source.right).ok_or(NC_E_INVALID_ARG)? > rect.width
                || source.top.checked_add(source.bottom).ok_or(NC_E_INVALID_ARG)? > rect.height
            {
                return Err(NC_E_INVALID_ARG);
            }
            NinePatch::image(insets, NinePatchImage::new(icon, source, color(tint)))
        }
    };
    patch.center_visible = boolean(value.center_visible)?;
    Ok(Visual::new(patch, color(value.content_color)))
}

/// Copies four named native geometry fields without changing their author-supplied values.
#[cfg(test)]
fn out_insets(value: SliceInsets) -> Insets {
    Insets {
        left: value.left,
        top: value.top,
        right: value.right,
        bottom: value.bottom,
    }
}

/// Rejects negative portable geometry before the native normalization path could hide it.
fn in_insets(value: Insets) -> Result<SliceInsets, NcStatus> {
    if [value.left, value.top, value.right, value.bottom].iter().any(|value| *value < 0) {
        return Err(NC_E_INVALID_ARG);
    }
    Ok(SliceInsets::new(value.left, value.top, value.right, value.bottom))
}

/// Encodes one native caption edge through the concrete shared domain.
#[cfg(test)]
fn out_side(value: crate::CaptionButtonSide) -> CaptionSide {
    match value {
        crate::CaptionButtonSide::Leading => CaptionSide::Leading,
        crate::CaptionButtonSide::Trailing => CaptionSide::Trailing,
    }
}

/// Rejects unknown caption edge tags instead of treating them as a default.
fn in_side(value: CaptionSide) -> Result<crate::CaptionButtonSide, NcStatus> {
    match value {
        CaptionSide::Leading => Ok(crate::CaptionButtonSide::Leading),
        CaptionSide::Trailing => Ok(crate::CaptionButtonSide::Trailing),
        _ => Err(NC_E_INVALID_ARG),
    }
}

/// Validates the explicit byte encoding used for all portable boolean fields.
fn boolean(value: u8) -> Result<bool, NcStatus> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(NC_E_INVALID_ARG),
    }
}

/// Packs color channels independently of native endianness.
#[cfg(test)]
fn rgba(value: Color) -> u32 {
    u32::from_le_bytes([value.r, value.g, value.b, value.a])
}

/// Decodes the renderer's canonical packed color convention.
fn color(value: u32) -> Color {
    let [r, g, b, a] = value.to_le_bytes();
    Color { r, g, b, a }
}

/// Flat transparent cells and explicit emptiness share one canonical wire representation.
#[cfg(test)]
fn cell(value: NinePatchCell) -> u32 {
    match value {
        NinePatchCell::Empty => 0,
        NinePatchCell::Color { color } => rgba(color),
    }
}

#[cfg(test)]
use crate::NinePatchContent;
