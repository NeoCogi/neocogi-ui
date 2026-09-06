//! CPU PNG decoding and renderer upload share one bounded RGBA representation.
use super::*;
use nc_com::{ComRef, NcCopyFromBufferOffset, com_class};
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
    /// Decodes one static PNG, checking its dimensions before allocating pixel storage.
    pub(crate) fn decode(bytes: &[u8]) -> io::Result<Self> {
        use ::png::{BitDepth, ColorType, Decoder, Transformations};

        // Bound encoded input independently of the decoded image so both callers use
        // the same policy before handing bytes to the PNG implementation.
        if bytes.is_empty() || bytes.len() > MAX_ASSET_PIXELS_BYTES {
            return Err(io::Error::other("PNG input must be nonempty and at most 64 MiB"));
        }
        let mut decoder = Decoder::new(Cursor::new(bytes));
        decoder.set_transformations(Transformations::normalize_to_color8());
        let mut reader = decoder.read_info().map_err(io::Error::other)?;
        let width = reader.info().width;
        let height = reader.info().height;
        let size = Size {
            width: i32::try_from(width).map_err(io::Error::other)?,
            height: i32::try_from(height).map_err(io::Error::other)?,
        };
        let rgba_len = Self::byte_count(size)?;
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

#[com_class(class_id(PNG_DECODER_CLASS), name = "microui.PngDecoder", version = "0.9.0-alpha.0",
    constructor(args = none, default), interfaces(IPngDecoder, IAssetDiagnostics))]
#[derive(Default)]
/// PNG-only deployments need neither font rasterization nor JSON constructors in their descriptor.
pub struct PngDecoder {
    /// Last failed decode or upload; no diagnostic borrow spans the foreign factory call.
    diagnostic: Diagnostic,
}
impl IPngDecoder for PngDecoder {
    /// Decodes fully before invoking the renderer, retaining all temporary buffers for the call only.
    fn upload(&self, png: NcCopyFromBufferOffset<'_, u8>, target: ComRef<'_, ITextureFactoryAbi>) -> Result<ComPtr<ITextureAbi>, NcStatus> {
        if png.offset() != 0 {
            return Err(self.diagnostic.failure("PNG input must start at offset zero"));
        }
        let image = RgbaImage::decode(png.as_slice()).map_err(|error| self.diagnostic.failure(error))?;
        // Upload borrows the final CPU allocation only for this synchronous call.
        target
            .create_texture(image.size, NcCopyFromBufferOffset::new(0, &image.pixels))
            .inspect_err(|status| {
                self.diagnostic.failure(status);
            })
    }
}
impl IAssetDiagnostics for PngDecoder {
    /// Returns an immutable failure snapshot that remains usable through later uploads.
    fn diagnostic(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        Ok(self.diagnostic.snapshot())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nc_com::INcStringComPtrExt;
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

    #[com_class(interfaces(ITexture))]
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

    #[com_class(interfaces(ITextureFactory))]
    /// A foreign factory reenters diagnostics and can fail after receiving decoded bytes.
    struct Factory {
        /// Shared observations survive the independently reference-counted factory allocation.
        calls: Rc<Cell<u32>>,
        /// Controls one upload failure without replacing or recreating the decoder.
        fail: Rc<Cell<bool>>,
        /// Retained diagnostics exercise reentry across the borrowed upload boundary.
        diagnostics: ComPtr<IAssetDiagnosticsAbi>,
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
                return Err(UI_E_RENDER);
            }
            Texture::from_value(Texture { size }).query()
        }
    }

    /// Invalid input never reaches a foreign factory; failed uploads preserve status and allow retry.
    #[test]
    fn bounded_png_upload_reentry_failure_and_recovery() {
        let decoder: ComPtr<IPngDecoderAbi> = PngDecoder::from_value(PngDecoder::default()).query().unwrap();
        let diagnostics: ComPtr<IAssetDiagnosticsAbi> = decoder.query().unwrap();
        let calls = Rc::new(Cell::new(0));
        let fail = Rc::new(Cell::new(false));
        let factory: ComPtr<ITextureFactoryAbi> = Factory::from_value(Factory {
            calls: calls.clone(),
            fail: fail.clone(),
            diagnostics: diagnostics.clone(),
        })
        .query()
        .unwrap();
        let png = include_bytes!("../../../assets/WHITE.png");
        for input in [
            NcCopyFromBufferOffset::new(1, png),
            NcCopyFromBufferOffset::new(0, &[]),
            NcCopyFromBufferOffset::new(0, b"invalid PNG"),
        ] {
            assert_eq!(decoder.upload(input, factory.as_com_ref()).unwrap_err(), NC_E_INVALID_ARG);
        }
        assert_eq!(calls.get(), 0);
        fail.set(true);
        assert_eq!(
            decoder.upload(NcCopyFromBufferOffset::new(0, png), factory.as_com_ref()).unwrap_err(),
            UI_E_RENDER
        );
        let failure = diagnostics.diagnostic().unwrap();
        let message = failure.to_string().unwrap();
        assert!(!message.is_empty());
        fail.set(false);
        let texture = decoder.upload(NcCopyFromBufferOffset::new(0, png), factory.as_com_ref()).unwrap();
        assert!(texture.size().unwrap().width > 0);
        assert_eq!(calls.get(), 2);
        assert_eq!(failure.to_string().unwrap(), message);
    }
}
