//! Runtime boundary regressions use deliberately retaining and reentrant components.
use super::*;
use neocogi_ui_nodes_contracts::*;
use neocogi_com::{ComBox, WeakComBox, INcStringComPtrExt, NC_E_BUSY, NcOutputWriter};
mod composite_fixture;
mod containers;
mod direct_posting;
mod dispatch_errors;
mod dispatch_geometry;
mod documentation;
mod embedded_input;
mod input_boundaries;
mod ordinary_construction;
mod pending_changes;
mod recovery;
mod resource_refresh;
mod retention;
mod shutdown;
mod widget_flags;

/// Positive drawable size shared by the deterministic headless fixtures.
const SIZE: Size = Size { width: 640, height: 400 };
/// Logical window allocation with room for retained chrome and the test leaf.
const RECT: Rect = Rect { x: 20, y: 20, width: 400, height: 200 };

/// Creates the production runtime with a small deterministic atlas.
fn runtime() -> ComPtr<UiRuntimeAbi> {
    crate::test_support::create_runtime(crate::test_support::test_atlas())
}

/// Creates a concrete button without bypassing its production factory.
fn button(runtime: &ComPtr<UiRuntimeAbi>) -> ComBox<ButtonAbi> {
    nc_ui_nodes::UiFactory::bind(runtime.as_com_ref())
        .unwrap()
        .create_button(
            NcCopyFromBufferOffset::new(0, b""),
            neocogi_com::NcValueRef::new(&neocogi_com::events::EventBinding::Unobserved).unwrap(),
        )
        .unwrap()
}

/// Oversized chrome titles fail before a widget is mounted or title text is retained.
#[test]
fn oversized_window_title_returns_the_unmounted_root_for_retry() {
    let runtime = runtime();
    let manager: ComPtr<IWindowManagerAbi> = runtime.query().unwrap();
    let root = button(&runtime);
    let observed = root.downgrade();
    let oversized = vec![b'x'; MAX_WINDOW_TITLE_BYTES + 1];
    let result = manager.create_window(
        NcCopyFromBufferOffset::new(0, &oversized),
        RECT,
        root.query().unwrap(),
        neocogi_com::NcValueRef::new(&neocogi_com::events::EventBinding::Unobserved).unwrap(),
    );
    let (status, root) = result.unwrap_err();
    assert_eq!(status, neocogi_com::NC_E_OUT_OF_RANGE);
    assert!(observed.label().is_ok());
    let window = mount(&runtime, root).unwrap();
    window.close().unwrap();
    assert_eq!(observed.label().unwrap_err(), neocogi_com::NC_E_DROPPED);
}

/// The facade cannot combine a node provider bound to one retained forest with another runtime.
#[test]
fn facade_rejects_a_node_factory_bound_to_another_runtime() {
    let first = runtime();
    let second = runtime();
    let factory = nc_ui_nodes::UiFactory::bind(first.as_com_ref()).unwrap();
    let result = neocogi_ui::Ui::from_interfaces(second, factory);
    assert!(matches!(result, Err(UI_E_OWNER)));
}

#[com_object(interfaces(IAssetBundle))]
/// Counts portable bundle calls while delegating the exact data to a valid immutable source.
struct CountingAssets {
    /// Valid bundle which remains the sole authority for returned records and catalog identity.
    source: ComPtr<IAssetBundleAbi>,
    /// Test-owned count of every method that would repeat portable asset decoding work.
    calls: Rc<Cell<u32>>,
}

impl CountingAssets {
    /// Records one delegated portable operation without changing its result or ownership.
    fn called(&self) {
        self.calls.set(self.calls.get() + 1);
    }
}

impl IAssetBundle for CountingAssets {
    /// Delegates the immutable image capability after recording the lookup.
    fn image(&self) -> Result<ComPtr<IRgbaImageAbi>, NcStatus> {
        self.called();
        self.source.image()
    }
    /// Delegates catalog identity after recording the lookup.
    fn catalog(&self) -> Result<ComPtr<IAssetCatalogAbi>, NcStatus> {
        self.called();
        self.source.catalog()
    }
    /// Delegates bounded table metadata after recording the lookup.
    fn info(&self) -> Result<AssetInfo, NcStatus> {
        self.called();
        self.source.info()
    }
    /// Delegates the requested font range after recording the copy.
    fn fonts(&self, output: NcOutputWriter<'_, AssetFont>) -> Result<(), NcStatus> {
        self.called();
        self.source.fonts(output.forward()?)
    }
    /// Delegates the requested glyph range after recording the copy.
    fn glyphs(&self, output: NcOutputWriter<'_, AssetGlyph>) -> Result<(), NcStatus> {
        self.called();
        self.source.glyphs(output.forward()?)
    }
    /// Delegates the requested icon range after recording the copy.
    fn icons(&self, output: NcOutputWriter<'_, AssetIcon>) -> Result<(), NcStatus> {
        self.called();
        self.source.icons(output.forward()?)
    }
    /// Delegates the requested UTF-8 name range after recording the copy.
    fn names(&self, output: NcOutputWriter<'_, u8>) -> Result<(), NcStatus> {
        self.called();
        self.source.names(output.forward()?)
    }
    /// Delegates scalar layout metrics after recording the lookup.
    fn metrics(&self) -> Result<AssetMetrics, NcStatus> {
        self.called();
        self.source.metrics()
    }
    /// Delegates window chrome policy after recording the lookup.
    fn chrome(&self) -> Result<AssetChrome, NcStatus> {
        self.called();
        self.source.chrome()
    }
    /// Delegates complete surface-state records after recording the copy.
    fn surface_visuals(&self, output: NcOutputWriter<'_, AssetSurfaceVisuals>) -> Result<(), NcStatus> {
        self.called();
        self.source.surface_visuals(output.forward()?)
    }
    /// Delegates complete control-state records after recording the copy.
    fn control_visuals(&self, output: NcOutputWriter<'_, AssetControlVisuals>) -> Result<(), NcStatus> {
        self.called();
        self.source.control_visuals(output.forward()?)
    }
    /// Delegates complete menu-state records after recording the copy.
    fn menu_visuals(&self, output: NcOutputWriter<'_, AssetMenuVisuals>) -> Result<(), NcStatus> {
        self.called();
        self.source.menu_visuals(output.forward()?)
    }
    /// Delegates complete chrome-state records after recording the copy.
    fn chrome_visuals(&self, output: NcOutputWriter<'_, AssetChromeVisuals>) -> Result<(), NcStatus> {
        self.called();
        self.source.chrome_visuals(output.forward()?)
    }
}

/// A separately bound node provider imports once, then all products reuse that provider cache.
#[test]
fn factory_construction_and_widget_phases_do_not_reimport_assets() {
    let source = crate::test_support::assets::export_bundle(&crate::SkinBundle::from_atlas(crate::test_support::test_atlas())).unwrap();
    let calls = Rc::new(Cell::new(0));
    let counted: ComPtr<IAssetBundleAbi> = CountingAssets::from_value(CountingAssets { source, calls: calls.clone() }).query().unwrap();
    let runtime = crate::test_support::runtime_from_assets(counted.as_com_ref()).unwrap();
    calls.set(0);
    let factory = nc_ui_nodes::UiFactory::bind(runtime.as_com_ref()).unwrap();
    assert!(calls.get() > 0, "an independent provider must validate and import its initial revision");
    calls.set(0);
    let label = NcCopyFromBufferOffset::new(0, b"shared");
    let button = factory
        .create_button(label, neocogi_com::NcValueRef::new(&neocogi_com::events::EventBinding::Unobserved).unwrap())
        .unwrap();
    let _checkbox = factory
        .create_checkbox(
            false,
            label,
            neocogi_com::NcValueRef::new(&neocogi_com::events::EventBinding::Unobserved).unwrap(),
        )
        .unwrap();
    let _textbox = factory
        .create_textbox(label, neocogi_com::NcValueRef::new(&neocogi_com::events::EventBinding::Unobserved).unwrap())
        .unwrap();
    let _text_block = factory.create_text_block(label, TextWrapping::Word).unwrap();
    let _slider = factory
        .create_slider(
            SliderOptions {
                value: 0.0,
                low: 0.0,
                high: 1.0,
                step: 0.1,
                precision: 1,
            },
            neocogi_com::NcValueRef::new(&neocogi_com::events::EventBinding::Unobserved).unwrap(),
        )
        .unwrap();
    let _number = factory
        .create_number(
            NumberOptions { value: 0.0, step: 1.0, precision: 0 },
            neocogi_com::NcValueRef::new(&neocogi_com::events::EventBinding::Unobserved).unwrap(),
        )
        .unwrap();
    let _swatch = factory.create_color_swatch(u32::MAX, label).unwrap();
    let _list_item = factory
        .create_list_item(label, neocogi_com::NcValueRef::new(&neocogi_com::events::EventBinding::Unobserved).unwrap())
        .unwrap();
    let _list_box = factory
        .create_list_box(label, neocogi_com::NcValueRef::new(&neocogi_com::events::EventBinding::Unobserved).unwrap())
        .unwrap();
    let _scrollbar = factory
        .create_scrollbar(
            ScrollRange {
                axis: ScrollAxis::Vertical,
                viewport: 1,
                content: 2,
                offset: 0,
            },
            neocogi_com::NcValueRef::new(&neocogi_com::events::EventBinding::Unobserved).unwrap(),
        )
        .unwrap();
    let _linear = factory.create_linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let tracks = [LayoutTrack::Content];
    let _grid = factory
        .create_grid(
            NcCopyFromBufferOffset::new(0, &tracks),
            NcCopyFromBufferOffset::new(0, &tracks),
            neocogi_ui::GridLimits { max_columns: 2, max_rows: 2 },
        )
        .unwrap();
    let _disclosure = factory.create_disclosure(label, DisclosureStyle::Header, false).unwrap();
    let content = button.query::<IWidgetAbi>().unwrap();
    let _scroll = factory.create_scroll_area(content, true).unwrap();
    let area = factory
        .create_text_area(
            label,
            TextWrapping::Word,
            true,
            neocogi_com::NcValueRef::new(&neocogi_com::events::EventBinding::Unobserved).unwrap(),
        )
        .unwrap();
    let root = area.query::<IWidgetAbi>().unwrap();
    let _window = mount(&runtime, root).unwrap();
    runtime.update(SIZE).unwrap();
    assert_eq!(calls.get(), 0, "factory products and update must reuse the provider's imported value");
}

