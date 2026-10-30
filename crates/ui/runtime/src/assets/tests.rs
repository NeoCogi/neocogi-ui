//! Boundary tests use real generated COM calls, malformed producer tables and an upload-failing renderer.
use crate::{SkinBundle, Skin};
use crate::assets::ImportedAssets;
use neocogi_ui::abi::core::*;
use neocogi_ui::abi::nodes::ButtonAbi;
use crate::test_support::assets::{export_bundle, export_bundle_data, export_imported_data};
use neocogi_gfx_support::abi::AssetPublisherAbi;
use neocogi_ui::assets::AssetData;
use neocogi_appearance::skin::{PaletteEdit, StyleCompilerAbi};
use neocogi_com::{com_object, ComPtr, ComRef, NcStatus, NcCopyFromBufferOffset, NC_E_BUSY, NC_E_INVALID_ARG};
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
        assert_eq!(a.patch, b.patch);
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
    let mut skin = Skin::from_atlas(atlas.clone());
    *skin.window_chrome_mut() = crate::test_support::classic_mac_chrome(crate::Color { r: 12, g: 34, b: 56, a: 255 });
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
    let original = SkinBundle::from_skin(skin);
    let source = export_bundle(&original).unwrap();
    let imported = ImportedAssets::read(source.as_com_ref(), None).unwrap();
    equivalent(
        &export_bundle_data(&original).unwrap(),
        &export_imported_data(&imported, imported.bundle()).unwrap(),
    );
    assert_ne!(original.atlas().font_id("body"), imported.bundle().atlas().font_id("body"));

    // An invalid source range must not publish any result, including a valid prefix.
    let mut output = neocogi_com::NcOutputBuffer::new();
    let count = source.info().unwrap().rgba_bytes().unwrap();
    assert_eq!(source.image().unwrap().pixels(output.output(count as u64 - 1, 2)), Err(NC_E_INVALID_ARG));
    assert!(output.is_empty());
}

/// Catalog authentication precedes ID preservation, and missing original names cannot enter a runtime.
#[test]
fn derived_identity_is_preserved_but_unrelated_and_incomplete_catalogs_are_rejected() {
    let source = export_bundle(&bundle()).unwrap();
    let first = ImportedAssets::read(source.as_com_ref(), None).unwrap();
    let mut skin = first.bundle().skin().clone();
    skin.metrics_mut().padding += 7;
    let changed = SkinBundle::from_skin(skin);
    let revision = first.export_derived(&changed).unwrap();
    let second = ImportedAssets::read(revision.as_com_ref(), Some(&first.catalog())).unwrap();
    assert_eq!(first.bundle().atlas().font_id("application"), second.bundle().atlas().font_id("application"));
    assert_eq!(second.bundle().skin().metrics().padding, changed.skin().metrics().padding);
    assert_ne!(first.bundle().skin().revision(), second.bundle().skin().revision());

    let unrelated = export_bundle(&bundle()).unwrap();
    assert!(matches!(ImportedAssets::read(unrelated.as_com_ref(), Some(&first.catalog())), Err(UI_E_OWNER)));
    let incomplete = SkinBundle::from_atlas(crate::test_support::test_atlas());
    let incomplete = export_bundle_data(&incomplete).unwrap().into_revision(source.as_com_ref()).unwrap();
    assert!(matches!(
        ImportedAssets::read(incomplete.as_com_ref(), Some(&first.catalog())),
        Err(NC_E_INVALID_ARG)
    ));
}

/// Malformed Unicode, partitions, pixels, dimensions and tagged geometry fail before native construction.
#[test]
fn malformed_portable_metadata_never_reaches_a_live_skin() {
    for case in 0..10 {
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
            8 => {
                data.tables.controls.to_mut()[0].disabled.patch = AssetPatch::Image {
                    icon: u32::MAX,
                    source: Insets::default(),
                    tint: 0,
                }
            }
            9 => data.tables.controls.to_mut()[0].disabled.center_visible = 2,
            _ => unreachable!(),
        }
        let source = data.into_bundle().unwrap();
        let expected = NC_E_INVALID_ARG;
        assert!(
            matches!(ImportedAssets::read(source.as_com_ref(), None), Err(status) if status == expected),
            "case {case}"
        );
    }
    let mut data = export_bundle_data(&bundle()).unwrap();
    data.tables.size.width = i32::MAX;
    assert_eq!(data.into_bundle().unwrap_err(), NC_E_INVALID_ARG);
}

