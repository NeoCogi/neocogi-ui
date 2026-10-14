//! Optional font/atlas construction and JSON theme assembly.
use super::Diagnostic;
use crate::atlas::{Builder, Config};
use neocogi_com::{ComPtr, ComRef, NcCopyFromBufferOffset, NcStatus, INcStringAbi, NC_E_INVALID_ARG, com_factory, com_object};
use neocogi_gfx_contracts::*;
use neocogi_gfx_support::AssetData;
use neocogi_platform_contracts::INativePathAbi;
use std::cell::RefCell;
use std::path::Path;

#[com_factory(neocogi_gfx_contracts::AssetLoader)]
#[derive(Default)]
/// Stateless construction root with owned diagnostics; drafts contain the actual mutable builder.
pub struct AssetLoaderObject {
    /// Last construction failure, independent of any earlier returned draft.
    diagnostic: Diagnostic,
}

impl AssetLoader for AssetLoaderObject {
    /// Checks dimensions before decoding the supplied path or reading the white tile.
    fn create_builder(&self, size: Size, white: ComRef<'_, INativePathAbi>) -> Result<ComPtr<IAssetBuilderAbi>, NcStatus> {
        dimensions(size)?;
        let white_icon = crate::path::decode(white)?;
        let builder = Builder::from_config(&Config {
            texture_width: size.width as usize,
            texture_height: size.height as usize,
            white_icon,
            icons: Vec::new(),
            fonts: Vec::new(),
        })
        .map_err(|error| self.diagnostic.failure(error))?;
        Ok(BuildSession::create(Draft { builder, base: None }))
    }
    /// Copies validated records before packing; the draft retains the original catalog for publication.
    fn derive_builder(&self, size: Size, base: ComRef<'_, IAssetBundleAbi>, options: AssetDeriveOptions) -> Result<ComPtr<IAssetBuilderAbi>, NcStatus> {
        dimensions(size)?;
        let data = AssetData::read(base)?;
        let semantic_fonts = FontRole::ALL.map(|role| role.canonical_name().expect("declared font roles have canonical names"));
        let semantic_icons = IconRole::ALL.map(|role| role.canonical_name().expect("declared icon roles have canonical names"));
        let excluded_fonts = if options.replace_semantic_fonts { semantic_fonts.as_slice() } else { &[] };
        let excluded_icons = if options.replace_semantic_icons { semantic_icons.as_slice() } else { &[] };
        let builder = Builder::from_data(&data, size.width as usize, size.height as usize, excluded_fonts, excluded_icons)
            .map_err(|error| self.diagnostic.failure(error))?;
        Ok(BuildSession::create(Draft { builder, base: Some(base.add_ref()) }))
    }
}
impl IGfxDiagnostics for AssetLoaderObject {
    /// Returns a snapshot that remains unchanged by later failures.
    fn diagnostic(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        Ok(self.diagnostic.snapshot())
    }
}

/// One owned construction transaction; optional base establishes resource identity, not behavior erasure.
struct Draft {
    /// Native packing and rasterization state, always private to this binary.
    builder: Builder,
    /// A derived session must retain the original resource namespace through successful finish.
    base: Option<ComPtr<IAssetBundleAbi>>,
}

/// Exclusive mutable session lifecycle, including an observable terminal finish state.
enum BuildState {
    /// One boxed working draft keeps the state slot small while operations move ownership.
    Idle(Box<Draft>),
    /// Owned operation has detached the draft across any foreign path calls.
    Busy,
    /// Finish consumed the native builder, including a failed final validation.
    Finished,
}

#[com_object(interfaces(IAssetBuilder, IGfxDiagnostics))]
/// Concrete builder object with no callbacks, native resource handles or erased payloads.
struct BuildSession {
    /// State is taken, never borrowed, while caller-provided COM path methods run.
    state: RefCell<BuildState>,
    /// Last operational failure can be inspected after a terminal failed finish.
    diagnostic: Diagnostic,
}
impl BuildSession {
    /// Publishes an idle builder only after initial decoding and packing succeeded.
    fn create(draft: Draft) -> ComPtr<IAssetBuilderAbi> {
        Self::from_value(Self {
            state: RefCell::new(BuildState::Idle(Box::new(draft))),
            diagnostic: Diagnostic::default(),
        })
        .query()
        .expect("BuildSession declares IAssetBuilder")
    }
    /// Takes one native draft and rejects reentry before calling any foreign path capability.
    fn edit(&self) -> Result<Edit<'_>, NcStatus> {
        let mut slot = self.state.try_borrow_mut().map_err(|_| GFX_E_BUSY)?;
        match std::mem::replace(&mut *slot, BuildState::Busy) {
            BuildState::Idle(draft) => Ok(Edit { session: self, draft: Some(draft) }),
            BuildState::Busy => Err(GFX_E_BUSY),
            BuildState::Finished => {
                *slot = BuildState::Finished;
                Err(GFX_E_CLOSED)
            }
        }
    }
}

/// Restores an edited draft on errors; consuming finish instead publishes terminal state.
struct Edit<'a> {
    /// Session outlives this synchronous local borrow and receives the draft on return.
    session: &'a BuildSession,
    /// None only after finish has consumed the draft.
    draft: Option<Box<Draft>>,
}
impl Edit<'_> {
    /// Borrows the locally owned draft after its COM-visible slot is marked busy.
    fn draft(&mut self) -> &mut Draft {
        self.draft.as_mut().expect("unconsumed edit")
    }
}
impl Drop for Edit<'_> {
    /// Restores ownership before the caller may invoke another builder operation.
    fn drop(&mut self) {
        *self.session.state.borrow_mut() = self.draft.take().map_or(BuildState::Finished, BuildState::Idle);
    }
}

