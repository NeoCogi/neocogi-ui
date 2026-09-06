//
// Copyright 2026-Present (c) Raja Lehtihet & Wael El Oraiby
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the conditions in LICENSE are met.
//

//! Compiles flat palette rules into portable resolved appearance tables.
use microui_contracts::*;
use microui_contracts::appearance::*;
use microui_redux::assets::AssetData;

/// Resolves every state once, so UI painting only selects stored values.
pub(crate) fn apply_flat(data: &mut AssetData, insets: Insets, palette: &FlatPalette) {
    // These loops enumerate closed role/state domains; no behavior or callback is stored.
    let mut surfaces = Vec::new();
    for role in SurfaceRole::ALL {
        for state in SurfaceState::ALL {
            surfaces.push(flat_surface_visual(insets, palette, role, state));
        }
    }
    let mut controls = Vec::new();
    for role in ControlRole::ALL {
        for state in ControlState::ALL {
            controls.push(flat_control_visual(insets, palette, role, state));
        }
    }
    let mut menus = Vec::new();
    for role in MenuRole::ALL {
        for state in MenuState::ALL {
            menus.push(flat_menu_visual(insets, palette, role, state));
        }
    }
    let mut chrome = Vec::new();
    for role in ChromeRole::ALL {
        for state in ChromeState::ALL {
            chrome.push(flat_chrome_visual(insets, palette, role, state));
        }
    }
    data.surfaces = surfaces.into();
    data.controls = controls.into();
    data.menus = menus.into();
    data.window_chrome = chrome.into();
}

/// Returns the authored default geometry used by fresh bundles.
pub(crate) fn default_metrics() -> AssetMetrics {
    AssetMetrics {
        default_cell_width: 68, padding: 5, window_content_insets: uniform(5),
        spacing: 4, indent: 24, title_height: 24, window_border: uniform(1),
        scrollbar_size: 12, thumb_size: 8,
    }
}

/// Returns the authored default caption policy used by fresh bundles.
pub(crate) fn default_chrome() -> AssetChrome {
    AssetChrome {
        title_alignment: TitleAlignment::LEADING,
        close_side: CaptionSide::TRAILING, minimize_side: CaptionSide::TRAILING, maximize_side: CaptionSide::TRAILING,
        vertical_spacing: 2, minimum_extent: 0, inner_spacing: 0,
        show_without_activation: 1, minimized_content_height: 2,
        has_backdrop: 0, backdrop_rgba: 0, backdrop_padding: 0,
    }
}

/// Stores four equal inset values without a second geometry type.
pub(crate) const fn uniform(value: i32) -> Insets {
    Insets { left: value, top: value, right: value, bottom: value }
}

/// Makes every active frame edge visible while preserving thicker authored edges.
fn at_least_one(value: Insets) -> Insets {
    Insets { left: value.left.max(1), top: value.top.max(1), right: value.right.max(1), bottom: value.bottom.max(1) }
}

/// Packs channels in the renderer's explicit little-endian RGBA convention.
pub(crate) const fn rgba(color: Color) -> u32 {
    u32::from_le_bytes([color.r, color.g, color.b, color.a])
}

/// Produces a solid center with collapsed border geometry.
fn solid(fill: Color) -> AssetVisual {
    framed(Insets::default(), Color::default(), Some(fill))
}

/// Produces eight equal border cells and an optional center, retaining exact insets.
fn framed(insets: Insets, border: Color, fill: Option<Color>) -> AssetVisual {
    // Canonicalize fully transparent cells just like the runtime drawing vocabulary.
    let border = if border.a == 0 { 0 } else { rgba(border) };
    let center = match fill { Some(color) if color.a != 0 => rgba(color), _ => 0 };
    AssetVisual {
        insets, center_visible: 1, content_color: 0,
        patch: AssetPatch::Flat {
            top_left: border, top: border, top_right: border, left: border, center,
            right: border, bottom_left: border, bottom: border, bottom_right: border,
        }.into(),
    }
}

