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

//! Bounded raw RGBA storage used by runtime texture and atlas consumers.
use crate::{Color4b, color4b};
use std::fmt;

/// Shared upper bound on one raw RGBA image allocation.
pub const MAX_DECODED_RGBA_BYTES: usize = microui_contracts::MAX_ASSET_PIXELS_BYTES;

/// Borrowed packed RGBA pixels with explicit signed dimensions.
#[derive(Copy, Clone)]
pub struct ImageSource<'a> {
    /// Positive width in pixels, checked before allocation or upload.
    pub width: i32,
    /// Positive height in pixels, checked before allocation or upload.
    pub height: i32,
    /// Exactly width × height × 4 bytes in RGBA8888 channel order.
    pub pixels: &'a [u8],
}

impl ImageSource<'_> {
    /// Checks the complete raw storage contract without copying pixels.
    pub fn validate(self) -> Result<(), ImageError> {
        // All renderer entry points apply the same dimensions, byte count and allocation bound.
        validate_rgba_buffer(self.width, self.height, self.pixels.len())
    }
}

/// Concrete failure returned before a raw image is copied or uploaded.
/// Atlas and texture consumers share these checks so their storage bounds cannot diverge.
#[derive(Debug)]
pub enum ImageError {
    /// A raw source uses a non-positive signed dimension.
    RawDimensionsOutOfRange {
        /// Invalid raw-image width in pixels.
        width: i32,
        /// Invalid raw-image height in pixels.
        height: i32,
    },
    /// Image dimensions cannot produce an addressable, policy-bounded decoded buffer.
    Storage {
        /// Concrete dimension, arithmetic, or allocation-budget failure.
        source: ImageStorageError,
    },
    /// Raw RGBA bytes do not exactly fill their checked dimensions.
    RawPixelLengthMismatch {
        /// Exact byte count required by the checked dimensions.
        expected: usize,
        /// Actual byte count supplied by the caller.
        actual: usize,
    },
    /// The image dimensions disagree with the dimensions required by the caller.
    DimensionMismatch {
        /// Width required by the caller's metadata.
        expected_width: usize,
        /// Height required by the caller's metadata.
        expected_height: usize,
        /// Width declared by the raw source.
        actual_width: usize,
        /// Height declared by the raw source.
        actual_height: usize,
    },
}

impl fmt::Display for ImageError {
    /// Reports invalid raw dimensions, lengths or allocation bounds.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Dimension values are included explicitly so callers can diagnose stale sidecar metadata
        // without parsing diagnostic text.
        match self {
            Self::RawDimensionsOutOfRange { width, height } => {
                write!(f, "raw image dimensions must be positive, received {width}x{height}")
            }
            Self::Storage { source } => write!(f, "{source}"),
            Self::RawPixelLengthMismatch { expected, actual } => {
                write!(f, "expected {expected} RGBA bytes, received {actual}")
            }
            Self::DimensionMismatch {
                expected_width,
                expected_height,
                actual_width,
                actual_height,
            } => write!(
                f,
                "expected image dimensions {expected_width}x{expected_height}, received {actual_width}x{actual_height}"
            ),
        }
    }
}

impl std::error::Error for ImageError {
    /// Exposes the concrete storage-bound error when dimension arithmetic fails.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // A mismatch is fully represented by this enum's integer fields and has no lower-level
        // cause. Storage failures retain their exact arithmetic or budget classification.
        match self {
            Self::RawDimensionsOutOfRange { .. } => None,
            Self::Storage { source } => Some(source),
            Self::RawPixelLengthMismatch { .. } => None,
            Self::DimensionMismatch { .. } => None,
        }
    }
}

impl From<ImageStorageError> for ImageError {
    /// Preserves a concrete checked-storage failure on the shared image-loading path.
    fn from(source: ImageStorageError) -> Self {
        // Keeping storage as a concrete error lets callers inspect the same invariant through
        // ImageError and any higher-level error that composes it.
        Self::Storage { source }
    }
}

