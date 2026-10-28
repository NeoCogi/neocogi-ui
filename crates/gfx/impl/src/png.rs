//! CPU PNG decoding and renderer upload share one bounded RGBA representation.
use super::Diagnostic;
use neocogi_com::{ComPtr, ComRef, INcStringAbi, NcCopyFromBufferOffset, NcStatus, com_object};
use neocogi_gfx_support::abi::*;
use neocogi_gfx_support::abi::MAX_ASSET_PIXELS_BYTES;
use std::io::{self, Cursor};

/// Decoded CPU pixels, ready for atlas packing or a texture-factory upload.
#[derive(Debug)]
pub(crate) struct RgbaImage {
    /// Positive dimensions whose exact RGBA storage fits the asset budget.
    pub(crate) size: Size,
    /// Tightly packed rows with four bytes per pixel and no math-library conversion.
    pub(crate) pixels: Vec<u8>,
}

impl RgbaImage {
    /// Builder/test convenience uses the complete asset budget; uploads supply their remaining limit.
    #[cfg(any(test, feature = "builder"))]
    pub(crate) fn decode(bytes: &[u8]) -> io::Result<Self> {
        Self::decode_bounded(bytes, MAX_ASSET_PIXELS_BYTES)
    }

    /// Applies a caller's aggregate resource budget before allocating decoded frame storage.
    fn decode_bounded(bytes: &[u8], limit: usize) -> io::Result<Self> {
        use ::png::{BitDepth, ColorType, DecodeOptions, Decoder, Transformations};

        // Bound encoded input independently of the decoded image so both callers use
        // the same policy before handing bytes to the PNG implementation.
        if bytes.is_empty() || bytes.len() > MAX_ASSET_PIXELS_BYTES {
            return Err(io::Error::other("PNG input must be nonempty and at most 64 MiB"));
        }
        let mut input = Cursor::new(bytes);
        let mut options = DecodeOptions::default();
        options.set_ignore_checksums(false);
        options.set_skip_ancillary_crc_failures(false);
        // Cursor implements BufRead directly: its position advances by the parser's
        // consumed bytes, without an outer buffer's read-ahead hiding trailing input.
        let mut decoder = Decoder::new_with_options(&mut input, options);
        decoder.set_transformations(Transformations::normalize_to_color8());
        let mut reader = decoder.read_info().map_err(io::Error::other)?;
        let width = reader.info().width;
        let height = reader.info().height;
        let size = Size {
            width: i32::try_from(width).map_err(io::Error::other)?,
            height: i32::try_from(height).map_err(io::Error::other)?,
        };
        let rgba_len = Self::byte_count(size)?;
        if rgba_len > limit {
            return Err(io::Error::other("PNG exceeds the remaining decoded-image budget"));
        }
        if reader.info().animation_control.is_some() {
            return Err(io::Error::other("animated PNG images are not supported"));
        }

        // Decode into a bounded buffer. RGBA output becomes the final allocation;
        // other normalized color models are expanded exactly once below.
        let output_len = reader.output_buffer_size().ok_or_else(|| io::Error::other("missing PNG output size"))?;
        if output_len > rgba_len {
            return Err(io::Error::other("PNG output exceeds its RGBA storage"));
        }
        let mut decoded = vec![0; output_len];
        let info = reader.next_frame(&mut decoded).map_err(io::Error::other)?;
        if info.width != width || info.height != height || info.bit_depth != BitDepth::Eight {
            return Err(io::Error::other("PNG frame does not match its normalized header"));
        }
        let channels = match info.color_type {
            ColorType::Grayscale => 1,
            ColorType::GrayscaleAlpha => 2,
            ColorType::Rgb => 3,
            ColorType::Rgba => 4,
            ColorType::Indexed => return Err(io::Error::other("PNG palette was not expanded")),
        };
        if info.line_size != width as usize * channels || info.buffer_size() != output_len {
            return Err(io::Error::other("PNG output does not contain complete packed rows"));
        }
        // Pixel-frame completion does not consume IEND's CRC or later metadata.
        // Validate the complete stream before either RGBA publication branch.
        reader.finish().map_err(io::Error::other)?;
        drop(reader);
        if input.position() != bytes.len() as u64 {
            return Err(io::Error::other("PNG input must contain exactly one complete image"));
        }
        if channels == 4 {
            return Ok(Self { size, pixels: decoded });
        }
        let mut pixels = Vec::with_capacity(rgba_len);
        for pixel in decoded.chunks_exact(channels) {
            // Grayscale repeats luminance; images without alpha become fully opaque.
            let rgba = match info.color_type {
                ColorType::Grayscale => [pixel[0], pixel[0], pixel[0], 255],
                ColorType::GrayscaleAlpha => [pixel[0], pixel[0], pixel[0], pixel[1]],
                ColorType::Rgb => [pixel[0], pixel[1], pixel[2], 255],
                _ => unreachable!("RGBA returned above and indexed output was rejected"),
            };
            pixels.extend_from_slice(&rgba);
        }
        Ok(Self { size, pixels })
    }

