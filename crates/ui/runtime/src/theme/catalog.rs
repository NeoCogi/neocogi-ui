//
// Copyright 2026-Present (c) Raja Lehtihet & Wael El Oraiby
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the conditions in LICENSE are met.
//

//! Family-specific storage for resolved runtime appearances.

use std::rc::Rc;

use super::{ChromeRole, ChromeState, ControlRole, ControlState, MenuRole, MenuState, SurfaceRole, SurfaceState, Visual};
#[cfg(test)]
use super::{FlatPalette, PointerState};
#[cfg(test)]
use crate::{Color, NinePatch, SliceInsets};

/// Complete resolved appearance values grouped by their meaningful role and state domains.
///
/// Cloned skins share this immutable block. The first edit detaches it with ordinary value
/// semantics, while reads remain direct enum-indexed array lookups without maps or type erasure.
#[derive(Clone)]
pub(crate) struct AppearanceCatalog {
    /// Shared family arrays detached only when a cloned skin is customized.
    values: Rc<AppearanceValues>,
}

/// Concrete arrays underlying one appearance catalog.
#[derive(Clone)]
struct AppearanceValues {
    /// Structural surface values indexed by [`SurfaceRole`] and [`SurfaceState`].
    surfaces: [[Visual; SurfaceState::COUNT]; SurfaceRole::COUNT],
    /// Interactive control values indexed by [`ControlRole`] and [`ControlState`].
    controls: [[Visual; ControlState::COUNT]; ControlRole::COUNT],
    /// Menu values indexed by [`MenuRole`] and [`MenuState`].
    menus: [[Visual; MenuState::COUNT]; MenuRole::COUNT],
    /// Window chrome values indexed by [`ChromeRole`] and [`ChromeState`].
    chrome: [[Visual; ChromeState::COUNT]; ChromeRole::COUNT],
}

impl AppearanceCatalog {
    /// Stores four fully resolved families without interpreting authoring defaults.
    pub(crate) fn from_values(
        surfaces: [[Visual; SurfaceState::COUNT]; SurfaceRole::COUNT],
        controls: [[Visual; ControlState::COUNT]; ControlRole::COUNT],
        menus: [[Visual; MenuState::COUNT]; MenuRole::COUNT],
        chrome: [[Visual; ChromeState::COUNT]; ChromeRole::COUNT],
    ) -> Self {
        // The importer has resolved every image slot into its local typed resource identity.
        Self {
            values: Rc::new(AppearanceValues { surfaces, controls, menus, chrome }),
        }
    }

    /// Compiles the one concrete flat-palette representation into complete runtime arrays.
    #[cfg(test)]
    pub(crate) fn from_flat_palette(frame_insets: SliceInsets, palette: &FlatPalette) -> Self {
        // Local closures only drive array initialization; they are not retained, accepted through
        // an API, or treated as interchangeable appearance sources. Each invokes one concrete,
        // exhaustively matched family function below.
        let surfaces = std::array::from_fn(|role_index| {
            let role = SurfaceRole::ALL[role_index];
            std::array::from_fn(|state_index| flat_surface_visual(frame_insets, palette, role, SurfaceState::ALL[state_index]))
        });
        let controls = std::array::from_fn(|role_index| {
            let role = ControlRole::ALL[role_index];
            std::array::from_fn(|state_index| flat_control_visual(frame_insets, palette, role, ControlState::ALL[state_index]))
        });
        let menus = std::array::from_fn(|role_index| {
            let role = MenuRole::ALL[role_index];
            std::array::from_fn(|state_index| flat_menu_visual(frame_insets, palette, role, MenuState::ALL[state_index]))
        });
        let chrome = std::array::from_fn(|role_index| {
            let role = ChromeRole::ALL[role_index];
            std::array::from_fn(|state_index| flat_chrome_visual(frame_insets, palette, role, ChromeState::ALL[state_index]))
        });
        Self {
            // UI ownership is single-threaded throughout the retained tree, so Rc expresses the
            // actual sharing contract without implying synchronization that no caller can use.
            values: Rc::new(AppearanceValues { surfaces, controls, menus, chrome }),
        }
    }

