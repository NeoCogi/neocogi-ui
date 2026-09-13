//
// Copyright 2026-Present (c) Raja Lehtihet & Wael El Oraiby
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

//! Versioned JSON theme decoding and compilation into portable appearance tables.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

use serde::Deserialize;

use neocogi_com::{ComPtr, ComRef, NcCopyFromBufferOffset, NcStatus};
#[cfg(feature = "builder")]
use neocogi_gfx_impl::atlas::Builder;
use neocogi_appearance::{AppearanceTables, rgba};
use neocogi_gfx_contracts::*;
use neocogi_gfx_contracts::Insets as SliceInsets;
#[cfg(feature = "builder")]
use neocogi_gfx_support::AssetData;
use neocogi_gfx_support::{ChromeState, ControlState, MenuState, PointerState, SurfaceState};

/// Explicit byte source for a theme's referenced fonts and images.
pub trait AssetSource {
    /// Reads one anchored asset path; implementations own their storage and I/O policy.
    fn read(&mut self, path: &Path) -> io::Result<Vec<u8>>;
}

/// Filesystem source with the same allocation bounds as atlas construction.
pub struct FileAssets;

impl AssetSource for FileAssets {
    /// Reads a bounded file without exposing a hidden capture or callback.
    fn read(&mut self, path: &Path) -> io::Result<Vec<u8>> {
        read_asset_file(path)
    }
}

/// Reads one complete external theme resource without pulling the GFX builder into Skin.
fn read_asset_file(path: &Path) -> io::Result<Vec<u8>> {
    let file = File::open(path)?;
    if file.metadata()?.len() > MAX_ASSET_PIXELS_BYTES as u64 {
        return Err(io::Error::other(format!("{} exceeds 64 MiB", path.display())));
    }
    let mut bytes = Vec::new();
    file.take(MAX_ASSET_PIXELS_BYTES as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_ASSET_PIXELS_BYTES {
        return Err(io::Error::other(format!("{} exceeds 64 MiB", path.display())));
    }
    Ok(bytes)
}

/// Theme-file schema version understood by this crate release.
pub const THEME_SCHEMA_VERSION: u32 = 1;

/// Named, fully resolved resource tables produced by a theme definition.
#[cfg(feature = "builder")]
#[derive(Clone)]
pub struct LoadedTheme {
    /// Human-readable name supplied by the JSON definition.
    name: String,
    /// Validated pixels, resource metadata and appearance tables ready for publication.
    data: AssetData,
}

#[cfg(feature = "builder")]
impl LoadedTheme {
    /// Borrows the authored selection label without retaining the source document.
    pub fn name(&self) -> &str {
        // The parser has already rejected empty labels.
        &self.name
    }

    /// Borrows the complete resource revision for build-time export.
    pub fn data(&self) -> &AssetData {
        // Pixels and appearances stay paired throughout export and publication.
        &self.data
    }

    /// Transfers the compiled storage to ordinary bundle publication.
    pub fn into_data(self) -> AssetData {
        // Moving the data avoids another copy of the packed atlas.
        self.data
    }
}

/// Concrete failure produced while reading, decoding, validating, or compiling a theme.
#[derive(Debug)]
pub enum ThemeLoadError {
    /// The JSON definition file could not be read.
    DefinitionRead {
        /// Exact path requested by the caller.
        path: PathBuf,
        /// Concrete filesystem failure.
        source: io::Error,
    },
    /// The definition is not valid JSON for the strict schema.
    DefinitionJson {
        /// Exact JSON path whose bytes were parsed.
        path: PathBuf,
        /// Structured serde JSON diagnostic.
        source: serde_json::Error,
    },
    /// The document targets a different schema version.
    UnsupportedSchema {
        /// Only schema version accepted by this crate release.
        expected: u32,
        /// Version declared by the document.
        actual: u32,
    },
    /// The document's display name is empty or whitespace-only.
    EmptyName,
    /// Destination or source slice insets are negative or do not fit their source image.
    InvalidInsets {
        /// Semantic appearance key containing the invalid value.
        appearance: String,
        /// Schema field that supplied the invalid value.
        field: &'static str,
        /// Complete rejected inset value.
        insets: SliceInsets,
        /// Optional image dimensions used for source-bound validation.
        image_size: Option<(i32, i32)>,
    },
    /// The theme's fonts or image artwork could not be read, decoded, packed, or finalized.
    AtlasBuild {
        /// Concrete atlas-builder failure retaining its asset or validation classification.
        source: io::Error,
    },
    /// The explicitly supplied GFX capability rejected one construction operation.
    GfxBuild {
        /// Stable operation category without exposing a GFX implementation type.
        operation: &'static str,
        /// Exact status returned across the typed plugin boundary.
        status: NcStatus,
    },
}

impl fmt::Display for ThemeLoadError {
    /// Formats each failure with the relevant document, appearance, or nested asset diagnostic.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Keep classifications visible in text while retaining structured sources for callers.
        match self {
            Self::DefinitionRead { path, source } => write!(formatter, "failed to read theme definition {}: {source}", path.display()),
            Self::DefinitionJson { path, source } => write!(formatter, "invalid theme definition {}: {source}", path.display()),
            Self::UnsupportedSchema { expected, actual } => {
                write!(formatter, "unsupported theme schema {actual}; expected {expected}")
            }
            Self::EmptyName => formatter.write_str("theme name must not be empty"),
            Self::InvalidInsets { appearance, field, insets, image_size } => {
                write!(
                    formatter,
                    "invalid {field} for appearance `{appearance}`: left={}, top={}, right={}, bottom={}",
                    insets.left, insets.top, insets.right, insets.bottom
                )?;
                if let Some((width, height)) = image_size {
                    write!(formatter, " for image {width}x{height}")?;
                }
                Ok(())
            }
            Self::AtlasBuild { source } => write!(formatter, "failed to build theme resource atlas: {source}"),
            Self::GfxBuild { operation, status } => write!(formatter, "GFX {operation} failed while building the theme: {status}"),
        }
    }
}

impl Error for ThemeLoadError {
    /// Exposes concrete I/O, JSON, image, or texture causes without erasing theme classifications.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        // Schema and inset errors are fully represented by their own fields and have no lower cause.
        match self {
            Self::DefinitionRead { source, .. } => Some(source),
            Self::DefinitionJson { source, .. } => Some(source),
            Self::AtlasBuild { source } => Some(source),
            Self::UnsupportedSchema { .. } | Self::EmptyName | Self::InvalidInsets { .. } | Self::GfxBuild { .. } => None,
        }
    }
}

/// One fully constructed theme atlas and the icon slots assigned to unique image paths.
///
/// Keeping this intermediate concrete prevents installation from rediscovering numeric icon slots
/// or decoding files a second time. It is private because only a matching [`ThemeDefinition`] may
/// translate these construction slots into semantic appearances.
#[cfg(feature = "builder")]
struct ThemeAtlas {
    /// Immutable pixels and metadata ready for renderer installation.
    atlas: AssetData,
    /// Fully resolved image paths mapped to their packed icon slots.
    images: BTreeMap<PathBuf, AssetImagePlacement>,
}

/// GFX-owned atlas revision and the placement returned for each authored appearance image.
///
/// Skin retains only the public immutable bundle capability and small placement values. It never
/// copies pixels, resource metadata, or the concrete GFX publisher into its own implementation.
struct GfxThemeAtlas {
    /// Immutable resource revision constructed and validated by the selected GFX plugin.
    bundle: ComPtr<IAssetBundleAbi>,
    /// Fully resolved image paths mapped to bundle-local slots and decoded dimensions.
    images: BTreeMap<PathBuf, AssetImagePlacement>,
}

