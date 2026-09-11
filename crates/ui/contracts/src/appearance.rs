//! Rust behavior for appearance roles and states declared by `neocogi.ui`.
//!
//! The IDL owns every type, discriminant, and documentation string. This file
//! contains only Rust conveniences that cannot be expressed as ABI declarations.

pub use crate::{ChromeRole, ChromeState, ControlRole, ControlState, MenuRole, MenuState, PointerState, SurfaceRole, SurfaceState};

/// Adds ordered-table metadata to one contiguous IDL integer enum.
macro_rules! indexed_values {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        impl $name {
            /// Number of known values in the IDL declaration.
            pub const COUNT: usize = [$(Self::$variant),+].len();

            /// Every known value in declaration and wire-table order.
            pub const ALL: [Self; Self::COUNT] = [$(Self::$variant),+];

            /// Converts a known contiguous value to its table index.
            pub const fn index(self) -> usize {
                // These IDL declarations explicitly assign zero-based contiguous
                // discriminants. Rejecting unknown wrappers prevents malformed ABI
                // input from indexing a trusted local table.
                assert!(self.is_known(), "unknown appearance value has no table index");
                self.raw() as usize
            }
        }
    };
}

indexed_values!(SurfaceRole { WINDOW, GENERIC_FRAME, PANEL });

indexed_values!(ControlRole {
    BUTTON,
    CHECKBOX,
    TEXT_INPUT,
    ITEM,
    COMBO,
    SLIDER_TRACK,
    SLIDER_THUMB,
    SCROLLBAR_TRACK,
    SCROLLBAR_THUMB,
    CLOSE_BUTTON,
    MINIMIZE_BUTTON,
    MAXIMIZE_BUTTON,
    RESTORE_BUTTON,
    RESIZE_GRIP,
});
indexed_values!(MenuRole { BAR, TITLE, POPUP, ITEM });
indexed_values!(ChromeRole { WINDOW_FRAME, DIALOG_FRAME, TITLE });

indexed_values!(PointerState { NORMAL, HOVERED, PRESSED });
indexed_values!(SurfaceState { NORMAL, DISABLED });
indexed_values!(MenuState {
    NORMAL,
    HOVERED,
    PRESSED,
    FOCUSED,
    OPEN,
    DISABLED
});
indexed_values!(ChromeState { BASE, ACTIVE, DISABLED });

impl ControlState {
    /// Number of concrete control states stored for each control role.
    pub const COUNT: usize = 1 + PointerState::COUNT * 2;

    /// Every concrete control state in family-catalog order.
    pub const ALL: [Self; Self::COUNT] = [
        Self::Disabled,
        Self::Enabled(PointerState::NORMAL),
        Self::Enabled(PointerState::HOVERED),
        Self::Enabled(PointerState::PRESSED),
        Self::Focused(PointerState::NORMAL),
        Self::Focused(PointerState::HOVERED),
        Self::Focused(PointerState::PRESSED),
    ];

    /// Resolves one exact control state from retained interaction facts.
    pub const fn from_interaction(enabled: bool, hovered: bool, focused: bool, pressed: bool) -> Self {
        if !enabled {
            return Self::Disabled;
        }
        let pointer = if pressed {
            PointerState::PRESSED
        } else if hovered {
            PointerState::HOVERED
        } else {
            PointerState::NORMAL
        };
        if focused { Self::Focused(pointer) } else { Self::Enabled(pointer) }
    }

    /// Converts this closed state into its control-catalog array index.
    pub const fn index(self) -> usize {
        match self {
            Self::Disabled => 0,
            Self::Enabled(pointer) => 1 + pointer.index(),
            Self::Focused(pointer) => 1 + PointerState::COUNT + pointer.index(),
        }
    }
}

impl SurfaceState {
    /// Resolves surface availability without accepting pointer or focus facts.
    pub const fn from_enabled(enabled: bool) -> Self {
        if enabled { Self::NORMAL } else { Self::DISABLED }
    }
}

impl MenuState {
    /// Resolves menu selection using its documented precedence order.
    pub const fn from_interaction(enabled: bool, open: bool, hovered: bool, focused: bool, pressed: bool) -> Self {
        if !enabled {
            Self::DISABLED
        } else if open {
            Self::OPEN
        } else if pressed {
            Self::PRESSED
        } else if hovered {
            Self::HOVERED
        } else if focused {
            Self::FOCUSED
        } else {
            Self::NORMAL
        }
    }
}

impl ChromeState {
    /// Resolves chrome state from availability and window activation.
    pub const fn from_window(enabled: bool, active: bool) -> Self {
        if !enabled {
            Self::DISABLED
        } else if active {
            Self::ACTIVE
        } else {
            Self::BASE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Keeps every closed state domain synchronized with its IDL declaration order.
    #[test]
    fn state_metadata_is_complete_and_index_ordered() {
        for (index, state) in ControlState::ALL.into_iter().enumerate() {
            assert_eq!(state.index(), index);
        }
        for (index, state) in SurfaceState::ALL.into_iter().enumerate() {
            assert_eq!(state.index(), index);
        }
        for (index, state) in MenuState::ALL.into_iter().enumerate() {
            assert_eq!(state.index(), index);
        }
        for (index, state) in ChromeState::ALL.into_iter().enumerate() {
            assert_eq!(state.index(), index);
        }
    }

    /// Verifies disabled, focus, and pointer facts resolve without contradictions.
    #[test]
    fn control_state_nests_pointer_interaction_only_inside_enabled_branches() {
        assert_eq!(ControlState::from_interaction(false, true, true, true), ControlState::Disabled);
        assert_eq!(
            ControlState::from_interaction(true, true, false, false),
            ControlState::Enabled(PointerState::HOVERED)
        );
        assert_eq!(
            ControlState::from_interaction(true, true, true, true),
            ControlState::Focused(PointerState::PRESSED)
        );
    }
}