/// Mounts the common capability through the production manager's transactional boundary.
fn mount(runtime: &ComPtr<UiRuntimeAbi>, root: ComBox<IWidgetAbi>) -> Result<ComPtr<IWindowAbi>, NcStatus> {
    runtime
        .query::<IWindowManagerAbi>()?
        .create_window(
            NcCopyFromBufferOffset::new(0, b"test"),
            RECT,
            root,
            neocogi_com::NcValueRef::new(&neocogi_com::events::EventBinding::Unobserved).unwrap(),
        )
        .map_err(|(status, _rejected)| status)
}

/// Observable state retained by a fake backend independently of its COM allocation.
#[derive(Default)]
struct RenderProbe {
    /// Complete renderer invocations, including deliberately failing submissions.
    calls: Cell<u32>,
    /// Last copied geometry lets independent tests inspect actual clipped output.
    vertices: RefCell<Vec<RenderVertex>>,
    /// Old retained stream/token used to prove that expiry survives later reuse.
    streams: RefCell<Vec<(ComPtr<IRenderStreamAbi>, u64)>>,
    /// Test-controlled explicit renderer failure.
    fail: Cell<bool>,
}

#[com_object(interfaces(IRenderer))]
/// Typed backend records real final vertices and retains the stream maliciously but safely.
struct Renderer {
    /// Shared observations owned by the test, with no borrowed stack reference in COM.
    probe: Rc<RenderProbe>,
}

impl IRenderer for Renderer {
    /// This CPU recorder accepts validated images without a GPU upload.
    fn prepare_atlas(&self, _atlas: ComRef<'_, IRgbaImageAbi>) -> Result<(), NcStatus> {
        Ok(())
    }
    /// Copies bulk geometry during its valid scope and retains authority for later rejection tests.
    fn render(&self, info: RenderInfo, _atlas: ComRef<'_, IRgbaImageAbi>, stream: ComRef<'_, IRenderStreamAbi>) -> Result<(), NcStatus> {
        let mut vertices = neocogi_com::NcOutputBuffer::<RenderVertex>::new();
        assert_eq!(
            stream.vertices(info.invocation, vertices.output(0, info.vertex_count as usize))?,
            info.vertex_count
        );
        let mut batches = neocogi_com::NcOutputBuffer::<RenderBatch>::new();
        assert_eq!(stream.batches(info.invocation, batches.output(0, info.batch_count as usize))?, info.batch_count);
        // These built-in fixtures use one coalesced atlas range regardless of glyph/primitive count.
        assert_eq!(batches.len(), 1);
        assert!(matches!(batches[0], RenderBatch::Atlas { first: 0, count } if count == info.vertex_count));
        assert_eq!(stream.texture(info.invocation, 0).unwrap_err(), NC_E_INVALID_ARG);
        assert_eq!(stream.item(info.invocation, 0).unwrap_err(), NC_E_INVALID_ARG);
        assert!(vertices.len() >= 6);
        assert!(vertices.iter().all(|v| v.x.is_finite() && v.y.is_finite()));
        *self.probe.vertices.borrow_mut() = vertices.into_vec();
        self.probe.calls.set(self.probe.calls.get() + 1);
        self.probe.streams.borrow_mut().push((stream.try_add_ref()?, info.invocation));
        if self.probe.fail.get() { Err(UI_E_RENDER) } else { Ok(()) }
    }
}

/// Returns the COM renderer and independently observable probe state.
fn renderer() -> (ComPtr<IRendererAbi>, Rc<RenderProbe>) {
    let probe = Rc::new(RenderProbe::default());
    (Renderer::from_value(Renderer { probe: probe.clone() }).query().unwrap(), probe)
}

/// Wrong owners and duplicate attachment fail without consuming the legitimate mount opportunity.
#[test]
fn mount_validation_destroys_rejected_owners_and_removal_expires_observers() {
    let first = runtime();
    let second = runtime();
    let candidate = button(&first);
    let rejected = candidate.downgrade();
    assert_eq!(mount(&second, candidate.query().unwrap()).unwrap_err(), UI_E_OWNER);
    assert_eq!(rejected.label().unwrap_err(), neocogi_com::NC_E_DROPPED);
    let owner = button(&first);
    let mut observed = owner.downgrade();
    let window = mount(&first, owner.query().unwrap()).unwrap();
    assert_eq!(observed.set_label(NcCopyFromBufferOffset::new(1, b"bad")), Err(NC_E_INVALID_ARG));
    assert_eq!(observed.set_label(NcCopyFromBufferOffset::new(0, &[0xff])), Err(NC_E_INVALID_ARG));
    assert_eq!(observed.label().unwrap().to_string().unwrap(), "");
    window.close().unwrap();
    assert_eq!(observed.set_label(NcCopyFromBufferOffset::new(0, b"expired")), Err(neocogi_com::NC_E_DROPPED));
    let detached = button(&first);
    drop(first);
    assert_eq!(detached.label().unwrap_err(), UI_E_CLOSED);
    assert_eq!(window.set_rect(RECT), Err(UI_E_CLOSED));
    second.update(SIZE).unwrap();
}