    /// Checks image dimensions and multiplication before any decoder or atlas allocation.
    pub(crate) fn byte_count(size: Size) -> io::Result<usize> {
        if size.width <= 0 || size.height <= 0 {
            return Err(io::Error::other("image dimensions must be positive"));
        }
        let bytes = (size.width as usize)
            .checked_mul(size.height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| io::Error::other("image dimensions overflow RGBA storage"))?;
        if bytes > MAX_ASSET_PIXELS_BYTES {
            return Err(io::Error::other("decoded image exceeds 64 MiB"));
        }
        Ok(bytes)
    }
}

#[com_object(neocogi_gfx_support::abi::PngDecoder)]
#[derive(Default)]
/// Decodes PNGs independently of the optional font rasterizer and JSON compiler.
pub struct PngDecoderObject {
    /// Last failed decode or upload; no diagnostic borrow spans the foreign factory call.
    diagnostic: Diagnostic,
}
impl PngDecoder for PngDecoderObject {
    /// Decodes fully before invoking the renderer, retaining all temporary buffers for the call only.
    fn upload(&self, png: NcCopyFromBufferOffset<'_, u8>, rgba_limit: u64, target: ComRef<'_, ITextureFactoryAbi>) -> Result<ComPtr<ITextureAbi>, NcStatus> {
        if png.offset() != 0 {
            return Err(self.diagnostic.failure("PNG input must start at offset zero"));
        }
        let image = RgbaImage::decode_bounded(png.as_slice(), rgba_limit.min(MAX_ASSET_PIXELS_BYTES as u64) as usize)
            .map_err(|error| self.diagnostic.failure(error))?;
        // Upload borrows the final CPU allocation only for this synchronous call.
        target
            .create_texture(image.size, NcCopyFromBufferOffset::new(0, &image.pixels))
            .inspect_err(|status| {
                self.diagnostic.failure(status);
            })
    }
}
impl IGfxDiagnostics for PngDecoderObject {
    /// Returns an immutable failure snapshot that remains usable through later uploads.
    fn diagnostic(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        Ok(self.diagnostic.snapshot())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neocogi_com::{INcStringComPtrExt, NC_E_INVALID_ARG, com_object};
    use neocogi_render_core::abi::RENDER_E_BACKEND;
    use std::{cell::Cell, rc::Rc};

    /// Encodes one tiny source image using an independent writer configuration.
    fn encoded(color: ::png::ColorType, pixels: &[u8], animated: bool) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = ::png::Encoder::new(&mut bytes, 1, 1);
            encoder.set_color(color);
            encoder.set_depth(::png::BitDepth::Eight);
            if color == ::png::ColorType::Indexed {
                encoder.set_palette(&[10, 20, 30][..]);
            }
            if animated {
                encoder.set_animated(1, 0).unwrap();
            }
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(pixels).unwrap();
        }
        bytes
    }

    /// Normalization preserves alpha and expands every supported source color model.
    #[test]
    fn normalized_pixels_have_exact_rgba_channels() {
        use ::png::ColorType::*;
        for (color, input, expected) in [
            (Grayscale, &[7][..], [7, 7, 7, 255]),
            (GrayscaleAlpha, &[7, 9][..], [7, 7, 7, 9]),
            (Rgb, &[10, 20, 30][..], [10, 20, 30, 255]),
            (Rgba, &[10, 20, 30, 40][..], [10, 20, 30, 40]),
            (Indexed, &[0][..], [10, 20, 30, 255]),
        ] {
            let image = RgbaImage::decode(&encoded(color, input, false)).unwrap();
            assert_eq!((image.size.width, image.size.height), (1, 1));
            assert_eq!(image.pixels, expected);
        }
    }

    /// Static-image policy rejects animation and malformed input while bounding allocations.
    #[test]
    fn invalid_images_and_storage_are_rejected() {
        assert!(RgbaImage::decode(&[]).is_err());
        assert!(RgbaImage::decode(b"not PNG").is_err());
        let animated = encoded(::png::ColorType::Rgba, &[255; 4], true);
        assert!(RgbaImage::decode(&animated).unwrap_err().to_string().contains("animated"));
        assert_eq!(RgbaImage::byte_count(Size { width: 4096, height: 4096 }).unwrap(), MAX_ASSET_PIXELS_BYTES);
        for size in [Size { width: 0, height: 1 }, Size { width: -1, height: 1 }, Size { width: 4096, height: 4097 }] {
            assert!(RgbaImage::byte_count(size).is_err());
        }
    }

    #[com_object(interfaces(ITexture))]
    /// CPU-only result verifies that decoding returns the factory's actual owned capability.
    struct Texture {
        /// Immutable dimensions supplied by the decoder.
        size: Size,
    }
    impl ITexture for Texture {
        /// Returns the uploaded dimensions without requiring a GPU in this boundary test.
        fn size(&self) -> Result<Size, NcStatus> {
            Ok(self.size)
        }
    }

    #[com_object(interfaces(ITextureFactory))]
    /// A foreign factory reenters diagnostics and can fail after receiving decoded bytes.
    struct Factory {
        /// Shared observations survive the independently reference-counted factory allocation.
        calls: Rc<Cell<u32>>,
        /// Controls one upload failure without replacing or recreating the decoder.
        fail: Rc<Cell<bool>>,
        /// Retained diagnostics exercise reentry across the borrowed upload boundary.
        diagnostics: ComPtr<IGfxDiagnosticsAbi>,
    }
    impl ITextureFactory for Factory {
        /// Validates normalized RGBA pixels and deliberately calls back before returning.
        fn create_texture(&self, size: Size, pixels: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<ITextureAbi>, NcStatus> {
            self.calls.set(self.calls.get() + 1);
            assert_eq!(pixels.offset(), 0);
            assert_eq!(pixels.len(), size.width as usize * size.height as usize * 4);
            assert!(pixels.as_slice().iter().all(|channel| *channel == 255));
            self.diagnostics.diagnostic()?.to_string()?;
            if self.fail.get() {
                return Err(RENDER_E_BACKEND);
            }
            Texture::from_value(Texture { size }).query()
        }
    }

    /// Invalid input never reaches a foreign factory; failed uploads preserve status and allow retry.
    #[test]
    fn bounded_png_upload_reentry_failure_and_recovery() {
        let decoder: ComPtr<PngDecoderAbi> = PngDecoderObject::from_value(PngDecoderObject::default()).query().unwrap();
        let diagnostics: ComPtr<IGfxDiagnosticsAbi> = decoder.query().unwrap();
        let calls = Rc::new(Cell::new(0));
        let fail = Rc::new(Cell::new(false));
        let factory: ComPtr<ITextureFactoryAbi> = Factory::from_value(Factory {
            calls: calls.clone(),
            fail: fail.clone(),
            diagnostics: diagnostics.clone(),
        })
        .query()
        .unwrap();
        let png = include_bytes!("../../../../assets/WHITE.png");
        for input in [
            NcCopyFromBufferOffset::new(1, png),
            NcCopyFromBufferOffset::new(0, &[]),
            NcCopyFromBufferOffset::new(0, b"invalid PNG"),
        ] {
            assert_eq!(
                decoder.upload(input, MAX_ASSET_PIXELS_BYTES as u64, factory.as_com_ref()).unwrap_err(),
                NC_E_INVALID_ARG
            );
        }
        // Even valid compressed input cannot allocate/upload past the caller's remaining budget.
        assert!(decoder.upload(NcCopyFromBufferOffset::new(0, png), 0, factory.as_com_ref()).is_err());
        assert_eq!(calls.get(), 0);
        fail.set(true);
        assert_eq!(
            decoder
                .upload(NcCopyFromBufferOffset::new(0, png), MAX_ASSET_PIXELS_BYTES as u64, factory.as_com_ref())
                .unwrap_err(),
            RENDER_E_BACKEND
        );
        let failure = diagnostics.diagnostic().unwrap();
        let message = failure.to_string().unwrap();
        assert!(!message.is_empty());
        fail.set(false);
        let texture = decoder
            .upload(NcCopyFromBufferOffset::new(0, png), MAX_ASSET_PIXELS_BYTES as u64, factory.as_com_ref())
            .unwrap();
        assert!(texture.size().unwrap().width > 0);
        assert_eq!(calls.get(), 2);
        assert_eq!(failure.to_string().unwrap(), message);
    }
    /// Computes the CRC-32 checksum used by PNG chunks without adding a test-only dependency.
    fn png_crc32(bytes: &[u8]) -> u32 {
        // PNG uses the reflected IEEE polynomial. Applying its eight bit steps explicitly keeps the
        // malicious-header fixture local and avoids involving the image decoder in fixture creation.
        let mut crc = !0_u32;
        for byte in bytes {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                let low_bit_mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & low_bit_mask);
            }
        }
        !crc
    }

    /// Inserts a valid text chunk after IDAT, before the encoded image's IEND.
    /// This independently checks end-of-file handling without rejecting legal metadata.
    fn with_post_image_text(bytes: &[u8]) -> Vec<u8> {
        let end = bytes.len() - 12;
        assert_eq!(&bytes[end + 4..end + 8], b"IEND");
        let text = b"Note\0valid post-IDAT text";
        let mut decorated = bytes[..end].to_vec();
        decorated.extend_from_slice(&(text.len() as u32).to_be_bytes());
        let checksum_start = decorated.len();
        decorated.extend_from_slice(b"tEXt");
        decorated.extend_from_slice(text);
        let checksum = png_crc32(&decorated[checksum_start..]);
        decorated.extend_from_slice(&checksum.to_be_bytes());
        decorated.extend_from_slice(&bytes[end..]);
        decorated
    }

    /// Every prefix and corrupt ending rejects before a real generated factory
    /// call, including both direct RGBA reuse and normalized-color expansion.
    #[test]
    fn complete_input_validation_precedes_factory_publication() {
        let decoder: ComPtr<PngDecoderAbi> = PngDecoderObject::from_value(PngDecoderObject::default()).query().unwrap();
        let calls = Rc::new(Cell::new(0));
        let factory: ComPtr<ITextureFactoryAbi> = Factory::from_value(Factory {
            calls: calls.clone(),
            fail: Rc::new(Cell::new(false)),
            diagnostics: decoder.query().unwrap(),
        })
        .query()
        .unwrap();
        for (color, pixels) in [
            (::png::ColorType::Rgba, &[255; 4][..]),
            (::png::ColorType::Rgb, &[255; 3][..]),
            (::png::ColorType::Grayscale, &[255][..]),
        ] {
            let plain = encoded(color, pixels, false);
            let with_text = with_post_image_text(&plain);
            for valid in [&plain, &with_text] {
                let before = calls.get();
                for end in 0..valid.len() {
                    assert_eq!(
                        decoder
                            .upload(
                                NcCopyFromBufferOffset::new(0, &valid[..end]),
                                MAX_ASSET_PIXELS_BYTES as u64,
                                factory.as_com_ref()
                            )
                            .unwrap_err(),
                        NC_E_INVALID_ARG,
                        "{color:?}: prefix {end}"
                    );
                }
                let mut corrupt = valid.clone();
                *corrupt.last_mut().unwrap() ^= 1;
                let mut trailing = valid.clone();
                trailing.extend_from_slice(b"trailing");
                let mut concatenated = valid.clone();
                concatenated.extend_from_slice(&plain);
                for invalid in [corrupt, trailing, concatenated] {
                    assert_eq!(
                        decoder
                            .upload(NcCopyFromBufferOffset::new(0, &invalid), MAX_ASSET_PIXELS_BYTES as u64, factory.as_com_ref())
                            .unwrap_err(),
                        NC_E_INVALID_ARG
                    );
                }
                assert_eq!(calls.get(), before, "malformed inputs never enter foreign factory code");
                decoder
                    .upload(NcCopyFromBufferOffset::new(0, valid), MAX_ASSET_PIXELS_BYTES as u64, factory.as_com_ref())
                    .unwrap();
                assert_eq!(calls.get(), before + 1);
            }
            let mut corrupt_text = with_text.clone();
            // The four bytes immediately before IEND are the tEXt chunk's CRC.
            let crc = corrupt_text.len() - 13;
            corrupt_text[crc] ^= 1;
            let before = calls.get();
            assert_eq!(
                decoder
                    .upload(
                        NcCopyFromBufferOffset::new(0, &corrupt_text),
                        MAX_ASSET_PIXELS_BYTES as u64,
                        factory.as_com_ref()
                    )
                    .unwrap_err(),
                NC_E_INVALID_ARG
            );
            assert_eq!(calls.get(), before);
        }
    }

    /// A valid PNG chunk CRC cannot conceal a corrupt zlib checksum inside IDAT.
    #[test]
    fn compressed_checksum_is_validated_independently_of_chunk_crc() {
        let mut bytes = encoded(::png::ColorType::Rgba, &[255; 4], false);
        let mut chunk = 8;
        loop {
            let len = u32::from_be_bytes(bytes[chunk..chunk + 4].try_into().unwrap()) as usize;
            let end = chunk + 8 + len;
            if &bytes[chunk + 4..chunk + 8] == b"IDAT" {
                assert!(len > 6);
                bytes[end - 1] ^= 1;
                let crc = png_crc32(&bytes[chunk + 4..end]);
                bytes[end..end + 4].copy_from_slice(&crc.to_be_bytes());
                break;
            }
            chunk = end + 4;
        }
        assert!(RgbaImage::decode(&bytes).is_err());
    }

    /// Rewrites a PNG fixture's IHDR extent and repairs that chunk's checksum.
    fn rewrite_png_dimensions(bytes: &mut [u8], width: u32, height: u32) {
        // Every PNG begins with an eight-byte signature followed by the fixed 13-byte IHDR chunk. The
        // encoded fixture therefore has stable offsets for width, height, and the four-byte checksum.
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&bytes[12..16], b"IHDR");
        bytes[16..20].copy_from_slice(&width.to_be_bytes());
        bytes[20..24].copy_from_slice(&height.to_be_bytes());
        let checksum = png_crc32(&bytes[12..29]);
        bytes[29..33].copy_from_slice(&checksum.to_be_bytes());
    }

    /// Rejects an oversized, checksummed IHDR before decompressing the tiny frame payload.
    #[test]
    fn oversized_header_is_rejected_before_frame_allocation() {
        let mut bytes = encoded(::png::ColorType::Rgba, &[1, 2, 3, 255], false);
        rewrite_png_dimensions(&mut bytes, 4096, 4097);
        // The diagnostic must come from the allocation bound, not the mismatched IDAT payload.
        let error = RgbaImage::decode(&bytes).unwrap_err();
        assert!(error.to_string().contains("exceeds 64 MiB"), "{error}");
    }
}
