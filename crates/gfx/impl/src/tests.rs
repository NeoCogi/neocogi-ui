//! The runtime COM builder and generated direct exporter must publish equivalent portable data.
use super::*;
use crate::AssetPublisherObject;
use neocogi_com::{ComPtr, INcStringAbi, INcStringComPtrExt, NcStatus, com_object};
use neocogi_ui::{NativePath, PathEncoding};
use neocogi_ui_runtime::assets::ImportedAssets;
use neocogi_platform_contracts::*;

#[com_object(interfaces(INativePath))]
/// A caller-owned path deliberately tries to consume its builder during path decoding.
struct ReenterPath {
    /// Exact native path delegated after probing the mutation gate.
    path: NativePath,
    /// Same live draft whose outer operation owns the detached native state.
    builder: ComPtr<IAssetBuilderAbi>,
    /// Test observation independent of the COM allocation's lifetime.
    calls: std::rc::Rc<std::cell::Cell<u32>>,
}
impl INativePath for ReenterPath {
    /// A nested finish must report BUSY without consuming or exposing the outer draft.
    fn encoding(&self) -> Result<PathEncoding, NcStatus> {
        assert_eq!(self.builder.finish().unwrap_err(), GFX_E_BUSY);
        self.calls.set(self.calls.get() + 1);
        self.path.as_interface().encoding()
    }
    /// Forwards exact native units after the reentry probe succeeded.
    fn unit_count(&self) -> Result<u64, NcStatus> {
        self.path.as_interface().unit_count()
    }
    /// Forwards bytes without a lossy conversion through display text.
    fn unix_bytes(&self, output: neocogi_com::NcCopyToBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.path.as_interface().unix_bytes(output)
    }
    /// Keeps the same test valid for native Windows UTF-16 paths.
    fn windows_units(&self, output: neocogi_com::NcCopyToBufferOffset<'_, u16>) -> Result<(), NcStatus> {
        self.path.as_interface().windows_units(output)
    }
    /// Display remains an independent human-readable snapshot.
    fn display(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.path.as_interface().display()
    }
}

/// Copies complete portable values out of an imported revision for deterministic comparisons.
fn data(source: &ComPtr<IAssetBundleAbi>) -> neocogi_gfx_support::AssetData {
    AssetData::read(source.as_com_ref()).unwrap()
}

/// Iterates every named appearance field for producer-output comparison only.
fn visuals(tables: &neocogi_gfx_support::AssetTables) -> impl Iterator<Item = &AssetVisual> {
    let surfaces = tables.surfaces.iter().flat_map(|values| [&values.normal, &values.disabled]);
    let controls = tables.controls.iter().flat_map(|values| {
        [
            &values.disabled,
            &values.enabled.normal,
            &values.enabled.hovered,
            &values.enabled.pressed,
            &values.focused.normal,
            &values.focused.hovered,
            &values.focused.pressed,
        ]
    });
    let menus = tables.menus.iter().flat_map(|values| {
        [
            &values.normal,
            &values.hovered,
            &values.pressed,
            &values.focused,
            &values.open,
            &values.disabled,
        ]
    });
    let chrome = tables.window_chrome.iter().flat_map(|values| [&values.base, &values.active, &values.disabled]);
    // Flattening is an assertion view; production storage and transport remain structured.
    surfaces.chain(controls).chain(menus).chain(chrome)
}