/// Fully resolved appearance policy awaiting either offline installation or GFX publication.
///
/// This private value is deliberately resource-free: JSON interpretation ends here, while GFX
/// remains responsible for pairing these records with pixels and publishing an immutable bundle.
struct CompiledTheme {
    /// Complete scalar geometry selected by the theme.
    metrics: AssetMetrics,
    /// Complete caption and backdrop policy selected by the theme.
    chrome: AssetChrome,
    /// Complete fixed role/state tables with all sparse overrides resolved.
    appearance: AppearanceTables,
}

impl CompiledTheme {
    /// Transfers complete appearance records to GFX for validation and immutable publication.
    fn publish(self, publisher: ComRef<'_, AssetPublisherAbi>, base: ComRef<'_, IAssetBundleAbi>) -> Result<ComPtr<IAssetBundleAbi>, ThemeLoadError> {
        let Self { metrics, chrome, appearance } = self;
        crate::style::publish_appearance(&appearance, publisher, base, metrics, chrome).map_err(|status| gfx_failure("appearance revision publication", status))
    }
}

/// Strict syntax tree and compiler input for exactly one self-contained JSON theme file.
///
/// The document deliberately has no parent link or overlay semantics. Optional fields mean
/// "retain the built-in fallback" rather than "search another document," so every authored value
/// has one source directory and one unambiguous effect.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeDefinition {
    /// Version selecting the exact schema contract.
    schema_version: u32,
    /// Human-readable selector label for the loaded theme.
    name: String,
    /// Optional complete semantic font recipe replacing the base atlas's semantic fonts.
    fonts: Option<FontCatalogDocument>,
    /// Optional complete semantic icon recipe replacing the base atlas's semantic icons.
    icons: Option<IconCatalogDocument>,
    /// Sparse scalar and flat-color overrides applied to the built-in skin.
    #[serde(default)]
    skin: ThemeSkinDocument,
    /// Sparse appearance overrides applied after flat fallback construction.
    #[serde(default)]
    appearances: AppearanceCatalogDocument,
}

/// Strict categorized appearance syntax for one source document.
///
/// Each JSON object key is deserialized directly into its family's concrete role enum. The loader
/// therefore never receives an unvalidated role string and cannot acquire knowledge of the widget
/// or manager implementation that eventually consumes the visual.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct AppearanceCatalogDocument {
    /// Structural surface appearances keyed by their category-local names.
    #[serde(deserialize_with = "crate::serde_com::deserialize_map")]
    surface: BTreeMap<SurfaceRole, SurfaceAppearanceDocument>,
    /// Interactive control appearances keyed by their category-local names.
    #[serde(deserialize_with = "crate::serde_com::deserialize_map")]
    control: BTreeMap<ControlRole, ControlAppearanceDocument>,
    /// Compact menu appearances keyed by their category-local names.
    #[serde(deserialize_with = "crate::serde_com::deserialize_map")]
    menu: BTreeMap<MenuRole, MenuAppearanceDocument>,
    /// Manager-owned window chrome appearances keyed by their category-local names.
    #[serde(deserialize_with = "crate::serde_com::deserialize_map")]
    chrome: BTreeMap<ChromeRole, ChromeAppearanceDocument>,
}

/// Defines the JSON spelling owned by this asset format for one IDL role enum.
macro_rules! document_role {
    ($name:ty) => {
        impl crate::serde_com::DocumentKey for $name {
            const NAMES: &'static [&'static str] = &<$name>::NAMES;

            /// Converts the decoder's checked name index without accepting raw integers.
            fn from_index(index: u32) -> Option<Self> {
                // The IDL generator has proved that `ALL` is contiguous and aligned with `NAMES`.
                usize::try_from(index).ok().and_then(|index| <$name>::ALL.get(index).copied())
            }
        }
    };
}

document_role!(SurfaceRole);
document_role!(ControlRole);
document_role!(MenuRole);
document_role!(ChromeRole);

impl AppearanceCatalogDocument {
    /// Resolves every authored image path relative to its declaring source document.
    fn resolve_paths(&mut self, directory: &Path) {
        // Each family keeps its concrete document shape; explicit loops avoid a typeless common
        // appearance object whose state fields would admit invalid combinations.
        for appearance in self.surface.values_mut() {
            appearance.resolve_paths(directory);
        }
        for appearance in self.control.values_mut() {
            appearance.resolve_paths(directory);
        }
        for appearance in self.menu.values_mut() {
            appearance.resolve_paths(directory);
        }
        for appearance in self.chrome.values_mut() {
            appearance.resolve_paths(directory);
        }
    }
}

impl ThemeDefinition {
    /// Reads, validates, and anchors every asset in one JSON theme definition.
    fn read(path: &Path) -> Result<Self, ThemeLoadError> {
        // The file path is retained verbatim in diagnostics. Its lexical parent is sufficient to
        // anchor assets because one document is now their only possible owner.
        let bytes = read_asset_file(path).map_err(|source| ThemeLoadError::DefinitionRead { path: path.to_path_buf(), source })?;
        Self::read_bytes(path, bytes.as_slice())
    }

    /// Parses one definition supplied by an embedding host and anchors its assets to `path`.
    fn read_bytes(path: &Path, bytes: &[u8]) -> Result<Self, ThemeLoadError> {
        let mut definition: Self = serde_json::from_slice(bytes).map_err(|source| ThemeLoadError::DefinitionJson { path: path.to_path_buf(), source })?;
        definition.validate_header()?;

        let directory = path.parent().unwrap_or_else(|| Path::new("."));
        definition.resolve_paths(directory);
        Ok(definition)
    }

    /// Validates the root fields that require semantic checks beyond JSON deserialization.
    fn validate_header(&self) -> Result<(), ThemeLoadError> {
        // Keep schema and name validation next to decoding while giving filesystem and unit-test
        // entry points one identical contract.
        if self.schema_version != THEME_SCHEMA_VERSION {
            return Err(ThemeLoadError::UnsupportedSchema {
                expected: THEME_SCHEMA_VERSION,
                actual: self.schema_version,
            });
        }
        if self.name.trim().is_empty() {
            return Err(ThemeLoadError::EmptyName);
        }
        Ok(())
    }

    /// Resolves every relative asset path against this document's directory exactly once.
    fn resolve_paths(&mut self, directory: &Path) {
        // Fonts, semantic icons, and appearance images share one source owner. No later compilation
        // stage needs to retain the definition filename or reinterpret a path.
        if let Some(fonts) = &mut self.fonts {
            fonts.resolve_paths(directory);
        }
        if let Some(icons) = &mut self.icons {
            icons.resolve_paths(directory);
        }
        self.appearances.resolve_paths(directory);
    }

    /// Strictly parses one standalone source string for schema-focused unit tests.
    #[cfg(all(test, feature = "builder"))]
    fn parse_for_test(json: &str) -> Result<Self, ThemeLoadError> {
        // A synthetic relative path keeps schema diagnostics and asset anchoring deterministic.
        let path = Path::new("theme.json");
        let mut definition: Self = serde_json::from_str(json).map_err(|source| ThemeLoadError::DefinitionJson { path: path.to_path_buf(), source })?;
        definition.validate_header()?;
        definition.resolve_paths(Path::new("."));
        Ok(definition)
    }

