//
// Copyright 2026-Present (c) Raja Lehtihet & Wael El Oraiby
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the conditions in LICENSE are met.
//

//! Family-specific storage for resolved runtime appearances.

use std::rc::Rc;

use super::{ChromeRole, ChromeState, ControlRole, ControlState, MenuRole, MenuState, PointerState, SurfaceRole, SurfaceState, Visual};
#[cfg(test)]
use super::FlatPalette;
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
    /// Structural surface values indexed only by the dense IDL role domain.
    surfaces: [SurfaceVisuals; SurfaceRole::COUNT],
    /// Interactive control values indexed only by the dense IDL role domain.
    controls: [ControlVisuals; ControlRole::COUNT],
    /// Menu values indexed only by the dense IDL role domain.
    menus: [MenuVisuals; MenuRole::COUNT],
    /// Window chrome values indexed only by the dense IDL role domain.
    chrome: [ChromeVisuals; ChromeRole::COUNT],
}

/// Native visuals for the two explicit structural-surface branches.
#[derive(Copy, Clone)]
pub(crate) struct SurfaceVisuals {
    /// Enabled structural appearance.
    pub(crate) normal: Visual,
    /// Disabled structural appearance.
    pub(crate) disabled: Visual,
}

/// Native visuals for one pointer-state branch of an available control.
#[derive(Copy, Clone)]
pub(crate) struct PointerVisuals {
    /// Pointer-neutral appearance.
    pub(crate) normal: Visual,
    /// Pointer-hover appearance.
    pub(crate) hovered: Visual,
    /// Pointer-press appearance.
    pub(crate) pressed: Visual,
}

/// Native visuals preserving disabled, enabled, and focused control structure.
#[derive(Copy, Clone)]
pub(crate) struct ControlVisuals {
    /// Disabled appearance with no nested interaction state.
    pub(crate) disabled: Visual,
    /// Pointer appearances without visible keyboard focus.
    pub(crate) enabled: PointerVisuals,
    /// Pointer appearances with visible keyboard focus.
    pub(crate) focused: PointerVisuals,
}

/// Native visuals for every named menu branch.
#[derive(Copy, Clone)]
pub(crate) struct MenuVisuals {
    /// Enabled appearance without transient selection.
    pub(crate) normal: Visual,
    /// Pointer-hover appearance.
    pub(crate) hovered: Visual,
    /// Pointer-press appearance.
    pub(crate) pressed: Visual,
    /// Keyboard-selection appearance.
    pub(crate) focused: Visual,
    /// Open-popup-owner appearance.
    pub(crate) open: Visual,
    /// Disabled appearance.
    pub(crate) disabled: Visual,
}

/// Native visuals for every named window-chrome branch.
#[derive(Copy, Clone)]
pub(crate) struct ChromeVisuals {
    /// Enabled inactive-window appearance.
    pub(crate) base: Visual,
    /// Enabled active-window appearance.
    pub(crate) active: Visual,
    /// Disabled-window appearance.
    pub(crate) disabled: Visual,
}

