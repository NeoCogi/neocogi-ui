//! Named-asset limits exercise generated builder calls and actual bulk readers with owned storage.
use super::*;
use neocogi_com::{NcOutputWriter, ComInlineValue, com_object};
use neocogi_gfx_support::AssetTables;
use neocogi_ui::{NativePath, PathEncoding};
use neocogi_platform_contracts::*;
use std::{cell::Cell, rc::Rc};

/// Independent encoder creates a valid one-pixel icon without file or atlas dependencies.
fn tiny_png() -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = ::png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(::png::ColorType::Rgba);
        encoder.set_depth(::png::BitDepth::Eight);
        encoder.write_header().unwrap().write_image_data(&[255; 4]).unwrap();
    }
    bytes
}

/// One-pixel packing gaps and inherited glyphs fit alongside the complete named-asset limit.
fn derived(base: &ComPtr<IAssetBundleAbi>, options: AssetDeriveOptions) -> ComPtr<IAssetBuilderAbi> {
    let loader: ComPtr<AssetLoaderAbi> = AssetLoaderObject::from_value(AssetLoaderObject::default()).query().unwrap();
    loader.derive_builder(Size { width: 512, height: 256 }, base.as_com_ref(), options).unwrap()
}

/// All inherited named resources consume slots in the derived draft.
fn retain_all() -> AssetDeriveOptions {
    AssetDeriveOptions {
        replace_semantic_fonts: false,
        replace_semantic_icons: false,
    }
}

#[com_object(interfaces(INativePath))]
/// Counts foreign path activity independently of the builder's detached draft.
struct PathProbe {
    /// Shared test observation; admitted methods return errors instead of touching the filesystem.
    calls: Rc<Cell<u32>>,
}
impl PathProbe {
    /// Unexpected access remains a valid COM error, avoiding a panic across the ABI boundary.
    fn accessed<T>(&self) -> Result<T, NcStatus> {
        self.calls.set(self.calls.get() + 1);
        Err(neocogi_com::NC_E_INVALID_ARG)
    }
}
impl INativePath for PathProbe {
    /// Oversized names and full counts must reject before this first foreign method.
    fn encoding(&self) -> Result<PathEncoding, NcStatus> {
        self.accessed()
    }
    /// Every subsequent path method is counted too.
    fn unit_count(&self) -> Result<u64, NcStatus> {
        self.accessed()
    }
    /// No actual path bytes are needed for a rejected insertion.
    fn unix_bytes(&self, _: NcOutputWriter<'_, u8>) -> Result<(), NcStatus> {
        self.accessed()
    }
    /// The same admission policy applies to UTF-16 path providers.
    fn windows_units(&self, _: NcOutputWriter<'_, u16>) -> Result<(), NcStatus> {
        self.accessed()
    }
    /// Diagnostic display must also remain untouched before admission.
    fn display(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.accessed()
    }
}

