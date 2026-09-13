//! Boundary tests use real generated COM calls, malformed producer tables and an upload-failing renderer.
use super::*;
use neocogi_gfx_contracts::AssetPublisherAbi;
use neocogi_ui::assets::AssetData;
use neocogi_skin_contracts::{PaletteEdit, PaletteEditAbi, PaletteEditTag, StyleCompilerAbi};
use neocogi_com::{com_object, NcCopyFromBufferOffset, NcCopyToBufferOffset, NC_E_BUSY};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Small deterministic source contains a named application font beyond the required semantic set.
fn bundle() -> SkinBundle {
    SkinBundle::from_atlas(crate::test_support::test_atlas_with_font_sizes(&[("body", 10), ("application", 15)]))
}

/// Compares complete portable values while deliberately ignoring new binary-local resource IDs.
fn equivalent(a: &AssetData, b: &AssetData) {
    assert_eq!(a.tables.size, b.tables.size);
    assert_eq!(a.pixels, b.pixels);
    assert_eq!(a.tables.fonts, b.tables.fonts);
    assert_eq!(a.tables.glyphs, b.tables.glyphs);
    assert_eq!(a.tables.icons, b.tables.icons);
    assert_eq!(a.tables.names, b.tables.names);
    assert_eq!(a.tables.metrics, b.tables.metrics);
    assert_eq!(a.tables.chrome, b.tables.chrome);
    let a_visuals = visuals(&a.tables).collect::<Vec<_>>();
    let b_visuals = visuals(&b.tables).collect::<Vec<_>>();
    assert_eq!(a_visuals.len(), b_visuals.len());
    for (a, b) in a_visuals.into_iter().zip(b_visuals) {
        assert_eq!(a.insets, b.insets);
        assert_eq!(a.center_visible, b.center_visible);
        assert_eq!(a.content_color, b.content_color);
        assert_eq!(AssetPatch::try_from(a.patch).unwrap(), AssetPatch::try_from(b.patch).unwrap());
    }
}

/// Iterates every named appearance field without reconstructing a flattened storage format.
fn visuals(tables: &neocogi_ui::assets::AssetTables) -> impl Iterator<Item = &AssetVisual> {
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
    // Chaining is local test observation; the owned representation remains structured throughout.
    surfaces.chain(controls).chain(menus).chain(chrome)
}

/// Bulk transport preserves every pixel, glyph metric, semantic visual and scalar recipe.
#[test]
fn round_trip_preserves_complete_resources_and_image_patches() {
    let atlas = bundle().atlas().clone();
    let mut skin = Skin::from_atlas(&atlas);
    skin.window_chrome = crate::WindowChromeSkin::classic_mac(crate::Color { r: 12, g: 34, b: 56, a: 255 });
    skin.set_control(
        crate::ControlRole::Button,
        crate::ControlState::Disabled,
        crate::Visual::new(
            crate::NinePatch::image(
                crate::SliceInsets::uniform(1),
                crate::NinePatchImage::new(atlas.white_icon(), crate::SliceInsets::ZERO, crate::Color { r: 30, g: 60, b: 90, a: 255 }),
            ),
            crate::Color { r: 90, g: 60, b: 30, a: 255 },
        ),
    );
    let original = SkinBundle::new(atlas, skin);
    let source = export_bundle(&original).unwrap();
    let imported = ImportedAssets::read(source.as_com_ref(), None).unwrap();
    equivalent(&export_bundle_data(&original).unwrap(), &export_bundle_data(imported.bundle()).unwrap());
    assert_ne!(original.atlas().font_id("body"), imported.bundle().atlas().font_id("body"));

    // Whole-range copy validation must preserve output even when only its final byte is invalid.
    let mut output = [77; 2];
    let count = source.info().unwrap().rgba_bytes().unwrap();
    assert_eq!(
        source
            .query::<IRgbaImageAbi>()
            .unwrap()
            .pixels(NcCopyToBufferOffset::new(count as u64 - 1, &mut output)),
        Err(NC_E_INVALID_ARG)
    );
    assert_eq!(output, [77; 2]);
}