/// A malformed nested enum from a foreign asset table fails before constructing a skin.
#[test]
fn unknown_nested_asset_tag_is_rejected_at_the_output_boundary() {
    /// Supplies initialized raw records with one deliberately invalid nested tag.
    unsafe extern "C" fn malformed_controls(
        _object: *mut core::ffi::c_void,
        _offset: u64,
        count: u64,
        output: *mut <AssetControlVisuals as neocogi_com::ComValue>::Wire,
        written: *mut u64,
    ) -> NcStatus {
        let data = export_bundle_data(&bundle()).unwrap();
        if count != data.tables.controls.len() as u64 {
            return NC_E_INVALID_ARG;
        }
        for (index, value) in data.tables.controls.iter().enumerate() {
            let mut wire = neocogi_com::encode_inline(value).unwrap();
            wire.disabled.patch.tag = 255;
            // SAFETY: generated caller supplied count writable raw elements.
            unsafe {
                output.add(index).write(wire);
            }
        }
        // SAFETY: raw ABI supplies a distinct writable prefix count.
        unsafe {
            *written = count;
        }
        neocogi_com::NC_OK
    }
    let source = export_bundle(&bundle()).unwrap();
    let raw = source.as_raw();
    // SAFETY: copy the function-pointer table, retaining the real source object.
    // The replacement table and source outlive the single borrowed read.
    let mut table = unsafe { core::ptr::read(raw.vtable) };
    table.control_visuals = malformed_controls;
    let borrowed = unsafe { ComRef::from_raw_borrowed(neocogi_com::ComRaw { object: raw.object, vtable: &table }) }.unwrap();
    assert!(matches!(ImportedAssets::read(borrowed, None), Err(neocogi_com::NC_E_INVALID_TAG)));
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

/// Production text block and the text area's actual child use exact concatenated glyph widths.
#[test]
fn overhanging_text_widgets_measure_exact_source_slices_through_com() {
    let mut data = neocogi_ui_support::test_support::test_asset_data_with_font_sizes(&[("body", 16)]);
    for glyph in data.tables.glyphs.to_mut() {
        glyph.offset_x = 2;
        glyph.advance_x = 4;
    }
    let source = data.into_bundle().unwrap();
    let runtime = crate::test_support::runtime_from_assets(source.as_com_ref()).unwrap();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime).unwrap();
    let metrics: ComPtr<ITextMetricsAbi> = UnusedMetrics::from_value(UnusedMetrics).query().unwrap();
    for wrapping in [TextWrapping::Word, TextWrapping::WordOrCharacter] {
        let block = ui.widgets().text_block("a a", wrapping).unwrap();
        let mut block_layout = block.as_interface().downgrade().query::<ILeafLayoutAbi>().unwrap();
        let area = ui.widgets().text_area_unobserved("a a", wrapping, false).unwrap();
        // Follow the public read-only topology to the sole editor. No concrete downcast or
        // substitute implementation stands in for the production scrolling composition.
        let topology = area.node_interface().unwrap().query::<IContainerLayoutAbi>().unwrap();
        let surface = topology.children().unwrap().child(0).unwrap().query::<IContainerLayoutAbi>().unwrap();
        let mut area_layout = surface.children().unwrap().child(0).unwrap().query::<ILeafLayoutAbi>().unwrap();
        for width in [14, 16] {
            let limits = LayoutLimits {
                width: AxisLimit { kind: LimitKind::Finite, extent: width },
                height: AxisLimit { kind: LimitKind::Unbounded, extent: 0 },
            };
            assert_eq!(block_layout.measure(limits, metrics.as_com_ref()).unwrap(), Size { width: 14, height: 16 });
            assert_eq!(area_layout.measure(limits, metrics.as_com_ref()).unwrap(), Size { width: 14, height: 16 });
        }
    }
}