impl AppearanceCatalog {
    /// Stores four fully resolved families without interpreting authoring defaults.
    pub(crate) fn from_values(
        surfaces: [SurfaceVisuals; SurfaceRole::COUNT],
        controls: [ControlVisuals; ControlRole::COUNT],
        menus: [MenuVisuals; MenuRole::COUNT],
        chrome: [ChromeVisuals; ChromeRole::COUNT],
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
            SurfaceVisuals {
                normal: flat_surface_visual(frame_insets, palette, role, SurfaceState::Normal),
                disabled: flat_surface_visual(frame_insets, palette, role, SurfaceState::Disabled),
            }
        });
        let controls = std::array::from_fn(|role_index| {
            let role = ControlRole::ALL[role_index];
            ControlVisuals {
                disabled: flat_control_visual(frame_insets, palette, role, ControlState::Disabled),
                enabled: flat_pointer_visuals(frame_insets, palette, role, ControlState::Enabled),
                focused: flat_pointer_visuals(frame_insets, palette, role, ControlState::Focused),
            }
        });
        let menus = std::array::from_fn(|role_index| {
            let role = MenuRole::ALL[role_index];
            MenuVisuals {
                normal: flat_menu_visual(frame_insets, palette, role, MenuState::Normal),
                hovered: flat_menu_visual(frame_insets, palette, role, MenuState::Hovered),
                pressed: flat_menu_visual(frame_insets, palette, role, MenuState::Pressed),
                focused: flat_menu_visual(frame_insets, palette, role, MenuState::Focused),
                open: flat_menu_visual(frame_insets, palette, role, MenuState::Open),
                disabled: flat_menu_visual(frame_insets, palette, role, MenuState::Disabled),
            }
        });
        let chrome = std::array::from_fn(|role_index| {
            let role = ChromeRole::ALL[role_index];
            ChromeVisuals {
                base: flat_chrome_visual(frame_insets, palette, role, ChromeState::Base),
                active: flat_chrome_visual(frame_insets, palette, role, ChromeState::Active),
                disabled: flat_chrome_visual(frame_insets, palette, role, ChromeState::Disabled),
            }
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
        self.values.surfaces[role.raw() as usize].get(state)
    }

    /// Replaces one structural surface visual.
    pub(crate) fn set_surface(&mut self, role: SurfaceRole, state: SurfaceState, visual: Visual) {
        // Detach shared storage before changing exactly one typed family slot.
        Rc::make_mut(&mut self.values).surfaces[role.raw() as usize].set(state, visual);
    }

    /// Returns one interactive control visual.
    pub(crate) fn control(&self, role: ControlRole, state: ControlState) -> Visual {
        // Control lookup accepts the nested enabled/focused pointer state and no other domain.
        self.values.controls[role.raw() as usize].get(state)
    }

    /// Replaces one interactive control visual.
    pub(crate) fn set_control(&mut self, role: ControlRole, state: ControlState, visual: Visual) {
        // Detach shared storage before changing exactly one typed family slot.
        Rc::make_mut(&mut self.values).controls[role.raw() as usize].set(state, visual);
    }

    /// Returns one menu visual.
    pub(crate) fn menu(&self, role: MenuRole, state: MenuState) -> Visual {
        // Menu lookup is the only API that can receive the Open state.
        self.values.menus[role.raw() as usize].get(state)
    }

    /// Replaces one menu visual.
    pub(crate) fn set_menu(&mut self, role: MenuRole, state: MenuState, visual: Visual) {
        // Detach shared storage before changing exactly one typed family slot.
        Rc::make_mut(&mut self.values).menus[role.raw() as usize].set(state, visual);
    }

    /// Returns one window chrome visual.
    pub(crate) fn chrome(&self, role: ChromeRole, state: ChromeState) -> Visual {
        // Chrome lookup accepts activation state without admitting pointer interaction states.
        self.values.chrome[role.raw() as usize].get(state)
    }

    /// Replaces one window chrome visual.
    pub(crate) fn set_chrome(&mut self, role: ChromeRole, state: ChromeState, visual: Visual) {
        // Detach shared storage before changing exactly one typed family slot.
        Rc::make_mut(&mut self.values).chrome[role.raw() as usize].set(state, visual);
    }

    /// Iterates over every resolved visual for atlas-ownership validation.
    pub(crate) fn iter(&self) -> impl Iterator<Item = &Visual> {
        // Flatten each concrete family locally; no synthetic cross-family role or state is created.
        self.values
            .surfaces
            .iter()
            .flat_map(SurfaceVisuals::iter)
            .chain(self.values.controls.iter().flat_map(ControlVisuals::iter))
            .chain(self.values.menus.iter().flat_map(MenuVisuals::iter))
            .chain(self.values.chrome.iter().flat_map(ChromeVisuals::iter))
    }
}

impl SurfaceVisuals {
    /// Returns the field named by one native surface state.
    fn get(self, state: SurfaceState) -> Visual {
        match state {
            SurfaceState::Normal => self.normal,
            SurfaceState::Disabled => self.disabled,
        }
    }

    /// Replaces the field named by one native surface state.
    fn set(&mut self, state: SurfaceState, visual: Visual) {
        *match state {
            SurfaceState::Normal => &mut self.normal,
            SurfaceState::Disabled => &mut self.disabled,
        } = visual;
    }

    /// Iterates both explicit surface branches.
    fn iter(&self) -> impl Iterator<Item = &Visual> {
        [&self.normal, &self.disabled].into_iter()
    }
}

impl PointerVisuals {
    /// Returns the field named by one native pointer state.
    fn get(self, state: PointerState) -> Visual {
        match state {
            PointerState::Normal => self.normal,
            PointerState::Hovered => self.hovered,
            PointerState::Pressed => self.pressed,
        }
    }