    /// Builds the atlas selected by this definition while preserving required base resources.
    ///
    /// Every unique image-patch source is packed once beside fonts and icons. A completely flat document
    /// without a font recipe can reuse the base allocation exactly; adding artwork without new
    /// fonts repacks existing glyph bitmaps so the original font files are not required.
    #[cfg(feature = "builder")]
    fn build_atlas(&self, base: &AssetData) -> Result<ThemeAtlas, ThemeLoadError> {
        self.build_atlas_with(base, &mut FileAssets)
    }

    /// Builds a theme atlas from assets supplied by the caller instead of requiring filesystem
    /// access. The same builder validation and resource-identity rules apply to both paths.
    #[cfg(feature = "builder")]
    fn build_atlas_with(&self, base: &AssetData, source: &mut impl AssetSource) -> Result<ThemeAtlas, ThemeLoadError> {
        let image_paths = self.image_paths();
        if self.fonts.is_none() && self.icons.is_none() && image_paths.is_empty() {
            // No resource files need decoding. Retain the exact resource order and pixels while
            // compiling a complete set of appearance values for this theme.
            return Ok(ThemeAtlas {
                atlas: base.clone(),
                images: BTreeMap::new(),
            });
        }

        let (width, height) = self
            .fonts
            .as_ref()
            .map(|fonts| (fonts.texture_width, fonts.texture_height))
            .unwrap_or_else(|| (base.tables.size.width as usize, base.tables.size.height as usize));
        // Replacement is expressed entirely in the atlas's two resource namespaces. The builder
        // copies application-owned resources and replaces only the declared semantic names.
        // UI-local identities are assigned by the consumer when this revision is installed.
        let semantic_font_names = FontRole::ALL.map(font_role_name);
        let semantic_icon_names = IconRole::ALL.map(icon_role_name);
        let excluded_fonts = self.fonts.as_ref().map(|_| semantic_font_names.as_slice()).unwrap_or(&[]);
        let excluded_icons = self.icons.as_ref().map(|_| semantic_icon_names.as_slice()).unwrap_or(&[]);
        let mut builder = Builder::from_data(base, width, height, excluded_fonts, excluded_icons).map_err(|source| ThemeLoadError::AtlasBuild { source })?;

        if let Some(fonts) = &self.fonts {
            for (role, font) in fonts.entries() {
                let bytes = source.read(font.path.as_path()).map_err(|source| ThemeLoadError::AtlasBuild { source })?;
                builder
                    .add_font_bytes_named(font_role_name(role), font.path.as_path(), font.size, bytes.as_slice())
                    .map_err(|source| ThemeLoadError::AtlasBuild { source })?;
            }
        }

        if let Some(icons) = &self.icons {
            for (role, icon) in icons.entries() {
                let bytes = source.read(icon.path.as_path()).map_err(|source| ThemeLoadError::AtlasBuild { source })?;
                builder
                    .add_icon_bytes_named(icon_role_name(role), icon.path.as_path(), bytes.as_slice())
                    .map_err(|source| ThemeLoadError::AtlasBuild { source })?;
            }
        }

        let mut images = BTreeMap::new();
        for (index, path) in image_paths.into_iter().enumerate() {
            // Builder owns bounded file I/O, PNG decoding, transactional packing, and typed image
            // diagnostics. The loader supplies only a deterministic private resource name.
            let bytes = source.read(path.as_path()).map_err(|source| ThemeLoadError::AtlasBuild { source })?;
            let icon = builder
                .add_icon_bytes_named(format!("@neocogi-theme-image/{index}").as_str(), path.as_path(), bytes.as_slice())
                .map_err(|source| ThemeLoadError::AtlasBuild { source })?;
            images.insert(path, icon);
        }

        let atlas = builder.build().map_err(|source| ThemeLoadError::AtlasBuild { source })?;
        Ok(ThemeAtlas { atlas, images })
    }

    /// Submits external resource bytes to one typed GFX construction session.
    ///
    /// Skin owns schema interpretation and final appearances, while GFX remains the sole owner of
    /// PNG decoding, font rasterization, packing, and the mutable atlas draft. The boundary is
    /// crossed once per authored resource, atlas finalization, and appearance publication, never
    /// during rendering.
    fn build_atlas_with_gfx(
        &self,
        base: ComRef<'_, IAssetBundleAbi>,
        loader: ComRef<'_, AssetLoaderAbi>,
        source: &mut impl AssetSource,
    ) -> Result<GfxThemeAtlas, ThemeLoadError> {
        let image_paths = self.image_paths();
        if self.fonts.is_none() && self.icons.is_none() && image_paths.is_empty() {
            return Ok(GfxThemeAtlas {
                bundle: base.add_ref(),
                images: BTreeMap::new(),
            });
        }

        let base_size = base.info().map_err(|status| gfx_failure("base bundle metadata", status))?.size;
        let (width, height) = self
            .fonts
            .as_ref()
            .map(|fonts| (fonts.texture_width, fonts.texture_height))
            .unwrap_or((base_size.width as usize, base_size.height as usize));
        let size = Size {
            width: i32::try_from(width).map_err(|source| ThemeLoadError::AtlasBuild { source: io::Error::other(source) })?,
            height: i32::try_from(height).map_err(|source| ThemeLoadError::AtlasBuild { source: io::Error::other(source) })?,
        };
        let builder = loader
            .derive_builder(
                size,
                base,
                AssetDeriveOptions {
                    replace_semantic_fonts: self.fonts.is_some(),
                    replace_semantic_icons: self.icons.is_some(),
                },
            )
            .map_err(|status| gfx_failure("derived-atlas creation", status))?;

        if let Some(fonts) = &self.fonts {
            for (role, font) in fonts.entries() {
                let name = font_role_name(role);
                let bytes = source.read(font.path.as_path()).map_err(|source| ThemeLoadError::AtlasBuild { source })?;
                let pixels = u32::try_from(font.size).map_err(|source| ThemeLoadError::AtlasBuild { source: io::Error::other(source) })?;
                builder
                    .add_font_bytes(
                        NcCopyFromBufferOffset::new(0, name.as_bytes()),
                        NcCopyFromBufferOffset::new(0, bytes.as_slice()),
                        pixels,
                    )
                    .map_err(|status| gfx_failure("font rasterization", status))?;
            }
        }

        if let Some(icons) = &self.icons {
            for (role, icon) in icons.entries() {
                let name = icon_role_name(role);
                let bytes = source.read(icon.path.as_path()).map_err(|source| ThemeLoadError::AtlasBuild { source })?;
                builder
                    .add_icon_bytes(
                        NcCopyFromBufferOffset::new(0, name.as_bytes()),
                        NcCopyFromBufferOffset::new(0, bytes.as_slice()),
                    )
                    .map_err(|status| gfx_failure("semantic icon decoding", status))?;
            }
        }

        let mut images = BTreeMap::new();
        for (index, path) in image_paths.into_iter().enumerate() {
            let name = format!("@neocogi-theme-image/{index}");
            let bytes = source.read(path.as_path()).map_err(|source| ThemeLoadError::AtlasBuild { source })?;
            let icon = builder
                .add_icon_bytes(
                    NcCopyFromBufferOffset::new(0, name.as_bytes()),
                    NcCopyFromBufferOffset::new(0, bytes.as_slice()),
                )
                .map_err(|status| gfx_failure("appearance image decoding", status))?;
            images.insert(path, icon);
        }

        let bundle = builder.finish().map_err(|status| gfx_failure("atlas finalization", status))?;
        Ok(GfxThemeAtlas { bundle, images })
    }