impl IAssetBuilder for BuildSession {
    /// Reads and packs one named PNG using the existing transactional native builder operation.
    fn add_icon(&self, name: NcCopyFromBufferOffset<'_, u8>, path: ComRef<'_, INativePathAbi>) -> Result<(), NcStatus> {
        let mut edit = self.edit()?;
        let name = text(name)?;
        edit.draft().builder.check_icon_name(name).map_err(|error| self.diagnostic.failure(error))?;
        let path = crate::path::decode(path)?;
        edit.draft()
            .builder
            .add_icon_named(name, &path)
            .map(|_| ())
            .map_err(|error| self.diagnostic.failure(error))
    }
    /// Decodes and packs one caller-owned PNG buffer without retaining the input bytes.
    fn add_icon_bytes(&self, name: NcCopyFromBufferOffset<'_, u8>, png: NcCopyFromBufferOffset<'_, u8>) -> Result<AssetImagePlacement, NcStatus> {
        let mut edit = self.edit()?;
        let name = text(name)?;
        let png = blob(png)?;
        edit.draft()
            .builder
            .add_icon_bytes_named(name, Path::new(name), png)
            .map_err(|error| self.diagnostic.failure(error))
    }
    /// Lets the namespace owner choose a free artwork name within the same draft
    /// transaction as decoding/packing. Errors restore the entire owned draft.
    fn add_artwork_bytes(&self, png: NcCopyFromBufferOffset<'_, u8>) -> Result<AssetImagePlacement, NcStatus> {
        let mut edit = self.edit()?;
        let png = blob(png)?;
        edit.draft().builder.add_artwork_bytes(png).map_err(|error| self.diagnostic.failure(error))
    }
    /// Rasterizes named font data without constructing any UI runtime resource IDs.
    fn add_font(&self, name: NcCopyFromBufferOffset<'_, u8>, path: ComRef<'_, INativePathAbi>, pixels: u32) -> Result<(), NcStatus> {
        let mut edit = self.edit()?;
        let name = text(name)?;
        if pixels == 0 || pixels > i32::MAX as u32 {
            return Err(NC_E_INVALID_ARG);
        }
        edit.draft()
            .builder
            .check_font(name, pixels as usize)
            .map_err(|error| self.diagnostic.failure(error))?;
        let path = crate::path::decode(path)?;
        edit.draft()
            .builder
            .add_font_named(name, &path, pixels as usize)
            .map(|_| ())
            .map_err(|error| self.diagnostic.failure(error))
    }
    /// Rasterizes one caller-owned font buffer without retaining the source bytes.
    fn add_font_bytes(&self, name: NcCopyFromBufferOffset<'_, u8>, font: NcCopyFromBufferOffset<'_, u8>, pixels: u32) -> Result<(), NcStatus> {
        let mut edit = self.edit()?;
        let name = text(name)?;
        let font = blob(font)?;
        if pixels == 0 || pixels > i32::MAX as u32 {
            return Err(NC_E_INVALID_ARG);
        }
        edit.draft()
            .builder
            .add_font_bytes_named(name, Path::new(name), pixels as usize, font)
            .map_err(|error| self.diagnostic.failure(error))
    }
    /// Consumes native packing state, validates standard semantic resources and publishes one revision.
    fn finish(&self) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        let mut edit = self.edit()?;
        let draft = edit.draft.take().expect("finish consumes the taken draft once");
        let data = draft.builder.build().map_err(|error| self.diagnostic.failure(error))?;
        // Publication retains the catalog capability; no producer-local UI IDs are needed.
        match draft.base {
            Some(base) => data.into_revision(base.as_com_ref()),
            None => data.into_bundle(),
        }
    }
}
impl IGfxDiagnostics for BuildSession {
    /// Retains terminal failure detail without reopening or retaining a consumed native builder.
    fn diagnostic(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        Ok(self.diagnostic.snapshot())
    }
}

/// Applies the shared decoded-pixel limit before filesystem or allocation work begins.
fn dimensions(size: Size) -> Result<(), NcStatus> {
    AssetInfo {
        schema: ASSET_SCHEMA_VERSION,
        size,
        fonts: 1,
        icons: 1,
        glyphs: 0,
        names: 0,
    }
    .rgba_bytes()
    .map(|_| ())
}

/// Accepts only complete, nonempty UTF-8 names within the shared metadata bound.
fn text(input: NcCopyFromBufferOffset<'_, u8>) -> Result<&str, NcStatus> {
    if input.offset() != 0 || input.is_empty() || input.len() > MAX_ASSET_NAME_BYTES as usize {
        return Err(NC_E_INVALID_ARG);
    }
    std::str::from_utf8(input.as_slice()).map_err(|_| NC_E_INVALID_ARG)
}

/// Accepts one complete nonempty binary input within the shared decoding ceiling.
fn blob(input: NcCopyFromBufferOffset<'_, u8>) -> Result<&[u8], NcStatus> {
    if input.offset() != 0 || input.is_empty() || input.len() > MAX_ASSET_PIXELS_BYTES {
        return Err(NC_E_INVALID_ARG);
    }
    Ok(input.as_slice())
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
#[path = "asset_limits_tests.rs"]
mod limit_tests;