    /// Returns a mutable field named by one native pointer state.
    fn get_mut(&mut self, state: PointerState) -> &mut Visual {
        match state {
            PointerState::Normal => &mut self.normal,
            PointerState::Hovered => &mut self.hovered,
            PointerState::Pressed => &mut self.pressed,
        }
    }

    /// Iterates all explicit pointer branches.
    fn iter(&self) -> impl Iterator<Item = &Visual> {
        [&self.normal, &self.hovered, &self.pressed].into_iter()
    }
}

impl ControlVisuals {
    /// Returns the branch selected by the structured native control state.
    fn get(self, state: ControlState) -> Visual {
        match state {
            ControlState::Disabled => self.disabled,
            ControlState::Enabled(pointer) => self.enabled.get(pointer),
            ControlState::Focused(pointer) => self.focused.get(pointer),
        }
    }

    /// Replaces the branch selected by the structured native control state.
    fn set(&mut self, state: ControlState, visual: Visual) {
        let destination = match state {
            ControlState::Disabled => &mut self.disabled,
            ControlState::Enabled(pointer) => self.enabled.get_mut(pointer),
            ControlState::Focused(pointer) => self.focused.get_mut(pointer),
        };
        *destination = visual;
    }

    /// Iterates the disabled branch followed by both named pointer branches.
    fn iter(&self) -> impl Iterator<Item = &Visual> {
        core::iter::once(&self.disabled).chain(self.enabled.iter()).chain(self.focused.iter())
    }
}

impl MenuVisuals {
    /// Returns the field named by one native menu state.
    fn get(self, state: MenuState) -> Visual {
        match state {
            MenuState::Normal => self.normal,
            MenuState::Hovered => self.hovered,
            MenuState::Pressed => self.pressed,
            MenuState::Focused => self.focused,
            MenuState::Open => self.open,
            MenuState::Disabled => self.disabled,
        }
    }

    /// Replaces the field named by one native menu state.
    fn set(&mut self, state: MenuState, visual: Visual) {
        *match state {
            MenuState::Normal => &mut self.normal,
            MenuState::Hovered => &mut self.hovered,
            MenuState::Pressed => &mut self.pressed,
            MenuState::Focused => &mut self.focused,
            MenuState::Open => &mut self.open,
            MenuState::Disabled => &mut self.disabled,
        } = visual;
    }

    /// Iterates all explicit menu branches.
    fn iter(&self) -> impl Iterator<Item = &Visual> {
        [&self.normal, &self.hovered, &self.pressed, &self.focused, &self.open, &self.disabled].into_iter()
    }
}

impl ChromeVisuals {
    /// Returns the field named by one native chrome state.
    fn get(self, state: ChromeState) -> Visual {
        match state {
            ChromeState::Base => self.base,
            ChromeState::Active => self.active,
            ChromeState::Disabled => self.disabled,
        }
    }

    /// Replaces the field named by one native chrome state.
    fn set(&mut self, state: ChromeState, visual: Visual) {
        *match state {
            ChromeState::Base => &mut self.base,
            ChromeState::Active => &mut self.active,
            ChromeState::Disabled => &mut self.disabled,
        } = visual;
    }

    /// Iterates all explicit chrome branches.
    fn iter(&self) -> impl Iterator<Item = &Visual> {
        [&self.base, &self.active, &self.disabled].into_iter()
    }
}

/// Compiles all pointer states below one control-state constructor for test skins.
#[cfg(test)]
fn flat_pointer_visuals(frame_insets: SliceInsets, palette: &FlatPalette, role: ControlRole, state: fn(PointerState) -> ControlState) -> PointerVisuals {
    PointerVisuals {
        normal: flat_control_visual(frame_insets, palette, role, state(PointerState::Normal)),
        hovered: flat_control_visual(frame_insets, palette, role, state(PointerState::Hovered)),
        pressed: flat_control_visual(frame_insets, palette, role, state(PointerState::Pressed)),
    }
}