    /// Returns one structural surface visual.
    pub(crate) fn surface(&self, role: SurfaceRole, state: SurfaceState) -> Visual {
        // Both indices belong to the surface family, so unrelated states cannot enter this lookup.
        self.values.surfaces[role.index()][state.index()]
    }

    /// Replaces one structural surface visual.
    pub(crate) fn set_surface(&mut self, role: SurfaceRole, state: SurfaceState, visual: Visual) {
        // Detach shared storage before changing exactly one typed family slot.
        Rc::make_mut(&mut self.values).surfaces[role.index()][state.index()] = visual;
    }

    /// Returns one interactive control visual.
    pub(crate) fn control(&self, role: ControlRole, state: ControlState) -> Visual {
        // Control lookup accepts the nested enabled/focused pointer state and no other domain.
        self.values.controls[role.index()][state.index()]
    }

    /// Replaces one interactive control visual.
    pub(crate) fn set_control(&mut self, role: ControlRole, state: ControlState, visual: Visual) {
        // Detach shared storage before changing exactly one typed family slot.
        Rc::make_mut(&mut self.values).controls[role.index()][state.index()] = visual;
    }

    /// Returns one menu visual.
    pub(crate) fn menu(&self, role: MenuRole, state: MenuState) -> Visual {
        // Menu lookup is the only API that can receive the Open state.
        self.values.menus[role.index()][state.index()]
    }

    /// Replaces one menu visual.
    pub(crate) fn set_menu(&mut self, role: MenuRole, state: MenuState, visual: Visual) {
        // Detach shared storage before changing exactly one typed family slot.
        Rc::make_mut(&mut self.values).menus[role.index()][state.index()] = visual;
    }

    /// Returns one window chrome visual.
    pub(crate) fn chrome(&self, role: ChromeRole, state: ChromeState) -> Visual {
        // Chrome lookup accepts activation state without admitting pointer interaction states.
        self.values.chrome[role.index()][state.index()]
    }

    /// Replaces one window chrome visual.
    pub(crate) fn set_chrome(&mut self, role: ChromeRole, state: ChromeState, visual: Visual) {
        // Detach shared storage before changing exactly one typed family slot.
        Rc::make_mut(&mut self.values).chrome[role.index()][state.index()] = visual;
    }

    /// Iterates over every resolved visual for atlas-ownership validation.
    pub(crate) fn iter(&self) -> impl Iterator<Item = &Visual> {
        // Flatten each concrete family locally; no synthetic cross-family role or state is created.
        self.values
            .surfaces
            .iter()
            .flatten()
            .chain(self.values.controls.iter().flatten())
            .chain(self.values.menus.iter().flatten())
            .chain(self.values.chrome.iter().flatten())
    }
}

/// Resolves one structural surface from the flat fallback palette.
#[cfg(test)]
fn flat_surface_visual(frame_insets: SliceInsets, palette: &FlatPalette, role: SurfaceRole, state: SurfaceState) -> Visual {
    // Surfaces react only to inherited availability. Window owns the root fill independently of
    // activation, GenericFrame remains hollow, and Panel combines its border with its own fill.
    let patch = match (role, state) {
        (SurfaceRole::WINDOW, SurfaceState::NORMAL) => NinePatch::solid(palette.window_background),
        (SurfaceRole::WINDOW, SurfaceState::DISABLED) => NinePatch::solid(palette.disabled_background),
        (SurfaceRole::GENERIC_FRAME, _) => NinePatch::framed(frame_insets, palette.border, None),
        (SurfaceRole::PANEL, SurfaceState::NORMAL) => NinePatch::framed(frame_insets, palette.border, Some(palette.panel_background)),
        (SurfaceRole::PANEL, SurfaceState::DISABLED) => NinePatch::framed(frame_insets, palette.border, Some(palette.disabled_background)),
        // Test compilation enumerates only the known IDL values through `ALL`.
        _ => unreachable!("flat test palette received an unknown surface role or state"),
    };
    let content_color = match state {
        SurfaceState::NORMAL => palette.text,
        SurfaceState::DISABLED => palette.disabled_foreground,
        // Test compilation enumerates only the known IDL values through `ALL`.
        _ => unreachable!("flat test palette received an unknown surface state"),
    };
    Visual::new(patch, content_color)
}