    /// Resolves flat fallbacks and binds image patches to placements assigned by GFX.
    fn compile(&self, images: &BTreeMap<PathBuf, AssetImagePlacement>) -> Result<CompiledTheme, ThemeLoadError> {
        let metrics = self.skin.metrics;
        let chrome = self.skin.window_chrome.compile();
        let frame_insets = self.skin.fallback_frame_insets;
        validate_non_negative("generic_frame", "skin.fallback_frame_insets", frame_insets)?;
        validate_non_negative("window_content", "skin.metrics.window_content_insets", metrics.window_content_insets)?;
        validate_non_negative("window_frame", "skin.metrics.window_border", metrics.window_border)?;
        let mut appearance = AppearanceTables::flat(frame_insets, &self.skin.palette);
        for (role, document) in &self.appearances.surface {
            // Shared role/state enums determine the wire-table order. Every authored override
            // replaces one concrete record without an intermediate runtime object.
            let name = format!("surface.{role:?}");
            let destination_insets = destination_insets(&name, document.insets, appearance.surfaces[role.raw() as usize].normal.insets)?;
            for (state, authored) in document.states() {
                let role_index = role.raw() as usize;
                let fallback = surface_visual(&appearance.surfaces[role_index], state);
                let visual = compile_visual(images, &name, destination_insets, authored, fallback)?;
                *surface_visual_mut(&mut appearance.surfaces[role_index], state) = visual;
            }
        }
        for (role, document) in &self.appearances.control {
            let name = format!("control.{role:?}");
            let destination_insets = destination_insets(&name, document.insets, appearance.controls[role.raw() as usize].enabled.normal.insets)?;
            for (state, authored) in document.states() {
                let role_index = role.raw() as usize;
                let fallback = control_visual(&appearance.controls[role_index], state);
                let visual = compile_visual(images, &name, destination_insets, authored, fallback)?;
                *control_visual_mut(&mut appearance.controls[role_index], state) = visual;
            }
        }
        for (role, document) in &self.appearances.menu {
            let name = format!("menu.{role:?}");
            let destination_insets = destination_insets(&name, document.insets, appearance.menus[role.raw() as usize].normal.insets)?;
            for (state, authored) in document.states() {
                let role_index = role.raw() as usize;
                let fallback = menu_visual(&appearance.menus[role_index], state);
                let visual = compile_visual(images, &name, destination_insets, authored, fallback)?;
                *menu_visual_mut(&mut appearance.menus[role_index], state) = visual;
            }
        }
        for (role, document) in &self.appearances.chrome {
            let name = format!("chrome.{role:?}");
            let destination_insets = destination_insets(&name, document.insets, appearance.window_chrome[role.raw() as usize].base.insets)?;
            for (state, authored) in document.states() {
                let role_index = role.raw() as usize;
                let fallback = chrome_visual(&appearance.window_chrome[role_index], state);
                let visual = compile_visual(images, &name, destination_insets, authored, fallback)?;
                *chrome_visual_mut(&mut appearance.window_chrome[role_index], state) = visual;
            }
        }

        Ok(CompiledTheme { metrics, chrome, appearance })
    }

    /// Installs compiled appearance records into concrete storage for offline export and tests.
    #[cfg(feature = "builder")]
    fn install(self, theme_atlas: ThemeAtlas) -> Result<LoadedTheme, ThemeLoadError> {
        let ThemeAtlas { atlas, images } = theme_atlas;
        let compiled = self.compile(&images)?;
        let mut data = atlas;
        data.tables.metrics = compiled.metrics;
        data.tables.chrome = compiled.chrome;
        data.tables.surfaces = Vec::from(compiled.appearance.surfaces).into();
        data.tables.controls = Vec::from(compiled.appearance.controls).into();
        data.tables.menus = Vec::from(compiled.appearance.menus).into();
        data.tables.window_chrome = Vec::from(compiled.appearance.window_chrome).into();

        // Validate the complete revision before publication or source export.
        data.validate().map_err(|status| ThemeLoadError::AtlasBuild {
            source: io::Error::other(status.to_string()),
        })?;
        Ok(LoadedTheme { name: self.name, data })
    }

    /// Returns every fully resolved image path in deterministic deduplicated order.
    fn image_paths(&self) -> BTreeSet<PathBuf> {
        // Image patch variants retain independent slicing/tint metadata, while one path set is
        // sufficient for immutable pixel packing. BTreeSet also stabilizes generated atlas names.
        let mut paths = BTreeSet::new();
        for appearance in self.appearances.surface.values() {
            for (_, visual) in appearance.states() {
                collect_image_path(visual, &mut paths);
            }
        }
        for appearance in self.appearances.control.values() {
            for (_, visual) in appearance.states() {
                collect_image_path(visual, &mut paths);
            }
        }
        for appearance in self.appearances.menu.values() {
            for (_, visual) in appearance.states() {
                collect_image_path(visual, &mut paths);
            }
        }
        for appearance in self.appearances.chrome.values() {
            for (_, visual) in appearance.states() {
                collect_image_path(visual, &mut paths);
            }
        }
        paths
    }
}

/// Reads and compiles one self-contained JSON theme against the stable application resource atlas.
///
/// This is the loader module's only entry point. Definition parsing, path anchoring, atlas
/// construction, and visual installation remain private stages that cannot be called out of order
/// or supplied with mismatched intermediate values.
#[cfg(feature = "builder")]
pub fn load(path: &Path, base: &AssetData) -> Result<LoadedTheme, ThemeLoadError> {
    // Compilation is CPU-local. The caller publishes the resulting revision and explicitly
    // installs it through the UI runtime, which coordinates renderer preparation.
    let definition = ThemeDefinition::read(path)?;
    let atlas = definition.build_atlas(base)?;
    definition.install(atlas)
}

/// Compiles an in-memory definition through an explicit source for referenced assets.
#[cfg(feature = "builder")]
pub fn load_bytes(path: &Path, bytes: &[u8], base: &AssetData, source: &mut impl AssetSource) -> Result<LoadedTheme, ThemeLoadError> {
    // Both entry points use the same parser, resource builder and final validation.
    let definition = ThemeDefinition::read_bytes(path, bytes)?;
    let atlas = definition.build_atlas_with(base, source)?;
    definition.install(atlas)
}

/// Compiles one external theme through an explicitly supplied GFX construction capability.
///
/// JSON and appearance policy remain local to Skin. Encoded resource bytes cross the ABI only
/// during this infrequent loading operation, and the resulting portable bundle contains no
/// parser, builder, filesystem, or provider-specific object.
pub fn load_with_gfx(
    path: &Path,
    base: ComRef<'_, IAssetBundleAbi>,
    loader: ComRef<'_, AssetLoaderAbi>,
    publisher: ComRef<'_, AssetPublisherAbi>,
) -> Result<ComPtr<IAssetBundleAbi>, ThemeLoadError> {
    let definition = ThemeDefinition::read(path)?;
    let atlas = definition.build_atlas_with_gfx(base, loader, &mut FileAssets)?;
    let compiled = definition.compile(&atlas.images)?;
    compiled.publish(publisher, atlas.bundle.as_com_ref())
}

/// Classifies a failed typed GFX operation without exposing its implementation details.
fn gfx_failure(operation: &'static str, status: NcStatus) -> ThemeLoadError {
    ThemeLoadError::GfxBuild { operation, status }
}