/// Resolves one structural surface from the flat fallback palette.
#[cfg(test)]
fn flat_surface_visual(frame_insets: SliceInsets, palette: &FlatPalette, role: SurfaceRole, state: SurfaceState) -> Visual {
    // Surfaces react only to inherited availability. Window owns the root fill independently of
    // activation, GenericFrame remains hollow, and Panel combines its border with its own fill.
    let patch = match (role, state) {
        (SurfaceRole::WINDOW, SurfaceState::Normal) => NinePatch::solid(palette.window_background),
        (SurfaceRole::WINDOW, SurfaceState::Disabled) => NinePatch::solid(palette.disabled_background),
        (SurfaceRole::GENERIC_FRAME, _) => NinePatch::framed(frame_insets, palette.border, None),
        (SurfaceRole::PANEL, SurfaceState::Normal) => NinePatch::framed(frame_insets, palette.border, Some(palette.panel_background)),
        (SurfaceRole::PANEL, SurfaceState::Disabled) => NinePatch::framed(frame_insets, palette.border, Some(palette.disabled_background)),
        // Test compilation enumerates only the known IDL values through `ALL`.
        _ => unreachable!("flat test palette received an unknown surface role or state"),
    };
    let content_color = match state {
        SurfaceState::Normal => palette.text,
        SurfaceState::Disabled => palette.disabled_foreground,
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
            ControlState::Enabled(PointerState::Normal) => framed(palette.button),
            ControlState::Enabled(PointerState::Hovered) => framed(palette.button_hovered),
            ControlState::Enabled(PointerState::Pressed) => framed(palette.input),
            ControlState::Focused(PointerState::Normal | PointerState::Hovered) => framed(palette.control_focus),
            ControlState::Focused(PointerState::Pressed) => framed(palette.input),
        },
        ControlRole::CHECKBOX | ControlRole::TEXT_INPUT | ControlRole::SLIDER_TRACK | ControlRole::RESIZE_GRIP => match state {
            ControlState::Disabled => framed(palette.disabled_background),
            ControlState::Enabled(PointerState::Normal) => framed(palette.input),
            ControlState::Enabled(PointerState::Hovered | PointerState::Pressed) => framed(palette.input_hovered),
            ControlState::Focused(PointerState::Normal | PointerState::Hovered) => framed(palette.control_focus),
            ControlState::Focused(PointerState::Pressed) => framed(palette.input_hovered),
        },
        ControlRole::ITEM => match state {
            ControlState::Disabled | ControlState::Enabled(PointerState::Normal) => NinePatch::solid(transparent),
            ControlState::Enabled(PointerState::Hovered | PointerState::Pressed)
            | ControlState::Focused(PointerState::Normal | PointerState::Hovered | PointerState::Pressed) => NinePatch::solid(palette.selection_background),
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
            ControlState::Enabled(PointerState::Normal) => palette.text,
            ControlState::Enabled(PointerState::Hovered | PointerState::Pressed)
            | ControlState::Focused(PointerState::Normal | PointerState::Hovered | PointerState::Pressed) => palette.selection_foreground,
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
    let selected = matches!(state, MenuState::Hovered | MenuState::Pressed | MenuState::Focused | MenuState::Open);
    let transparent = Color { r: 0, g: 0, b: 0, a: 0 };
    let patch = match role {
        MenuRole::BAR => match state {
            MenuState::Disabled => NinePatch::solid(palette.disabled_background),
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
            MenuState::Disabled => NinePatch::framed(frame_insets, palette.border, Some(palette.disabled_background)),
            _ => NinePatch::framed(frame_insets, palette.border, Some(palette.menu_background)),
        },
        // Test compilation enumerates only the known IDL values through `ALL`.
        _ => unreachable!("flat test palette received an unknown menu role"),
    };
    let content_color = match state {
        MenuState::Disabled => palette.disabled_foreground,
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
            ChromeState::Base | ChromeState::Disabled => NinePatch::framed(frame_insets, palette.border, None),
            ChromeState::Active => NinePatch::framed(frame_insets.at_least(1), palette.window_active, None),
        },
        ChromeRole::TITLE => match state {
            ChromeState::Base => NinePatch::solid(palette.title_background),
            ChromeState::Active => NinePatch::solid(palette.window_active),
            ChromeState::Disabled => NinePatch::solid(palette.title_background),
        },
        // Test compilation enumerates only the known IDL values through `ALL`.
        _ => unreachable!("flat test palette received an unknown chrome role"),
    };
    let content_color = match (role, state) {
        (ChromeRole::TITLE, ChromeState::Disabled) => palette.disabled_title_foreground,
        (ChromeRole::TITLE, ChromeState::Base | ChromeState::Active) => palette.title_foreground,
        (_, ChromeState::Disabled) => palette.disabled_foreground,
        (_, ChromeState::Base | ChromeState::Active) => palette.text,
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
        let state = ControlState::Focused(PointerState::Pressed);
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
        for (state, expected) in [(SurfaceState::Normal, (11, 23, 37, 255)), (SurfaceState::Disabled, (47, 61, 79, 255))] {
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
        let state = ControlState::Focused(PointerState::Normal);
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
