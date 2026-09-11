//
// Copyright 2026-Present (c) Raja Lehtihet & Wael El Oraiby
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the conditions in LICENSE are met.
//

//! Compiles flat palette rules into portable resolved appearance tables.
use neocogi_ui_contracts::*;
use neocogi_ui::assets::AssetData;

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
    data.tables.surfaces = surfaces.into();
    data.tables.controls = controls.into();
    data.tables.menus = menus.into();
    data.tables.window_chrome = chrome.into();
}

/// Returns the conventional caption geometry in its checked transfer form.
#[cfg(feature = "builder")]
pub(crate) fn default_chrome() -> AssetChrome {
    // Keep builder output identical to the default JSON document without
    // constructing a runtime-only skin representation in this plugin.
    AssetChrome {
        title_alignment: TitleAlignment::LEADING,
        close_side: CaptionSide::TRAILING,
        minimize_side: CaptionSide::TRAILING,
        maximize_side: CaptionSide::TRAILING,
        vertical_spacing: 2,
        minimum_extent: 0,
        inner_spacing: 0,
        show_without_activation: 1,
        minimized_content_height: 2,
        has_backdrop: 0,
        backdrop_rgba: 0,
        backdrop_padding: 0,
    }
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
    let center = match fill {
        Some(color) if color.a != 0 => rgba(color),
        _ => 0,
    };
    AssetVisual {
        insets,
        center_visible: 1,
        content_color: 0,
        patch: AssetPatch::Flat {
            top_left: border,
            top: border,
            top_right: border,
            left: border,
            center,
            right: border,
            bottom_left: border,
            bottom: border,
            bottom_right: border,
        }
        .into(),
    }
}