/// Concrete failure returned before allocating decoded or normalized image storage.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ImageStorageError {
    /// One image axis is zero or exceeds the runtime rectangle coordinate domain.
    DimensionsOutOfRange {
        /// Requested image width in pixels.
        width: usize,
        /// Requested image height in pixels.
        height: usize,
    },
    /// Pixel or byte count arithmetic exceeds the platform allocation domain.
    Overflow {
        /// Requested image width in pixels.
        width: usize,
        /// Requested image height in pixels.
        height: usize,
    },
    /// Addressable storage exceeds the crate's deliberate decoded-image budget.
    TooLarge {
        /// Requested image width in pixels.
        width: usize,
        /// Requested image height in pixels.
        height: usize,
        /// Larger of the RGBA-byte and normalized-color allocations required by the image.
        required_bytes: usize,
        /// Maximum permitted size of either decoded allocation.
        maximum_bytes: usize,
    },
}

impl fmt::Display for ImageStorageError {
    /// Formats the concrete invalid dimensions, arithmetic failure, or exceeded budget.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Diagnostics carry every numeric input needed to adjust an image without parsing a generic
        // allocation failure from the platform.
        match self {
            Self::DimensionsOutOfRange { width, height } => {
                write!(formatter, "image dimensions must each be in 1..=i32::MAX, received {width}x{height}")
            }
            Self::Overflow { width, height } => {
                write!(formatter, "image dimensions {width}x{height} overflow addressable RGBA storage")
            }
            Self::TooLarge {
                width,
                height,
                required_bytes,
                maximum_bytes,
            } => write!(
                formatter,
                "image dimensions {width}x{height} require {required_bytes} bytes, exceeding the {maximum_bytes}-byte allocation limit"
            ),
        }
    }
}

impl std::error::Error for ImageStorageError {}

/// Validated image extent and exact allocation counts shared by image and atlas construction.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct CheckedImageDimensions {
    /// Positive image width representable by runtime rectangles.
    pub(crate) width: usize,
    /// Positive image height representable by runtime rectangles.
    pub(crate) height: usize,
    /// Exact number of decoded pixels.
    pub(crate) pixel_count: usize,
    /// Exact number of source RGBA8888 bytes.
    pub(crate) rgba_byte_count: usize,
}

impl CheckedImageDimensions {
    /// Checks coordinate representation, allocation arithmetic, and the decoded-image budget once.
    pub(crate) fn try_new(width: usize, height: usize) -> Result<Self, ImageStorageError> {
        if width == 0 || height == 0 || width > i32::MAX as usize || height > i32::MAX as usize {
            // Every accepted image can become a runtime texture rectangle without truncating an
            // axis. Applying this before multiplication also preserves a precise dimension error.
            return Err(ImageStorageError::DimensionsOutOfRange { width, height });
        }
        let Some(pixel_count) = width.checked_mul(height) else {
            return Err(ImageStorageError::Overflow { width, height });
        };
        let Some(rgba_byte_count) = pixel_count.checked_mul(4) else {
            return Err(ImageStorageError::Overflow { width, height });
        };
        let Some(normalized_byte_count) = pixel_count.checked_mul(std::mem::size_of::<Color4b>()) else {
            return Err(ImageStorageError::Overflow { width, height });
        };
        let required_bytes = rgba_byte_count.max(normalized_byte_count);
        if required_bytes > isize::MAX as usize {
            // Vec cannot address one allocation larger than isize::MAX even when usize arithmetic
            // itself succeeds on the target.
            return Err(ImageStorageError::Overflow { width, height });
        }
        if required_bytes > MAX_DECODED_RGBA_BYTES {
            // Apply the shared resource budget before a runtime consumer reserves pixel storage.
            return Err(ImageStorageError::TooLarge {
                width,
                height,
                required_bytes,
                maximum_bytes: MAX_DECODED_RGBA_BYTES,
            });
        }
        Ok(Self {
            width,
            height,
            pixel_count,
            rgba_byte_count,
        })
    }
}