/// Oversized names preserve the draft; legal long names can collectively exceed the old pool cap.
#[test]
fn bounded_names_reject_before_foreign_calls_and_publish_a_large_legal_pool() {
    let base = crate::test_support::test_bundle();
    let builder = derived(&base, retain_all());
    let png = tiny_png();
    let calls = Rc::new(Cell::new(0));
    let path: ComPtr<INativePathAbi> = PathProbe::from_value(PathProbe { calls: calls.clone() }).query().unwrap();
    for name in ["a".repeat(2049), "é".repeat(1025)] {
        let name = NcCopyFromBufferOffset::new(0, name.as_bytes());
        assert_eq!(builder.add_icon(name, path.as_com_ref()), Err(neocogi_com::NC_E_INVALID_ARG));
        assert_eq!(builder.add_font(name, path.as_com_ref(), 8), Err(neocogi_com::NC_E_INVALID_ARG));
        assert_eq!(
            builder.add_icon_bytes(name, NcCopyFromBufferOffset::new(0, &png)).unwrap_err(),
            neocogi_com::NC_E_INVALID_ARG
        );
        assert_eq!(
            builder.add_font_bytes(name, NcCopyFromBufferOffset::new(0, &[0]), 8),
            Err(neocogi_com::NC_E_INVALID_ARG)
        );
    }
    assert_eq!(calls.get(), 0);
    for length in [2047, 2048] {
        let name = "b".repeat(length);
        builder
            .add_icon_bytes(NcCopyFromBufferOffset::new(0, name.as_bytes()), NcCopyFromBufferOffset::new(0, &png))
            .unwrap();
    }
    let font_name = "é".repeat(1024);
    let recipe = crate::test_recipe::atlas_config();
    builder
        .add_font(
            NcCopyFromBufferOffset::new(0, font_name.as_bytes()),
            NativePath::new(&recipe.fonts[0].path).as_interface(),
            8,
        )
        .unwrap();
    for index in 0..2050 {
        let prefix = format!("private{index:08}");
        let name = prefix.clone() + &"x".repeat(2048 - prefix.len());
        builder
            .add_icon_bytes(NcCopyFromBufferOffset::new(0, name.as_bytes()), NcCopyFromBufferOffset::new(0, &png))
            .unwrap();
    }
    let result = builder.finish().unwrap();
    let data = AssetData::read(result.as_com_ref()).unwrap();
    assert!(data.tables.names.len() > 4 * 1024 * 1024, "the old independent pool cap must not survive");
    assert!(data.tables.fonts.iter().any(|font| data.name(font.name) == Ok(font_name.as_str())));
    assert!(
        data.tables
            .fonts
            .iter()
            .map(|font| font.name)
            .chain(data.tables.icons.iter().map(|icon| icon.name))
            .all(|name| name.count <= 2048)
    );
    assert_eq!(builder.finish().unwrap_err(), GFX_E_CLOSED);
}

/// Adds inherited private icons to an otherwise valid small fixture; overlapping source tiles are legal.
fn catalog_with_count(count: u32) -> AssetData {
    let mut data = crate::test_support::data(&crate::test_support::test_bundle());
    for icon in data.tables.icons.to_mut() {
        icon.rect = Rect { x: 0, y: 0, width: 1, height: 1 };
    }
    let existing = data.tables.fonts.len() + data.tables.icons.len();
    for index in existing..count as usize {
        let name = format!("inherited{index}");
        let range = AssetRange {
            first: data.tables.names.len() as u32,
            count: name.len() as u32,
        };
        data.tables.names.to_mut().extend_from_slice(name.as_bytes());
        data.tables.icons.to_mut().push(AssetIcon {
            name: range,
            rect: Rect { x: 0, y: 0, width: 1, height: 1 },
        });
    }
    data.validate().unwrap();
    data
}