/// Setter and resize invalidation are independent of user activation's NoChange result.
#[test]
fn rendering_requires_synchronized_geometry_and_streams_expire_after_success_and_failure() {
    let runtime = runtime();
    let button_owner = button(&runtime);
    let mut button = button_owner.downgrade();
    let node = button_owner.query().unwrap();
    let _window = mount(&runtime, node).unwrap();
    let (renderer, probe) = renderer();
    assert_eq!(runtime.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    runtime.update(SIZE).unwrap();
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    button.set_label(NcCopyFromBufferOffset::new(0, b"longer")).unwrap();
    assert_eq!(runtime.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    assert_eq!(probe.calls.get(), 1);
    runtime.update(SIZE).unwrap();
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    button.set_label(NcCopyFromBufferOffset::new(0, b"longer")).unwrap();
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    let resized = Size { width: 800, height: 480 };
    assert_eq!(runtime.render(resized, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    runtime.update(resized).unwrap();
    probe.fail.set(true);
    assert_eq!(runtime.render(resized, renderer.as_com_ref()), Err(UI_E_RENDER));
    for (stream, scope) in probe.streams.borrow().iter() {
        assert_eq!(stream.vertices(*scope, neocogi_com::NcOutputBuffer::new().output(0, 0)), Err(NC_E_INVALID_ARG));
        assert_eq!(stream.batches(*scope, neocogi_com::NcOutputBuffer::new().output(0, 0)), Err(NC_E_INVALID_ARG));
        assert_eq!(stream.texture(*scope, 0).unwrap_err(), NC_E_INVALID_ARG);
        assert_eq!(stream.item(*scope, 0).unwrap_err(), NC_E_INVALID_ARG);
    }
}

/// A component can retain COM services; the runtime must enforce phase and scope at runtime.
#[derive(Default)]
struct WidgetProbe {
    /// Most recent copied update geometry, used to verify concrete container allocations.
    latest: Cell<Option<WidgetUpdate>>,
    /// One-shot attempt to close an actively editing parent during child mounting.
    clear_on_mount: RefCell<Option<WeakComBox<IWidgetAbi>>>,
    /// Records the exact gate/preflight result without assuming which object is busy.
    clear_mount_result: Cell<Option<Result<(), NcStatus>>>,
    /// Active parent authority deliberately reentered from a leaf measurement.
    parent_layout: RefCell<Option<(ComPtr<ILayoutServiceAbi>, LayoutScope)>>,
    /// Every update, including eventless updates and routed text/key transactions.
    updates: Cell<u32>,
    /// Next content width to commit during update, rather than before traversal.
    requested_width: Cell<i32>,
    /// Authoritative preferred width after an update has committed it.
    width: Cell<i32>,
    /// Decoded input snapshots prove that the whole typed payload arrived once.
    events: RefCell<Vec<WidgetEvent>>,
    /// Independently owned text observations survive the original borrowed buffer.
    texts: RefCell<Vec<String>>,
    /// Whether this custom widget currently accepts committed text.
    text_input: Cell<bool>,
    /// Provider destruction follows owner removal once, despite retained weak handles.
    closes: Cell<u32>,
    /// A transient geometry error must be retried without a second setter change.
    fail_measure: Cell<bool>,
    /// Paint calls retain these values beyond their valid invocation.
    painters: RefCell<Vec<(ComPtr<IPainterAbi>, PaintScope)>>,
    /// Number of actual preferred measurement calls after cache handling.
    measures: Cell<u32>,
    /// Injected paint failure must prevent the complete backend submission.
    fail_paint: Cell<bool>,
}

#[com_object(interfaces(IWidget, ILeafLayout), ownership = exclusive)]
/// Independently implemented leaf delegates lifetime only to a factory-owned fixture object.
struct RetainingWidget {
    /// Gives the custom implementation the same runtime ownership identity and mount-once rules.
    /// Actual concrete setter used to test observational mutation rejection.
    button: ComBox<ButtonAbi>,
    /// Retained runtime deliberately tries to reenter during measurement and painting.
    runtime: ComPtr<UiRuntimeAbi>,
    /// Test state is owned and survives every COM call.
    probe: Rc<WidgetProbe>,
}

impl ILeafLayout for RetainingWidget {
    /// Checks reentry and mutation rejection while returning deterministic preferred geometry.
    fn measure(&mut self, _bounds: LayoutLimits, _metrics: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        self.probe.measures.set(self.probe.measures.get() + 1);
        let parent = self.probe.parent_layout.borrow().clone();
        if let Some((service, scope)) = parent {
            assert_eq!(service.child_count(scope), Err(UI_E_BUSY));
        }
        if self.probe.fail_measure.get() {
            return Err(NC_E_INVALID_ARG);
        }
        assert_eq!(self.runtime.update(SIZE), Err(UI_E_BUSY));
        assert_eq!(self.button.set_label(NcCopyFromBufferOffset::new(0, b"forbidden")), Err(UI_E_BUSY));
        // Even no-op flag edits must respect the host's observational phase.
        let before = self.button.properties()?;
        assert_eq!(self.button.replace_flags(before.options), Err(UI_E_BUSY));
        assert_eq!(self.button.set_flags(WidgetOptions::NONE), Err(UI_E_BUSY));
        assert_eq!(self.button.clear_flags(WidgetOptions::NONE), Err(UI_E_BUSY));
        assert_eq!(self.button.properties()?, before);
        Ok(Size {
            width: self.probe.width.get().max(100),
            height: 30,
        })
    }
}
impl Drop for RetainingWidget {
    /// Unique-owner destruction is the only terminal transition.
    fn drop(&mut self) {
        self.probe.closes.set(self.probe.closes.get() + 1);
    }
}
impl IWidget for RetainingWidget {
    /// This widget forwards outputs and has no private inbox.

    /// Returns the shared owner without exposing runtime storage.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        self.button.host()
    }
    /// Delegates mount-once state to its independent lifetime fixture.
    fn mount(&mut self) -> Result<(), NcStatus> {
        let parent = self.probe.clear_on_mount.borrow_mut().take();
        if let Some(parent) = parent {
            self.probe.clear_mount_result.set(Some(parent.query::<neocogi_ui::LinearAbi>()?.clear()));
        }
        self.button.mount()
    }
    /// Reuses real common widget policy in this deliberately retaining implementation.
    fn properties(&self) -> Result<WidgetProperties, NcStatus> {
        let mut properties = self.button.properties()?;
        if self.probe.text_input.get() {
            properties.keyboard |= 256;
        }
        Ok(properties)
    }
    /// Reads the requested configuration without including transient runtime policy.
    fn requested_flags(&self) -> Result<WidgetOptions, NcStatus> {
        self.button.requested_flags()
    }
    /// Common options are delegated to the shared fixture state.
    fn replace_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.button.replace_flags(flags)
    }
    /// Sets current requested bits through the same widget-owned mutation authority.
    fn set_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.button.set_flags(flags)
    }
    /// Clears requested bits without copying effective properties back into settings.
    fn clear_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.button.clear_flags(flags)
    }
    /// Eventless updates do not change this fixture's fixed content geometry.
    fn update(&mut self, state: WidgetUpdate, event: WidgetEvent, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
        let (event, text) = decode_widget_event(event, text)?;
        self.probe.latest.set(Some(state));
        self.probe.updates.set(self.probe.updates.get() + 1);
        self.probe.events.borrow_mut().push(event);
        if matches!(event, WidgetEvent::Text) {
            self.probe.texts.borrow_mut().push(text.to_owned());
        }
        let width = self.probe.requested_width.replace(0);
        if width > 0 && self.probe.width.replace(width) != width {
            Ok(GeometryChange::Changed)
        } else {
            Ok(GeometryChange::NoChange)
        }
    }

    /// Setter changes stay sticky even when this custom widget is hidden.
    fn take_change(&mut self) -> Result<GeometryChange, NcStatus> {
        self.button.take_change()
    }
    /// Retains authority and probes old tokens while a later scope is active.
    fn paint(&mut self, scope: PaintScope, state: WidgetPaint, mut painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
        assert_eq!(self.runtime.update(SIZE), Err(UI_E_BUSY));
        assert_eq!(self.button.set_label(NcCopyFromBufferOffset::new(0, b"forbidden")), Err(UI_E_BUSY));
        // Even no-op flag edits must respect the host's observational phase.
        let before = self.button.properties()?;
        assert_eq!(self.button.replace_flags(before.options), Err(UI_E_BUSY));
        assert_eq!(self.button.set_flags(WidgetOptions::NONE), Err(UI_E_BUSY));
        assert_eq!(self.button.clear_flags(WidgetOptions::NONE), Err(UI_E_BUSY));
        assert_eq!(self.button.properties()?, before);
        for (old, expired) in self.probe.painters.borrow_mut().iter_mut() {
            assert_eq!(old.fill(*expired, state.rect, 0), Err(UI_E_EXPIRED_SCOPE));
            assert_eq!(old.triangles(*expired, NcCopyFromBufferOffset::new(0, &[])), Err(UI_E_EXPIRED_SCOPE));
            assert_eq!(old.colored_triangles(*expired, NcCopyFromBufferOffset::new(0, &[])), Err(UI_E_EXPIRED_SCOPE));
            assert_eq!(
                old.stroke_line(*expired, PaintPoint::default(), PaintPoint::default(), 1.0, 0),
                Err(UI_E_EXPIRED_SCOPE)
            );
            assert_eq!(old.fill_polygon(*expired, NcCopyFromBufferOffset::new(0, &[]), 0), Err(UI_E_EXPIRED_SCOPE));
        }
        // Malformed bulk batches fail transactionally before a valid command follows.
        assert_eq!(painter.triangles(scope, NcCopyFromBufferOffset::new(1, &[])), Err(NC_E_INVALID_ARG));
        assert_eq!(painter.colored_triangles(scope, NcCopyFromBufferOffset::new(1, &[])), Err(NC_E_INVALID_ARG));
        assert_eq!(
            painter.colored_triangles(scope, NcCopyFromBufferOffset::new(0, &[ColoredVertex::default()])),
            Err(NC_E_INVALID_ARG)
        );
        let image = crate::test_support::test_image(1, 1);
        for source in [
            TextureRegion { u0: f32::NAN, ..TextureRegion::FULL },
            TextureRegion { u1: 2., ..TextureRegion::FULL },
            TextureRegion {
                u0: 0.75,
                u1: 0.25,
                ..TextureRegion::FULL
            },
        ] {
            assert_eq!(
                painter.texture(scope, state.rect, source, u32::MAX, image.as_interface()),
                Err(NC_E_INVALID_ARG)
            );
        }
        let malformed_solid = [ColoredVertex { x: f32::NAN, y: 0.0, rgba: 0 }; 3];
        assert_eq!(
            painter.colored_triangles(scope, NcCopyFromBufferOffset::new(0, &malformed_solid)),
            Err(NC_E_INVALID_ARG)
        );
        assert_eq!(
            painter.stroke_line(scope, PaintPoint { x: f32::INFINITY, y: 0.0 }, PaintPoint::default(), 1.0, 0),
            Err(NC_E_INVALID_ARG)
        );
        assert_eq!(
            painter.stroke_line(scope, PaintPoint::default(), PaintPoint::default(), -1.0, 0),
            Err(NC_E_INVALID_ARG)
        );
        assert_eq!(painter.fill_polygon(scope, NcCopyFromBufferOffset::new(1, &[]), 0), Err(NC_E_INVALID_ARG));
        assert_eq!(
            painter.fill_polygon(scope, NcCopyFromBufferOffset::new(0, &[PaintPoint { x: 0.0, y: f32::NAN }]), 0),
            Err(NC_E_INVALID_ARG)
        );
        let invalid_clip = PaintScope {
            invocation: scope.invocation,
            clip: Rect { x: 0, y: 0, width: -1, height: 1 },
        };
        assert_eq!(painter.fill(invalid_clip, state.rect, 0), Err(NC_E_INVALID_ARG));
        assert_eq!(
            painter.triangles(scope, NcCopyFromBufferOffset::new(0, &[RenderVertex::default()])),
            Err(NC_E_INVALID_ARG)
        );
        let malformed = [RenderVertex { x: f32::NAN, ..RenderVertex::default() }; 3];
        assert_eq!(painter.triangles(scope, NcCopyFromBufferOffset::new(0, &malformed)), Err(NC_E_INVALID_ARG));
        painter.fill(scope, state.rect, 0xff00ff00)?;
        self.probe.painters.borrow_mut().push((painter.try_add_ref()?, scope));
        if self.probe.fail_paint.get() { Err(NC_E_INVALID_ARG) } else { Ok(()) }
    }
}

/// Retained scope authority expires, reentry is rejected, and paint failure never reaches the GPU.
#[test]
fn external_widget_cannot_reenter_mutate_during_observation_or_reuse_a_paint_scope() {
    let runtime = runtime();
    let button = button(&runtime);
    let probe = Rc::new(WidgetProbe::default());
    let widget = RetainingWidget::into_box(RetainingWidget {
        button,
        runtime: runtime.clone(),
        probe: probe.clone(),
    });
    let window = mount(&runtime, widget.query().unwrap()).unwrap();
    let (renderer, backend) = renderer();
    runtime.update(SIZE).unwrap();
    let measured = probe.measures.get();
    runtime.update(SIZE).unwrap();
    assert_eq!(probe.measures.get(), measured);
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    probe.fail_paint.set(true);
    assert_eq!(runtime.render(SIZE, renderer.as_com_ref()), Err(NC_E_INVALID_ARG));
    assert_eq!(backend.calls.get(), 2);
    window.close().unwrap();
    runtime.close().unwrap();
    for (painter, scope) in probe.painters.borrow_mut().iter_mut() {
        assert_eq!(painter.fill(*scope, RECT, 0), Err(UI_E_EXPIRED_SCOPE));
    }
}

/// Concrete window projections preserve modal, structural visibility and fixed-layer policy.
#[test]
fn window_com_capabilities_preserve_hierarchy_layers_visibility_and_subtree_closure() {
    let runtime = runtime();
    let manager: ComPtr<IWindowManagerAbi> = runtime.query().unwrap();
    // Runtime protocols share one identity; the callable factory is a separately constructible service.
    let identity: ComPtr<IUnknown> = runtime.query().unwrap();
    assert_eq!(identity.as_raw().object, manager.query::<IUnknown>().unwrap().as_raw().object);
    assert_eq!(runtime.query::<UiFactoryAbi>().unwrap_err(), neocogi_com::NC_E_NOINTERFACE);
    let construction = runtime.construction_host().unwrap();
    assert_ne!(identity.as_raw().object, construction.query::<IUnknown>().unwrap().as_raw().object);
    let root_button_owner = button(&runtime);
    let root_button = root_button_owner.downgrade();
    assert_eq!(
        construction.host().unwrap().query::<IUnknown>().unwrap().as_raw().object,
        root_button.host().unwrap().query::<IUnknown>().unwrap().as_raw().object
    );
    let root = mount(&runtime, root_button_owner.query().unwrap()).unwrap();
    let child_button_owner = button(&runtime);
    let child_button = child_button_owner.downgrade();
    let child = manager
        .create_child(
            root.as_com_ref(),
            NcCopyFromBufferOffset::new(0, b"child"),
            RECT,
            child_button_owner.query::<IWidgetAbi>().unwrap(),
            neocogi_com::NcValueRef::new(&neocogi_com::events::EventBinding::Unobserved).unwrap(),
        )
        .unwrap();
    let dialog_button_owner = button(&runtime);
    let dialog_button = dialog_button_owner.downgrade();
    let dialog = manager
        .create_dialog(
            child.as_com_ref(),
            NcCopyFromBufferOffset::new(0, b"dialog"),
            RECT,
            dialog_button_owner.query::<IWidgetAbi>().unwrap(),
            neocogi_com::NcValueRef::new(&neocogi_com::events::EventBinding::Unobserved).unwrap(),
        )
        .unwrap();
    root.set_layer(2).unwrap();
    assert_eq!(child.snapshot().unwrap().layer.number, 2);
    assert_eq!(child.set_layer(4), Err(UI_E_POLICY));
    assert_eq!(dialog.snapshot().unwrap().layer.kind, LayerKind::Modal);
    assert_eq!(dialog.snapshot().unwrap().visibility, Visibility::Hidden);
    dialog.set_visibility(Visibility::Visible).unwrap();
    root.set_visibility(Visibility::Hidden).unwrap();
    assert_eq!(child.snapshot().unwrap().visibility, Visibility::Hidden);
    root.set_visibility(Visibility::Visible).unwrap();
    assert_eq!(child.snapshot().unwrap().visibility, Visibility::Visible);
    // Hiding an owner closes modal visibility intent; showing it must not resurrect a dialog.
    assert_eq!(dialog.snapshot().unwrap().visibility, Visibility::Hidden);
    root.set_child_clip(ChildClip::Content).unwrap();
    assert_eq!(root.snapshot().unwrap().child_clip, ChildClip::Content);
    let before = root.snapshot().unwrap().options.raw();
    assert_eq!(root.set_options(WindowOptions::from_raw(32)), Err(neocogi_com::NC_E_INVALID_TAG));
    assert_eq!(root.snapshot().unwrap().options.raw(), before);
    assert_eq!(root.set_layer(16), Err(NC_E_INVALID_ARG));
    assert_eq!(root.snapshot().unwrap().layer.number, 2);
    root.set_title(NcCopyFromBufferOffset::new(0, b"new title")).unwrap();
    assert_eq!(root.title().unwrap().to_string().unwrap(), "new title");
    let oversized_title = vec![b'x'; MAX_WINDOW_TITLE_BYTES + 1];
    assert_eq!(
        root.set_title(NcCopyFromBufferOffset::new(0, &oversized_title)),
        Err(neocogi_com::NC_E_OUT_OF_RANGE)
    );
    assert_eq!(root.title().unwrap().to_string().unwrap(), "new title");
    runtime.update(SIZE).unwrap();
    root.close().unwrap();
    for mut button in [root_button, child_button, dialog_button] {
        assert_eq!(button.set_label(NcCopyFromBufferOffset::new(0, b"closed")), Err(neocogi_com::NC_E_DROPPED));
    }
    assert_eq!(child.snapshot().unwrap_err(), UI_E_CLOSED);
    assert_eq!(dialog.set_visibility(Visibility::Visible), Err(UI_E_CLOSED));
    // Parent destruction has already removed these definitions; close remains idempotent.
    child.close().unwrap();
    dialog.close().unwrap();
    runtime.update(SIZE).unwrap();
}