/// Copies checked raw RGBA bytes into the native color representation.
/// Invalid dimensions, a non-exact byte count or an excessive allocation returns `ImageError`.
pub fn load_image_bytes(source: ImageSource) -> Result<(usize, usize, Vec<Color4b>), ImageError> {
    // The public API accepts the dimensions carried by the image source itself. It deliberately
    // delegates to the same implementation as checked atlas loading so format normalization cannot
    // diverge between the two entry points.
    load_image_bytes_impl(source, None)
}

/// Copies raw pixels only when their dimensions match the caller's validated atlas extent.
pub(crate) fn load_image_bytes_checked(source: ImageSource, expected: CheckedImageDimensions) -> Result<(usize, usize, Vec<Color4b>), ImageError> {
    // The caller supplies the same checked token it will retain with the decoded pixels. Matching
    // sources therefore reuse its cached counts instead of evaluating the allocation policy twice.
    load_image_bytes_impl(source, Some(expected))
}

/// Shared raw copy path used by standalone images and dimension-checked atlas imports.
fn load_image_bytes_impl(source: ImageSource, expected: Option<CheckedImageDimensions>) -> Result<(usize, usize, Vec<Color4b>), ImageError> {
    // Validate dimensions and the exact byte count before allocating native color storage.
    let ImageSource { width, height, pixels } = source;
    if width <= 0 || height <= 0 {
        // Raw public input has signed dimensions, whereas the shared checked token stores
        // only valid positive extents. Reject invalid signs before converting to usize.
        return Err(ImageError::RawDimensionsOutOfRange { width, height });
    }
    let width_usize = width as usize;
    let height_usize = height as usize;
    let dimensions = resolve_dimensions(expected, width_usize, height_usize)?;
    if pixels.len() != dimensions.rgba_byte_count {
        return Err(ImageError::RawPixelLengthMismatch {
            expected: dimensions.rgba_byte_count,
            actual: pixels.len(),
        });
    }
    let mut colors = Vec::with_capacity(dimensions.pixel_count);
    // Exact buffer validation above guarantees every byte belongs to one complete RGBA
    // pixel, so the remainder returned by fixed-size slice partitioning is empty.
    for chunk in pixels.as_chunks::<4>().0 {
        colors.push(color4b(chunk[0], chunk[1], chunk[2], chunk[3]));
    }
    Ok((dimensions.width, dimensions.height, colors))
}

/// Resolves one checked dimensions token without allocating pixel storage.
fn resolve_dimensions(expected: Option<CheckedImageDimensions>, actual_width: usize, actual_height: usize) -> Result<CheckedImageDimensions, ImageError> {
    // Atlas loading reuses its existing token after an exact extent comparison. The public loader
    // has no token, so this is its single dimension/arithmetic/budget validation point.
    if let Some(expected) = expected {
        if actual_width != expected.width || actual_height != expected.height {
            return Err(ImageError::DimensionMismatch {
                expected_width: expected.width,
                expected_height: expected.height,
                actual_width,
                actual_height,
            });
        }
        return Ok(expected);
    }
    CheckedImageDimensions::try_new(actual_width, actual_height).map_err(ImageError::from)
}

/// Validates raw RGBA dimensions and exact byte length without allocating normalized pixels.
pub(crate) fn validate_rgba_buffer(width: i32, height: i32, len: usize) -> Result<(), ImageError> {
    if width <= 0 || height <= 0 {
        // Preserve both signed inputs because converting them first would lose which invalid value
        // entered the public raw-image API.
        return Err(ImageError::RawDimensionsOutOfRange { width, height });
    }
    // The shared checked token owns coordinate representation, arithmetic, and allocation-budget
    // policy for raw images, decoded images, atlases, and external textures alike.
    let dimensions = CheckedImageDimensions::try_new(width as usize, height as usize).map_err(ImageError::from)?;
    if len != dimensions.rgba_byte_count {
        return Err(ImageError::RawPixelLengthMismatch {
            expected: dimensions.rgba_byte_count,
            actual: len,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
