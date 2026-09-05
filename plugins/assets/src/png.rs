//! Stateless compressed-image decoding uses the existing bounded static-PNG implementation.
use super::*;
use nc_com::{ComRef, NcCopyFromBufferOffset, com_class};

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
        if png.offset() != 0 || png.is_empty() || png.len() > microui_ui::image::MAX_DECODED_RGBA_BYTES {
            return Err(self.diagnostic.failure("PNG input must be one nonempty buffer of at most 64 MiB"));
        }
        let (width, height, pixels) = microui_ui::image::load_image_bytes(microui_ui::image::ImageSource::Png { bytes: png.as_slice() })
            .map_err(|error| self.diagnostic.failure(error))?;
        // The shared decoder validates positive i32 dimensions and a 64 MiB decoded-image budget
        // before allocating. Convert channels explicitly; no math-library memory layout crosses COM.
        let pixels: Vec<_> = pixels.into_iter().flat_map(|p| [p.x, p.y, p.z, p.w]).collect();
        target
            .create_texture(
                Size {
                    width: width as i32,
                    height: height as i32,
                },
                NcCopyFromBufferOffset::new(0, &pixels),
            )
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