/// Returns the generated canonical resource name for one known semantic font role.
fn font_role_name(role: FontRole) -> &'static str {
    role.canonical_name().expect("FontRole::ALL contains only declared IDL values")
}

/// Returns the generated canonical resource name for one known semantic icon role.
fn icon_role_name(role: IconRole) -> &'static str {
    role.canonical_name().expect("IconRole::ALL contains only declared IDL values")
}

/// Selects one named surface branch without deriving a numeric state index.
fn surface_visual(values: &AssetSurfaceVisuals, state: SurfaceState) -> AssetVisual {
    match state {
        SurfaceState::Normal => values.normal,
        SurfaceState::Disabled => values.disabled,
    }
}

/// Selects one mutable named surface branch without deriving a numeric state index.
fn surface_visual_mut(values: &mut AssetSurfaceVisuals, state: SurfaceState) -> &mut AssetVisual {
    match state {
        SurfaceState::Normal => &mut values.normal,
        SurfaceState::Disabled => &mut values.disabled,
    }
}

/// Selects one named nested control branch without flattening [`ControlState`].
fn control_visual(values: &AssetControlVisuals, state: ControlState) -> AssetVisual {
    match state {
        ControlState::Disabled => values.disabled,
        ControlState::Enabled(PointerState::Normal) => values.enabled.normal,
        ControlState::Enabled(PointerState::Hovered) => values.enabled.hovered,
        ControlState::Enabled(PointerState::Pressed) => values.enabled.pressed,
        ControlState::Focused(PointerState::Normal) => values.focused.normal,
        ControlState::Focused(PointerState::Hovered) => values.focused.hovered,
        ControlState::Focused(PointerState::Pressed) => values.focused.pressed,
    }
}

/// Selects one mutable nested control branch without flattening [`ControlState`].
fn control_visual_mut(values: &mut AssetControlVisuals, state: ControlState) -> &mut AssetVisual {
    match state {
        ControlState::Disabled => &mut values.disabled,
        ControlState::Enabled(PointerState::Normal) => &mut values.enabled.normal,
        ControlState::Enabled(PointerState::Hovered) => &mut values.enabled.hovered,
        ControlState::Enabled(PointerState::Pressed) => &mut values.enabled.pressed,
        ControlState::Focused(PointerState::Normal) => &mut values.focused.normal,
        ControlState::Focused(PointerState::Hovered) => &mut values.focused.hovered,
        ControlState::Focused(PointerState::Pressed) => &mut values.focused.pressed,
    }
}

/// Selects one named menu branch without deriving a numeric state index.
fn menu_visual(values: &AssetMenuVisuals, state: MenuState) -> AssetVisual {
    match state {
        MenuState::Normal => values.normal,
        MenuState::Hovered => values.hovered,
        MenuState::Pressed => values.pressed,
        MenuState::Focused => values.focused,
        MenuState::Open => values.open,
        MenuState::Disabled => values.disabled,
    }
}

/// Selects one mutable named menu branch without deriving a numeric state index.
fn menu_visual_mut(values: &mut AssetMenuVisuals, state: MenuState) -> &mut AssetVisual {
    match state {
        MenuState::Normal => &mut values.normal,
        MenuState::Hovered => &mut values.hovered,
        MenuState::Pressed => &mut values.pressed,
        MenuState::Focused => &mut values.focused,
        MenuState::Open => &mut values.open,
        MenuState::Disabled => &mut values.disabled,
    }
}

/// Selects one named chrome branch without deriving a numeric state index.
fn chrome_visual(values: &AssetChromeVisuals, state: ChromeState) -> AssetVisual {
    match state {
        ChromeState::Base => values.base,
        ChromeState::Active => values.active,
        ChromeState::Disabled => values.disabled,
    }
}

/// Selects one mutable named chrome branch without deriving a numeric state index.
fn chrome_visual_mut(values: &mut AssetChromeVisuals, state: ChromeState) -> &mut AssetVisual {
    match state {
        ChromeState::Base => &mut values.base,
        ChromeState::Active => &mut values.active,
        ChromeState::Disabled => &mut values.disabled,
    }
}

/// Selects and validates the destination geometry shared by every state of one role.
fn destination_insets(name: &str, authored: Option<SliceInsets>, fallback: SliceInsets) -> Result<SliceInsets, ThemeLoadError> {
    // A role-wide value prevents interaction transitions from changing layout. Omitting it retains
    // the base state's concrete fallback geometry rather than copying another authored state.
    let insets = authored.unwrap_or(fallback);
    validate_non_negative(name, "insets", insets)?;
    Ok(insets)
}

/// Compiles one optional authored override over its exact family/state fallback visual.
fn compile_visual(
    images: &BTreeMap<PathBuf, AssetImagePlacement>,
    name: &str,
    destination_insets: SliceInsets,
    authored: Option<&VisualOverrideDocument>,
    mut visual: AssetVisual,
) -> Result<AssetVisual, ThemeLoadError> {
    // Even an omitted state receives the role-wide destination geometry while retaining its own
    // fallback cells. Authored fields then replace only the explicitly selected visual channels.
    visual.insets = destination_insets;
    if let Some(authored) = authored {
        if let Some(content_color) = authored.content_color {
            // The patch and semantic content color remain one complete runtime value at the mutation boundary.
            visual.content_color = rgba(content_color);
        }
        if let Some(patch) = &authored.patch {
            visual.patch = patch.compile(images, name, destination_insets)?.into();
        }
    }
    Ok(visual)
}

/// Adds the image path from one present image-backed visual to a deduplicated atlas work set.
fn collect_image_path(visual: Option<&VisualOverrideDocument>, paths: &mut BTreeSet<PathBuf>) {
    // Solid patches and content-color-only states have no resource dependency. Matching the closed
    // patch enum makes that distinction explicit without probing a group of loosely related fields.
    if let Some(VisualOverrideDocument {
        patch: Some(PatchDocument::Image { path, .. }),
        ..
    }) = visual
    {
        paths.insert(path.clone());
    }
}

/// Complete semantic font recipe for one rebuilt theme atlas.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FontCatalogDocument {
    /// Width in pixels of the rebuilt atlas texture.
    texture_width: usize,
    /// Height in pixels of the rebuilt atlas texture.
    texture_height: usize,
    /// Default font used by controls and ordinary text.
    body: FontDocument,
    /// Compact font used by supporting text.
    small: FontDocument,
    /// Font used by window titles and chrome.
    title: FontDocument,
    /// Larger font used by headings and typography demonstrations.
    heading: FontDocument,
    /// Fixed-width font used by console-oriented text.
    mono: FontDocument,
}

impl FontCatalogDocument {
    /// Returns all semantic recipes in stable atlas insertion order.
    fn entries(&self) -> [(FontRole, &FontDocument); 5] {
        // The explicit table keeps JSON keys, public FontRole values, and atlas names aligned
        // without reflection or string-driven role dispatch.
        [
            (FontRole::Body, &self.body),
            (FontRole::Small, &self.small),
            (FontRole::Title, &self.title),
            (FontRole::Heading, &self.heading),
            (FontRole::Mono, &self.mono),
        ]
    }

    /// Resolves every relative font filename against its declaring document directory.
    fn resolve_paths(&mut self, directory: &Path) {
        // Traverse through the same typed role list used by atlas insertion so no semantic font
        // retains a path whose base directory depends on a later compilation stage.
        self.body.resolve_path(directory);
        self.small.resolve_path(directory);
        self.title.resolve_path(directory);
        self.heading.resolve_path(directory);
        self.mono.resolve_path(directory);
    }
}