/// Resolves one structural surface from the flat fallback palette.
fn flat_surface_visual(frame_insets: Insets, palette: &FlatPalette, role: SurfaceRole, state: SurfaceState) -> AssetVisual {
    // Surfaces react only to inherited availability. Window owns the root fill independently of
    // activation, GenericFrame remains hollow, and Panel combines its border with its own fill.
    let mut patch = match (role, state) {
        (SurfaceRole::Window, SurfaceState::Normal) => solid(palette.window_background),
        (SurfaceRole::Window, SurfaceState::Disabled) => solid(palette.disabled_background),
        (SurfaceRole::GenericFrame, _) => framed(frame_insets, palette.border, None),
        (SurfaceRole::Panel, SurfaceState::Normal) => framed(frame_insets, palette.border, Some(palette.panel_background)),
        (SurfaceRole::Panel, SurfaceState::Disabled) => framed(frame_insets, palette.border, Some(palette.disabled_background)),
    };
    let content_color = match state {
        SurfaceState::Normal => palette.text,
        SurfaceState::Disabled => palette.disabled_foreground,
    };
    patch.content_color = rgba(content_color);
    patch
}

/// Resolves one interactive control from its concrete role and nested control state.
fn flat_control_visual(frame_insets: Insets, palette: &FlatPalette, role: ControlRole, state: ControlState) -> AssetVisual {
    // Each role group spells out its meaningful fill policy. A newly added role therefore fails
    // this exhaustive match instead of inheriting a generic button or body fallback.
    let transparent = Color { r: 0, g: 0, b: 0, a: 0 };
    let mut patch = match role {
        ControlRole::Button
        | ControlRole::Combo
        | ControlRole::SliderThumb
        | ControlRole::CloseButton
        | ControlRole::MinimizeButton
        | ControlRole::MaximizeButton
        | ControlRole::RestoreButton => match state {
            ControlState::Disabled => framed(frame_insets, palette.border, Some(palette.disabled_background)),
            ControlState::Enabled(PointerState::Normal) => framed(frame_insets, palette.border, Some(palette.button)),
            ControlState::Enabled(PointerState::Hovered) => framed(frame_insets, palette.border, Some(palette.button_hovered)),
            ControlState::Enabled(PointerState::Pressed) => framed(frame_insets, palette.border, Some(palette.input)),
            ControlState::Focused(PointerState::Normal | PointerState::Hovered) => framed(frame_insets, palette.border, Some(palette.control_focus)),
            ControlState::Focused(PointerState::Pressed) => framed(frame_insets, palette.border, Some(palette.input)),
        },
        ControlRole::Checkbox | ControlRole::TextInput | ControlRole::SliderTrack | ControlRole::ResizeGrip => match state {
            ControlState::Disabled => framed(frame_insets, palette.border, Some(palette.disabled_background)),
            ControlState::Enabled(PointerState::Normal) => framed(frame_insets, palette.border, Some(palette.input)),
            ControlState::Enabled(PointerState::Hovered | PointerState::Pressed) => framed(frame_insets, palette.border, Some(palette.input_hovered)),
            ControlState::Focused(PointerState::Normal | PointerState::Hovered) => framed(frame_insets, palette.border, Some(palette.control_focus)),
            ControlState::Focused(PointerState::Pressed) => framed(frame_insets, palette.border, Some(palette.input_hovered)),
        },
        ControlRole::Item => match state {
            ControlState::Disabled | ControlState::Enabled(PointerState::Normal) => solid(transparent),
            ControlState::Enabled(PointerState::Hovered | PointerState::Pressed)
            | ControlState::Focused(PointerState::Normal | PointerState::Hovered | PointerState::Pressed) => solid(palette.selection_background),
        },
        ControlRole::ScrollbarTrack => match state {
            ControlState::Disabled => solid(palette.disabled_background),
            ControlState::Enabled(_) | ControlState::Focused(_) => solid(palette.scrollbar_track),
        },
        ControlRole::ScrollbarThumb => match state {
            ControlState::Disabled => solid(palette.disabled_background),
            ControlState::Enabled(_) | ControlState::Focused(_) => solid(palette.scrollbar_thumb),
        },
    };
    // Content-color selection is separate from patch grouping because item selection and caption
    // symbols have explicit contrast policies that ordinary controls do not share.
    let content_color = match role {
        ControlRole::Item => match state {
            ControlState::Disabled => palette.disabled_foreground,
            ControlState::Enabled(PointerState::Normal) => palette.text,
            ControlState::Enabled(PointerState::Hovered | PointerState::Pressed)
            | ControlState::Focused(PointerState::Normal | PointerState::Hovered | PointerState::Pressed) => palette.selection_foreground,
        },
        ControlRole::CloseButton | ControlRole::MinimizeButton | ControlRole::MaximizeButton | ControlRole::RestoreButton => match state {
            ControlState::Disabled => palette.disabled_title_foreground,
            ControlState::Enabled(_) | ControlState::Focused(_) => palette.title_foreground,
        },
        _ => match state {
            ControlState::Disabled => palette.disabled_foreground,
            ControlState::Enabled(_) | ControlState::Focused(_) => palette.text,
        },
    };
    patch.content_color = rgba(content_color);
    patch
}

