//
// Copyright 2022-Present (c) Raja Lehtihet & Wael El Oraiby
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
// -----------------------------------------------------------------------------
// Ported to rust from https://github.com/rxi/microui/ and the original license
//
// Copyright (c) 2020 rxi
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to
// deal in the Software without restriction, including without limitation the
// rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
// sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
// IN THE SOFTWARE.
//
//! Concrete theme geometry shared by authoring and runtime consumers.
use crate::{AssetMetrics, Insets as SliceInsets, Color};

/// Layout metrics use the same checked C record in authoring and installed resources.
pub use crate::AssetMetrics as SkinMetrics;

impl Default for AssetMetrics {
    /// Returns the standard layout metrics used by a newly constructed skin.
    fn default() -> Self {
        // One concrete default supplies both programmatic skins and omitted JSON fields, removing
        // the former loader-owned optional mirror and its separate assignment table.
        Self {
            default_cell_width: 68,
            padding: 5,
            window_content_insets: SliceInsets::uniform(5),
            spacing: 4,
            indent: 24,
            title_height: 24,
            window_border: SliceInsets::uniform(1),
            scrollbar_size: 12,
            thumb_size: 8,
        }
    }
}

/// Horizontal alignment policy for manager-owned window title text.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, neocogi_com::ComDeserialize)]
#[com_deserialize(rename_all = "snake_case")]
pub enum WindowTitleAlignment {
    /// Uses the available title span from its leading edge.
    #[default]
    Leading,
    /// Reserves symmetric caption banks and centers text in the complete title.
    Centered,
}

/// Edge selected for one manager-owned caption button.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, neocogi_com::ComDeserialize)]
#[com_deserialize(rename_all = "snake_case")]
pub enum CaptionButtonSide {
    /// Allocates the button from the title's leading edge.
    Leading,
    /// Allocates the button from the title's trailing edge.
    #[default]
    Trailing,
}

/// Data recipe controlling manager-owned caption button geometry and presentation.
#[derive(Copy, Clone, neocogi_com::ComDeserialize)]
#[com_deserialize(default)]
pub struct CaptionButtonsSkin {
    /// Edge used by the close button.
    pub close_side: CaptionButtonSide,
    /// Edge used by the minimize button.
    pub minimize_side: CaptionButtonSide,
    /// Edge used by the maximize or restore button.
    pub maximize_side: CaptionButtonSide,
    /// Title pixels retained above and below each square caption button.
    pub vertical_spacing: i32,
    /// Minimum square caption extent after applying the inset.
    pub minimum_extent: i32,
    /// Unstriped title pixels reserved between each non-empty caption bank and title field.
    pub inner_spacing: i32,
    /// Whether caption controls remain visible and hittable without window activation.
    ///
    /// Activation selects [`ChromeState::ACTIVE`] for frame and title chrome; it does not rewrite
    /// the control state of the caption buttons themselves.
    pub show_without_activation: bool,
}

impl Default for CaptionButtonsSkin {
    /// Returns the conventional trailing-edge caption-button policy.
    fn default() -> Self {
        // Keep the independently deserializable caption recipe identical to the standard window
        // chrome constructor rather than maintaining a second set of implicit JSON defaults.
        Self {
            close_side: CaptionButtonSide::Trailing,
            minimize_side: CaptionButtonSide::Trailing,
            maximize_side: CaptionButtonSide::Trailing,
            vertical_spacing: 2,
            minimum_extent: 0,
            inner_spacing: 0,
            show_without_activation: true,
        }
    }
}

/// Optional flat field painted behind active centered title text.
#[derive(Copy, Clone, neocogi_com::ComDeserialize)]
pub struct TitleBackdropSkin {
    /// Flat background color interrupting title artwork behind the measured text.
    pub color: Color,
    /// Horizontal pixels added on each side of the measured title text.
    pub horizontal_padding: i32,
}

/// Manager-owned window chrome policy selected by the resolved skin.
#[derive(Copy, Clone, neocogi_com::ComDeserialize)]
#[com_deserialize(default)]
pub struct WindowChromeSkin {
    /// Alignment and caption-bank reservation policy for title text.
    pub title_alignment: WindowTitleAlignment,
    /// Independent placement, extent, and visibility policy for caption buttons.
    pub captions: CaptionButtonsSkin,
    /// Application-content height retained while a window is minimized.
    pub minimized_content_height: i32,
    /// Optional active-title field painted behind measured text.
    pub active_title_backdrop: Option<TitleBackdropSkin>,
}

impl WindowChromeSkin {
    /// Creates a leading-title recipe with every caption button on the trailing edge.
    pub const fn trailing_buttons() -> Self {
        // This conventional recipe retains visible caption controls without activation.
        Self {
            title_alignment: WindowTitleAlignment::Leading,
            captions: CaptionButtonsSkin {
                close_side: CaptionButtonSide::Trailing,
                minimize_side: CaptionButtonSide::Trailing,
                maximize_side: CaptionButtonSide::Trailing,
                vertical_spacing: 2,
                minimum_extent: 0,
                inner_spacing: 0,
                show_without_activation: true,
            },
            minimized_content_height: 2,
            active_title_backdrop: None,
        }
    }

    /// Creates a centered-title recipe with split compact caption banks.
    pub const fn classic_mac(title_backdrop: Color) -> Self {
        // The recipe describes each independent behavior directly; manager code contains no
        // Classic-Mac mode branch and can also represent mixed application-defined arrangements.
        Self {
            title_alignment: WindowTitleAlignment::Centered,
            captions: CaptionButtonsSkin {
                close_side: CaptionButtonSide::Leading,
                minimize_side: CaptionButtonSide::Trailing,
                maximize_side: CaptionButtonSide::Trailing,
                vertical_spacing: 3,
                minimum_extent: 1,
                inner_spacing: 4,
                show_without_activation: false,
            },
            minimized_content_height: 2,
            active_title_backdrop: Some(TitleBackdropSkin {
                color: title_backdrop,
                horizontal_padding: 4,
            }),
        }
    }

    /// Updates an existing optional title backdrop without changing chrome geometry policy.
    pub fn set_backdrop_color(&mut self, color: Color) {
        // Conventional recipes have no backdrop and therefore remain unchanged by palette edits.
        if let Some(backdrop) = &mut self.active_title_backdrop {
            backdrop.color = color;
        }
    }
}

impl Default for WindowChromeSkin {
    /// Returns the conventional trailing-caption recipe.
    fn default() -> Self {
        // Keep Skin::from_atlas and programmatic WindowChromeSkin defaults identical.
        Self::trailing_buttons()
    }
}