/// Observable renderer state contains only owned capabilities and ordinary single-threaded cells.
struct Probe {
    /// Reentry must fail while the runtime owns its installation transaction.
    runtime: std::rc::Weak<ComPtr<UiRuntimeAbi>>,
    /// Setter reentry must also fail during the upload boundary.
    button: neocogi_com::WeakComBox<ButtonAbi>,
    /// Last successfully uploaded renderer-owned image projection.
    current: RefCell<ComPtr<IRgbaImageAbi>>,
    /// Forces a recoverable upload failure without changing the current bundle.
    fail: Cell<bool>,
    /// Proves malformed candidates fail before reaching GPU preparation.
    uploads: Cell<u32>,
    /// Initial host binding is prepared before the test arms reentry/failure injection.
    armed: Cell<bool>,
    /// The observer must see the exact atlas version accepted by native preparation.
    prepared: Cell<u64>,
}
#[com_object(interfaces(neocogi_render_core::abi::IRenderHost))]
/// Native preparation probes run through the private graphics host, without owning their runtime.
struct UploadHost {
    /// CPU assembly authenticates real generic resources and blocks.
    inner: ComPtr<neocogi_render_core::abi::IRenderHostAbi>,
    /// Runtime and widget observations are weak to avoid a host-to-runtime ownership cycle.
    probe: Rc<Probe>,
}
impl neocogi_render_core::abi::IRenderHost for UploadHost {
    fn import_ui_texture(
        &self,
        resources: ComRef<'_, neocogi_render_core::abi::IRenderResourcesAbi>,
        image: ComRef<'_, ITextureAbi>,
    ) -> Result<neocogi_com::ComBox<neocogi_render_core::abi::IGpuTextureAbi>, NcStatus> {
        self.inner.import_ui_texture(resources, image)
    }
    fn create_ui_client(
        &self,
        limits: neocogi_render_core::abi::RenderClientLimits,
    ) -> Result<neocogi_com::ComBox<neocogi_render_core::abi::IRenderClientDomainAbi>, NcStatus> {
        self.inner.create_ui_client(limits)
    }
    fn recording_broker(&self) -> Result<ComPtr<neocogi_render_core::abi::IRenderViewBrokerAbi>, NcStatus> {
        self.inner.recording_broker()
    }
    fn prepare_ui_texture(
        &self,
        resources: ComRef<'_, neocogi_render_core::abi::IRenderResourcesAbi>,
        texture: ComRef<'_, neocogi_render_core::abi::IGpuTextureAbi>,
    ) -> Result<(), NcStatus> {
        let version = neocogi_gfx_support::testing::texture(resources, texture)?;
        self.inner.prepare_ui_texture(resources, texture)?;
        if self.probe.armed.get() {
            self.probe.uploads.set(self.probe.uploads.get() + 1);
            let installed = self.probe.button.host()?.assets()?.image()?;
            assert_eq!(installed.as_raw().object, self.probe.current.borrow().as_raw().object);
            let runtime = self.probe.runtime.upgrade().expect("test owns runtime reentry capability");
            assert_eq!(runtime.update(SIZE), Err(NC_E_BUSY));
            let mut button = self.probe.button.clone();
            assert_eq!(button.set_label(NcCopyFromBufferOffset::new(0, b"reentrant")), Err(UI_E_BUSY));
            let installed = self.probe.button.host()?.assets()?;
            let mut runtime = runtime.as_ref().clone();
            assert_eq!(runtime.install_assets(installed.as_com_ref()), Err(NC_E_BUSY));
            if self.probe.fail.get() {
                return Err(UI_E_RENDER);
            }
        }
        self.probe.prepared.set(version.id());
        Ok(())
    }
}

#[com_object(interfaces(IRenderer))]
/// Observes actual recorded atlas versions, independently of source image identity.
struct UploadRenderer(Rc<Probe>);
impl IRenderer for UploadRenderer {
    fn render(&self, info: RenderInfo, stream: ComRef<'_, IRenderStreamAbi>) -> Result<(), NcStatus> {
        let blocks = neocogi_gfx_support::testing::blocks(info, stream)?;
        let mut draws = 0;
        for block in blocks {
            for command in block.commands() {
                if let neocogi_render_gpu::command::Command::Draw(draw) = command {
                    assert_eq!(draw.textures[0].texture.id(), self.0.prepared.get());
                    draws += 1;
                }
            }
        }
        assert!(draws > 0);
        Ok(())
    }
}
/// Bind real host preparation before arming injection; no observer owns a strong runtime cycle.
fn upload_renderer(probe: Rc<Probe>) -> ComPtr<IRendererAbi> {
    let host: ComPtr<neocogi_render_core::abi::IRenderHostAbi> = UploadHost::from_value(UploadHost {
        inner: neocogi_gfx_support::testing::host(),
        probe: probe.clone(),
    })
    .query()
    .unwrap();
    let mut runtime = probe.runtime.upgrade().unwrap().as_ref().clone();
    runtime.set_render_host(host.as_com_ref()).unwrap();
    probe.armed.set(true);
    UploadRenderer::from_value(UploadRenderer(probe)).query().unwrap()
}
/// Drawable dimensions used by the complete runtime integration fixture.
const SIZE: Size = Size { width: 640, height: 400 };