/// Count rejection preserves the last admitted slot and the complete draft remains publishable.
#[test]
fn inherited_counts_share_one_limit_and_rejection_preserves_terminal_publication() {
    let base = catalog_with_count(MAX_ASSETS - 1).into_bundle().unwrap();
    let builder = derived(&base, retain_all());
    let png = tiny_png();
    builder
        .add_icon_bytes(NcCopyFromBufferOffset::new(0, b"last"), NcCopyFromBufferOffset::new(0, &png))
        .unwrap();
    let calls = Rc::new(Cell::new(0));
    let path: ComPtr<INativePathAbi> = PathProbe::from_value(PathProbe { calls: calls.clone() }).query().unwrap();
    assert_eq!(
        builder.add_icon(NcCopyFromBufferOffset::new(0, b"crossing-icon"), path.as_com_ref()),
        Err(neocogi_com::NC_E_INVALID_ARG)
    );
    assert_eq!(
        builder.add_font(NcCopyFromBufferOffset::new(0, b"crossing-font"), path.as_com_ref(), 8),
        Err(neocogi_com::NC_E_INVALID_ARG)
    );
    assert_eq!(calls.get(), 0);
    assert_eq!(
        builder
            .add_icon_bytes(NcCopyFromBufferOffset::new(0, b"crossing-bytes"), NcCopyFromBufferOffset::new(0, &png))
            .unwrap_err(),
        neocogi_com::NC_E_INVALID_ARG
    );
    assert_eq!(builder.add_artwork_bytes(NcCopyFromBufferOffset::new(0, &png)).unwrap_err(), neocogi_com::NC_E_INVALID_ARG);
    let result = builder.finish().unwrap();
    let info = result.info().unwrap();
    assert_eq!(info.fonts + info.icons, MAX_ASSETS);
    AssetData::read(result.as_com_ref()).unwrap();

    // Omitting semantic fonts/icons frees exactly their retained slots, while private assets remain.
    let retained = AssetData::read(base.as_com_ref()).unwrap();
    let removed_fonts = retained
        .tables
        .fonts
        .iter()
        .filter(|font| FontRole::ALL.iter().any(|role| role.canonical_name() == retained.name(font.name).ok()))
        .count();
    let removed_icons = retained
        .tables
        .icons
        .iter()
        .filter(|icon| IconRole::ALL.iter().any(|role| role.canonical_name() == retained.name(icon.name).ok()))
        .count();
    let replacements = derived(
        &base,
        AssetDeriveOptions {
            replace_semantic_fonts: true,
            replace_semantic_icons: true,
        },
    );
    assert!(removed_fonts > 0 && removed_icons > 0);
    let recipe = crate::test_recipe::atlas_config();
    for font in retained.tables.fonts.iter() {
        let name = retained.name(font.name).unwrap();
        if FontRole::ALL.iter().any(|role| role.canonical_name() == Some(name)) {
            replacements
                .add_font(
                    NcCopyFromBufferOffset::new(0, name.as_bytes()),
                    NativePath::new(&recipe.fonts[0].path).as_interface(),
                    8,
                )
                .unwrap();
        }
    }
    for icon in retained.tables.icons.iter() {
        let name = retained.name(icon.name).unwrap();
        if !IconRole::ALL.iter().any(|role| role.canonical_name() == Some(name)) {
            continue;
        }
        replacements
            .add_icon_bytes(NcCopyFromBufferOffset::new(0, name.as_bytes()), NcCopyFromBufferOffset::new(0, &png))
            .unwrap();
    }
    replacements
        .add_icon_bytes(NcCopyFromBufferOffset::new(0, b"replacement-extra"), NcCopyFromBufferOffset::new(0, &png))
        .unwrap();
    assert_eq!(
        replacements
            .add_icon_bytes(NcCopyFromBufferOffset::new(0, b"one-too-many"), NcCopyFromBufferOffset::new(0, &png))
            .unwrap_err(),
        neocogi_com::NC_E_INVALID_ARG
    );
    let replaced = replacements.finish().unwrap();
    let info = replaced.info().unwrap();
    assert_eq!(info.fonts + info.icons, MAX_ASSETS);
    AssetData::read(replaced.as_com_ref()).unwrap();
}

/// Font and icon cases rebuild the shared name partition while preserving every other valid field.
#[derive(Clone, Copy)]
enum NamedResource {
    /// Private font uses the existing valid underscore glyph and font metrics.
    Font,
    /// Private icon uses an existing valid one-pixel source rectangle.
    Icon,
}