/// Catalog authentication precedes ID preservation, and missing original names cannot enter a runtime.
#[test]
fn derived_identity_is_preserved_but_unrelated_and_incomplete_catalogs_are_rejected() {
    let source = export_bundle(&bundle()).unwrap();
    let first = ImportedAssets::read(source.as_com_ref(), None).unwrap();
    let mut skin = first.bundle().skin().clone();
    skin.metrics.padding += 7;
    let changed = SkinBundle::new(first.bundle().atlas().clone(), skin);
    let revision = first.export_derived(&changed).unwrap();
    let second = ImportedAssets::read(revision.as_com_ref(), Some(&first)).unwrap();
    assert_eq!(first.bundle().atlas().font_id("application"), second.bundle().atlas().font_id("application"));
    assert_eq!(second.bundle().skin().metrics.padding, changed.skin().metrics.padding);
    assert_ne!(first.bundle().skin().revision(), second.bundle().skin().revision());

    let unrelated = export_bundle(&bundle()).unwrap();
    assert!(matches!(ImportedAssets::read(unrelated.as_com_ref(), Some(&first)), Err(UI_E_OWNER)));
    let incomplete = SkinBundle::from_atlas(crate::test_support::test_atlas());
    let incomplete = export_bundle_data(&incomplete).unwrap().into_revision(source.as_com_ref()).unwrap();
    assert!(matches!(ImportedAssets::read(incomplete.as_com_ref(), Some(&first)), Err(NC_E_INVALID_ARG)));
}

/// Malformed Unicode, partitions, pixels, dimensions and tagged geometry fail before native construction.
#[test]
fn malformed_portable_metadata_never_reaches_a_live_skin() {
    for case in 0..11 {
        let mut data = export_bundle_data(&bundle()).unwrap();
        match case {
            0 => data.tables.glyphs.to_mut()[0].scalar = 0xd800,       // Invalid Unicode surrogate.
            1 => data.tables.fonts.to_mut()[0].glyphs.first = 1,       // Gap before the first font.
            2 => data.tables.icons.to_mut()[0].name.first = u32::MAX,  // Overflowing name span.
            3 => data.tables.names.to_mut()[0] = 0xff,                 // Invalid UTF-8.
            4 => data.tables.glyphs.to_mut()[0].rect.width = i32::MAX, // Out-of-atlas glyph.
            5 => data.pixels.to_mut().fill(0),                         // Required white tile is no longer opaque white.
            6 => data.tables.metrics.padding = -1,
            7 => data.tables.chrome.has_backdrop = 2,
            8 => data.tables.controls.to_mut()[0].disabled.patch = AssetPatchAbi::from_unknown_tag(AssetPatchTag::from_raw(255)).unwrap(),
            9 => {
                data.tables.controls.to_mut()[0].disabled.patch = AssetPatch::Image {
                    icon: u32::MAX,
                    source: Insets::default(),
                    tint: 0,
                }
                .into()
            }
            10 => data.tables.controls.to_mut()[0].disabled.center_visible = 2,
            _ => unreachable!(),
        }
        let source = data.into_bundle().unwrap();
        assert!(matches!(ImportedAssets::read(source.as_com_ref(), None), Err(NC_E_INVALID_ARG)), "case {case}");
    }
    let mut data = export_bundle_data(&bundle()).unwrap();
    data.tables.size.width = i32::MAX;
    assert_eq!(data.into_bundle().unwrap_err(), NC_E_INVALID_ARG);
}

#[com_object(interfaces(ITextMetrics))]
/// Built-ins use their own resource revision; this foreign measurement service must remain unused.
struct UnusedMetrics;
impl ITextMetrics for UnusedMetrics {
    /// Any unexpected call exposes a leak from local widget measurement into the caller's service.
    fn button_size(&self, _: NcCopyFromBufferOffset<'_, u8>) -> Result<Size, NcStatus> {
        panic!("built-in borrowed foreign metrics")
    }
}