/// One font file and raster size declared by a theme.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FontDocument {
    /// Font file path resolved relative to the theme JSON directory.
    path: PathBuf,
    /// Pixel size rasterized into printable-ASCII atlas glyphs.
    size: usize,
}

/// Complete semantic icon recipe for one rebuilt theme atlas.
///
/// The object is deliberately all-or-nothing, just like [`FontCatalogDocument`]. A theme that
/// takes ownership of semantic iconography cannot accidentally leave one widget using pixels from
/// an unrelated base atlas, and every accepted key maps directly to one closed [`IconRole`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IconCatalogDocument {
    /// Window and dialog close glyph.
    close: IconDocument,
    /// Collapsed disclosure glyph.
    expand: IconDocument,
    /// Expanded disclosure glyph.
    collapse: IconDocument,
    /// Checked checkbox and checked menu-item glyph.
    check: IconDocument,
    /// Combo-box dropdown glyph.
    expand_down: IconDocument,
    /// Open folder glyph used by file dialogs.
    open_folder: IconDocument,
    /// Closed folder glyph used by file dialogs.
    closed_folder: IconDocument,
    /// Regular file glyph used by file dialogs.
    file: IconDocument,
}

impl IconCatalogDocument {
    /// Returns every semantic recipe in stable atlas insertion order.
    fn entries(&self) -> [(IconRole, &IconDocument); IconRole::COUNT] {
        // This explicit table is the single bridge from schema fields to runtime roles. Widgets
        // remain unaware of filenames, while the loader remains unaware of widget implementations.
        [
            (IconRole::Close, &self.close),
            (IconRole::Expand, &self.expand),
            (IconRole::Collapse, &self.collapse),
            (IconRole::Check, &self.check),
            (IconRole::ExpandDown, &self.expand_down),
            (IconRole::OpenFolder, &self.open_folder),
            (IconRole::ClosedFolder, &self.closed_folder),
            (IconRole::File, &self.file),
        ]
    }

    /// Resolves every relative icon filename against its declaring document directory.
    fn resolve_paths(&mut self, directory: &Path) {
        // Traverse through the same typed table used for insertion so path ownership and resource
        // identity cannot diverge between parsing and atlas construction.
        self.close.resolve_path(directory);
        self.expand.resolve_path(directory);
        self.collapse.resolve_path(directory);
        self.check.resolve_path(directory);
        self.expand_down.resolve_path(directory);
        self.open_folder.resolve_path(directory);
        self.closed_folder.resolve_path(directory);
        self.file.resolve_path(directory);
    }
}

/// One PNG file declared for a semantic icon role.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IconDocument {
    /// Icon path resolved relative to the theme JSON directory.
    path: PathBuf,
}

impl IconDocument {
    /// Makes this icon path independent of later compilation stages.
    fn resolve_path(&mut self, directory: &Path) {
        // Path::join also preserves an absolute source supplied by an embedding application.
        self.path = directory.join(self.path.as_path());
    }
}

impl FontDocument {
    /// Makes this font path independent of any later child document directory.
    fn resolve_path(&mut self, directory: &Path) {
        // Path::join preserves an already absolute path and anchors an ordinary relative path to
        // the source document that owns this concrete font recipe.
        self.path = directory.join(self.path.as_path());
    }
}

/// JSON title-alignment spelling translated directly to the portable record.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, neocogi_com::ComDeserialize)]
#[com_deserialize(rename_all = "snake_case")]
enum WindowTitleAlignmentDocument {
    /// Uses the title span from its leading edge.
    #[default]
    Leading,
    /// Centers text after reserving symmetric caption banks.
    Centered,
}

/// JSON caption-edge spelling translated directly to the portable record.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, neocogi_com::ComDeserialize)]
#[com_deserialize(rename_all = "snake_case")]
enum CaptionButtonSideDocument {
    /// Allocates the button from the leading edge.
    Leading,
    /// Allocates the button from the trailing edge.
    #[default]
    Trailing,
}

/// Asset-document fields controlling caption-button geometry and visibility.
#[derive(Copy, Clone, neocogi_com::ComDeserialize)]
#[com_deserialize(default)]
struct CaptionButtonsDocument {
    /// Edge used by the close button.
    close_side: CaptionButtonSideDocument,
    /// Edge used by the minimize button.
    minimize_side: CaptionButtonSideDocument,
    /// Edge used by the maximize or restore button.
    maximize_side: CaptionButtonSideDocument,
    /// Title pixels retained above and below each square caption button.
    vertical_spacing: i32,
    /// Minimum square caption extent after applying the inset.
    minimum_extent: i32,
    /// Unstriped pixels between a caption bank and the title field.
    inner_spacing: i32,
    /// Whether caption controls remain visible without window activation.
    show_without_activation: bool,
}

impl Default for CaptionButtonsDocument {
    /// Returns the conventional trailing-edge document defaults.
    fn default() -> Self {
        Self {
            close_side: CaptionButtonSideDocument::Trailing,
            minimize_side: CaptionButtonSideDocument::Trailing,
            maximize_side: CaptionButtonSideDocument::Trailing,
            vertical_spacing: 2,
            minimum_extent: 0,
            inner_spacing: 0,
            show_without_activation: true,
        }
    }
}

/// Optional flat title field in the asset document.
#[derive(Copy, Clone, neocogi_com::ComDeserialize)]
struct TitleBackdropDocument {
    /// Flat background color painted behind measured title text.
    color: Color,
    /// Horizontal pixels added on each side of the measured text.
    horizontal_padding: i32,
}

/// Asset-owned title and caption syntax compiled to [`AssetChrome`].
#[derive(Copy, Clone, neocogi_com::ComDeserialize)]
#[com_deserialize(default)]
struct WindowChromeDocument {
    /// Title alignment and caption-bank reservation policy.
    title_alignment: WindowTitleAlignmentDocument,
    /// Independent caption placement, extent, and visibility policy.
    captions: CaptionButtonsDocument,
    /// Application-content height retained while minimized.
    minimized_content_height: i32,
    /// Optional active-title field painted behind measured text.
    active_title_backdrop: Option<TitleBackdropDocument>,
}

impl WindowChromeDocument {
    /// Compiles this JSON document directly into the checked wire record.
    fn compile(self) -> AssetChrome {
        // No runtime skin model is constructed or retained by the skin plugin.
        let captions = self.captions;
        AssetChrome {
            title_alignment: match self.title_alignment {
                WindowTitleAlignmentDocument::Leading => TitleAlignment::Leading,
                WindowTitleAlignmentDocument::Centered => TitleAlignment::Centered,
            },
            close_side: compile_caption_side(captions.close_side),
            minimize_side: compile_caption_side(captions.minimize_side),
            maximize_side: compile_caption_side(captions.maximize_side),
            vertical_spacing: captions.vertical_spacing,
            minimum_extent: captions.minimum_extent,
            inner_spacing: captions.inner_spacing,
            show_without_activation: u8::from(captions.show_without_activation),
            minimized_content_height: self.minimized_content_height,
            has_backdrop: u8::from(self.active_title_backdrop.is_some()),
            backdrop_rgba: self.active_title_backdrop.map_or(0, |value| rgba(value.color)),
            backdrop_padding: self.active_title_backdrop.map_or(0, |value| value.horizontal_padding),
        }
    }
}