/// A rejected parent/policy must leave the child available for a legitimate later attachment.
#[test]
fn child_creation_rejects_foreign_and_modal_parents_before_mounting() {
    let first = runtime();
    let second = runtime();
    let manager: ComPtr<IWindowManagerAbi> = first.query().unwrap();
    let root = mount(&first, button(&first).query().unwrap()).unwrap();
    let foreign = mount(&second, button(&second).query().unwrap()).unwrap();
    let content = button(&first);
    let rejected = content.downgrade();
    let node = content.query::<IWidgetAbi>().unwrap();
    assert_eq!(
        manager
            .create_child(
                foreign.as_com_ref(),
                NcCopyFromBufferOffset::new(0, b"foreign"),
                RECT,
                node,
                neocogi_com::NcValueRef::new(&neocogi_com::events::EventBinding::Unobserved).unwrap(),
            )
            .unwrap_err()
            .0,
        UI_E_OWNER
    );
    assert!(!rejected.is_alive());
    let dialog = manager
        .create_dialog(
            root.as_com_ref(),
            NcCopyFromBufferOffset::new(0, b"modal"),
            RECT,
            button(&first).query::<IWidgetAbi>().unwrap(),
            neocogi_com::NcValueRef::new(&neocogi_com::events::EventBinding::Unobserved).unwrap(),
        )
        .unwrap();
    assert_eq!(
        manager
            .create_child(
                dialog.as_com_ref(),
                NcCopyFromBufferOffset::new(0, b"invalid"),
                RECT,
                button(&first).query().unwrap(),
                neocogi_com::NcValueRef::new(&neocogi_com::events::EventBinding::Unobserved).unwrap(),
            )
            .unwrap_err()
            .0,
        UI_E_POLICY
    );
    let valid = manager
        .create_child(
            root.as_com_ref(),
            NcCopyFromBufferOffset::new(0, b"valid"),
            RECT,
            button(&first).query().unwrap(),
            neocogi_com::NcValueRef::new(&neocogi_com::events::EventBinding::Unobserved).unwrap(),
        )
        .unwrap();
    assert_eq!(valid.snapshot().unwrap().visibility, Visibility::Visible);
    first.close().unwrap();
    second.close().unwrap();
}

/// Popups remain independent typed capabilities while obeying their owner's active/modal policy.
#[test]
fn popup_com_capability_enforces_active_owner_policy_and_permanent_removal() {
    let runtime = runtime();
    let manager: ComPtr<IWindowManagerAbi> = runtime.query().unwrap();
    let root = mount(&runtime, button(&runtime).query().unwrap()).unwrap();
    let popup_button_owner = button(&runtime);
    let popup_button = popup_button_owner.downgrade();
    let mut popup = manager
        .create_popup(
            root.as_com_ref(),
            NcCopyFromBufferOffset::new(0, b"popup"),
            popup_button_owner.query::<IWidgetAbi>().unwrap(),
            neocogi_com::NcValueRef::new(&neocogi_com::events::EventBinding::Unobserved).unwrap(),
        )
        .unwrap();
    assert_eq!(popup.query::<IWindowAbi>().unwrap_err(), neocogi_com::NC_E_NOINTERFACE);
    assert_eq!(popup.visibility().unwrap(), Visibility::Hidden);
    runtime.update(SIZE).unwrap();
    popup.show_at(RECT).unwrap();
    assert_eq!(popup.visibility().unwrap(), Visibility::Visible);
    popup.hide().unwrap();
    assert_eq!(popup.visibility().unwrap(), Visibility::Hidden);
    let dialog = manager
        .create_dialog(
            root.as_com_ref(),
            NcCopyFromBufferOffset::new(0, b"modal"),
            RECT,
            button(&runtime).query::<IWidgetAbi>().unwrap(),
            neocogi_com::NcValueRef::new(&neocogi_com::events::EventBinding::Unobserved).unwrap(),
        )
        .unwrap();
    dialog.set_visibility(Visibility::Visible).unwrap();
    assert_eq!(popup.show_at(RECT), Err(UI_E_POLICY));
    dialog.set_visibility(Visibility::Hidden).unwrap();
    popup.show_at(RECT).unwrap();
    popup.close().unwrap();
    popup.close().unwrap();
    assert_eq!(popup.visibility(), Err(UI_E_CLOSED));
    assert_eq!(popup_button.label().unwrap_err(), neocogi_com::NC_E_DROPPED);
    runtime.update(SIZE).unwrap();
    let (renderer, _) = renderer();
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    runtime.close().unwrap();
}