/// Observable renderer state contains only owned capabilities and ordinary single-threaded cells.
struct Probe {
    /// Reentry must fail while the runtime owns its installation transaction.
    runtime: ComPtr<UiRuntimeAbi>,
    /// Setter reentry must also fail during the upload boundary.
    button: ComPtr<ButtonAbi>,
    /// Last successfully uploaded renderer-owned image projection.
    current: RefCell<ComPtr<IRgbaImageAbi>>,
    /// Forces a recoverable upload failure without changing the current bundle.
    fail: Cell<bool>,
    /// Proves malformed candidates fail before reaching GPU preparation.
    uploads: Cell<u32>,
}
#[com_object(interfaces(IRenderer))]
/// Concrete test backend models atomic native atlas upload and observes final frame submission.
struct UploadRenderer(Rc<Probe>);
impl IRenderer for UploadRenderer {
    /// Confirms mutation/reentry gates, then commits only a successful fake GPU upload.
    fn prepare_atlas(&self, source: ComRef<'_, IRgbaImageAbi>) -> Result<(), NcStatus> {
        self.0.uploads.set(self.0.uploads.get() + 1);
        // The independently allocated host still exposes the old, fully installed UI revision.
        let installed = self.0.button.host()?.assets()?.query::<IRgbaImageAbi>()?;
        assert_eq!(installed.as_ptr(), self.0.current.borrow().as_ptr());
        // The mutable runtime call excludes every overlapping call on that same COM allocation.
        assert_eq!(self.0.runtime.update(SIZE), Err(NC_E_BUSY));
        let mut button = self.0.button.clone();
        assert_eq!(button.set_label(NcCopyFromBufferOffset::new(0, b"reentrant")), Err(UI_E_BUSY));
        let recursive: ComPtr<IRendererAbi> = Self::from_value(Self(self.0.clone())).query()?;
        let mut runtime = self.0.runtime.clone();
        let installed = self.0.button.host()?.assets()?;
        assert_eq!(runtime.install_assets(installed.as_com_ref(), recursive.as_com_ref()), Err(NC_E_BUSY));
        if self.0.fail.get() {
            return Err(UI_E_RENDER);
        }
        let source = source.add_ref();
        let old = self.0.current.replace(source);
        drop(old);
        Ok(())
    }
    /// Final geometry must use the exact bundle prepared by the successful upload.
    fn render(&self, info: RenderInfo, source: ComRef<'_, IRgbaImageAbi>, stream: ComRef<'_, IRenderStreamAbi>) -> Result<(), NcStatus> {
        assert_eq!(source.as_ptr(), self.0.current.borrow().as_ptr());
        let mut vertices = vec![RenderVertex::default(); info.vertex_count as usize];
        assert_eq!(
            stream.vertices(info.invocation, NcCopyToBufferOffset::new(0, &mut vertices))?,
            info.vertex_count
        );
        assert!(!vertices.is_empty());
        Ok(())
    }
}
/// Drawable dimensions used by the complete runtime integration fixture.
const SIZE: Size = Size { width: 640, height: 400 };

/// A real activation owns the same capabilities used by an application's theme/style handler.
struct ThemeState {
    /// A retained facade proves a callback cannot claim exclusive runtime access during dispatch.
    ui: neocogi_ui::Ui,
    /// Validated candidate belongs to the current resource catalog.
    candidate: ComPtr<IAssetBundleAbi>,
    /// Test GPU upload also probes recursive update, installation and widget mutation.
    renderer: ComPtr<IRendererAbi>,
    /// Counts successful handler commits; failures must leave this unchanged.
    installed: u32,
}
/// Concrete handler deliberately attempts a replacement during an active shared runtime call.
struct InstallTheme;
impl neocogi_ui::EventHandler<ThemeState, neocogi_ui::events::Activation> for InstallTheme {
    /// The generated COM borrow gate rejects this call before the renderer observes a candidate.
    fn handle(&mut self, state: &mut ThemeState, _: &neocogi_ui::events::Activation) -> Result<(), NcStatus> {
        state.ui.install_assets(state.candidate.as_com_ref(), state.renderer.as_com_ref())?;
        state.installed += 1;
        Ok(())
    }
}

