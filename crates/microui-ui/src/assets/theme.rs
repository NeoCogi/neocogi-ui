//! Bulk resolved appearance transfer; parsing and per-frame lookup stay outside this module.
use super::*;
use crate::{Color, SliceInsets, NinePatch, NinePatchCell, NinePatchCells, NinePatchContent, NinePatchImage, Visual};
use microui_contracts::appearance::*;

/// Complete exported geometry and role-major appearance arrays.
pub(super) struct ExportedTheme {
    /// Scalar layout geometry copied together with artwork.
    pub metrics: AssetMetrics,
    /// Independent manager-owned title and caption recipe.
    pub chrome: AssetChrome,
    /// Structural surfaces, in canonical role/state order.
    pub(super) surfaces: Vec<AssetVisual>,
    /// Interactive control parts, in canonical role/state order.
    pub(super) controls: Vec<AssetVisual>,
    /// Menu surfaces and rows, in canonical role/state order.
    pub(super) menus: Vec<AssetVisual>,
    /// Window activation visuals, in canonical role/state order.
    pub(super) window_chrome: Vec<AssetVisual>,
}

impl ExportedTheme {
    /// Expands already-resolved values once; no fallback interpretation crosses the ABI.
    pub(super) fn new(bundle: &SkinBundle) -> Result<Self, NcStatus> {
        let skin = bundle.skin();
        let metrics = skin.metrics;
        let chrome = skin.window_chrome;
        let captions = chrome.captions;
        let mut result = Self {
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
                    crate::WindowTitleAlignment::Leading => TitleAlignment::LEADING,
                    crate::WindowTitleAlignment::Centered => TitleAlignment::CENTERED,
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
            surfaces: Vec::new(),
            controls: Vec::new(),
            menus: Vec::new(),
            window_chrome: Vec::new(),
        };
        let icons = bundle.atlas().clone_icon_table();
        // The shared role and state declarations define both traversal order and exact table size.
        macro_rules! family {
            ($field:ident, $role:ident, $state:ident, $get:ident) => {
                for role in $role::ALL {
                    for state in $state::ALL {
                        result.$field.push(out_visual(skin.$get(role, state), &icons)?);
                    }
                }
            };
        }
        family!(surfaces, SurfaceRole, SurfaceState, surface);
        family!(controls, ControlRole, ControlState, control);
        family!(menus, MenuRole, MenuState, menu);
        family!(window_chrome, ChromeRole, ChromeState, chrome);
        // Native programmatic skins can contain negative author input normalized on use. The
        // portable resolved form has stricter geometry, so validate it before publishing too.
        let mut checked = skin.clone();
        read_geometry(result.metrics, result.chrome, &mut checked)?;
        for values in [&result.surfaces, &result.controls, &result.menus, &result.window_chrome] {
            for visual in values {
                in_visual(*visual, bundle.atlas(), &icons)?;
            }
        }
        Ok(result)
    }
}

/// Copies all resolved families into one unpublished native skin after bounded metadata import.
pub(super) fn read(data: &microui_redux::assets::AssetData, atlas: &AtlasHandle, skin: &mut Skin) -> Result<(), NcStatus> {
    read_geometry(data.metrics, data.chrome, skin)?;
    let icons = atlas.clone_icon_table();
    macro_rules! family {
        ($field:ident, $role:ident, $state:ident, $set:ident) => {{
            let values = &data.$field;
            for role in $role::ALL {
                for state in $state::ALL {
                    let value = values[role.index() * $state::COUNT + state.index()];
                    skin.$set(role, state, in_visual(value, atlas, &icons)?);
                }
            }
        }};
    }
    family!(surfaces, SurfaceRole, SurfaceState, set_surface);
    family!(controls, ControlRole, ControlState, set_control);
    family!(menus, MenuRole, MenuState, set_menu);
    family!(window_chrome, ChromeRole, ChromeState, set_chrome);
    Ok(())
}

/// Validates and converts the one complete metric record for import and live style derivation alike.
pub(super) fn read_metrics(metrics: AssetMetrics) -> Result<crate::SkinMetrics, NcStatus> {
    if [metrics.default_cell_width, metrics.padding, metrics.spacing, metrics.indent,
        metrics.title_height, metrics.scrollbar_size, metrics.thumb_size].iter().any(|value| *value < 0) {
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
fn read_geometry(metrics: AssetMetrics, chrome: AssetChrome, skin: &mut Skin) -> Result<(), NcStatus> {
    skin.metrics = read_metrics(metrics)?;
    if [chrome.vertical_spacing, chrome.minimum_extent, chrome.inner_spacing,
        chrome.minimized_content_height, chrome.backdrop_padding].iter().any(|value| *value < 0) {
        return Err(NC_E_INVALID_ARG);
    }
    skin.window_chrome = crate::WindowChromeSkin {
        title_alignment: match chrome.title_alignment {
            TitleAlignment::LEADING => crate::WindowTitleAlignment::Leading,
            TitleAlignment::CENTERED => crate::WindowTitleAlignment::Centered,
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
    Ok(())
}

/// Copies named flat cells or resolves an exact native icon to its bundle-local table position.
fn out_visual(visual: Visual, icons: &[(String, crate::IconId)]) -> Result<AssetVisual, NcStatus> {
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
    Ok(AssetVisual {
        insets: out_insets(visual.patch.insets),
        center_visible: u8::from(visual.patch.center_visible),
        content_color: rgba(visual.content_color),
        patch: patch.into(),
    })
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
fn out_side(value: crate::CaptionButtonSide) -> CaptionSide {
    match value {
        crate::CaptionButtonSide::Leading => CaptionSide::LEADING,
        crate::CaptionButtonSide::Trailing => CaptionSide::TRAILING,
    }
}

/// Rejects unknown caption edge tags instead of treating them as a default.
fn in_side(value: CaptionSide) -> Result<crate::CaptionButtonSide, NcStatus> {
    match value {
        CaptionSide::LEADING => Ok(crate::CaptionButtonSide::Leading),
        CaptionSide::TRAILING => Ok(crate::CaptionButtonSide::Trailing),
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
fn rgba(value: Color) -> u32 {
    u32::from_le_bytes([value.r, value.g, value.b, value.a])
}

/// Decodes the renderer's canonical packed color convention.
fn color(value: u32) -> Color {
    let [r, g, b, a] = value.to_le_bytes();
    Color { r, g, b, a }
}

/// Flat transparent cells and explicit emptiness share one canonical wire representation.
fn cell(value: NinePatchCell) -> u32 {
    match value {
        NinePatchCell::Empty => 0,
        NinePatchCell::Color { color } => rgba(color),
    }
}
