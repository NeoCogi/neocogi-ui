//! The runtime COM builder and generated direct exporter must publish equivalent portable data.
use super::*;
use neocogi_com::{ComPtr, INcStringAbi, INcStringComPtrExt, NC_E_INVALID_ARG, NcStatus, com_object};
use neocogi_ui::{NativePath, PathEncoding};
use neocogi_ui_support::assets::ImportedAssets;

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
        assert_eq!(
            self.builder.add_artwork_bytes(NcCopyFromBufferOffset::new(0, b"not PNG")).unwrap_err(),
            GFX_E_BUSY
        );
        self.calls.set(self.calls.get() + 1);
        self.path.as_interface().encoding()
    }
    /// Forwards exact native units after the reentry probe succeeded.
    fn unit_count(&self) -> Result<u64, NcStatus> {
        self.path.as_interface().unit_count()
    }
    /// Forwards bytes without a lossy conversion through display text.
    fn unix_bytes(&self, output: neocogi_com::NcOutputWriter<'_, u8>) -> Result<(), NcStatus> {
        self.path.as_interface().unix_bytes(output.forward()?)
    }
    /// Keeps the same test valid for native Windows UTF-16 paths.
    fn windows_units(&self, output: neocogi_com::NcOutputWriter<'_, u16>) -> Result<(), NcStatus> {
        self.path.as_interface().windows_units(output.forward()?)
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
        assert_eq!(a.patch, b.patch);
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
    let changed = ImportedAssets::read(source.as_com_ref(), Some(&imported.catalog())).unwrap();
    assert_eq!(imported.original().font_id("body"), changed.bundle().atlas().font_id("body"));
    assert!(changed.bundle().atlas().icon_id("theme-private").is_some());
    assert!(changed.original().icon_id("theme-private").is_none());
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

/// Rejected PNG bytes cannot claim a name, consume packing space or poison the
/// COM draft. A later successful insertion must match an untouched control.
#[test]
fn incomplete_png_rejection_preserves_the_builder_transaction() {
    let base = test_bundle();
    let loader: ComPtr<AssetLoaderAbi> = AssetLoaderObject::from_value(AssetLoaderObject::default()).query().unwrap();
    let derive = || {
        loader
            .derive_builder(
                Size { width: 512, height: 256 },
                base.as_com_ref(),
                AssetDeriveOptions {
                    replace_semantic_fonts: false,
                    replace_semantic_icons: false,
                },
            )
            .unwrap()
    };
    let draft = derive();
    let control = derive();
    let png = include_bytes!("../../../../assets/WHITE.png");
    let name = NcCopyFromBufferOffset::new(0, b"complete-png");
    for end in 0..png.len() {
        assert_eq!(
            draft.add_icon_bytes(name, NcCopyFromBufferOffset::new(0, &png[..end])).unwrap_err(),
            NC_E_INVALID_ARG,
            "prefix {end}"
        );
    }
    let mut corrupt = png.to_vec();
    *corrupt.last_mut().unwrap() ^= 1;
    let mut trailing = png.to_vec();
    trailing.extend_from_slice(b"trailing");
    let mut concatenated = png.to_vec();
    concatenated.extend_from_slice(png);
    for invalid in [corrupt, trailing, concatenated] {
        assert_eq!(
            draft.add_icon_bytes(name, NcCopyFromBufferOffset::new(0, &invalid)).unwrap_err(),
            NC_E_INVALID_ARG
        );
    }
    let actual = draft.add_icon_bytes(name, NcCopyFromBufferOffset::new(0, png)).unwrap();
    let expected = control.add_icon_bytes(name, NcCopyFromBufferOffset::new(0, png)).unwrap();
    assert_eq!(
        (actual.index, actual.size.width, actual.size.height),
        (expected.index, expected.size.width, expected.size.height)
    );
    let actual = data(&draft.finish().unwrap());
    let expected = data(&control.finish().unwrap());
    assert_eq!(actual.pixels, expected.pixels);
    assert_eq!(actual.tables.names, expected.tables.names);
    assert_eq!(actual.tables.icons, expected.tables.icons);
}

/// Artwork allocation sees inherited and newly added names, skips canonical
/// occupied slots, and claims a name only when decoding/packing succeeds.
#[test]
fn artwork_names_are_allocated_inside_the_owned_builder_namespace() {
    let base = test_bundle();
    let loader: ComPtr<AssetLoaderAbi> = AssetLoaderObject::from_value(AssetLoaderObject::default()).query().unwrap();
    let builder = loader
        .derive_builder(
            Size { width: 512, height: 256 },
            base.as_com_ref(),
            AssetDeriveOptions {
                replace_semantic_fonts: false,
                replace_semantic_icons: false,
            },
        )
        .unwrap();
    let png = include_bytes!("../../../../assets/WHITE.png");
    let occupied = ["0", "1", "3", "9999", "01", "+2", "unrelated", "99999999999999999999999999999999999"];
    for suffix in occupied {
        let name = format!("@neocogi-theme-image/{suffix}");
        builder
            .add_icon_bytes(NcCopyFromBufferOffset::new(0, name.as_bytes()), NcCopyFromBufferOffset::new(0, png))
            .unwrap();
    }
    assert_eq!(
        builder.add_artwork_bytes(NcCopyFromBufferOffset::new(0, b"invalid PNG")).unwrap_err(),
        NC_E_INVALID_ARG
    );
    let first = builder.add_artwork_bytes(NcCopyFromBufferOffset::new(0, png)).unwrap();
    assert_eq!(
        builder.add_artwork_bytes(NcCopyFromBufferOffset::new(0, &png[..png.len() - 4])).unwrap_err(),
        NC_E_INVALID_ARG
    );
    let second = builder.add_artwork_bytes(NcCopyFromBufferOffset::new(0, png)).unwrap();
    let published = data(&builder.finish().unwrap());
    assert_eq!(
        published.name(published.tables.icons[first.index as usize].name).unwrap(),
        "@neocogi-theme-image/2"
    );
    assert_eq!(
        published.name(published.tables.icons[second.index as usize].name).unwrap(),
        "@neocogi-theme-image/4"
    );
    for suffix in occupied {
        let name = format!("@neocogi-theme-image/{suffix}");
        assert!(published.tables.icons.iter().any(|icon| published.name(icon.name).unwrap() == name));
    }
    assert_eq!(builder.add_artwork_bytes(NcCopyFromBufferOffset::new(0, png)).unwrap_err(), GFX_E_CLOSED);
}