impl Default for WindowChromeDocument {
    /// Returns the conventional leading-title, trailing-caption recipe.
    fn default() -> Self {
        Self {
            title_alignment: WindowTitleAlignmentDocument::Leading,
            captions: CaptionButtonsDocument::default(),
            minimized_content_height: 2,
            active_title_backdrop: None,
        }
    }
}

/// Converts an asset-document caption edge into its portable tag.
fn compile_caption_side(side: CaptionButtonSideDocument) -> CaptionSide {
    match side {
        CaptionButtonSideDocument::Leading => CaptionSide::Leading,
        CaptionButtonSideDocument::Trailing => CaptionSide::Trailing,
    }
}

/// Concrete runtime skin inputs applied before sparse appearance overrides.
///
/// The wrapper exists only to group independent runtime values in JSON. It does not mirror their
/// fields, translate names, or provide another customization API.
#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ThemeSkinDocument {
    /// Layout values consumed directly by the runtime skin.
    #[serde(deserialize_with = "crate::serde_com::deserialize")]
    metrics: AssetMetrics,
    /// Shared destination geometry used while compiling flat fallback patches.
    #[serde(deserialize_with = "crate::serde_com::deserialize")]
    fallback_frame_insets: SliceInsets,
    /// Concrete semantic colors compiled into the complete fallback appearance catalog.
    #[serde(deserialize_with = "crate::serde_com::deserialize")]
    palette: FlatPalette,
    /// Exact manager-owned title and caption-button policy.
    #[serde(deserialize_with = "crate::serde_com::deserialize")]
    window_chrome: WindowChromeDocument,
}

impl Default for ThemeSkinDocument {
    /// Returns the default geometry and palette used by the asset compiler.
    fn default() -> Self {
        // Container-level Serde defaults copy from this complete value. Missing JSON fields cannot
        // therefore drift from programmatic defaults or require optional mirror fields.
        Self {
            metrics: AssetMetrics::default(),
            fallback_frame_insets: SliceInsets::uniform(1),
            palette: FlatPalette::default(),
            window_chrome: WindowChromeDocument::default(),
        }
    }
}

/// One structural surface role's destination insets and availability states.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SurfaceAppearanceDocument {
    /// Destination-space outer row and column sizes shared by both states.
    #[serde(default, deserialize_with = "crate::serde_com::deserialize")]
    insets: Option<SliceInsets>,
    /// Enabled structural appearance.
    normal: Option<VisualOverrideDocument>,
    /// Disabled structural appearance.
    disabled: Option<VisualOverrideDocument>,
}

impl SurfaceAppearanceDocument {
    /// Resolves every surface PNG path against the declaring document directory.
    fn resolve_paths(&mut self, directory: &Path) {
        // Only the two valid structural states are traversed.
        resolve_visual_paths([self.normal.as_mut(), self.disabled.as_mut()], directory);
    }

    /// Returns every surface state paired with its optional authored value.
    fn states(&self) -> [(SurfaceState, Option<&VisualOverrideDocument>); 2] {
        // Declaration order matches the runtime surface catalog.
        [(SurfaceState::Normal, self.normal.as_ref()), (SurfaceState::Disabled, self.disabled.as_ref())]
    }
}

/// Pointer-state fields shared by the enabled and focused branches of a control.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PointerAppearanceDocument {
    /// Pointer-neutral branch appearance.
    normal: Option<VisualOverrideDocument>,
    /// Pointer-hover branch appearance.
    hovered: Option<VisualOverrideDocument>,
    /// Visible pointer-press branch appearance.
    pressed: Option<VisualOverrideDocument>,
}

impl PointerAppearanceDocument {
    /// Resolves every pointer-state image path against the declaring document directory.
    fn resolve_paths(&mut self, directory: &Path) {
        // The branch contains exactly the pointer states representable inside ControlState.
        resolve_visual_paths([self.normal.as_mut(), self.hovered.as_mut(), self.pressed.as_mut()], directory);
    }
}

/// One interactive control role's destination insets and nested control states.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ControlAppearanceDocument {
    /// Destination-space outer row and column sizes shared by every control state.
    #[serde(default, deserialize_with = "crate::serde_com::deserialize")]
    insets: Option<SliceInsets>,
    /// Disabled appearance, with no contradictory pointer or focus substate.
    disabled: Option<VisualOverrideDocument>,
    /// Pointer appearances while the control is enabled without keyboard focus.
    enabled: PointerAppearanceDocument,
    /// Pointer appearances while the control owns visible keyboard focus.
    focused: PointerAppearanceDocument,
}

impl ControlAppearanceDocument {
    /// Resolves every control PNG path against the declaring document directory.
    fn resolve_paths(&mut self, directory: &Path) {
        // Disabled is terminal; enabled and focused branches resolve their concrete pointer states.
        resolve_visual_paths([self.disabled.as_mut()], directory);
        self.enabled.resolve_paths(directory);
        self.focused.resolve_paths(directory);
    }

    /// Returns every concrete nested control state and its optional authored value.
    fn states(&self) -> [(ControlState, Option<&VisualOverrideDocument>); 7] {
        // This order is the exact flattened storage order internal to the control family only.
        [
            (ControlState::Disabled, self.disabled.as_ref()),
            (ControlState::Enabled(PointerState::Normal), self.enabled.normal.as_ref()),
            (ControlState::Enabled(PointerState::Hovered), self.enabled.hovered.as_ref()),
            (ControlState::Enabled(PointerState::Pressed), self.enabled.pressed.as_ref()),
            (ControlState::Focused(PointerState::Normal), self.focused.normal.as_ref()),
            (ControlState::Focused(PointerState::Hovered), self.focused.hovered.as_ref()),
            (ControlState::Focused(PointerState::Pressed), self.focused.pressed.as_ref()),
        ]
    }
}

/// One menu role's destination insets and menu-specific selection states.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct MenuAppearanceDocument {
    /// Destination-space outer row and column sizes shared by every menu state.
    #[serde(default, deserialize_with = "crate::serde_com::deserialize")]
    insets: Option<SliceInsets>,
    /// Enabled menu appearance without transient selection.
    normal: Option<VisualOverrideDocument>,
    /// Pointer-hover selection appearance.
    hovered: Option<VisualOverrideDocument>,
    /// Visible pointer-press selection appearance.
    pressed: Option<VisualOverrideDocument>,
    /// Keyboard-navigation selection appearance.
    focused: Option<VisualOverrideDocument>,
    /// Appearance of an entry that owns an open popup.
    open: Option<VisualOverrideDocument>,
    /// Disabled menu appearance.
    disabled: Option<VisualOverrideDocument>,
}

impl MenuAppearanceDocument {
    /// Resolves every menu PNG path against the declaring document directory.
    fn resolve_paths(&mut self, directory: &Path) {
        // The fixed field list is the complete menu state vocabulary.
        resolve_visual_paths(
            [
                self.normal.as_mut(),
                self.hovered.as_mut(),
                self.pressed.as_mut(),
                self.focused.as_mut(),
                self.open.as_mut(),
                self.disabled.as_mut(),
            ],
            directory,
        );
    }

    /// Returns every menu state paired with its optional authored value.
    fn states(&self) -> [(MenuState, Option<&VisualOverrideDocument>); 6] {
        // Declaration order matches the runtime menu catalog.
        [
            (MenuState::Normal, self.normal.as_ref()),
            (MenuState::Hovered, self.hovered.as_ref()),
            (MenuState::Pressed, self.pressed.as_ref()),
            (MenuState::Focused, self.focused.as_ref()),
            (MenuState::Open, self.open.as_ref()),
            (MenuState::Disabled, self.disabled.as_ref()),
        ]
    }
}

