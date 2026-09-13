//! Concrete appearance states shared by GFX compilers and retained UI consumers.
//!
//! These types describe local behavior and never cross the COM ABI. The IDL instead transfers
//! complete appearance records with named fields, so no state discriminant becomes wire format.

/// Pointer interaction nested inside an enabled control state.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PointerState {
    /// Pointer is not hovering or pressing the control.
    Normal,
    /// Pointer is hovering the control without an active press.
    Hovered,
    /// Pointer capture is pressed while the pointer remains over the control.
    Pressed,
}

impl PointerState {
    /// Every pointer state used when compiling a complete native appearance.
    pub const ALL: [Self; 3] = [Self::Normal, Self::Hovered, Self::Pressed];
}

/// State of an interactive control without contradictory disabled interaction facts.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ControlState {
    /// Control is unavailable; pointer and keyboard-focus presentation do not apply.
    Disabled,
    /// Control is available without visible keyboard focus.
    Enabled(PointerState),
    /// Control owns visible keyboard focus.
    Focused(PointerState),
}

impl ControlState {
    /// Every concrete control state used by native compilation and tests.
    pub const ALL: [Self; 7] = [
        Self::Disabled,
        Self::Enabled(PointerState::Normal),
        Self::Enabled(PointerState::Hovered),
        Self::Enabled(PointerState::Pressed),
        Self::Focused(PointerState::Normal),
        Self::Focused(PointerState::Hovered),
        Self::Focused(PointerState::Pressed),
    ];

    /// Resolves one exact control state from retained interaction facts.
    pub const fn from_interaction(enabled: bool, hovered: bool, focused: bool, pressed: bool) -> Self {
        if !enabled {
            return Self::Disabled;
        }
        let pointer = if pressed {
            PointerState::Pressed
        } else if hovered {
            PointerState::Hovered
        } else {
            PointerState::Normal
        };
        if focused { Self::Focused(pointer) } else { Self::Enabled(pointer) }
    }
}

/// Availability state of a noninteractive structural surface.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SurfaceState {
    /// Surface belongs to an enabled retained subtree.
    Normal,
    /// Surface belongs to a disabled retained subtree.
    Disabled,
}

impl SurfaceState {
    /// Every native structural state.
    pub const ALL: [Self; 2] = [Self::Normal, Self::Disabled];

    /// Resolves surface availability without accepting pointer or focus facts.
    pub const fn from_enabled(enabled: bool) -> Self {
        if enabled { Self::Normal } else { Self::Disabled }
    }
}

/// State of a menu surface or entry after menu-specific precedence is resolved.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MenuState {
    /// Enabled entry or surface without transient selection.
    Normal,
    /// Pointer is over an enabled entry.
    Hovered,
    /// Pointer capture is pressed over an enabled entry.
    Pressed,
    /// Keyboard navigation selected an enabled entry.
    Focused,
    /// Entry owns the popup currently open beneath it.
    Open,
    /// Entry or surface is unavailable.
    Disabled,
}

impl MenuState {
    /// Every native menu state.
    pub const ALL: [Self; 6] = [Self::Normal, Self::Hovered, Self::Pressed, Self::Focused, Self::Open, Self::Disabled];

    /// Resolves menu selection using its documented precedence order.
    pub const fn from_interaction(enabled: bool, open: bool, hovered: bool, focused: bool, pressed: bool) -> Self {
        if !enabled {
            Self::Disabled
        } else if open {
            Self::Open
        } else if pressed {
            Self::Pressed
        } else if hovered {
            Self::Hovered
        } else if focused {
            Self::Focused
        } else {
            Self::Normal
        }
    }
}

/// State of manager-owned chrome after activation and availability are resolved.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ChromeState {
    /// Enabled chrome for a window that does not own activation.
    Base,
    /// Enabled chrome for the active window.
    Active,
    /// Chrome belonging to a disabled window.
    Disabled,
}

impl ChromeState {
    /// Every native window-chrome state.
    pub const ALL: [Self; 3] = [Self::Base, Self::Active, Self::Disabled];

    /// Resolves chrome state from availability and window activation.
    pub const fn from_window(enabled: bool, active: bool) -> Self {
        if !enabled {
            Self::Disabled
        } else if active {
            Self::Active
        } else {
            Self::Base
        }
    }
}