/// Resource replacement requires an exclusive call after application dispatch has returned.
#[test]
fn event_callback_cannot_install_until_shared_runtime_dispatch_returns() {
    let source = export_bundle(&bundle()).unwrap();
    let runtime = crate::create_runtime_from_assets(source.as_com_ref()).unwrap();
    let ui = neocogi_ui::Ui::from_interface(runtime.clone()).unwrap();
    let button = ui.widgets().button("switch theme").unwrap();
    let probe = Rc::new(Probe {
        runtime,
        button: button.as_interface().add_ref(),
        current: RefCell::new(source.query::<IRgbaImageAbi>().unwrap()),
        fail: Cell::new(true),
        uploads: Cell::new(0),
    });
    let renderer: ComPtr<IRendererAbi> = UploadRenderer::from_value(UploadRenderer(probe.clone())).query().unwrap();
    let _window = ui.window("theme callback", Rect { x: 0, y: 0, width: 400, height: 250 }, &button).unwrap();
    let mut data = export_bundle_data(&bundle()).unwrap();
    data.tables.metrics.padding += 10;
    let candidate = data.into_revision(source.as_com_ref()).unwrap();
    let session = neocogi_ui::EventSession::new(ThemeState {
        ui: ui.clone(),
        candidate: candidate.clone(),
        renderer: renderer.clone(),
        installed: 0,
    });
    session.subscribe_activation(&button, InstallTheme).unwrap();
    for key in [KeyCode::Tab, KeyCode::Space] {
        ui.input(
            UiInput::Key(KeyStroke {
                key,
                value: 0,
                pressed: true,
                repeat: false,
                modifiers: 0,
            }),
            "",
        )
        .unwrap();
    }
    assert_eq!(ui.update_with_events(SIZE, &session), Err(NC_E_BUSY));
    assert_eq!(ui.assets().unwrap().as_ptr(), source.as_ptr());
    assert_eq!(session.state().unwrap().installed, 0);
    assert_eq!(probe.uploads.get(), 0);

    // Once dispatch has returned, the state owns ordinary exclusive access and upload failures
    // retain both the installed UI value and the renderer's current atlas.
    {
        let mut state = session.state_mut().unwrap();
        let candidate = state.candidate.clone();
        let renderer = state.renderer.clone();
        assert_eq!(state.ui.install_assets(candidate.as_com_ref(), renderer.as_com_ref()), Err(UI_E_RENDER));
    }
    assert_eq!(ui.assets().unwrap().as_ptr(), source.as_ptr());
    assert_eq!(probe.uploads.get(), 1);
    probe.fail.set(false);
    {
        let mut state = session.state_mut().unwrap();
        let candidate = state.candidate.clone();
        let renderer = state.renderer.clone();
        state.ui.install_assets(candidate.as_com_ref(), renderer.as_com_ref()).unwrap();
        state.installed += 1;
    }
    assert_eq!(probe.uploads.get(), 2);
    assert_eq!(session.state().unwrap().installed, 1);
    assert_eq!(ui.assets().unwrap().as_ptr(), candidate.as_ptr());
    // Installation occurred outside update, so one explicit synchronization commits the revision.
    ui.update_with_events(SIZE, &session).unwrap();
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
}