/// A real activation owns the same capabilities used by an application's theme/style handler.
struct ThemeState {
    /// A retained facade proves a callback cannot claim exclusive runtime access during dispatch.
    ui: neocogi_ui::Ui,
    /// Validated candidate belongs to the current resource catalog.
    candidate: ComPtr<IAssetBundleAbi>,
    /// Counts successful handler commits; failures must leave this unchanged.
    installed: u32,
}
/// Concrete handler deliberately attempts a replacement during an active shared runtime call.
struct InstallTheme;
impl InstallTheme {
    /// The generated COM borrow gate rejects this call before the renderer observes a candidate.
    fn handle(&mut self, state: &mut ThemeState, _: &()) -> Result<(), NcStatus> {
        state.ui.install_assets(state.candidate.as_com_ref())?;
        state.installed += 1;
        Ok(())
    }
}

/// Resource replacement requires an exclusive call after application dispatch has returned.
#[test]
fn event_callback_cannot_install_until_shared_runtime_dispatch_returns() {
    let source = export_bundle(&bundle()).unwrap();
    let runtime = crate::test_support::runtime_from_assets(source.as_com_ref()).unwrap();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let (session_tx, session) = std::sync::mpsc::channel();
    let button = ui
        .widgets()
        .button_with_queue(
            "switch theme",
            Some(
                neocogi_ui::test_event_queue!(neocogi_ui::ButtonAbi, (), session_tx.clone(), |_queue, _origin, _event| ())
                    .unwrap()
                    .as_com_ref(),
            ),
        )
        .unwrap();
    let runtime_owner = Rc::new(runtime.clone());
    let probe = Rc::new(Probe {
        runtime: Rc::downgrade(&runtime_owner),
        button: button.as_interface().downgrade(),
        current: RefCell::new(source.image().unwrap()),
        fail: Cell::new(true),
        uploads: Cell::new(0),
        armed: Cell::new(false),
        prepared: Cell::new(0),
    });
    let renderer = upload_renderer(probe.clone());
    let _window = ui.window("theme callback", Rect { x: 0, y: 0, width: 400, height: 250 }, button).unwrap();
    let mut data = export_bundle_data(&bundle()).unwrap();
    data.tables.metrics.padding += 10;
    let candidate = data.into_revision(source.as_com_ref()).unwrap();
    let state = ThemeState {
        ui: ui.clone(),
        candidate: candidate.clone(),
        installed: 0,
    };

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
    let app = neocogi_ui_support::test_support::events::TestApp::new(state, session, |state, message| InstallTheme.handle(state, &message));
    assert_eq!(app.update(&ui, SIZE), Err(NC_E_BUSY));
    assert_eq!(ui.assets().unwrap().as_raw().object, source.as_raw().object);
    assert_eq!(app.state().installed, 0);
    assert_eq!(probe.uploads.get(), 0);

    // Once dispatch has returned, the state owns ordinary exclusive access and upload failures
    // retain both the installed UI value and the renderer's current atlas.
    {
        let state = &mut *app.state();
        let candidate = state.candidate.clone();
        assert_eq!(state.ui.install_assets(candidate.as_com_ref()), Err(UI_E_RENDER));
    }
    assert_eq!(ui.assets().unwrap().as_raw().object, source.as_raw().object);
    assert_eq!(probe.uploads.get(), 1);
    probe.fail.set(false);
    {
        let state = &mut *app.state();
        let candidate = state.candidate.clone();
        state.ui.install_assets(candidate.as_com_ref()).unwrap();
        state.installed += 1;
    }
    assert_eq!(probe.uploads.get(), 2);
    assert_eq!(app.state().installed, 1);
    assert_eq!(ui.assets().unwrap().as_raw().object, candidate.as_raw().object);
    // Installation occurred outside update, so one explicit synchronization commits the revision.
    app.update(&ui, SIZE).unwrap();
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
}

