//
// Copyright 2023-Present (c) Raja Lehtihet & Wael El Oraiby
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
// this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
// this list of conditions and the following disclaimer in the documentation
// and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
// may be used to endorse or promote products derived from this software without
// specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
// LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
// CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
// SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
// CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGE.
//

//! Semantic resource names shared by asset recipes and resolved UI lookup.

/// Semantic font roles used by built-in widgets and skins.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum FontRole {
    /// Default body text used by most widgets.
    #[default]
    Body,
    /// Compact supporting text.
    Small,
    /// Window titles and similar chrome text.
    Title,
    /// Larger display text.
    Heading,
    /// Monospace-style text.
    Mono,
}

impl FontRole {
    /// Every semantic font role in declaration order.
    pub const ALL: [Self; 5] = [Self::Body, Self::Small, Self::Title, Self::Heading, Self::Mono];

    /// Number of semantic font roles stored by a complete skin.
    pub const COUNT: usize = Self::ALL.len();

    /// Returns this role's position in the skin's resolved font table.
    pub const fn index(self) -> usize {
        // Spell positions out so enum declaration changes cannot silently alter the private table
        // contract through a numeric cast.
        match self {
            Self::Body => 0,
            Self::Small => 1,
            Self::Title => 2,
            Self::Heading => 3,
            Self::Mono => 4,
        }
    }

    /// Returns the conventional atlas font name resolved by a complete resource bundle.
    pub const fn atlas_name(self) -> &'static str {
        // Exhaustive matching keeps the typed role catalog and atlas spellings synchronized when
        // theme loaders need to replace only these semantic entries.
        match self {
            Self::Body => "body",
            Self::Small => "small",
            Self::Title => "title",
            Self::Heading => "heading",
            Self::Mono => "mono",
        }
    }
}

/// Semantic icon roles required by built-in components.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum IconRole {
    /// Window and dialog close affordance.
    Close,
    /// Collapsed disclosure affordance.
    Expand,
    /// Expanded disclosure affordance.
    Collapse,
    /// Checked checkbox and menu-item mark.
    Check,
    /// Combo-box dropdown affordance.
    ExpandDown,
    /// Open-folder file-dialog item.
    OpenFolder,
    /// Closed-folder file-dialog item.
    ClosedFolder,
    /// Regular file-dialog item.
    File,
}

impl IconRole {
    /// Every semantic icon role in declaration order.
    pub const ALL: [Self; 8] = [
        Self::Close,
        Self::Expand,
        Self::Collapse,
        Self::Check,
        Self::ExpandDown,
        Self::OpenFolder,
        Self::ClosedFolder,
        Self::File,
    ];

    /// Number of semantic icon roles stored by a complete skin.
    pub const COUNT: usize = Self::ALL.len();

    /// Returns this role's position in the skin's resolved icon table.
    pub const fn index(self) -> usize {
        // Exhaustive positions prevent declaration-order casts from silently changing persisted
        // meaning if the enum is later reorganized for readability.
        match self {
            Self::Close => 0,
            Self::Expand => 1,
            Self::Collapse => 2,
            Self::Check => 3,
            Self::ExpandDown => 4,
            Self::OpenFolder => 5,
            Self::ClosedFolder => 6,
            Self::File => 7,
        }
    }

    /// Returns the conventional atlas name for this semantic icon.
    pub const fn atlas_name(self) -> &'static str {
        // Exhaustive matching makes a newly added role choose its exact resource spelling before
        // the crate compiles, avoiding parallel string lists in loaders and widgets.
        match self {
            Self::Close => "close",
            Self::Expand => "expand",
            Self::Collapse => "collapse",
            Self::Check => "check",
            Self::ExpandDown => "expand_down",
            Self::OpenFolder => "open_folder",
            Self::ClosedFolder => "closed_folder",
            Self::File => "file",
        }
    }
}