/// Eventless geometry changes, queued text, dynamic options and hidden setters share one path.
#[test]
fn com_nodes_update_without_activation_and_collect_hidden_widget_changes() {
    let runtime = runtime();
    let button = button(&runtime);
    let probe = Rc::new(WidgetProbe::default());
    probe.text_input.set(true);
    let observed = button.downgrade();
    let object = RetainingWidget::into_box(RetainingWidget {
        button,
        runtime: runtime.clone(),
        probe: probe.clone(),
    });
    let mut button = observed;
    let window = mount(&runtime, object.query().unwrap()).unwrap();
    runtime.update(SIZE).unwrap();
    let measured = probe.measures.get();
    probe.requested_width.set(240);
    runtime.update(SIZE).unwrap();
    assert!(probe.measures.get() > measured);
    assert_eq!(probe.width.get(), 240);

    let tab = UiInput::Key(KeyStroke {
        key: KeyCode::Tab,
        value: 0,
        pressed: true,
        repeat: false,
        modifiers: 0,
    });
    runtime.input(tab.into(), NcCopyFromBufferOffset::new(0, &[])).unwrap();
    runtime.update(SIZE).unwrap();
    let measured = probe.measures.get();
    runtime.input(UiInput::Text, NcCopyFromBufferOffset::new(0, "hello ☃".as_bytes())).unwrap();
    let (renderer, _) = renderer();
    assert_eq!(runtime.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    runtime.update(SIZE).unwrap();
    assert_eq!(&*probe.texts.borrow(), &["hello ☃"]);
    assert_eq!(probe.measures.get(), measured, "NoChange text processing must preserve measurement");

    let before = button.properties().unwrap().options;
    assert_eq!(button.replace_flags(WidgetOptions::from_raw(8)), Err(neocogi_com::NC_E_INVALID_TAG));
    assert_eq!(button.properties().unwrap().options, before);
    window.set_visibility(Visibility::Hidden).unwrap();
    runtime.update(SIZE).unwrap();
    let updates = probe.updates.get();
    let measured = probe.measures.get();
    button.replace_flags(WidgetOptions::NONE).unwrap();
    runtime.update(SIZE).unwrap();
    assert_eq!(probe.updates.get(), updates, "hidden widgets skip update but retain setter invalidation");
    window.set_visibility(Visibility::Visible).unwrap();
    runtime.update(SIZE).unwrap();
    assert!(probe.measures.get() > measured);
    window.close().unwrap();
    assert_eq!(probe.closes.get(), 1);
    runtime.close().unwrap();
    assert_eq!(probe.closes.get(), 1);
}

/// Malformed input changes no queue state, and a component's teardown error reaches the caller.
#[test]
fn input_validation_and_explicit_teardown_preserve_their_error_boundaries() {
    let runtime = runtime();
    let button = button(&runtime);
    let probe = Rc::new(WidgetProbe::default());
    let object = RetainingWidget::into_box(RetainingWidget {
        button,
        runtime: runtime.clone(),
        probe: probe.clone(),
    });
    let window = mount(&runtime, object.query().unwrap()).unwrap();
    runtime.update(SIZE).unwrap();
    let bad_key = UiInput::Key(KeyStroke {
        key: KeyCode::Character,
        value: 0xd800,
        pressed: true,
        repeat: false,
        modifiers: 0,
    });
    assert_eq!(runtime.input(bad_key.into(), NcCopyFromBufferOffset::new(0, &[])), Err(NC_E_INVALID_ARG));
    assert_eq!(runtime.input(UiInput::Text, NcCopyFromBufferOffset::new(1, b"bad")), Err(NC_E_INVALID_ARG));
    assert_eq!(runtime.input(UiInput::Text, NcCopyFromBufferOffset::new(0, &[0xff])), Err(NC_E_INVALID_ARG));
    assert_eq!(
        runtime.input(UiInput::Cancel, NcCopyFromBufferOffset::new(0, b"unexpected")),
        Err(NC_E_INVALID_ARG)
    );
    let (renderer, _) = renderer();
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    window.close().unwrap();
    assert_eq!(probe.closes.get(), 1);
    window.close().unwrap();
    assert_eq!(probe.closes.get(), 1);
    runtime.update(SIZE).unwrap();
}

/// A failed measure never commits a zero-size cache entry or suppresses the retry.
#[test]
fn failed_com_measurement_retries_after_component_recovery() {
    let runtime = runtime();
    let button = button(&runtime);
    let probe = Rc::new(WidgetProbe::default());
    probe.fail_measure.set(true);
    let object = RetainingWidget::into_box(RetainingWidget {
        button,
        runtime: runtime.clone(),
        probe: probe.clone(),
    });
    let window = mount(&runtime, object.query().unwrap()).unwrap();
    assert_eq!(runtime.update(SIZE), Err(NC_E_INVALID_ARG));
    let failed = probe.measures.get();
    assert!(failed > 0);
    probe.fail_measure.set(false);
    runtime.update(SIZE).unwrap();
    assert!(probe.measures.get() > failed, "component recovery must call measure again");
    let (renderer, _) = renderer();
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    window.close().unwrap();
}

/// Invalid edits leave mount claims intact; the public view cannot recover private edit authority.
#[test]
fn child_editor_rejects_inputs_without_leaking_owners_and_expires_retained_views() {
    let runtime = runtime();
    let other = self::runtime();
    let editor = button(&runtime).host().unwrap().create_children().unwrap();
    let view = editor.view().unwrap();
    assert_eq!(view.query::<IChildrenEditorAbi>().unwrap_err(), neocogi_com::NC_E_NOINTERFACE);
    let invalid = button(&runtime);
    let rejected = invalid.downgrade();
    assert_eq!(
        editor.insert(1, invalid.query().unwrap()).map_err(|(status, _rejected)| status),
        Err(neocogi_com::NC_E_OUT_OF_RANGE)
    );
    assert!(!rejected.is_alive());
    let foreign = button(&other);
    let rejected = foreign.downgrade();
    assert_eq!(
        editor.insert(0, foreign.query().unwrap()).map_err(|(status, _rejected)| status),
        Err(UI_E_OWNER)
    );
    assert!(!rejected.is_alive());
    editor.insert(0, button(&runtime).query().unwrap()).unwrap();
    let held_child = view.child(0).unwrap();
    let replacement = button(&runtime);
    let observed = replacement.downgrade();
    editor.replace(0, replacement.query().unwrap()).unwrap();
    assert_eq!(held_child.properties(), Err(neocogi_com::NC_E_DROPPED));
    assert_eq!(view.child_count().unwrap(), 1);
    drop(editor);
    assert_eq!(view.child_count(), Err(UI_E_CLOSED));
    assert_eq!(observed.label().unwrap_err(), neocogi_com::NC_E_DROPPED);
}

/// A failed old-child close reports a committed replacement, so matching metadata can still commit.
#[test]
fn child_edit_reports_committed_topology_separately_from_teardown_failure() {
    let runtime = runtime();
    let button = button(&runtime);
    let editor = button.host().unwrap().create_children().unwrap();
    let view = editor.view().unwrap();
    let probe = Rc::new(WidgetProbe::default());
    let object = RetainingWidget::into_box(RetainingWidget {
        button,
        runtime: runtime.clone(),
        probe: probe.clone(),
    });
    editor.insert(0, object.query::<IWidgetAbi>().unwrap()).unwrap();
    let replacement = self::button(&runtime);
    let observed = replacement.downgrade();
    editor.replace(0, replacement.query::<IWidgetAbi>().unwrap()).unwrap();
    let replacement = observed;
    assert_eq!(probe.closes.get(), 1);
    assert_eq!(view.child_count().unwrap(), 1);
    assert_eq!(
        view.child(0).unwrap().query::<IUnknown>().unwrap().as_raw().object,
        replacement.query::<IUnknown>().unwrap().as_raw().object
    );
    editor.clear().unwrap();
    assert_eq!(view.child_count().unwrap(), 0);
    assert_eq!(replacement.label().unwrap_err(), neocogi_com::NC_E_DROPPED);
}

/// The static facade observes one inherited state, normalized text and honest setter markers.
#[test]
fn builtin_facade_editing_preserves_normalization_geometry_and_resource_validation() {
    let runtime = runtime();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let mut checkbox = ui.widgets().checkbox_unobserved("Check", false).unwrap();
    let mut common = checkbox.node_interface().unwrap();
    common.take_change().unwrap();
    checkbox.set_checked(true).unwrap();
    assert!(checkbox.checked().unwrap());
    assert_eq!(common.take_change().unwrap(), GeometryChange::NoChange);
    checkbox.set_label("A longer checkbox").unwrap();
    assert_eq!(common.take_change().unwrap(), GeometryChange::Changed);
    assert_eq!(checkbox.label().unwrap(), "A longer checkbox");
    checkbox.set_font_role(FontRole::Body).unwrap();
    assert_eq!(common.take_change().unwrap(), GeometryChange::NoChange);
    assert_eq!(checkbox.set_named_font("missing"), Err(neocogi_com::NC_E_NOT_FOUND));
    assert_eq!(common.take_change().unwrap(), GeometryChange::NoChange);
    checkbox.set_named_font("body").unwrap();
    assert_eq!(common.take_change().unwrap(), GeometryChange::Changed);

    let mut textbox = ui.widgets().textbox_unobserved("é\r\n☃").unwrap();
    assert_eq!(textbox.text().unwrap(), "é☃");
    assert_eq!(textbox.cursor().unwrap(), 5);
    textbox.set_cursor(4).unwrap();
    assert_eq!(textbox.cursor().unwrap(), 2);
    let mut common = textbox.node_interface().unwrap();
    let concrete = textbox.as_interface().downgrade();
    assert_eq!(
        concrete.query::<IUnknown>().unwrap().as_raw().object,
        common.query::<IUnknown>().unwrap().as_raw().object
    );
    assert_eq!(common.query::<ITextEditorAbi>().unwrap_err(), neocogi_com::NC_E_NOINTERFACE);
    assert_eq!(common.query::<ITextContentAbi>().unwrap_err(), neocogi_com::NC_E_NOINTERFACE);
    common.take_change().unwrap();
    textbox.set_text("é\n☃").unwrap();
    assert_eq!(textbox.cursor().unwrap(), 5);
    assert_eq!(common.take_change().unwrap(), GeometryChange::NoChange);
    assert_eq!(
        textbox.as_interface().downgrade().set_text(NcCopyFromBufferOffset::new(0, &[0xff])),
        Err(NC_E_INVALID_ARG)
    );
    assert_eq!(textbox.text().unwrap(), "é☃");
    textbox.clear().unwrap();
    assert_eq!(textbox.cursor().unwrap(), 0);
    assert_eq!(common.take_change().unwrap(), GeometryChange::Changed);

    let mut block = ui.widgets().text_block("one\r\ntwo", TextWrapping::None).unwrap();
    assert_eq!(block.text().unwrap(), "one\ntwo");
    let mut common = block.node_interface().unwrap();
    common.take_change().unwrap();
    block.set_wrapping(TextWrapping::Word).unwrap();
    assert_eq!(block.wrapping().unwrap(), TextWrapping::Word);
    assert_eq!(common.take_change().unwrap(), GeometryChange::Changed);
    // The additive policy must cross the callable boundary and invalidate layout.
    block.set_wrapping(TextWrapping::WordOrCharacter).unwrap();
    assert_eq!(block.wrapping().unwrap(), TextWrapping::WordOrCharacter);
    assert_eq!(common.take_change().unwrap(), GeometryChange::Changed);
    assert_eq!(block.set_wrapping(TextWrapping::from_raw(99)), Err(neocogi_com::NC_E_INVALID_TAG));
    assert_eq!(common.take_change().unwrap(), GeometryChange::NoChange);
    runtime.close().unwrap();
    // These unmounted widgets still have owners; only their runtime service closed.
    assert_eq!(textbox.text(), Err(UI_E_CLOSED));
    assert_eq!(checkbox.set_checked(false), Err(UI_E_CLOSED));
}

/// Numeric/range setters keep validation atomic and distinguish paint-only values.
#[test]
fn builtin_numeric_and_scrollbar_facades_reject_invalid_inputs_without_losing_state() {
    let runtime = runtime();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime).unwrap();
    let mut slider = ui
        .widgets()
        .slider_unobserved(SliderOptions {
            value: 5.0,
            low: 0.0,
            high: 10.0,
            step: 1.0,
            precision: 1,
        })
        .unwrap();
    let mut common = slider.node_interface().unwrap();
    common.take_change().unwrap();
    assert_eq!(slider.set_value(f32::NAN), Err(NC_E_INVALID_ARG));
    assert_eq!(slider.value().unwrap(), 5.0);
    assert_eq!(common.take_change().unwrap(), GeometryChange::NoChange);
    slider.set_value(200.0).unwrap();
    assert_eq!(slider.value().unwrap(), 10.0);
    assert_eq!(common.take_change().unwrap(), GeometryChange::Changed);
    assert!(
        ui.widgets()
            .slider_unobserved(SliderOptions {
                value: 0.0,
                low: 2.0,
                high: 1.0,
                step: 0.0,
                precision: 0
            })
            .is_err()
    );
    assert!(ui.widgets().number_unobserved(NumberOptions { value: 0.0, step: 1.0, precision: 255 }).is_err());
    let mut number = ui.widgets().number_unobserved(NumberOptions { value: 4.0, step: 0.5, precision: 2 }).unwrap();
    number.set_value(-100.0).unwrap();
    assert_eq!(number.value().unwrap(), -100.0);
    assert!(!number.is_editing().unwrap());

    let mut scrollbar = ui
        .widgets()
        .scrollbar_unobserved(ScrollRange {
            axis: ScrollAxis::Vertical,
            viewport: 50,
            content: 100,
            offset: 80,
        })
        .unwrap();
    assert_eq!(scrollbar.range().unwrap().offset, 50);
    let mut common = scrollbar.node_interface().unwrap();
    common.take_change().unwrap();
    assert_eq!(scrollbar.set_lengths(-1, 100), Err(NC_E_INVALID_ARG));
    assert_eq!(scrollbar.range().unwrap().viewport, 50);
    scrollbar.set_lengths(90, 100).unwrap();
    assert_eq!(scrollbar.range().unwrap().offset, 10);
    scrollbar.set_offset(i32::MAX).unwrap();
    assert_eq!(scrollbar.range().unwrap().offset, 10);
    scrollbar.set_offset(-1).unwrap();
    assert_eq!(scrollbar.range().unwrap().offset, 0);
    // Range changes also change effective routing policy and conservatively invalidate geometry.
    assert_eq!(common.take_change().unwrap(), GeometryChange::Changed);
    scrollbar.set_lengths(100, 100).unwrap();
    assert!(common.properties().unwrap().options.contains(WidgetOptions::NO_INTERACT));
    let mut swatch = ui.widgets().color_swatch(0xff102030, "RGBA").unwrap();
    let mut common = swatch.node_interface().unwrap();
    common.take_change().unwrap();
    swatch.set_fill(0xffaabbcc).unwrap();
    assert_eq!(swatch.fill().unwrap(), 0xffaabbcc);
    assert_eq!(common.take_change().unwrap(), GeometryChange::NoChange);
}