/// Adds a private named record to valid owned tables; raw publication below deliberately bypasses validation.
fn append_resource(data: &mut AssetData, resource: NamedResource, value: &str) {
    match resource {
        NamedResource::Icon => {
            let range = AssetRange {
                first: data.tables.names.len() as u32,
                count: value.len() as u32,
            };
            data.tables.names.to_mut().extend_from_slice(value.as_bytes());
            data.tables.icons.to_mut().push(AssetIcon {
                name: range,
                rect: Rect { x: 0, y: 0, width: 1, height: 1 },
            });
        }
        NamedResource::Font => {
            let prefix = data.tables.icons[0].name.first as usize;
            let mut names = data.tables.names[..prefix].to_vec();
            let range = AssetRange {
                first: prefix as u32,
                count: value.len() as u32,
            };
            names.extend_from_slice(value.as_bytes());
            names.extend_from_slice(&data.tables.names[prefix..]);
            data.tables.names = names.into();
            for icon in data.tables.icons.to_mut() {
                icon.name.first += value.len() as u32;
            }
            let mut font = data.tables.fonts[0];
            font.name = range;
            font.glyphs = AssetRange {
                first: data.tables.glyphs.len() as u32,
                count: 1,
            };
            let fallback = *data.tables.glyphs.iter().find(|glyph| glyph.scalar == u32::from('_')).unwrap();
            data.tables.glyphs.to_mut().push(fallback);
            data.tables.fonts.to_mut().push(font);
        }
    }
}

#[com_object(interfaces(IAssetBundle))]
/// Real provider owns valid Rust storage but may advertise a deliberately malformed header or name.
struct RawBundle {
    /// Valid parent supplies independent image, catalog and appearance capabilities.
    base: ComPtr<IAssetBundleAbi>,
    /// Header is separately retained to test admission before any foreign table-copy calls.
    header: AssetInfo,
    /// Owned bulk tables contain no forged pointers or invalid Rust values.
    data: AssetData,
    /// Every call after the header is observable without borrowing provider-owned storage.
    reads: Rc<Cell<u32>>,
}
impl RawBundle {
    /// Counts each data/capability request; malformed headers must leave this at zero.
    fn accessed(&self) {
        self.reads.set(self.reads.get() + 1);
    }
    /// Publishes the deliberate raw provider without applying the normal validating constructor.
    fn source(base: ComPtr<IAssetBundleAbi>, data: AssetData, header: AssetInfo) -> (ComPtr<IAssetBundleAbi>, Rc<Cell<u32>>) {
        let reads = Rc::new(Cell::new(0));
        let source = Self::from_value(Self { base, data, header, reads: reads.clone() }).query().unwrap();
        (source, reads)
    }
}
/// Uses checked generated output writers with the real owned allocation extent.
fn copy<T: ComInlineValue>(mut output: NcOutputWriter<'_, T>, values: &[T]) -> Result<(), NcStatus> {
    let first = usize::try_from(output.offset()).map_err(|_| neocogi_com::NC_E_INVALID_ARG)?;
    let end = first.checked_add(output.capacity()).ok_or(neocogi_com::NC_E_INVALID_ARG)?;
    output.extend_from_slice(values.get(first..end).ok_or(neocogi_com::NC_E_INVALID_ARG)?)
}
impl IAssetBundle for RawBundle {
    /// Returns the untrusted advertised bounds without accessing any payload.
    fn info(&self) -> Result<AssetInfo, NcStatus> {
        Ok(self.header)
    }
    /// Delegates to real validated image ownership.
    fn image(&self) -> Result<ComPtr<neocogi_renderer_contracts::IRgbaImageAbi>, NcStatus> {
        self.accessed();
        self.base.image()
    }
    /// Delegates to the valid parent's catalog identity.
    fn catalog(&self) -> Result<ComPtr<IAssetCatalogAbi>, NcStatus> {
        self.accessed();
        self.base.catalog()
    }
    /// Copies font ranges through the generated checked writer.
    fn fonts(&self, output: NcOutputWriter<'_, AssetFont>) -> Result<(), NcStatus> {
        self.accessed();
        copy(output, &self.data.tables.fonts)
    }
    /// Copies initialized glyph values without pointer tricks.
    fn glyphs(&self, output: NcOutputWriter<'_, AssetGlyph>) -> Result<(), NcStatus> {
        self.accessed();
        copy(output, &self.data.tables.glyphs)
    }
    /// Copies icon ranges through the generated checked writer.
    fn icons(&self, output: NcOutputWriter<'_, AssetIcon>) -> Result<(), NcStatus> {
        self.accessed();
        copy(output, &self.data.tables.icons)
    }
    /// Copies exact UTF-8 storage, including deliberately oversized names.
    fn names(&self, output: NcOutputWriter<'_, u8>) -> Result<(), NcStatus> {
        self.accessed();
        copy(output, &self.data.tables.names)
    }
    /// Delegates validated geometry unrelated to the malformed case.
    fn metrics(&self) -> Result<AssetMetrics, NcStatus> {
        self.accessed();
        self.base.metrics()
    }
    /// Delegates the parent's valid caption policy.
    fn chrome(&self) -> Result<AssetChrome, NcStatus> {
        self.accessed();
        self.base.chrome()
    }
    /// Retains the same valid surface appearances.
    fn surface_visuals(&self, output: NcOutputWriter<'_, AssetSurfaceVisuals>) -> Result<(), NcStatus> {
        self.accessed();
        copy(output, &self.data.tables.surfaces)
    }
    /// Retains the same valid control appearances.
    fn control_visuals(&self, output: NcOutputWriter<'_, AssetControlVisuals>) -> Result<(), NcStatus> {
        self.accessed();
        copy(output, &self.data.tables.controls)
    }
    /// Retains the same valid menu appearances.
    fn menu_visuals(&self, output: NcOutputWriter<'_, AssetMenuVisuals>) -> Result<(), NcStatus> {
        self.accessed();
        copy(output, &self.data.tables.menus)
    }
    /// Retains the same valid chrome appearances.
    fn chrome_visuals(&self, output: NcOutputWriter<'_, AssetChromeVisuals>) -> Result<(), NcStatus> {
        self.accessed();
        copy(output, &self.data.tables.window_chrome)
    }
}

