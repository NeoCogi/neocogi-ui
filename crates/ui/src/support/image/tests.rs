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

//! Tests for atlas image loading and source conversion helpers.

use super::*;

/// Returns one checked image extent for tests that exercise the crate-private atlas path.
fn checked_dimensions(width: usize, height: usize) -> CheckedImageDimensions {
    // Small fixtures are necessarily within every coordinate and allocation bound; failure here
    // means the shared policy changed rather than that a decoder assertion should be updated.
    CheckedImageDimensions::try_new(width, height).expect("test image dimensions must satisfy the shared storage policy")
}

/// Verifies the fixed allocation budget accepts its exact RGBA boundary and rejects one extra row.
#[test]
fn checked_dimensions_enforce_the_decoded_storage_budget_without_allocating() {
    let accepted = CheckedImageDimensions::try_new(4096, 4096).expect("a 64-MiB RGBA image is the documented boundary");
    assert_eq!(accepted.pixel_count, 4096 * 4096);
    assert_eq!(accepted.rgba_byte_count, MAX_DECODED_RGBA_BYTES);

    let error = CheckedImageDimensions::try_new(4096, 4097).expect_err("one extra 4096-pixel row must exceed the fixed budget");
    assert_eq!(
        error,
        ImageStorageError::TooLarge {
            width: 4096,
            height: 4097,
            required_bytes: 4096 * 4097 * 4,
            maximum_bytes: MAX_DECODED_RGBA_BYTES,
        }
    );
}

/// Verifies the public raw loader exposes invalid signed dimensions without an I/O wrapper.
#[test]
fn public_raw_loader_returns_typed_dimension_errors() {
    let error =
        load_image_bytes(ImageSource { width: -3, height: 7, pixels: &[] }).expect_err("a negative raw-image axis must be rejected before byte validation");

    assert!(matches!(error, ImageError::RawDimensionsOutOfRange { width: -3, height: 7 }));
}

/// Verifies exact raw-byte requirements remain directly matchable by public callers.
#[test]
fn public_raw_loader_returns_typed_pixel_length_errors() {
    let error = load_image_bytes(ImageSource { width: 2, height: 1, pixels: &[0; 7] }).expect_err("seven bytes cannot fill two RGBA pixels");

    assert!(matches!(error, ImageError::RawPixelLengthMismatch { expected: 8, actual: 7 }));
}

/// Verifies the public loader preserves the shared allocation classification and error chain.
#[test]
fn public_raw_loader_composes_storage_errors() {
    let error = load_image_bytes(ImageSource { width: 4096, height: 4097, pixels: &[] })
        .expect_err("an oversized raw source must fail before its empty bytes are inspected");

    assert!(matches!(
        &error,
        ImageError::Storage {
            source: ImageStorageError::TooLarge {
                width: 4096,
                height: 4097,
                required_bytes: 67_125_248,
                maximum_bytes: MAX_DECODED_RGBA_BYTES,
            }
        }
    ));
    assert!(std::error::Error::source(&error).is_some());
}

/// Verifies raw sources report a concrete mismatch instead of requiring error-text inspection.
#[test]
fn checked_raw_decode_reports_both_dimension_pairs() {
    let bytes = [0_u8; 8];
    let error = load_image_bytes_checked(ImageSource { width: 2, height: 1, pixels: &bytes }, checked_dimensions(1, 1))
        .expect_err("raw dimensions must match the checked caller metadata");

    // Pattern matching exposes all four typed values without parsing the Display message.
    match error {
        ImageError::DimensionMismatch {
            expected_width,
            expected_height,
            actual_width,
            actual_height,
        } => {
            assert_eq!((expected_width, expected_height), (1, 1));
            assert_eq!((actual_width, actual_height), (2, 1));
        }

        ImageError::Storage { source } => panic!("small checked dimensions unexpectedly failed storage validation: {source}"),
        ImageError::RawDimensionsOutOfRange { .. } => panic!("positive raw dimensions were misclassified as invalid"),
        ImageError::RawPixelLengthMismatch { .. } => panic!("dimension comparison must precede raw byte-length validation"),
    }
}