/// Real routing and complete painting exercise all migrated classes through the facade.
#[test]
fn builtin_facade_routes_text_and_space_and_renders_every_concrete_leaf() {
    let runtime = runtime();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let textbox_owner = ui.widgets().textbox_unobserved("start").unwrap();
    let textbox = textbox_owner.handle();
    let window = ui.window("editor", RECT, textbox_owner).unwrap();
    ui.input(
        UiInput::Key(KeyStroke {
            key: KeyCode::Tab,
            value: 0,
            pressed: true,
            repeat: false,
            modifiers: 0,
        }),
        "",
    )
    .unwrap();
    ui.input(UiInput::Text, "é\n☃").unwrap();
    ui.update(SIZE).unwrap();
    assert_eq!(textbox.text().unwrap(), "starté☃");
    assert_eq!(textbox.cursor().unwrap(), 10);
    let (renderer, _) = renderer();
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    window.close().unwrap();

    let checkbox_owner = ui.widgets().checkbox_unobserved("toggle", false).unwrap();
    let checkbox = checkbox_owner.handle();
    let window = ui.window("checkbox", RECT, checkbox_owner).unwrap();
    ui.input(UiInput::Cancel, "").unwrap();
    ui.input(
        UiInput::Key(KeyStroke {
            key: KeyCode::Tab,
            value: 0,
            pressed: true,
            repeat: false,
            modifiers: 0,
        }),
        "",
    )
    .unwrap();
    ui.input(
        UiInput::Key(KeyStroke {
            key: KeyCode::Space,
            value: 0,
            pressed: true,
            repeat: false,
            modifiers: 0,
        }),
        "",
    )
    .unwrap();
    ui.update(SIZE).unwrap();
    assert!(checkbox.checked().unwrap());
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    window.close().unwrap();

    let roots = [
        ui.widgets()
            .button_unobserved("button")
            .unwrap()
            .into_interface()
            .query::<IWidgetAbi>()
            .unwrap(),
        ui.widgets()
            .text_block("display\ntext", TextWrapping::Word)
            .unwrap()
            .into_interface()
            .query::<IWidgetAbi>()
            .unwrap(),
        ui.widgets()
            .slider_unobserved(SliderOptions {
                value: 2.0,
                low: 0.0,
                high: 10.0,
                step: 1.0,
                precision: 0,
            })
            .unwrap()
            .into_interface()
            .query::<IWidgetAbi>()
            .unwrap(),
        ui.widgets()
            .number_unobserved(NumberOptions { value: 42.0, step: 1.0, precision: 0 })
            .unwrap()
            .into_interface()
            .query::<IWidgetAbi>()
            .unwrap(),
        ui.widgets()
            .color_swatch(0xff00ffff, "swatch")
            .unwrap()
            .into_interface()
            .query::<IWidgetAbi>()
            .unwrap(),
        ui.widgets()
            .list_item_unobserved("item")
            .unwrap()
            .into_interface()
            .query::<IWidgetAbi>()
            .unwrap(),
        ui.widgets()
            .list_box_unobserved("list box")
            .unwrap()
            .into_interface()
            .query::<IWidgetAbi>()
            .unwrap(),
        ui.widgets()
            .scrollbar_unobserved(ScrollRange {
                axis: ScrollAxis::Vertical,
                viewport: 40,
                content: 120,
                offset: 30,
            })
            .unwrap()
            .into_interface()
            .query::<IWidgetAbi>()
            .unwrap(),
    ];
    for root in roots {
        let observed = root.downgrade();
        let window = mount(&runtime, root).unwrap();
        ui.update(SIZE).unwrap();
        ui.render(SIZE, renderer.as_com_ref()).unwrap();
        window.close().unwrap();
        assert_eq!(observed.properties(), Err(neocogi_com::NC_E_DROPPED));
    }
}

/// Copied paint observations survive the deliberately failing recorder's call.
#[derive(Default)]
struct BuiltinPaintProbe {
    /// Semantic image rectangles, source coordinates and tints copied by the ordered boundary.
    images: RefCell<Vec<(Rect, TextureRegion, u32)>>,
    /// False marks semantic atlas work; true marks an image barrier.
    order: RefCell<Vec<bool>>,
    /// Injects a failure specifically after the preceding text batch succeeded.
    fail_image: Cell<bool>,
    /// Exact local text origins received before the runtime performs its one expansion pass.
    text_positions: RefCell<Vec<Point>>,
    /// One recoverable semantic-paint failure, used to verify later calls remain usable.
    fail: Cell<bool>,
}

#[com_object(interfaces(IPainter))]
/// A foreign-style painter that reenters a read-only editing method during copying.
struct BuiltinPainter {
    /// The producer's editing projection, independent of its paint helper's storage.
    button: WeakComBox<ButtonAbi>,
    /// Test-owned geometry and error observations.
    probe: Rc<BuiltinPaintProbe>,
}
impl IPainter for BuiltinPainter {
    /// Built-ins no longer resolve lines through this primitive.
    fn stroke_line(&mut self, _: PaintScope, _: PaintPoint, _: PaintPoint, _: f32, _: u32) -> Result<(), NcStatus> {
        panic!("expected atlas triangles")
    }
    /// Built-ins no longer resolve polygons through this primitive.
    fn fill_polygon(&mut self, _: PaintScope, _: NcCopyFromBufferOffset<'_, PaintPoint>, _: u32) -> Result<(), NcStatus> {
        panic!("expected atlas triangles")
    }

    /// Built-in semantic painting does not route through caller-authored colored geometry.
    fn colored_triangles(&mut self, _scope: PaintScope, _vertices: NcCopyFromBufferOffset<'_, ColoredVertex>) -> Result<(), NcStatus> {
        panic!("expected resolved built-in atlas geometry")
    }

    /// Captures the ordered image barrier after verifying that paint holds exclusive object access.
    fn texture(&mut self, _scope: PaintScope, _rect: Rect, _source: TextureRegion, _rgba: u32, _texture: ComRef<'_, ITextureAbi>) -> Result<(), NcStatus> {
        assert_eq!(self.button.label().err(), Some(NC_E_BUSY));
        self.probe.images.borrow_mut().push((_rect, _source, _rgba));
        self.probe.order.borrow_mut().push(true);
        if self.probe.fail_image.replace(false) { Err(UI_E_RENDER) } else { Ok(()) }
    }
    /// The complete button algorithm cannot emit application GPU work.
    fn item(&mut self, _scope: PaintScope, _rect: Rect, _item: ComRef<'_, IRenderItemAbi>) -> Result<(), NcStatus> {
        panic!("unexpected custom item in button paint")
    }
    /// Built-ins leave atlas expansion to the runtime instead of sending final triangle batches.
    fn triangles(&mut self, _scope: PaintScope, _vertices: NcCopyFromBufferOffset<'_, RenderVertex>) -> Result<(), NcStatus> {
        panic!("unexpected final triangle batch in semantic built-in paint")
    }
    /// Built-in painting uses the bulk path after resolving its complete theme state.
    fn fill(&mut self, _scope: PaintScope, _rect: Rect, _rgba: u32) -> Result<(), NcStatus> {
        panic!("expected one complete bulk batch")
    }
    /// Text expansion stays with the UI implementation and its immutable atlas.
    fn text(&mut self, _scope: PaintScope, _rect: Rect, _text: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        panic!("expected UI-owned text expansion")
    }
    /// Captures the compact semantic frame before the runtime expands its nine-patch.
    fn frame(&mut self, _scope: PaintScope, rect: Rect, _role: WidgetFrame, _state: WidgetVisualState, _center_only: bool) -> Result<Rect, NcStatus> {
        assert_eq!(self.button.label().err(), Some(NC_E_BUSY));
        Ok(rect)
    }
    /// This fixture paints text-only buttons, so an icon would violate its setup.
    fn icon(&mut self, _scope: PaintScope, _icon: PaintIcon, _rect: Rect, _rgba: u32) -> Result<(), NcStatus> {
        panic!("unexpected icon in text-only button paint")
    }
    /// Captures aligned text before the single runtime executor performs glyph expansion.
    fn text_run(&mut self, _scope: PaintScope, _font: PaintFont, position: Point, _rgba: u32, _text: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        assert_eq!(self.button.label().err(), Some(NC_E_BUSY));
        self.probe.text_positions.borrow_mut().push(position);
        self.probe.order.borrow_mut().push(false);
        if self.probe.fail.replace(false) { Err(UI_E_RENDER) } else { Ok(()) }
    }
}

/// Button style changes retain the full algorithm and failed semantic recording does not poison reuse.
#[test]
fn builtin_scoped_paint_preserves_alignment_with_inline_semantic_state() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let mut button = ui.widgets().button_unobserved("AB").unwrap();
    button.replace_flags(WidgetOptions::NONE).unwrap();
    button.set_fill_states(FillStates::NONE).unwrap();
    let mut common = button.node_interface().unwrap();
    common.take_change().unwrap();
    assert_eq!(button.set_fill_states(FillStates::from_raw(8)), Err(neocogi_com::NC_E_INVALID_TAG));
    assert_eq!(button.fill_states().unwrap(), FillStates::NONE);
    assert_eq!(common.take_change().unwrap(), GeometryChange::NoChange);
    button.set_icon_role(ContentIcon::Check).unwrap();
    assert_eq!(common.take_change().unwrap(), GeometryChange::Changed);
    button.clear_image().unwrap();
    assert_eq!(common.take_change().unwrap(), GeometryChange::NoChange);
    button.set_icon_role(ContentIcon::Check).unwrap();
    assert_eq!(common.take_change().unwrap(), GeometryChange::NoChange);
    assert_eq!(button.set_named_icon("missing"), Err(neocogi_com::NC_E_NOT_FOUND));
    button.set_icon_role(ContentIcon::None).unwrap();
    assert_eq!(button.label().unwrap(), "AB");
    let probe = Rc::new(BuiltinPaintProbe::default());
    let painter: ComPtr<IPainterAbi> = BuiltinPainter::from_value(BuiltinPainter {
        button: common.query().unwrap(),
        probe: probe.clone(),
    })
    .query()
    .unwrap();
    let bounds = Rect { x: 0, y: 0, width: 100, height: 30 };
    let paint = WidgetPaint {
        rect: bounds,
        clip: bounds,
        enabled: true,
        window_active: true,
        ..WidgetPaint::default()
    };
    let scope = PaintScope { invocation: 1, clip: RECT };
    probe.fail.set(true);
    assert_eq!(common.paint(scope, paint, painter.as_com_ref()), Err(UI_E_RENDER));
    common.paint(scope, paint, painter.as_com_ref()).unwrap();
    let left = probe.text_positions.borrow().last().expect("left-aligned text was recorded").x;
    button.replace_flags(WidgetOptions::ALIGN_RIGHT).unwrap();
    common.paint(scope, paint, painter.as_com_ref()).unwrap();
    let right = probe.text_positions.borrow().last().expect("right-aligned text was recorded").x;
    assert!(right > left + 40, "right alignment must reach the semantic paint algorithm");
    assert_eq!(
        common.paint(
            scope,
            WidgetPaint {
                rect: Rect { width: -1, ..bounds },
                ..paint
            },
            painter.as_com_ref()
        ),
        Err(NC_E_INVALID_ARG)
    );
}