/// Failed validation/upload preserves UI and GPU state; success changes built-in measurement and requires update.
#[test]
fn runtime_install_is_atomic_and_updates_shared_widget_measurement() {
    let source = export_bundle(&bundle()).unwrap();
    let runtime = crate::create_runtime_from_assets(source.as_com_ref()).unwrap();
    let mut ui = neocogi_ui::Ui::from_interface(runtime.clone()).unwrap();
    let button = ui.widgets().button("resources").unwrap();
    let button_interface = button.as_interface().add_ref();
    let probe = Rc::new(Probe {
        runtime: runtime.clone(),
        button: button_interface.clone(),
        current: RefCell::new(source.query::<IRgbaImageAbi>().unwrap()),
        fail: Cell::new(false),
        uploads: Cell::new(0),
    });
    let renderer: ComPtr<IRendererAbi> = UploadRenderer::from_value(UploadRenderer(probe.clone())).query().unwrap();
    let _window = ui.window("assets", Rect { x: 0, y: 0, width: 400, height: 250 }, &button).unwrap();
    let mut leaf: ComPtr<ILeafLayoutAbi> = button_interface.query().unwrap();
    let metrics: ComPtr<ITextMetricsAbi> = UnusedMetrics::from_value(UnusedMetrics).query().unwrap();
    let limits = LayoutLimits {
        width: AxisLimit { kind: LimitKind::Unbounded, extent: 0 },
        height: AxisLimit { kind: LimitKind::Unbounded, extent: 0 },
    };
    let before = leaf.measure(limits, metrics.as_com_ref()).unwrap();
    ui.update(SIZE).unwrap();
    ui.render(SIZE, renderer.as_com_ref()).unwrap();

    let mut data = export_bundle_data(&bundle()).unwrap();
    data.tables.metrics.padding += 10;
    let candidate = data.into_revision(source.as_com_ref()).unwrap();
    probe.fail.set(true);
    assert_eq!(ui.install_assets(candidate.as_com_ref(), renderer.as_com_ref()), Err(UI_E_RENDER));
    assert_eq!(ui.assets().unwrap().as_ptr(), source.as_ptr());
    assert_eq!(leaf.measure(limits, metrics.as_com_ref()).unwrap(), before);
    ui.render(SIZE, renderer.as_com_ref()).unwrap();

    let unrelated = export_bundle(&bundle()).unwrap();
    assert_eq!(ui.install_assets(unrelated.as_com_ref(), renderer.as_com_ref()), Err(UI_E_OWNER));
    assert_eq!(probe.uploads.get(), 1);
    probe.fail.set(false);
    ui.install_assets(candidate.as_com_ref(), renderer.as_com_ref()).unwrap();
    assert_eq!(probe.uploads.get(), 2);
    assert_eq!(button_interface.host().unwrap().assets().unwrap().as_ptr(), candidate.as_ptr());
    assert_eq!(ui.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    let after = leaf.measure(limits, metrics.as_com_ref()).unwrap();
    assert_eq!(after.width, before.width + 20);
    // Buttons use half-padding above and below text; horizontal padding is the full value.
    assert_eq!(after.height, before.height + 10);
    ui.update(SIZE).unwrap();
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
}

/// Live editor recipes reuse the installed compiler while keeping derivation separate from publication.
#[test]
fn asset_style_derivation_shares_flat_rules_and_preserves_the_installed_revision() {
    let source = export_bundle(&bundle()).unwrap();
    let publisher: ComPtr<AssetPublisherAbi> = neocogi_gfx_impl::AssetPublisherObject::from_value(neocogi_gfx_impl::AssetPublisherObject::default())
        .query()
        .unwrap();
    let compiler: ComPtr<StyleCompilerAbi> =
        neocogi_skin_impl::StyleCompilerObject::from_value(neocogi_skin_impl::StyleCompilerObject::new(publisher.as_com_ref()))
            .query()
            .unwrap();
    let ui = neocogi_ui::Ui::from_interface(crate::create_runtime_from_assets(source.as_com_ref()).unwrap()).unwrap();
    let original = ImportedAssets::read(source.as_com_ref(), None).unwrap();
    let mut metrics = source.metrics().unwrap();
    metrics.padding += 4;
    let candidate = compiler.derive_style(source.as_com_ref(), metrics, PaletteEdit::Preserve.into()).unwrap();
    assert_eq!(
        ui.assets().unwrap().as_ptr(),
        source.as_ptr(),
        "deriving a candidate cannot publish or schedule a UI edit"
    );
    let imported = ImportedAssets::read(candidate.as_com_ref(), Some(&original)).unwrap();
    let mut expected = export_bundle_data(original.bundle()).unwrap();
    expected.tables.metrics = metrics;
    equivalent(&export_bundle_data(imported.bundle()).unwrap(), &expected);
    let palette = crate::FlatPalette {
        text: crate::color(11, 22, 33, 44),
        button: crate::color(51, 62, 73, 84),
        ..crate::FlatPalette::default()
    };
    let flat = compiler.derive_style(source.as_com_ref(), metrics, PaletteEdit::Flat(palette).into()).unwrap();
    let imported = ImportedAssets::read(flat.as_com_ref(), Some(&original)).unwrap();
    let mut skin = original.bundle().skin().clone();
    skin.metrics.padding = metrics.padding;
    skin.apply_flat_palette(palette);
    let native = SkinBundle::new(original.bundle().atlas().clone(), skin);
    equivalent(&export_bundle_data(imported.bundle()).unwrap(), &export_bundle_data(&native).unwrap());
    let invalid = AssetMetrics { padding: -1, ..metrics };
    assert_eq!(
        compiler.derive_style(source.as_com_ref(), invalid, PaletteEdit::Preserve.into()).unwrap_err(),
        NC_E_INVALID_ARG
    );
    let invalid = PaletteEditAbi::from_unknown_tag(PaletteEditTag::from_raw(255)).unwrap();
    assert_eq!(compiler.derive_style(source.as_com_ref(), metrics, invalid).unwrap_err(), NC_E_INVALID_ARG);
    assert_eq!(ui.assets().unwrap().as_ptr(), source.as_ptr());
    ui.close().unwrap();
    // Compilation owns no UI lifetime: retained resource data remains authorable after closure.
    compiler.derive_style(source.as_com_ref(), metrics, PaletteEdit::Preserve.into()).unwrap();
}