/// Both complete-data and metadata-only readers must admit the header before fetching storage.
#[test]
fn malformed_bulk_headers_are_rejected_before_any_payload_calls() {
    let base = crate::test_support::test_bundle();
    let data = crate::test_support::data(&base);
    let header = base.info().unwrap();
    for header in [
        AssetInfo { fonts: u32::MAX, ..header },
        AssetInfo { icons: MAX_ASSETS, ..header },
        AssetInfo {
            names: (header.fonts + header.icons) * MAX_ASSET_NAME_BYTES + 1,
            ..header
        },
    ] {
        let (source, reads) = RawBundle::source(base.clone(), data.clone(), header);
        assert_eq!(AssetTables::read(source.as_com_ref()).err(), Some(neocogi_com::NC_E_INVALID_ARG));
        assert_eq!(AssetData::read(source.as_com_ref()).err(), Some(neocogi_com::NC_E_INVALID_ARG));
        assert_eq!(reads.get(), 0);
    }
}

/// An otherwise valid provider cannot bypass the per-name bound with a small aggregate pool.
#[test]
fn bulk_readers_validate_individual_font_and_icon_name_ranges() {
    for resource in [NamedResource::Font, NamedResource::Icon] {
        for length in [2048, 2049] {
            let base = crate::test_support::test_bundle();
            let mut data = crate::test_support::data(&base);
            append_resource(&mut data, resource, &"x".repeat(length));
            let header = data.info().unwrap();
            let (source, _) = RawBundle::source(base, data, header);
            match length {
                2048 => {
                    AssetTables::read(source.as_com_ref()).unwrap();
                    AssetData::read(source.as_com_ref()).unwrap();
                }
                2049 => {
                    assert_eq!(AssetTables::read(source.as_com_ref()).err(), Some(neocogi_com::NC_E_INVALID_ARG));
                    assert_eq!(AssetData::read(source.as_com_ref()).err(), Some(neocogi_com::NC_E_INVALID_ARG));
                }
                _ => unreachable!(),
            }
        }
    }
}
