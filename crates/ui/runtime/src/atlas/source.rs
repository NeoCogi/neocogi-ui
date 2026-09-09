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

//! Serializable atlas source metadata and decoding.

use super::*;
use crate::image::{ImageSource, load_image_bytes_checked};

/// Describes a font baked into an [`AtlasSource`].
pub struct FontEntry<'a> {
    /// Distance between baselines in pixels.
    pub line_size: usize,
    /// Offset from the top of the line to the baseline.
    pub baseline: i32,
    /// Requested pixel size.
    pub font_size: usize,
    /// Glyph metadata table keyed by Unicode scalar value.
    ///
    /// Runtime drawing substitutes the `_` entry for a missing character. Every font must contain
    /// exactly one underscore entry; atlas validation rejects a missing or duplicate fallback.
    pub entries: &'a [(char, CharEntry)],
}

/// Borrowed raw metadata used to construct a validated local atlas.
pub struct AtlasSource<'a> {
    /// Positive width of the atlas texture, representable by an `i32` rectangle coordinate.
    ///
    /// Together with [`AtlasSource::height`], this must require no more than
    /// [`crate::image::MAX_DECODED_RGBA_BYTES`] of normalized pixel storage.
    pub width: usize,
    /// Positive height of the atlas texture, representable by an `i32` rectangle coordinate.
    ///
    /// The combined allocation limit is documented on [`AtlasSource::width`].
    pub height: usize,
    /// Packed raw RGBA bytes filling the declared dimensions.
    pub pixels: &'a [u8],
    /// Icon lookup table.
    ///
    /// Names must be unique and every rectangle must be positive and in bounds. An entry named
    /// `white` must exist and every pixel in its rectangle must be opaque white.
    /// Atlas finalization resolves that name once into the stable typed identity used for solid
    /// geometry; table position has no public meaning.
    pub icons: &'a [(&'a str, Recti)],
    /// Fonts baked into the atlas.
    ///
    /// Font names and per-font characters must be unique. Metrics must fit runtime coordinates,
    /// rectangles must be nonnegative and in bounds, and every font must contain `_`. Unlike the
    /// asset compiler's printable-ASCII output, these tables may contain arbitrary Unicode scalar
    /// values.
    pub fonts: &'a [(&'a str, FontEntry<'a>)],
}

impl<'source> TryFrom<&AtlasSource<'source>> for AtlasHandle {
    type Error = AtlasError;

    /// Copies and validates one raw atlas without a panic or lossy fallback path.
    ///
    /// # Errors
    ///
    /// Returns the precise [`AtlasError`] for invalid dimensions or pixels, raw storage failures, duplicate resource keys, invalid metrics or rectangles, a missing glyph
    /// fallback, or an absent/non-white rendering tile.
    fn try_from(source: &AtlasSource<'source>) -> Result<Self, Self::Error> {
        // Validate dimension arithmetic before converting coordinates or allocating runtime tables.
        let dimensions = super::validation::checked_atlas_dimensions(source.width, source.height)?;

        let image = ImageSource {
            // CheckedImageDimensions proved both values fit i32 exactly.
            width: source.width as i32,
            height: source.height as i32,
            pixels: source.pixels,
        };
        let (_, _, pixels) = load_image_bytes_checked(image, dimensions).map_err(AtlasError::from)?;

        // Copy borrowed metadata into the duplicate-preserving candidate representation. Runtime
        // hash maps are deliberately built only after the common finalizer accepts every key.
        let icons = source
            .icons
            .iter()
            .map(|(name, rectangle)| {
                // Raw metadata introduces a fresh local resource set. COM import separately
                // authenticates catalogs before preserving IDs across a replacement.
                (name.to_string(), Icon { id: IconId::allocate(), rect: *rectangle })
            })
            .collect();
        let fonts = source
            .fonts
            .iter()
            .map(|(name, font)| {
                let candidate = FontCandidate {
                    // The source name is a loading key, while this ID becomes the retained typed
                    // identity used after construction.
                    id: FontId::allocate(),
                    line_size: font.line_size,
                    baseline: font.baseline,
                    font_size: font.font_size,
                    entries: font.entries.to_vec(),
                };
                (name.to_string(), candidate)
            })
            .collect();
        let candidate = AtlasCandidate::from_decoded(dimensions, pixels, fonts, icons);
        AtlasHandle::finish(candidate)
    }
}