/// Resolves one structural surface from the flat fallback palette.
fn flat_surface_visual(frame_insets: Insets, palette: &FlatPalette, role: SurfaceRole, state: SurfaceState) -> AssetVisual {
    // Surfaces react only to inherited availability. Window owns the root fill independently of
    // activation, GenericFrame remains hollow, and Panel combines its border with its own fill.
    let mut patch = match (role, state) {
        (SurfaceRole::WINDOW, SurfaceState::NORMAL) => solid(palette.window_background),
        (SurfaceRole::WINDOW, SurfaceState::DISABLED) => solid(palette.disabled_background),
        (SurfaceRole::GENERIC_FRAME, _) => framed(frame_insets, palette.border, None),
        (SurfaceRole::PANEL, SurfaceState::NORMAL) => framed(frame_insets, palette.border, Some(palette.panel_background)),
        (SurfaceRole::PANEL, SurfaceState::DISABLED) => framed(frame_insets, palette.border, Some(palette.disabled_background)),
        _ => unreachable!("flat styles iterate only known surface roles and states"),
    };
    let content_color = match state {
        SurfaceState::NORMAL => palette.text,
        SurfaceState::DISABLED => palette.disabled_foreground,
        _ => unreachable!("flat styles iterate only known surface states"),
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
        ControlRole::BUTTON
        | ControlRole::COMBO
        | ControlRole::SLIDER_THUMB
        | ControlRole::CLOSE_BUTTON
        | ControlRole::MINIMIZE_BUTTON
        | ControlRole::MAXIMIZE_BUTTON
        | ControlRole::RESTORE_BUTTON => match state {
            ControlState::Disabled => framed(frame_insets, palette.border, Some(palette.disabled_background)),
            ControlState::Enabled(PointerState::NORMAL) => framed(frame_insets, palette.border, Some(palette.button)),
            ControlState::Enabled(PointerState::HOVERED) => framed(frame_insets, palette.border, Some(palette.button_hovered)),
            ControlState::Enabled(PointerState::PRESSED) => framed(frame_insets, palette.border, Some(palette.input)),
            ControlState::Focused(PointerState::NORMAL | PointerState::HOVERED) => framed(frame_insets, palette.border, Some(palette.control_focus)),
            ControlState::Focused(PointerState::PRESSED) => framed(frame_insets, palette.border, Some(palette.input)),
            _ => unreachable!("flat styles construct only known pointer states"),
        },
        ControlRole::CHECKBOX | ControlRole::TEXT_INPUT | ControlRole::SLIDER_TRACK | ControlRole::RESIZE_GRIP => match state {
            ControlState::Disabled => framed(frame_insets, palette.border, Some(palette.disabled_background)),
            ControlState::Enabled(PointerState::NORMAL) => framed(frame_insets, palette.border, Some(palette.input)),
            ControlState::Enabled(PointerState::HOVERED | PointerState::PRESSED) => framed(frame_insets, palette.border, Some(palette.input_hovered)),
            ControlState::Focused(PointerState::NORMAL | PointerState::HOVERED) => framed(frame_insets, palette.border, Some(palette.control_focus)),
            ControlState::Focused(PointerState::PRESSED) => framed(frame_insets, palette.border, Some(palette.input_hovered)),
            _ => unreachable!("flat styles construct only known pointer states"),
        },
        ControlRole::ITEM => match state {
            ControlState::Disabled | ControlState::Enabled(PointerState::NORMAL) => solid(transparent),
            ControlState::Enabled(PointerState::HOVERED | PointerState::PRESSED)
            | ControlState::Focused(PointerState::NORMAL | PointerState::HOVERED | PointerState::PRESSED) => solid(palette.selection_background),
            _ => unreachable!("flat styles construct only known pointer states"),
        },
        ControlRole::SCROLLBAR_TRACK => match state {
            ControlState::Disabled => solid(palette.disabled_background),
            ControlState::Enabled(_) | ControlState::Focused(_) => solid(palette.scrollbar_track),
        },
        ControlRole::SCROLLBAR_THUMB => match state {
            ControlState::Disabled => solid(palette.disabled_background),
            ControlState::Enabled(_) | ControlState::Focused(_) => solid(palette.scrollbar_thumb),
        },
        _ => unreachable!("flat styles iterate only known control roles"),
    };
    // Content-color selection is separate from patch grouping because item selection and caption
    // symbols have explicit contrast policies that ordinary controls do not share.
    let content_color = match role {
        ControlRole::ITEM => match state {
            ControlState::Disabled => palette.disabled_foreground,
            ControlState::Enabled(PointerState::NORMAL) => palette.text,
            ControlState::Enabled(PointerState::HOVERED | PointerState::PRESSED)
            | ControlState::Focused(PointerState::NORMAL | PointerState::HOVERED | PointerState::PRESSED) => palette.selection_foreground,
            _ => unreachable!("flat styles construct only known pointer states"),
        },
        ControlRole::CLOSE_BUTTON | ControlRole::MINIMIZE_BUTTON | ControlRole::MAXIMIZE_BUTTON | ControlRole::RESTORE_BUTTON => match state {
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
    let selected = matches!(state, MenuState::HOVERED | MenuState::PRESSED | MenuState::FOCUSED | MenuState::OPEN);
    let transparent = Color { r: 0, g: 0, b: 0, a: 0 };
    let mut patch = match role {
        MenuRole::BAR => match state {
            MenuState::DISABLED => solid(palette.disabled_background),
            _ => solid(palette.menu_background),
        },
        MenuRole::TITLE | MenuRole::ITEM => {
            if selected {
                solid(palette.selection_background)
            } else {
                solid(transparent)
            }
        }
        MenuRole::POPUP => match state {
            MenuState::DISABLED => framed(frame_insets, palette.border, Some(palette.disabled_background)),
            _ => framed(frame_insets, palette.border, Some(palette.menu_background)),
        },
        _ => unreachable!("flat styles iterate only known menu roles"),
    };
    let content_color = match state {
        MenuState::DISABLED => palette.disabled_foreground,
        _ if selected && matches!(role, MenuRole::TITLE | MenuRole::ITEM) => palette.selection_foreground,
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
        ChromeRole::WINDOW_FRAME | ChromeRole::DIALOG_FRAME => match state {
            ChromeState::BASE | ChromeState::DISABLED => framed(frame_insets, palette.border, None),
            ChromeState::ACTIVE => framed(frame_insets.at_least(1), palette.window_active, None),
            _ => unreachable!("flat styles iterate only known chrome states"),
        },
        ChromeRole::TITLE => match state {
            ChromeState::BASE => solid(palette.title_background),
            ChromeState::ACTIVE => solid(palette.window_active),
            ChromeState::DISABLED => solid(palette.title_background),
            _ => unreachable!("flat styles iterate only known chrome states"),
        },
        _ => unreachable!("flat styles iterate only known chrome roles"),
    };
    let content_color = match (role, state) {
        (ChromeRole::TITLE, ChromeState::DISABLED) => palette.disabled_title_foreground,
        (ChromeRole::TITLE, ChromeState::BASE | ChromeState::ACTIVE) => palette.title_foreground,
        (_, ChromeState::DISABLED) => palette.disabled_foreground,
        (_, ChromeState::BASE | ChromeState::ACTIVE) => palette.text,
        _ => unreachable!("flat styles iterate only known chrome states"),
    };
    patch.content_color = rgba(content_color);
    patch
}

#[neocogi_com::com_factory(neocogi_ui_contracts::StyleCompiler)]
#[derive(Default)]
/// Compiles resource revisions without borrowing a UI or retaining a renderer.
pub struct StyleCompilerObject {
    /// Owned diagnostics remain inspectable independently of each immutable result.
    diagnostic: crate::Diagnostic,
}

impl StyleCompiler for StyleCompilerObject {
    /// Copies the base once, applies the explicit edit, validates and publishes in its catalog.
    fn derive_style(
        &self,
        base: neocogi_com::ComRef<'_, IAssetBundleAbi>,
        metrics: AssetMetrics,
        palette: PaletteEditAbi,
    ) -> Result<neocogi_com::ComPtr<IAssetBundleAbi>, neocogi_com::NcStatus> {
        // Validate foreign records and the closed edit tag before using their geometry or artwork.
        let edit = PaletteEdit::try_from(palette).map_err(|_| neocogi_com::NC_E_INVALID_ARG)?;
        let mut data = AssetData::read(base)?;
        data.tables.metrics = metrics;
        if let PaletteEdit::Flat(palette) = edit {
            let insets = data.tables.surfaces[SurfaceRole::GENERIC_FRAME.index() * SurfaceState::COUNT + SurfaceState::NORMAL.index()].insets;
            apply_flat(&mut data, insets, &palette);
            if data.tables.chrome.has_backdrop == 1 {
                data.tables.chrome.backdrop_rgba = rgba(palette.title_background);
            }
        }
        // A rejected candidate leaves both the source and every UI using it unchanged.
        data.validate().inspect_err(|error| {
            self.diagnostic.failure(error);
        })?;
        data.into_revision(base)
    }
}

impl IAssetDiagnostics for StyleCompilerObject {
    /// Returns an owned failure snapshot without a borrow into the compiler.
    fn diagnostic(&self) -> Result<neocogi_com::ComPtr<neocogi_com::INcStringAbi>, neocogi_com::NcStatus> {
        Ok(self.diagnostic.snapshot())
    }
}