/// Resolves one interactive control from its concrete role and nested control state.
#[cfg(test)]
fn flat_control_visual(frame_insets: SliceInsets, palette: &FlatPalette, role: ControlRole, state: ControlState) -> Visual {
    // Each role group spells out its meaningful fill policy. A newly added role therefore fails
    // this exhaustive match instead of inheriting a generic button or body fallback.
    let framed = |fill| NinePatch::framed(frame_insets, palette.border, Some(fill));
    let transparent = Color { r: 0, g: 0, b: 0, a: 0 };
    let patch = match role {
        ControlRole::BUTTON
        | ControlRole::COMBO
        | ControlRole::SLIDER_THUMB
        | ControlRole::CLOSE_BUTTON
        | ControlRole::MINIMIZE_BUTTON
        | ControlRole::MAXIMIZE_BUTTON
        | ControlRole::RESTORE_BUTTON => match state {
            ControlState::Disabled => framed(palette.disabled_background),
            ControlState::Enabled(PointerState::NORMAL) => framed(palette.button),
            ControlState::Enabled(PointerState::HOVERED) => framed(palette.button_hovered),
            ControlState::Enabled(PointerState::PRESSED) => framed(palette.input),
            ControlState::Focused(PointerState::NORMAL | PointerState::HOVERED) => framed(palette.control_focus),
            ControlState::Focused(PointerState::PRESSED) => framed(palette.input),
            // The test compiler is called only with `ControlState::ALL`.
            _ => unreachable!("flat test palette received an unknown pointer state"),
        },
        ControlRole::CHECKBOX | ControlRole::TEXT_INPUT | ControlRole::SLIDER_TRACK | ControlRole::RESIZE_GRIP => match state {
            ControlState::Disabled => framed(palette.disabled_background),
            ControlState::Enabled(PointerState::NORMAL) => framed(palette.input),
            ControlState::Enabled(PointerState::HOVERED | PointerState::PRESSED) => framed(palette.input_hovered),
            ControlState::Focused(PointerState::NORMAL | PointerState::HOVERED) => framed(palette.control_focus),
            ControlState::Focused(PointerState::PRESSED) => framed(palette.input_hovered),
            // The test compiler is called only with `ControlState::ALL`.
            _ => unreachable!("flat test palette received an unknown pointer state"),
        },
        ControlRole::ITEM => match state {
            ControlState::Disabled | ControlState::Enabled(PointerState::NORMAL) => NinePatch::solid(transparent),
            ControlState::Enabled(PointerState::HOVERED | PointerState::PRESSED)
            | ControlState::Focused(PointerState::NORMAL | PointerState::HOVERED | PointerState::PRESSED) => NinePatch::solid(palette.selection_background),
            // The test compiler is called only with `ControlState::ALL`.
            _ => unreachable!("flat test palette received an unknown pointer state"),
        },
        ControlRole::SCROLLBAR_TRACK => match state {
            ControlState::Disabled => NinePatch::solid(palette.disabled_background),
            ControlState::Enabled(_) | ControlState::Focused(_) => NinePatch::solid(palette.scrollbar_track),
        },
        ControlRole::SCROLLBAR_THUMB => match state {
            ControlState::Disabled => NinePatch::solid(palette.disabled_background),
            ControlState::Enabled(_) | ControlState::Focused(_) => NinePatch::solid(palette.scrollbar_thumb),
        },
        // Test compilation enumerates only the known IDL values through `ALL`.
        _ => unreachable!("flat test palette received an unknown control role"),
    };
    // Content-color selection is separate from patch grouping because item selection and caption
    // symbols have explicit contrast policies that ordinary controls do not share.
    let content_color = match role {
        ControlRole::ITEM => match state {
            ControlState::Disabled => palette.disabled_foreground,
            ControlState::Enabled(PointerState::NORMAL) => palette.text,
            ControlState::Enabled(PointerState::HOVERED | PointerState::PRESSED)
            | ControlState::Focused(PointerState::NORMAL | PointerState::HOVERED | PointerState::PRESSED) => palette.selection_foreground,
            // The test compiler is called only with `ControlState::ALL`.
            _ => unreachable!("flat test palette received an unknown pointer state"),
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
    Visual::new(patch, content_color)
}

/// Resolves one menu surface or entry from the menu-specific state domain.
#[cfg(test)]
fn flat_menu_visual(frame_insets: SliceInsets, palette: &FlatPalette, role: MenuRole, state: MenuState) -> Visual {
    // Open is a menu state rather than a duplicate title role. Popup owners and selected submenu
    // entries consequently share the same explicit selection precedence.
    let selected = matches!(state, MenuState::HOVERED | MenuState::PRESSED | MenuState::FOCUSED | MenuState::OPEN);
    let transparent = Color { r: 0, g: 0, b: 0, a: 0 };
    let patch = match role {
        MenuRole::BAR => match state {
            MenuState::DISABLED => NinePatch::solid(palette.disabled_background),
            _ => NinePatch::solid(palette.menu_background),
        },
        MenuRole::TITLE | MenuRole::ITEM => {
            if selected {
                NinePatch::solid(palette.selection_background)
            } else {
                NinePatch::solid(transparent)
            }
        }
        MenuRole::POPUP => match state {
            MenuState::DISABLED => NinePatch::framed(frame_insets, palette.border, Some(palette.disabled_background)),
            _ => NinePatch::framed(frame_insets, palette.border, Some(palette.menu_background)),
        },
        // Test compilation enumerates only the known IDL values through `ALL`.
        _ => unreachable!("flat test palette received an unknown menu role"),
    };
    let content_color = match state {
        MenuState::DISABLED => palette.disabled_foreground,
        _ if selected && matches!(role, MenuRole::TITLE | MenuRole::ITEM) => palette.selection_foreground,
        _ => palette.menu_foreground,
    };
    Visual::new(patch, content_color)
}

/// Resolves one window-chrome visual from activation and availability only.
#[cfg(test)]
fn flat_chrome_visual(frame_insets: SliceInsets, palette: &FlatPalette, role: ChromeRole, state: ChromeState) -> Visual {
    // Window activation belongs solely to chrome. Control descendants continue resolving their
    // own enabled, focus, and pointer states independently of the active window.
    let patch = match role {
        ChromeRole::WINDOW_FRAME | ChromeRole::DIALOG_FRAME => match state {
            ChromeState::BASE | ChromeState::DISABLED => NinePatch::framed(frame_insets, palette.border, None),
            ChromeState::ACTIVE => NinePatch::framed(frame_insets.at_least(1), palette.window_active, None),
            // Test compilation enumerates only the known IDL values through `ALL`.
            _ => unreachable!("flat test palette received an unknown chrome state"),
        },
        ChromeRole::TITLE => match state {
            ChromeState::BASE => NinePatch::solid(palette.title_background),
            ChromeState::ACTIVE => NinePatch::solid(palette.window_active),
            ChromeState::DISABLED => NinePatch::solid(palette.title_background),
            // Test compilation enumerates only the known IDL values through `ALL`.
            _ => unreachable!("flat test palette received an unknown chrome state"),
        },
        // Test compilation enumerates only the known IDL values through `ALL`.
        _ => unreachable!("flat test palette received an unknown chrome role"),
    };
    let content_color = match (role, state) {
        (ChromeRole::TITLE, ChromeState::DISABLED) => palette.disabled_title_foreground,
        (ChromeRole::TITLE, ChromeState::BASE | ChromeState::ACTIVE) => palette.title_foreground,
        (_, ChromeState::DISABLED) => palette.disabled_foreground,
        (_, ChromeState::BASE | ChromeState::ACTIVE) => palette.text,
        // Test compilation enumerates only the known IDL values through `ALL`.
        _ => unreachable!("flat test palette received an unknown chrome role or state"),
    };
    Visual::new(patch, content_color)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NinePatch, PointerState, color};

    /// Converts a color into comparable channel data without changing the render value API.
    fn channels(value: Color) -> (u8, u8, u8, u8) {
        // Tests care about exact authored channels rather than requiring Color to implement Eq.
        (value.r, value.g, value.b, value.a)
    }

    /// Verifies cloning and changing one family slot leaves the source catalog untouched.
    #[test]
    fn catalog_has_copy_on_write_value_semantics() {
        let replacement = Visual::new(NinePatch::solid(color(7, 8, 9, 255)), color(10, 11, 12, 255));
        let original = AppearanceCatalog::from_flat_palette(SliceInsets::uniform(1), &FlatPalette::default());
        let mut changed = original.clone();
        let state = ControlState::Focused(PointerState::PRESSED);
        changed.set_control(ControlRole::BUTTON, state, replacement);

        assert_ne!(original.control(ControlRole::BUTTON, state).content_color.r, 10);
        assert_eq!(changed.control(ControlRole::BUTTON, state).content_color.r, 10);
    }

    /// Verifies root backgrounds are concrete surface values rather than chrome side effects.
    #[test]
    fn window_surface_compiles_client_background_colors() {
        let palette = FlatPalette {
            window_background: color(11, 23, 37, 255),
            disabled_background: color(47, 61, 79, 255),
            ..FlatPalette::default()
        };
        let visuals = AppearanceCatalog::from_flat_palette(SliceInsets::uniform(3), &palette);

        // Window and modal-dialog roots intentionally share this semantic surface. Their outer
        // decorations remain independently selectable through the two ChromeRole frame values.
        for (state, expected) in [(SurfaceState::NORMAL, (11, 23, 37, 255)), (SurfaceState::DISABLED, (47, 61, 79, 255))] {
            let patch = visuals.surface(SurfaceRole::WINDOW, state).patch;
            assert!(matches!(
                patch.content,
                crate::NinePatchContent::Flat { cells }
                    if matches!(cells.center, crate::NinePatchCell::Color { color } if channels(color) == expected)
            ));
        }

        // Flat chrome fallbacks contain border cells only. The runtime applies the same rule to
        // authored image patches by omitting their center during the decorative overlay pass.
        for state in ChromeState::ALL {
            let patch = visuals.chrome(ChromeRole::WINDOW_FRAME, state).patch;
            assert!(matches!(
                patch.content,
                crate::NinePatchContent::Flat { cells } if matches!(cells.center, crate::NinePatchCell::Empty)
            ));
        }
    }

    /// Verifies a dark control-focus accent cannot darken an independently selected item.
    #[test]
    fn control_focus_and_item_selection_compile_to_independent_visuals() {
        let palette = FlatPalette {
            control_focus: color(0, 0, 0, 255),
            selection_background: color(0, 0, 170, 255),
            selection_foreground: color(255, 255, 255, 255),
            ..FlatPalette::default()
        };

        // Compile once through the production fallback constructor so this assertion covers the
        // concrete family/state mapping responsible for focused tree and list rows.
        let visuals = AppearanceCatalog::from_flat_palette(SliceInsets::uniform(1), &palette);
        let state = ControlState::Focused(PointerState::NORMAL);
        let control = visuals.control(ControlRole::BUTTON, state);
        let item = visuals.control(ControlRole::ITEM, state);

        assert!(matches!(
            control.patch.content,
            crate::NinePatchContent::Flat { cells }
                if matches!(cells.center, crate::NinePatchCell::Color { color } if channels(color) == (0, 0, 0, 255))
        ));
        assert!(matches!(
            item.patch.content,
            crate::NinePatchContent::Flat { cells }
                if matches!(cells.center, crate::NinePatchCell::Color { color } if channels(color) == (0, 0, 170, 255))
        ));
        assert_eq!(channels(item.content_color), (255, 255, 255, 255));
    }
}