/// Both producer modes share the exact recipe and immutable object representation.
#[test]
fn com_builder_matches_direct_pixels_metrics_and_resolved_visuals() {
    let loader: ComPtr<AssetLoaderAbi> = AssetLoaderObject::from_value(AssetLoaderObject::default()).query().unwrap();
    let config = crate::test_recipe::atlas_config();
    let white = NativePath::new(&config.white_icon);
    let builder = loader
        .create_builder(
            Size {
                width: config.texture_width as i32,
                height: config.texture_height as i32,
            },
            white.as_interface(),
        )
        .unwrap();
    for icon in config.icons {
        builder
            .add_icon(NcCopyFromBufferOffset::new(0, icon.name.as_bytes()), NativePath::new(&icon.path).as_interface())
            .unwrap();
    }
    for font in config.fonts {
        builder
            .add_font(
                NcCopyFromBufferOffset::new(0, font.name.as_bytes()),
                NativePath::new(&font.path).as_interface(),
                font.size as u32,
            )
            .unwrap();
    }
    let runtime = builder.finish().unwrap();
    assert_eq!(builder.finish().unwrap_err(), GFX_E_CLOSED);
    let runtime = data(&runtime);
    let direct = data(&test_bundle());
    assert_eq!(runtime.tables.size, direct.tables.size);
    assert_eq!(runtime.pixels, direct.pixels);
    assert_eq!(runtime.tables.fonts, direct.tables.fonts);
    assert_eq!(runtime.tables.glyphs, direct.tables.glyphs);
    assert_eq!(runtime.tables.icons, direct.tables.icons);
    assert_eq!(runtime.tables.names, direct.tables.names);
    assert_eq!(runtime.tables.metrics, direct.tables.metrics);
    assert_eq!(runtime.tables.chrome, direct.tables.chrome);
    let runtime_visuals = visuals(&runtime.tables).collect::<Vec<_>>();
    let direct_visuals = visuals(&direct.tables).collect::<Vec<_>>();
    assert_eq!(runtime_visuals.len(), direct_visuals.len());
    for (a, b) in runtime_visuals.into_iter().zip(direct_visuals) {
        assert_eq!(a.insets, b.insets);
        assert_eq!(a.center_visible, b.center_visible);
        assert_eq!(a.content_color, b.content_color);
        assert_eq!(AssetPatch::try_from(a.patch).unwrap(), AssetPatch::try_from(b.patch).unwrap());
    }
}

/// Recoverable edits restore the draft; final failure remains closed and diagnostically inspectable.
#[test]
fn failed_edit_can_retry_and_failed_finish_is_terminal() {
    let loader: ComPtr<AssetLoaderAbi> = AssetLoaderObject::from_value(AssetLoaderObject::default()).query().unwrap();
    let config = crate::test_recipe::atlas_config();
    let white = NativePath::new(&config.white_icon);
    assert_eq!(
        loader.create_builder(Size { width: i32::MAX, height: 5 }, white.as_interface()).unwrap_err(),
        NC_E_INVALID_ARG
    );
    let builder = loader.create_builder(Size { width: 96, height: 96 }, white.as_interface()).unwrap();
    assert_eq!(
        builder.add_icon(NcCopyFromBufferOffset::new(0, &[0xff]), white.as_interface()),
        Err(NC_E_INVALID_ARG)
    );
    // Buffer forms accept only one complete, nonempty input beginning at offset zero. Rejecting
    // either range leaves the same transactional draft available for the following valid edit.
    assert_eq!(
        builder.add_icon_bytes(NcCopyFromBufferOffset::new(0, b"empty-image"), NcCopyFromBufferOffset::new(0, &[]),),
        Err(NC_E_INVALID_ARG)
    );
    assert_eq!(
        builder.add_font_bytes(NcCopyFromBufferOffset::new(0, b"offset-font"), NcCopyFromBufferOffset::new(1, &[0]), 12,),
        Err(NC_E_INVALID_ARG)
    );
    builder.add_icon(NcCopyFromBufferOffset::new(0, b"extra"), white.as_interface()).unwrap();
    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    let reenter: ComPtr<INativePathAbi> = ReenterPath::from_value(ReenterPath {
        path: NativePath::new(&config.white_icon),
        builder: builder.clone(),
        calls: calls.clone(),
    })
    .query()
    .unwrap();
    builder
        .add_icon(NcCopyFromBufferOffset::new(0, b"reentrant-path"), reenter.as_com_ref())
        .unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(
        builder.add_icon(NcCopyFromBufferOffset::new(0, b"extra"), white.as_interface()),
        Err(NC_E_INVALID_ARG)
    );
    let diagnostic: ComPtr<IGfxDiagnosticsAbi> = builder.query().unwrap();
    let snapshot = diagnostic.diagnostic().unwrap();
    assert!(snapshot.to_string().unwrap().contains("extra"));
    // This draft intentionally omits the required body and semantic icons.
    assert_eq!(builder.finish().unwrap_err(), NC_E_INVALID_ARG);
    assert_eq!(builder.finish().unwrap_err(), GFX_E_CLOSED);
    assert!(snapshot.to_string().unwrap().contains("extra"));
}