/// One window chrome role's destination insets and activation states.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ChromeAppearanceDocument {
    /// Destination-space outer row and column sizes shared by every chrome state.
    #[serde(default, deserialize_with = "crate::serde_com::deserialize")]
    insets: Option<SliceInsets>,
    /// Enabled chrome appearance without window activation.
    base: Option<VisualOverrideDocument>,
    /// Enabled chrome appearance with window activation.
    active: Option<VisualOverrideDocument>,
    /// Disabled chrome appearance.
    disabled: Option<VisualOverrideDocument>,
}

impl ChromeAppearanceDocument {
    /// Resolves every chrome PNG path against the declaring document directory.
    fn resolve_paths(&mut self, directory: &Path) {
        // Chrome has no pointer or keyboard-focus states to normalize.
        resolve_visual_paths([self.base.as_mut(), self.active.as_mut(), self.disabled.as_mut()], directory);
    }

    /// Returns every chrome state paired with its optional authored value.
    fn states(&self) -> [(ChromeState, Option<&VisualOverrideDocument>); 3] {
        // Declaration order matches the runtime chrome catalog.
        [
            (ChromeState::Base, self.base.as_ref()),
            (ChromeState::Active, self.active.as_ref()),
            (ChromeState::Disabled, self.disabled.as_ref()),
        ]
    }
}

/// Sparse overrides for one exact family role and state.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct VisualOverrideDocument {
    /// Optional text and semantic-glyph color for this exact role and state.
    #[serde(default, deserialize_with = "crate::serde_com::deserialize")]
    content_color: Option<Color>,
    /// Optional concrete patch source replacing the exact state's flat fallback.
    patch: Option<PatchDocument>,
}

impl VisualOverrideDocument {
    /// Anchors this state's optional image path to the one containing theme document.
    fn resolve_path(&mut self, directory: &Path) {
        // Content colors and resource-free flat patches own no filesystem state. The closed patch
        // enum routes path handling only to the image variant that can legally contain a path.
        if let Some(patch) = &mut self.patch {
            patch.resolve_path(directory);
        }
    }
}

/// Concrete source for an authored visual patch.
///
/// Tagging the alternatives makes image-only fields structurally unavailable to flat patches.
/// A state that omits `patch` retains its exact family/state fallback instead of manufacturing an
/// empty image recipe.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum PatchDocument {
    /// A flat color painted across all nine destination cells.
    Solid {
        /// RGBA color covering the complete patch while role insets retain layout geometry.
        #[serde(deserialize_with = "crate::serde_com::deserialize")]
        color: Color,
    },
    /// A resource-free three-by-three patch with one border and one center color.
    Framed {
        /// RGBA color painted into every fixed edge and corner cell.
        #[serde(deserialize_with = "crate::serde_com::deserialize")]
        border: Color,
        /// RGBA color painted into the stretchable center cell.
        #[serde(deserialize_with = "crate::serde_com::deserialize")]
        center: Color,
    },
    /// One PNG divided into a typed three-by-three image patch.
    Image {
        /// PNG path resolved relative to the containing JSON document.
        path: PathBuf,
        /// Optional source-space slices; omission reuses the role's destination insets.
        #[serde(default, deserialize_with = "crate::serde_com::deserialize")]
        source_insets: Option<SliceInsets>,
        /// Optional RGBA image modulation; omission preserves the source pixels.
        #[serde(default, deserialize_with = "crate::serde_com::deserialize")]
        tint: Option<Color>,
    },
}

impl PatchDocument {
    /// Anchors an image patch path while leaving resource-free flat patches unchanged.
    fn resolve_path(&mut self, directory: &Path) {
        // Path ownership belongs to the concrete image variant, so no independent fields can fall
        // out of sync while the document advances from decoding to atlas construction.
        if let Self::Image { path, .. } = self {
            *path = directory.join(path.as_path());
        }
    }

    /// Compiles this closed source choice into one runtime patch owned by `atlas`.
    fn compile(
        &self,
        images: &BTreeMap<PathBuf, AssetImagePlacement>,
        appearance: &str,
        destination_insets: SliceInsets,
    ) -> Result<AssetPatch, ThemeLoadError> {
        // Exhaustive matching keeps solid construction independent of atlas resources and confines
        // source slicing, tinting, and capability lookup to the image alternative.
        match self {
            Self::Solid { color } => Ok(flat_patch(*color, *color)),
            Self::Framed { border, center } => Ok(flat_patch(*border, *center)),
            Self::Image { path, source_insets, tint } => {
                let source_insets = source_insets.unwrap_or(destination_insets);
                let placement = *images.get(path).expect("every declared theme image must have one baked atlas capability");
                validate_source_insets(appearance, source_insets, placement.size.width, placement.size.height)?;
                let tint = tint.unwrap_or(Color { r: 255, g: 255, b: 255, a: 255 });
                Ok(AssetPatch::Image {
                    icon: placement.index,
                    source: source_insets,
                    tint: rgba(tint),
                })
            }
        }
    }
}

/// Resolves a fixed family-specific set of optional state paths.
fn resolve_visual_paths<const N: usize>(states: [Option<&mut VisualOverrideDocument>; N], directory: &Path) {
    // Missing states remain absent; every present recipe is anchored to the containing document.
    for state in states.into_iter().flatten() {
        state.resolve_path(directory);
    }
}

/// Rejects negative destination or source inset components.
fn validate_non_negative(appearance: &str, field: &'static str, insets: SliceInsets) -> Result<(), ThemeLoadError> {
    // Destination insets may exceed a tiny runtime allocation because NinePatch collapses them, but
    // negative authored values are always a schema error instead of being silently normalized.
    if insets.left < 0 || insets.top < 0 || insets.right < 0 || insets.bottom < 0 {
        return Err(ThemeLoadError::InvalidInsets {
            appearance: appearance.to_owned(),
            field,
            insets,
            image_size: None,
        });
    }
    Ok(())
}

/// Rejects source insets that are negative or overlap inside their PNG rectangle.
fn validate_source_insets(appearance: &str, insets: SliceInsets, width: i32, height: i32) -> Result<(), ThemeLoadError> {
    // Unlike destinations, a PNG source has fixed pixels and cannot proportionally invent a valid
    // center when opposing source slices exceed its dimensions.
    let invalid = insets.left < 0
        || insets.top < 0
        || insets.right < 0
        || insets.bottom < 0
        || insets.left.saturating_add(insets.right) > width
        || insets.top.saturating_add(insets.bottom) > height;
    if invalid {
        return Err(ThemeLoadError::InvalidInsets {
            appearance: appearance.to_owned(),
            field: "source_insets",
            insets,
            image_size: Some((width, height)),
        });
    }
    Ok(())
}

/// Produces nine flat cells with transparent colors canonicalized to empty.
fn flat_patch(border: Color, center: Color) -> AssetPatch {
    // Flat cells use alpha zero for absence, so discard invisible RGB values at compilation.
    let border = if border.a == 0 { 0 } else { rgba(border) };
    let center = if center.a == 0 { 0 } else { rgba(center) };
    AssetPatch::Flat {
        top_left: border,
        top: border,
        top_right: border,
        left: border,
        center,
        right: border,
        bottom_left: border,
        bottom: border,
        bottom_right: border,
    }
}

#[cfg(all(test, feature = "builder"))]
mod tests;