/// Failed validation/upload preserves UI and GPU state; success changes built-in measurement and requires update.
#[test]
fn runtime_install_is_atomic_and_updates_shared_widget_measurement() {
    let source = export_bundle(&bundle()).unwrap();
    let runtime = crate::test_support::runtime_from_assets(source.as_com_ref()).unwrap();
    let mut ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let button = ui.widgets().button_unobserved("resources").unwrap();
    let button_interface = button.as_interface().downgrade();
    let runtime_owner = Rc::new(runtime.clone());
    let probe = Rc::new(Probe {
        runtime: Rc::downgrade(&runtime_owner),
        button: button_interface.clone(),
        current: RefCell::new(source.image().unwrap()),
        fail: Cell::new(false),
        uploads: Cell::new(0),
        armed: Cell::new(false),
        prepared: Cell::new(0),
    });
    let renderer = upload_renderer(probe.clone());
    let _window = ui.window("assets", Rect { x: 0, y: 0, width: 400, height: 250 }, button).unwrap();
    let mut leaf: neocogi_com::WeakComBox<ILeafLayoutAbi> = button_interface.query().unwrap();
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
    assert_eq!(ui.install_assets(candidate.as_com_ref()), Err(UI_E_RENDER));
    assert_eq!(ui.assets().unwrap().as_raw().object, source.as_raw().object);
    assert_eq!(leaf.measure(limits, metrics.as_com_ref()).unwrap(), before);
    ui.render(SIZE, renderer.as_com_ref()).unwrap();

    let unrelated = export_bundle(&bundle()).unwrap();
    assert_eq!(ui.install_assets(unrelated.as_com_ref()), Err(UI_E_OWNER));
    assert_eq!(probe.uploads.get(), 1);
    probe.fail.set(false);
    ui.install_assets(candidate.as_com_ref()).unwrap();
    assert_eq!(probe.uploads.get(), 2);
    assert_eq!(button_interface.host().unwrap().assets().unwrap().as_raw().object, candidate.as_raw().object);
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
        neocogi_skin_impl::StyleCompilerObject::from_value(neocogi_skin_impl::StyleCompilerObject::new(publisher.as_com_ref()).unwrap())
            .query()
            .unwrap();
    let ui = nc_ui_nodes::bind_builtin_nodes(crate::test_support::runtime_from_assets(source.as_com_ref()).unwrap()).unwrap();
    let original = ImportedAssets::read(source.as_com_ref(), None).unwrap();
    let mut metrics = source.metrics().unwrap();
    metrics.padding += 4;
    let candidate = compiler.derive_style(source.as_com_ref(), metrics, PaletteEdit::Preserve).unwrap();
    assert_eq!(
        ui.assets().unwrap().as_raw().object,
        source.as_raw().object,
        "deriving a candidate cannot publish or schedule a UI edit"
    );
    let imported = ImportedAssets::read(candidate.as_com_ref(), Some(&original.catalog())).unwrap();
    let mut expected = export_imported_data(&original, original.bundle()).unwrap();
    expected.tables.metrics = metrics;
    equivalent(&export_imported_data(&imported, imported.bundle()).unwrap(), &expected);
    let palette = crate::FlatPalette {
        text: crate::color(11, 22, 33, 44),
        button: crate::color(51, 62, 73, 84),
        ..crate::FlatPalette::default()
    };
    let flat = compiler.derive_style(source.as_com_ref(), metrics, PaletteEdit::Flat(palette)).unwrap();
    let imported = ImportedAssets::read(flat.as_com_ref(), Some(&original.catalog())).unwrap();
    let mut skin = original.bundle().skin().clone();
    skin.metrics_mut().padding = metrics.padding;
    skin.apply_flat_palette(palette);
    let native = SkinBundle::from_skin(skin);
    equivalent(
        &export_imported_data(&imported, imported.bundle()).unwrap(),
        &export_imported_data(&original, &native).unwrap(),
    );
    let invalid = AssetMetrics { padding: -1, ..metrics };
    assert_eq!(
        compiler.derive_style(source.as_com_ref(), invalid, PaletteEdit::Preserve).unwrap_err(),
        NC_E_INVALID_ARG
    );
    // An invalid discriminant exists only in foreign wire storage. Exercise
    // the actual provider thunk before it can dispatch a native PaletteEdit.
    let mut invalid = neocogi_com::encode_inline(&PaletteEdit::Preserve).unwrap();
    invalid.tag = 255;
    let raw = compiler.as_raw();
    let mut output = core::mem::MaybeUninit::uninit();
    // SAFETY: the live base capability, initialized metrics and wire enum, and
    // writable result all satisfy the raw call's storage/lifetime obligations.
    assert_eq!(
        unsafe {
            ((*raw.vtable).derive_style)(
                raw.object,
                source.as_raw(),
                neocogi_com::encode_inline(&metrics).unwrap(),
                invalid,
                output.as_mut_ptr(),
            )
        },
        neocogi_com::NC_E_INVALID_TAG
    );
    assert_eq!(ui.assets().unwrap().as_raw().object, source.as_raw().object);
    ui.close().unwrap();
    // Compilation owns no UI lifetime: retained resource data remains authorable after closure.
    compiler.derive_style(source.as_com_ref(), metrics, PaletteEdit::Preserve).unwrap();
}