/// Repacking into a larger atlas retains exact application resource identities at the receiving UI.
#[test]
fn derived_builder_preserves_catalog_and_can_add_private_resources() {
    let base = test_bundle();
    let imported = ImportedAssets::read(base.as_com_ref(), None).unwrap();
    let loader: ComPtr<AssetLoaderAbi> = AssetLoaderObject::from_value(AssetLoaderObject::default()).query().unwrap();
    let builder = loader
        .derive_builder(
            Size { width: 1024, height: 512 },
            base.as_com_ref(),
            AssetDeriveOptions {
                replace_semantic_fonts: false,
                replace_semantic_icons: false,
            },
        )
        .unwrap();
    let config = crate::test_recipe::atlas_config();
    builder
        .add_icon(
            NcCopyFromBufferOffset::new(0, b"theme-private"),
            NativePath::new(&config.white_icon).as_interface(),
        )
        .unwrap();
    let source = builder.finish().unwrap();
    let changed = ImportedAssets::read(source.as_com_ref(), Some(&imported)).unwrap();
    assert_eq!(imported.original().font_id("body"), changed.bundle().atlas().font_id("body"));
    assert!(changed.bundle().atlas().icon_id("theme-private").is_some());
    assert!(changed.original().icon_id("theme-private").is_none());
}

/// Appearance publication preserves resources and rejects malformed bulk records inside GFX.
#[test]
fn appearance_revision_is_a_validated_resource_preserving_transaction() {
    let base = test_bundle();
    let original = data(&base);
    let publisher: ComPtr<AssetPublisherAbi> = AssetPublisherObject::from_value(AssetPublisherObject::default()).query().unwrap();
    let mut metrics = original.tables.metrics;
    metrics.padding += 3;
    let revision = AssetAppearanceRevision { metrics, chrome: original.tables.chrome };
    let candidate = publisher
        .revise_appearance(
            base.as_com_ref(),
            revision,
            NcCopyFromBufferOffset::new(0, &original.tables.surfaces),
            NcCopyFromBufferOffset::new(0, &original.tables.controls),
            NcCopyFromBufferOffset::new(0, &original.tables.menus),
            NcCopyFromBufferOffset::new(0, &original.tables.window_chrome),
        )
        .unwrap();
    let changed = data(&candidate);
    assert_eq!(candidate.catalog().unwrap().as_ptr(), base.catalog().unwrap().as_ptr());
    assert_eq!(changed.pixels, original.pixels);
    assert_eq!(changed.tables.fonts, original.tables.fonts);
    assert_eq!(changed.tables.icons, original.tables.icons);
    assert_eq!(changed.tables.metrics, metrics);

    // A role table is all-or-nothing. The service validates range shape before copying any
    // caller-owned record or publishing a candidate with a partially replaced appearance family.
    assert_eq!(
        publisher
            .revise_appearance(
                base.as_com_ref(),
                revision,
                NcCopyFromBufferOffset::new(1, &original.tables.surfaces),
                NcCopyFromBufferOffset::new(0, &original.tables.controls),
                NcCopyFromBufferOffset::new(0, &original.tables.menus),
                NcCopyFromBufferOffset::new(0, &original.tables.window_chrome),
            )
            .unwrap_err(),
        NC_E_INVALID_ARG
    );
}

/// Constructs one standalone catalog without importing application deployment machinery.
fn test_bundle() -> ComPtr<IAssetBundleAbi> {
    // Each invocation owns a distinct catalog so revision authentication tests remain meaningful.
    Builder::from_config(&crate::test_recipe::atlas_config())
        .unwrap()
        .build()
        .unwrap()
        .into_bundle()
        .unwrap()
}