/// Resolves one menu surface or entry from the menu-specific state domain.
fn flat_menu_visual(frame_insets: Insets, palette: &FlatPalette, role: MenuRole, state: MenuState) -> AssetVisual {
    // Open is a menu state rather than a duplicate title role. Popup owners and selected submenu
    // entries consequently share the same explicit selection precedence.
    let selected = matches!(state, MenuState::Hovered | MenuState::Pressed | MenuState::Focused | MenuState::Open);
    let transparent = Color { r: 0, g: 0, b: 0, a: 0 };
    let mut patch = match role {
        MenuRole::Bar => match state {
            MenuState::Disabled => solid(palette.disabled_background),
            _ => solid(palette.menu_background),
        },
        MenuRole::Title | MenuRole::Item => {
            if selected {
                solid(palette.selection_background)
            } else {
                solid(transparent)
            }
        }
        MenuRole::Popup => match state {
            MenuState::Disabled => framed(frame_insets, palette.border, Some(palette.disabled_background)),
            _ => framed(frame_insets, palette.border, Some(palette.menu_background)),
        },
    };
    let content_color = match state {
        MenuState::Disabled => palette.disabled_foreground,
        _ if selected && matches!(role, MenuRole::Title | MenuRole::Item) => palette.selection_foreground,
        _ => palette.menu_foreground,
    };
    patch.content_color = rgba(content_color);
    patch
}

/// Resolves one window-chrome visual from activation and availability only.
fn flat_chrome_visual(frame_insets: Insets, palette: &FlatPalette, role: ChromeRole, state: ChromeState) -> AssetVisual {
    // Window activation belongs solely to chrome. Control descendants continue resolving their
    // own enabled, focus, and pointer states independently of the active window.
    let mut patch = match role {
        ChromeRole::WindowFrame | ChromeRole::DialogFrame => match state {
            ChromeState::Base | ChromeState::Disabled => framed(frame_insets, palette.border, None),
            ChromeState::Active => framed(at_least_one(frame_insets), palette.window_active, None),
        },
        ChromeRole::Title => match state {
            ChromeState::Base => solid(palette.title_background),
            ChromeState::Active => solid(palette.window_active),
            ChromeState::Disabled => solid(palette.title_background),
        },
    };
    let content_color = match (role, state) {
        (ChromeRole::Title, ChromeState::Disabled) => palette.disabled_title_foreground,
        (ChromeRole::Title, ChromeState::Base | ChromeState::Active) => palette.title_foreground,
        (_, ChromeState::Disabled) => palette.disabled_foreground,
        (_, ChromeState::Base | ChromeState::Active) => palette.text,
    };
    patch.content_color = rgba(content_color);
    patch
}

