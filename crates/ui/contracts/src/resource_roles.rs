//! Rust lookup conveniences for semantic resource roles declared by `neocogi.ui`.

use crate::{FontRole, IconRole};

impl FontRole {
    /// Every semantic font role in declaration order.
    pub const ALL: [Self; 5] = [Self::BODY, Self::SMALL, Self::TITLE, Self::HEADING, Self::MONO];

    /// Number of semantic font roles stored by a complete skin.
    pub const COUNT: usize = Self::ALL.len();

    /// Returns this known role's position in the resolved font table.
    pub const fn index(self) -> usize {
        match self {
            Self::BODY => 0,
            Self::SMALL => 1,
            Self::TITLE => 2,
            Self::HEADING => 3,
            Self::MONO => 4,
            _ => panic!("unknown font role has no table index"),
        }
    }

    /// Returns the conventional atlas font name for this known role.
    pub const fn atlas_name(self) -> &'static str {
        match self {
            Self::BODY => "body",
            Self::SMALL => "small",
            Self::TITLE => "title",
            Self::HEADING => "heading",
            Self::MONO => "mono",
            _ => panic!("unknown font role has no resource name"),
        }
    }
}

impl IconRole {
    /// Every required semantic icon role in declaration order.
    pub const ALL: [Self; 8] = [
        Self::CLOSE,
        Self::EXPAND,
        Self::COLLAPSE,
        Self::CHECK,
        Self::EXPAND_DOWN,
        Self::OPEN_FOLDER,
        Self::CLOSED_FOLDER,
        Self::FILE,
    ];

    /// Number of semantic icon roles stored by a complete skin.
    pub const COUNT: usize = Self::ALL.len();

    /// Returns this known role's position in the resolved icon table.
    pub const fn index(self) -> usize {
        match self {
            Self::CLOSE => 0,
            Self::EXPAND => 1,
            Self::COLLAPSE => 2,
            Self::CHECK => 3,
            Self::EXPAND_DOWN => 4,
            Self::OPEN_FOLDER => 5,
            Self::CLOSED_FOLDER => 6,
            Self::FILE => 7,
            _ => panic!("unknown icon role has no table index"),
        }
    }

    /// Returns the conventional atlas name for this known semantic icon.
    pub const fn atlas_name(self) -> &'static str {
        match self {
            Self::CLOSE => "close",
            Self::EXPAND => "expand",
            Self::COLLAPSE => "collapse",
            Self::CHECK => "check",
            Self::EXPAND_DOWN => "expand_down",
            Self::OPEN_FOLDER => "open_folder",
            Self::CLOSED_FOLDER => "closed_folder",
            Self::FILE => "file",
            _ => panic!("unknown icon role has no resource name"),
        }
    }
}