mod component_events;
mod document_events;
mod event_origins;
mod event_queues;
mod events;
mod list_item_events;
mod widget_dispatch;

mod combo;
mod menus;

/// Input is a real inherited capability and IME placement never exposes uncommitted geometry.
#[test]
fn platform_input_projection_and_committed_text_area_share_runtime_state() {
    let runtime = runtime();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let input = ui.input_sink().unwrap();
    let identity: ComPtr<neocogi_com::IUnknown> = input.query().unwrap();
    assert_eq!(identity.as_raw().object, runtime.query::<neocogi_com::IUnknown>().unwrap().as_raw().object);
    let editor = ui.widgets().textbox_unobserved("start").unwrap();
    let editor_owner = editor;
    let editor = editor_owner.handle();
    let window = ui.window("IME", RECT, editor_owner).unwrap();
    assert_eq!(ui.text_input_area().unwrap(), TextInputArea::None);
    input
        .input(
            UiInput::Key(KeyStroke {
                key: KeyCode::Tab,
                value: 0,
                pressed: true,
                repeat: false,
                modifiers: 0,
            }),
            NcCopyFromBufferOffset::new(0, &[]),
        )
        .unwrap();
    ui.update(SIZE).unwrap();
    let TextInputArea::Area(first) = ui.text_input_area().unwrap() else {
        panic!("focused editor must request IME")
    };
    assert!(first.width > 0 && first.height > 0);
    input.input(UiInput::Text, NcCopyFromBufferOffset::new(0, "é☃".as_bytes())).unwrap();
    assert_eq!(ui.text_input_area().unwrap(), TextInputArea::None);
    ui.update(SIZE).unwrap();
    assert_eq!(editor.text().unwrap(), "starté☃");
    let mut rect = RECT;
    rect.x += 30;
    window.set_rect(rect).unwrap();
    assert_eq!(ui.text_input_area().unwrap(), TextInputArea::None);
    ui.update(SIZE).unwrap();
    let TextInputArea::Area(moved) = ui.text_input_area().unwrap() else {
        panic!("moved editor keeps focus")
    };
    assert_eq!(moved.x, first.x + 30);
    window.close().unwrap();
    ui.update(SIZE).unwrap();
    assert_eq!(ui.text_input_area().unwrap(), TextInputArea::None);
    ui.close().unwrap();
    assert_eq!(ui.text_input_area().err(), Some(UI_E_CLOSED));
    assert_eq!(input.input(UiInput::Cancel, NcCopyFromBufferOffset::new(0, &[])), Err(UI_E_CLOSED));
}

/// Concrete external content observes ownership, geometry and lifecycle without retaining its COM self.
struct LocalContent {
    /// Weak application slot permits reentry tests without a self-owning widget cycle.
    target: std::rc::Weak<RefCell<Option<WeakComBox<IWidgetAbi>>>>,
    /// Explicit model state remains independently observable during calls.
    probe: Rc<LocalContentProbe>,
}
#[derive(Default)]
/// Application-visible counters and intentional one-shot failures for the static leaf adapter.
struct LocalContentProbe {
    /// Desired width changes before the intentionally failed update returns.
    width: Cell<i32>,
    /// Last committed content origin used to verify final drawable-space clipping.
    origin: Cell<Point>,
    /// Number of actual preferred-size observations.
    measures: Cell<u32>,
    /// Fails one measurement to prove an error releases the generated shared-call gate.
    fail_measure: Cell<bool>,
    /// Number of entered updates, including failures, for traversal-order assertions.
    updates: Cell<u32>,
    /// Fails one update after a committed model change.
    fail_update: Cell<bool>,
    /// Logical closure must reach content exactly once despite retained COM handles.
    closes: Cell<u32>,
    /// Final content destruction must occur once after all COM references retire.
    drops: Cell<u32>,
}
impl neocogi_ui::LeafContent for LocalContent {
    /// Recursive calls through any projection reject while mutable measurement owns the object.
    fn measure(&self, bounds: LayoutLimits, metrics: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        self.probe.measures.set(self.probe.measures.get() + 1);
        let target = self.target.upgrade().unwrap().borrow().as_ref().unwrap().clone();
        assert_eq!(target.properties(), Err(NC_E_BUSY));
        assert_eq!(target.query::<ILeafLayoutAbi>().unwrap().measure(bounds, metrics), Err(NC_E_BUSY));
        if self.probe.fail_measure.replace(false) {
            return Err(NC_E_INVALID_ARG);
        }
        Ok(Size {
            width: self.probe.width.get(),
            height: 20,
        })
    }
    /// A failed partial update must invalidate the earlier preferred-size cache for recovery.
    fn update(&mut self, state: WidgetUpdate, _: WidgetEvent, _: &str) -> Result<GeometryChange, NcStatus> {
        self.probe.updates.set(self.probe.updates.get() + 1);
        self.probe.origin.set(Point { x: state.screen.x, y: state.screen.y });
        if self.probe.fail_update.replace(false) {
            self.probe.width.set(120);
            return Err(NC_E_INVALID_ARG);
        }
        Ok(GeometryChange::NoChange)
    }
    /// Solid geometry reaches the ordinary display-list arena without an application UV lookup.
    fn paint(&mut self, _: WidgetPaint, painter: &mut neocogi_ui::Painter<'_>) -> Result<(), NcStatus> {
        let mut clipped = painter.clipped(Rect { x: 2, y: 2, width: 3, height: 3 })?;
        clipped.colored_triangles(&[
            ColoredVertex { x: 0.0, y: 0.0, rgba: 0xff030201 },
            ColoredVertex { x: 10.0, y: 0.0, rgba: 0xff030201 },
            ColoredVertex { x: 0.0, y: 10.0, rgba: 0xff030201 },
        ])?;
        // Returning to the parent must not inherit the smaller clip or need a restoring call.
        painter.colored_triangles(&[
            ColoredVertex { x: 8.0, y: 8.0, rgba: 0xff060504 },
            ColoredVertex { x: 9.0, y: 8.0, rgba: 0xff060504 },
            ColoredVertex { x: 8.0, y: 9.0, rgba: 0xff060504 },
        ])
    }
}
impl Drop for LocalContent {
    /// The final inline value has one destructor, independent of the number of COM projections.
    fn drop(&mut self) {
        self.probe.closes.set(self.probe.closes.get() + 1);
        self.probe.drops.set(self.probe.drops.get() + 1);
    }
}

/// The generic local helper is one real COM object, survives failures and preserves normal cache rules.
#[test]
fn static_leaf_content_recovers_from_reentry_and_errors_with_one_owned_lifetime() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let target = Rc::new(RefCell::new(None));
    let probe = Rc::new(LocalContentProbe::default());
    probe.width.set(80);
    let widget = ui
        .widgets()
        .leaf(
            LocalContent {
                target: Rc::downgrade(&target),
                probe: probe.clone(),
            },
            WidgetOptions::NO_INTERACT,
        )
        .unwrap();
    let mut node = widget.node_interface().unwrap();
    // The static custom-leaf adapter shares the same current-requested-flags contract.
    node.set_flags(WidgetOptions::FRAME).unwrap();
    assert_eq!(node.properties().unwrap().options, WidgetOptions::FRAME | WidgetOptions::NO_INTERACT);
    node.clear_flags(WidgetOptions::NO_INTERACT).unwrap();
    assert_eq!(node.properties().unwrap().options, WidgetOptions::FRAME);
    node.replace_flags(WidgetOptions::NO_INTERACT).unwrap();
    *target.borrow_mut() = Some(node.clone());
    let widget_owner = widget;
    let widget = widget_owner.handle();
    let window = ui.window("Local leaf", RECT, widget_owner).unwrap();
    probe.fail_measure.set(true);
    assert!(ui.update(SIZE).is_err());
    ui.update(SIZE).unwrap();
    let measured = probe.measures.get();
    ui.update(SIZE).unwrap();
    assert_eq!(probe.measures.get(), measured, "unchanged eventless updates must preserve measurement");
    probe.fail_update.set(true);
    assert!(ui.update(SIZE).is_err());
    ui.update(SIZE).unwrap();
    assert!(probe.measures.get() > measured, "recovery must remeasure the partially changed model");
    let (renderer, rendered) = renderer();
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    let origin = probe.origin.get();
    let vertices = rendered.vertices.borrow();
    let clipped: Vec<_> = vertices.iter().filter(|vertex| vertex.rgba == 0xff030201).collect();
    assert!(clipped.len() >= 3);
    for vertex in clipped {
        assert!((origin.x as f32 + 2.0..=origin.x as f32 + 5.0).contains(&vertex.x));
        assert!((origin.y as f32 + 2.0..=origin.y as f32 + 5.0).contains(&vertex.y));
    }
    assert_eq!(vertices.iter().filter(|vertex| vertex.rgba == 0xff060504).count(), 3);
    drop(vertices);
    window.close().unwrap();
    assert_eq!(probe.closes.get(), 1);
    assert_eq!(probe.drops.get(), 1);
    assert_eq!(node.properties().err(), Some(neocogi_com::NC_E_DROPPED));
    target.borrow_mut().take();
    drop(node);
    drop(widget);
    assert_eq!(probe.drops.get(), 1);
}

mod images;

mod retained;

/// A failed ordinary update stops before later roots and leaves the phase uncommitted for retry.
#[test]
fn update_failure_stops_later_roots_without_losing_retry_invalidation() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let mut retained = Vec::new();
    for title in ["first", "later"] {
        let target = Rc::new(RefCell::new(None));
        let probe = Rc::new(LocalContentProbe::default());
        probe.width.set(80);
        let widget = ui
            .widgets()
            .leaf(
                LocalContent {
                    target: Rc::downgrade(&target),
                    probe: probe.clone(),
                },
                WidgetOptions::NO_INTERACT,
            )
            .unwrap();
        *target.borrow_mut() = Some(widget.node_interface().unwrap());
        let widget_owner = widget;
        let widget = widget_owner.handle();
        let window = ui.window(title, RECT, widget_owner).unwrap();
        retained.push((target, probe, widget, window));
    }
    ui.update(SIZE).unwrap();
    let first = &retained[0].1;
    let later = &retained[1].1;
    let before = (first.updates.get(), later.updates.get(), first.measures.get());
    first.fail_update.set(true);
    assert_eq!(ui.update(SIZE), Err(NC_E_INVALID_ARG));
    assert_eq!(first.updates.get(), before.0 + 1);
    assert_eq!(later.updates.get(), before.1);
    let (renderer, _) = renderer();
    assert_eq!(ui.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    ui.update(SIZE).unwrap();
    assert_eq!(first.updates.get(), before.0 + 2);
    assert_eq!(later.updates.get(), before.1 + 1);
    assert!(first.measures.get() > before.2);
}

struct ReviewDispatchTrace {
    name: &'static str,
    trace: Rc<RefCell<Vec<&'static str>>>,
}
impl neocogi_ui::LeafContent for ReviewDispatchTrace {
    fn measure(&self, _: LayoutLimits, _: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        Ok(Size { width: 20, height: 20 })
    }
    fn paint(&mut self, _: WidgetPaint, _: &mut neocogi_ui::Painter<'_>) -> Result<(), NcStatus> {
        Ok(())
    }
}

#[test]
fn review_dispatch_cache_follows_raised_root_chronology() {
    let runtime = runtime();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime).unwrap();
    let trace = Rc::new(RefCell::new(Vec::new()));
    let first = ui
        .widgets()
        .dispatching_leaf(ReviewDispatchTrace { name: "first", trace: trace.clone() }, WidgetOptions::NONE)
        .unwrap();
    let first = ui.window("first", RECT, first).unwrap();
    let second = ui
        .widgets()
        .dispatching_leaf(ReviewDispatchTrace { name: "second", trace: trace.clone() }, WidgetOptions::NONE)
        .unwrap();
    let _second = ui.window("second", RECT, second).unwrap();
    ui.update(SIZE).unwrap();
    assert_eq!(&trace.borrow()[..2], &["second", "first"]);
    trace.borrow_mut().clear();
    first.raise().unwrap();
    ui.update(SIZE).unwrap();
    assert_eq!(
        &trace.borrow()[..2],
        &["first", "second"],
        "a fresh authoritative reverse-chronology snapshot follows raise"
    );
}

#[test]
fn review_dispatch_child_surface_precedes_raised_parent() {
    let runtime = runtime();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let manager: ComPtr<IWindowManagerAbi> = runtime.query().unwrap();
    let trace = Rc::new(RefCell::new(Vec::new()));
    let parent = ui
        .widgets()
        .dispatching_leaf(ReviewDispatchTrace { name: "parent", trace: trace.clone() }, WidgetOptions::NONE)
        .unwrap();
    let parent = ui.window("parent", RECT, parent).unwrap();
    let child = ui
        .widgets()
        .dispatching_leaf(ReviewDispatchTrace { name: "child", trace: trace.clone() }, WidgetOptions::NONE)
        .unwrap();
    let _child = manager
        .create_child(
            parent.as_com_ref(),
            NcCopyFromBufferOffset::new(0, b"child"),
            RECT,
            child.into_interface(),
            neocogi_com::NcValueRef::new(&neocogi_com::events::EventBinding::Unobserved).unwrap(),
        )
        .unwrap();
    parent.raise().unwrap();
    ui.update(SIZE).unwrap();
    assert_eq!(
        &trace.borrow()[..2],
        &["child", "parent"],
        "documented descendant-first delivery survives ancestor raising"
    );
}

impl neocogi_ui::DispatchingLeafContent for ReviewDispatchTrace {
    fn dispatch_next(&mut self, _: &mut neocogi_ui::LeafDispatchContext<'_>) -> Result<bool, NcStatus> {
        self.trace.borrow_mut().push(self.name);
        Ok(false)
    }
}

#[com_object(interfaces(IWidget, ILeafLayout, ITextContent), ownership = exclusive)]
struct ReviewStickyWidget {
    state: neocogi_ui::com::widget::WidgetState,
    text: String,
    sticky: bool,
    collections: Rc<Cell<usize>>,
}
impl IWidget for ReviewStickyWidget {
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        self.state.host().try_add_ref()
    }
    fn mount(&mut self) -> Result<(), NcStatus> {
        self.state.mount()
    }
    fn properties(&self) -> Result<WidgetProperties, NcStatus> {
        Ok(WidgetProperties {
            options: self.state.requested_flags()?,
            keyboard: 0,
            frame: WidgetFrame::Generic,
        })
    }
    fn requested_flags(&self) -> Result<WidgetOptions, NcStatus> {
        self.state.requested_flags()
    }
    fn replace_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.state.replace_flags(flags)
    }
    fn set_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.state.set_flags(flags)
    }
    fn clear_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.state.clear_flags(flags)
    }
    fn update(&mut self, _: WidgetUpdate, _: WidgetEvent, _: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
        Ok(GeometryChange::NoChange)
    }
    fn take_change(&mut self) -> Result<GeometryChange, NcStatus> {
        self.collections.set(self.collections.get() + 1);
        let own = std::mem::replace(&mut self.sticky, false);
        let common = self.state.take_change()?;
        Ok(if own || common == GeometryChange::Changed {
            GeometryChange::Changed
        } else {
            GeometryChange::NoChange
        })
    }
    fn paint(&mut self, _: PaintScope, _: WidgetPaint, _: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
        Ok(())
    }
}
impl ILeafLayout for ReviewStickyWidget {
    fn measure(&mut self, _: LayoutLimits, _: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        Ok(Size {
            width: self.text.len() as i32 * 10,
            height: 20,
        })
    }
}
impl ITextContent for ReviewStickyWidget {
    fn text(&self) -> Result<ComPtr<neocogi_com::INcStringAbi>, NcStatus> {
        Ok(neocogi_com::NcString::from_utf8_str(&self.text))
    }
    fn set_text(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.state.writable()?;
        if input.offset() != 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let text = std::str::from_utf8(input.as_slice()).map_err(|_| NC_E_INVALID_ARG)?;
        if self.text != text {
            self.text = text.to_owned();
            // Independent providers retain their own marker AND conservatively notify authority.
            self.sticky = true;
            self.state.host().mark_geometry_changed()?;
        }
        Ok(())
    }
}
fn review_sticky_widget(runtime: &ComPtr<UiRuntimeAbi>) -> (ComBox<IWidgetAbi>, WeakComBox<ITextContentAbi>, Rc<Cell<usize>>) {
    let collections = Rc::new(Cell::new(0));
    let host = runtime.construction_host().unwrap().host().unwrap();
    let object = ReviewStickyWidget::into_box(ReviewStickyWidget {
        state: neocogi_ui::com::widget::WidgetState::new(host, WidgetOptions::NONE),
        text: "a".to_owned(),
        sticky: false,
        collections: collections.clone(),
    });
    let text = object.query_weak::<ITextContentAbi>().unwrap();
    (object.query::<IWidgetAbi>().unwrap(), text, collections)
}

#[test]
fn review_independent_sticky_setter_requires_update_before_render() {
    let runtime = runtime();
    let (widget, mut text, _) = review_sticky_widget(&runtime);
    let _window = mount(&runtime, widget).unwrap();
    runtime.update(SIZE).unwrap();
    let (renderer, probe) = renderer();
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    text.set_text(NcCopyFromBufferOffset::new(0, b"a much longer label")).unwrap();
    assert_eq!(runtime.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    assert_eq!(probe.calls.get(), 1);
}

#[test]
fn review_idle_update_does_not_collect_any_widget_changes() {
    let runtime = runtime();
    let (widget, _, collections) = review_sticky_widget(&runtime);
    let _window = mount(&runtime, widget).unwrap();
    runtime.update(SIZE).unwrap();
    runtime.update(SIZE).unwrap();
    collections.set(0);
    runtime.update(SIZE).unwrap();
    assert_eq!(collections.get(), 0, "documented idle acceptance: no take_change COM calls");
}

/// A real optional receiver changes surface chronology without claiming a message.
struct RaiseDuringDispatch {
    target: Option<ComPtr<IWindowAbi>>,
    trace: Rc<RefCell<Vec<&'static str>>>,
}
impl neocogi_ui::LeafContent for RaiseDuringDispatch {
    fn measure(&self, _: LayoutLimits, _: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        Ok(Size { width: 20, height: 20 })
    }
    fn paint(&mut self, _: WidgetPaint, _: &mut neocogi_ui::Painter<'_>) -> Result<(), NcStatus> {
        Ok(())
    }
}
impl neocogi_ui::DispatchingLeafContent for RaiseDuringDispatch {
    fn dispatch_next(&mut self, _: &mut neocogi_ui::LeafDispatchContext<'_>) -> Result<bool, NcStatus> {
        self.trace.borrow_mut().push("raising");
        if let Some(target) = self.target.take() {
            target.raise()?;
        }
        Ok(false)
    }
}
/// A chronology edit inside a false-reporting handler invalidates the same-update snapshot.
#[test]
fn false_dispatch_callback_reconciles_raised_surface_before_next_receiver() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let trace = Rc::new(RefCell::new(Vec::new()));
    let first = ui
        .widgets()
        .dispatching_leaf(ReviewDispatchTrace { name: "first", trace: trace.clone() }, WidgetOptions::NONE)
        .unwrap();
    let first = ui.window("first", RECT, first).unwrap();
    let later = ui
        .widgets()
        .dispatching_leaf(ReviewDispatchTrace { name: "later", trace: trace.clone() }, WidgetOptions::NONE)
        .unwrap();
    let _later = ui.window("later", RECT, later).unwrap();
    let raising = ui
        .widgets()
        .dispatching_leaf(
            RaiseDuringDispatch {
                target: Some(first),
                trace: trace.clone(),
            },
            WidgetOptions::NONE,
        )
        .unwrap();
    let _raising = ui.window("raising", RECT, raising).unwrap();
    ui.update(SIZE).unwrap();
    assert_eq!(&trace.borrow()[..4], &["raising", "first", "raising", "later"]);
}
