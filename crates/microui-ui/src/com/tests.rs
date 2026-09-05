//! Boundary regressions use the real runtime and deliberately retaining/reentrant components.
use super::*;
use nc_com::{NcCopyToBufferOffset, INcStringComPtrExt};
mod containers;

/// Positive drawable size shared by the deterministic headless fixtures.
const SIZE: Size = Size { width: 640, height: 400 };
/// Logical window allocation with room for retained chrome and the test leaf.
const RECT: Rect = Rect { x: 20, y: 20, width: 400, height: 200 };

/// Creates the production runtime with a small deterministic atlas.
fn runtime() -> ComPtr<IUiRuntimeAbi> {
    create_runtime(crate::test_support::test_atlas())
}

/// Creates a concrete button without bypassing its production factory.
fn button(runtime: &ComPtr<IUiRuntimeAbi>) -> ComPtr<IButtonAbi> {
    runtime.query::<IUiFactoryAbi>().unwrap().create_button().unwrap()
}

/// Mounts the common capability through the production manager's transactional boundary.
fn mount(runtime: &ComPtr<IUiRuntimeAbi>, root: &ComPtr<IWidgetAbi>) -> Result<ComPtr<IWindowAbi>, NcStatus> {
    runtime
        .query::<IWindowManagerAbi>()?
        .create_window(NcCopyFromBufferOffset::new(0, b"test"), RECT, root.as_com_ref())
}

/// Observable state retained by a fake backend independently of its COM allocation.
#[derive(Default)]
struct RenderProbe {
    /// Complete renderer invocations, including deliberately failing submissions.
    calls: Cell<u32>,
    /// Old retained stream/token used to prove that expiry survives later reuse.
    streams: RefCell<Vec<(ComPtr<IRenderStreamAbi>, u64)>>,
    /// Test-controlled explicit renderer failure.
    fail: Cell<bool>,
}

#[com_class(interfaces(IRenderer))]
/// Typed backend records real final vertices and retains the stream maliciously but safely.
struct Renderer {
    /// Shared observations owned by the test, with no borrowed stack reference in COM.
    probe: Rc<RenderProbe>,
}

impl IRenderer for Renderer {
    /// This CPU recorder accepts validated bundles without a GPU upload.
    fn prepare_assets(&self, _assets: ComRef<'_, IAssetBundleAbi>) -> Result<(), NcStatus> {
        Ok(())
    }
    /// Copies bulk geometry during its valid scope and retains authority for later rejection tests.
    fn render(&self, info: RenderInfo, _assets: ComRef<'_, IAssetBundleAbi>, stream: ComRef<'_, IRenderStreamAbi>) -> Result<(), NcStatus> {
        let mut vertices = vec![RenderVertex::default(); info.vertex_count as usize];
        assert_eq!(
            stream.vertices(info.invocation, NcCopyToBufferOffset::new(0, &mut vertices))?,
            info.vertex_count
        );
        let mut batches = vec![RenderBatch::Atlas { first: 0, count: 0 }.into(); info.batch_count as usize];
        assert_eq!(stream.batches(info.invocation, NcCopyToBufferOffset::new(0, &mut batches))?, info.batch_count);
        // These built-in fixtures use one coalesced atlas range regardless of glyph/primitive count.
        assert_eq!(batches.len(), 1);
        assert!(matches!(RenderBatch::try_from(batches[0])?, RenderBatch::Atlas { first: 0, count } if count == info.vertex_count));
        assert_eq!(stream.texture(info.invocation, 0).unwrap_err(), NC_E_INVALID_ARG);
        assert_eq!(stream.item(info.invocation, 0).unwrap_err(), NC_E_INVALID_ARG);
        assert!(vertices.len() >= 6);
        assert!(vertices.iter().all(|v| v.x.is_finite() && v.y.is_finite()));
        self.probe.calls.set(self.probe.calls.get() + 1);
        self.probe.streams.borrow_mut().push((stream.add_ref(), info.invocation));
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
fn mount_validation_is_transactional_and_closure_outlives_tree_storage() {
    let first = runtime();
    let second = runtime();
    let button = button(&first);
    let node = button.query().unwrap();
    assert_eq!(mount(&second, &node).unwrap_err(), UI_E_OWNER);
    let window = mount(&first, &node).unwrap();
    assert_eq!(mount(&first, &node).unwrap_err(), UI_E_OWNER);
    assert_eq!(button.set_label(NcCopyFromBufferOffset::new(1, b"bad")), Err(NC_E_INVALID_ARG));
    assert_eq!(button.set_label(NcCopyFromBufferOffset::new(0, &[0xff])), Err(NC_E_INVALID_ARG));
    assert_eq!(button.label().unwrap().to_string().unwrap(), "");
    window.close().unwrap();
    assert_eq!(button.set_label(NcCopyFromBufferOffset::new(0, b"closed")), Err(UI_E_CLOSED));
    let unmounted = self::button(&first);
    drop(first);
    assert_eq!(unmounted.label().unwrap_err(), UI_E_CLOSED);
    assert_eq!(window.set_rect(RECT), Err(UI_E_CLOSED));
    second.update(SIZE).unwrap();
}

/// Setter and resize invalidation are independent of user activation's NoChange result.
#[test]
fn rendering_requires_synchronized_geometry_and_streams_expire_after_success_and_failure() {
    let runtime = runtime();
    let button = button(&runtime);
    let node = button.query().unwrap();
    let _window = mount(&runtime, &node).unwrap();
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
        assert_eq!(stream.vertices(*scope, NcCopyToBufferOffset::new(0, &mut [])), Err(UI_E_EXPIRED_SCOPE));
        assert_eq!(stream.batches(*scope, NcCopyToBufferOffset::new(0, &mut [])), Err(UI_E_EXPIRED_SCOPE));
        assert_eq!(stream.texture(*scope, 0).unwrap_err(), UI_E_EXPIRED_SCOPE);
        assert_eq!(stream.item(*scope, 0).unwrap_err(), UI_E_EXPIRED_SCOPE);
    }
}

/// A component can retain COM services; the runtime must enforce phase and scope at runtime.
#[derive(Default)]
struct WidgetProbe {
    /// Most recent copied update geometry, used to verify concrete container allocations.
    latest: Cell<Option<WidgetUpdate>>,
    /// One-shot attempt to close an actively editing parent during child mounting.
    close_on_mount: RefCell<Option<ComPtr<IWidgetAbi>>>,
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
    /// Logical closure must follow node removal once, even with retained COM handles.
    closes: Cell<u32>,
    /// Deliberate close failure verifies explicit teardown reports component errors.
    fail_close: Cell<bool>,
    /// A transient geometry error must be retried without a second setter change.
    fail_measure: Cell<bool>,
    /// Paint calls retain these values beyond their valid invocation.
    painters: RefCell<Vec<(ComPtr<IPainterAbi>, PaintScope)>>,
    /// Number of actual preferred measurement calls after cache handling.
    measures: Cell<u32>,
    /// Injected paint failure must prevent the complete backend submission.
    fail_paint: Cell<bool>,
}

#[com_class(interfaces(IWidget, ILeafLayout))]
/// Independently implemented leaf delegates lifetime only to a factory-owned fixture object.
struct RetainingWidget {
    /// Gives the custom implementation the same runtime ownership identity and mount-once rules.
    lifetime: ComPtr<IWidgetAbi>,
    /// Actual concrete setter used to test observational mutation rejection.
    button: ComPtr<IButtonAbi>,
    /// Retained runtime deliberately tries to reenter during measurement and painting.
    runtime: ComPtr<IUiRuntimeAbi>,
    /// Test state is owned and survives every COM call.
    probe: Rc<WidgetProbe>,
}

impl ILeafLayout for RetainingWidget {
    /// Checks reentry and mutation rejection while returning deterministic preferred geometry.
    fn measure(&self, _bounds: LayoutLimits, _metrics: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
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
        assert_eq!(self.lifetime.close(), Err(UI_E_BUSY));
        Ok(Size {
            width: self.probe.width.get().max(100),
            height: 30,
        })
    }
}
impl IWidget for RetainingWidget {
    /// Returns the shared owner without exposing runtime storage.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        self.lifetime.host()
    }
    /// Delegates mount-once state to its independent lifetime fixture.
    fn mount(&self) -> Result<(), NcStatus> {
        let parent = self.probe.close_on_mount.borrow_mut().take();
        if let Some(parent) = parent {
            assert_eq!(parent.close(), Err(UI_E_BUSY));
        }
        self.lifetime.mount()
    }
    /// Delegates logical closure to its lifetime fixture.
    fn close(&self) -> Result<(), NcStatus> {
        self.probe.closes.set(self.probe.closes.get() + 1);
        self.lifetime.close()?;
        if self.probe.fail_close.get() { Err(NC_E_INVALID_ARG) } else { Ok(()) }
    }

    /// Reuses real common widget policy in this deliberately retaining implementation.
    fn properties(&self) -> Result<WidgetProperties, NcStatus> {
        let mut properties = self.button.properties()?;
        if self.probe.text_input.get() {
            properties.keyboard |= 256;
        }
        Ok(properties)
    }
    /// Common options are delegated to the shared fixture state.
    fn set_options(&self, options: WidgetOptions) -> Result<(), NcStatus> {
        self.button.set_options(options)
    }
    /// Eventless updates do not change this fixture's fixed content geometry.
    fn update(&self, state: WidgetUpdate, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
        let (event, text) = decode_widget_event(event, text)?;
        self.probe.latest.set(Some(state));
        self.probe.updates.set(self.probe.updates.get() + 1);
        self.probe.events.borrow_mut().push(event);
        if matches!(event, WidgetEvent::Text) {
            self.probe.texts.borrow_mut().push(text.to_owned());
        }
        let width = self.probe.requested_width.replace(0);
        if width > 0 && self.probe.width.replace(width) != width {
            Ok(GeometryChange::CHANGED)
        } else {
            Ok(GeometryChange::NO_CHANGE)
        }
    }

    /// Setter changes stay sticky even when this custom widget is hidden.
    fn take_change(&self) -> Result<GeometryChange, NcStatus> {
        self.button.take_change()
    }
    /// Retains authority and probes old tokens while a later scope is active.
    fn paint(&self, scope: PaintScope, state: WidgetPaint, painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
        assert_eq!(self.runtime.update(SIZE), Err(UI_E_BUSY));
        assert_eq!(self.button.set_label(NcCopyFromBufferOffset::new(0, b"forbidden")), Err(UI_E_BUSY));
        for (old, expired) in self.probe.painters.borrow().iter() {
            assert_eq!(old.fill(*expired, state.rect, 0), Err(UI_E_EXPIRED_SCOPE));
            assert_eq!(old.triangles(*expired, NcCopyFromBufferOffset::new(0, &[])), Err(UI_E_EXPIRED_SCOPE));
        }
        // Malformed bulk batches fail transactionally before a valid command follows.
        assert_eq!(painter.triangles(scope, NcCopyFromBufferOffset::new(1, &[])), Err(NC_E_INVALID_ARG));
        assert_eq!(
            painter.triangles(scope, NcCopyFromBufferOffset::new(0, &[RenderVertex::default()])),
            Err(NC_E_INVALID_ARG)
        );
        let malformed = [RenderVertex { x: f32::NAN, ..RenderVertex::default() }; 3];
        assert_eq!(painter.triangles(scope, NcCopyFromBufferOffset::new(0, &malformed)), Err(NC_E_INVALID_ARG));
        painter.fill(scope, state.rect, 0xff00ff00)?;
        self.probe.painters.borrow_mut().push((painter.add_ref(), scope));
        if self.probe.fail_paint.get() { Err(NC_E_INVALID_ARG) } else { Ok(()) }
    }
}

/// Retained scope authority expires, reentry is rejected, and paint failure never reaches the GPU.
#[test]
fn external_widget_cannot_reenter_mutate_during_observation_or_reuse_a_paint_scope() {
    let runtime = runtime();
    let button = button(&runtime);
    let probe = Rc::new(WidgetProbe::default());
    let widget = RetainingWidget::from_value(RetainingWidget {
        lifetime: button.query().unwrap(),
        button,
        runtime: runtime.clone(),
        probe: probe.clone(),
    });
    let window = mount(&runtime, &widget.query().unwrap()).unwrap();
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
    for (painter, scope) in probe.painters.borrow().iter() {
        assert_eq!(painter.fill(*scope, RECT, 0), Err(UI_E_EXPIRED_SCOPE));
    }
}

/// Concrete window projections preserve modal, structural visibility and fixed-layer policy.
#[test]
fn window_com_capabilities_preserve_hierarchy_layers_visibility_and_subtree_closure() {
    let runtime = runtime();
    let manager: ComPtr<IWindowManagerAbi> = runtime.query().unwrap();
    // Runtime, factory and manager are projections of one object, not service-owned peers.
    let identity: ComPtr<IUnknown> = runtime.query().unwrap();
    assert_eq!(identity.as_ptr(), manager.query::<IUnknown>().unwrap().as_ptr());
    assert_eq!(
        identity.as_ptr(),
        runtime.query::<IUiFactoryAbi>().unwrap().query::<IUnknown>().unwrap().as_ptr()
    );
    let root_button = button(&runtime);
    let root = mount(&runtime, &root_button.query().unwrap()).unwrap();
    let child_button = button(&runtime);
    let child = manager
        .create_child(
            root.as_com_ref(),
            NcCopyFromBufferOffset::new(0, b"child"),
            RECT,
            child_button.query::<IWidgetAbi>().unwrap().as_com_ref(),
        )
        .unwrap();
    let dialog_button = button(&runtime);
    let dialog = manager
        .create_dialog(
            child.as_com_ref(),
            NcCopyFromBufferOffset::new(0, b"dialog"),
            RECT,
            dialog_button.query::<IWidgetAbi>().unwrap().as_com_ref(),
        )
        .unwrap();
    root.set_layer(2).unwrap();
    assert_eq!(child.snapshot().unwrap().layer.number, 2);
    assert_eq!(child.set_layer(4), Err(UI_E_POLICY));
    assert_eq!(dialog.snapshot().unwrap().layer.kind, LayerKind::MODAL);
    assert_eq!(dialog.snapshot().unwrap().visibility, Visibility::HIDDEN);
    dialog.set_visibility(Visibility::VISIBLE).unwrap();
    root.set_visibility(Visibility::HIDDEN).unwrap();
    assert_eq!(child.snapshot().unwrap().visibility, Visibility::HIDDEN);
    root.set_visibility(Visibility::VISIBLE).unwrap();
    assert_eq!(child.snapshot().unwrap().visibility, Visibility::VISIBLE);
    // Hiding an owner closes modal visibility intent; showing it must not resurrect a dialog.
    assert_eq!(dialog.snapshot().unwrap().visibility, Visibility::HIDDEN);
    root.set_child_clip(ChildClip::CONTENT).unwrap();
    assert_eq!(root.snapshot().unwrap().child_clip, ChildClip::CONTENT);
    let before = root.snapshot().unwrap().options.bits;
    assert_eq!(root.set_options(WindowOptions { bits: 32 }), Err(NC_E_INVALID_ARG));
    assert_eq!(root.snapshot().unwrap().options.bits, before);
    assert_eq!(root.set_layer(16), Err(NC_E_INVALID_ARG));
    assert_eq!(root.snapshot().unwrap().layer.number, 2);
    root.set_title(NcCopyFromBufferOffset::new(0, b"new title")).unwrap();
    assert_eq!(root.title().unwrap().to_string().unwrap(), "new title");
    runtime.update(SIZE).unwrap();
    root.close().unwrap();
    for button in [root_button, child_button, dialog_button] {
        assert_eq!(button.set_label(NcCopyFromBufferOffset::new(0, b"closed")), Err(UI_E_CLOSED));
    }
    assert_eq!(child.snapshot().unwrap_err(), UI_E_CLOSED);
    assert_eq!(dialog.set_visibility(Visibility::VISIBLE), Err(UI_E_CLOSED));
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
    let root = mount(&first, &button(&first).query().unwrap()).unwrap();
    let foreign = mount(&second, &button(&second).query().unwrap()).unwrap();
    let content = button(&first);
    let node: ComPtr<IWidgetAbi> = content.query().unwrap();
    assert_eq!(
        manager
            .create_child(foreign.as_com_ref(), NcCopyFromBufferOffset::new(0, b"foreign"), RECT, node.as_com_ref())
            .unwrap_err(),
        UI_E_OWNER
    );
    let dialog = manager
        .create_dialog(
            root.as_com_ref(),
            NcCopyFromBufferOffset::new(0, b"modal"),
            RECT,
            button(&first).query::<IWidgetAbi>().unwrap().as_com_ref(),
        )
        .unwrap();
    assert_eq!(
        manager
            .create_child(dialog.as_com_ref(), NcCopyFromBufferOffset::new(0, b"invalid"), RECT, node.as_com_ref())
            .unwrap_err(),
        UI_E_POLICY
    );
    let valid = manager
        .create_child(root.as_com_ref(), NcCopyFromBufferOffset::new(0, b"valid"), RECT, node.as_com_ref())
        .unwrap();
    assert_eq!(valid.snapshot().unwrap().visibility, Visibility::VISIBLE);
    first.close().unwrap();
    second.close().unwrap();
}

/// Popups remain independent typed capabilities while obeying their owner's active/modal policy.
#[test]
fn popup_com_capability_enforces_active_owner_policy_and_permanent_removal() {
    let runtime = runtime();
    let manager: ComPtr<IWindowManagerAbi> = runtime.query().unwrap();
    let root = mount(&runtime, &button(&runtime).query().unwrap()).unwrap();
    let popup_button = button(&runtime);
    let popup = manager
        .create_popup(
            root.as_com_ref(),
            NcCopyFromBufferOffset::new(0, b"popup"),
            popup_button.query::<IWidgetAbi>().unwrap().as_com_ref(),
        )
        .unwrap();
    assert_eq!(popup.query::<IWindowAbi>().unwrap_err(), nc_com::NC_E_NOINTERFACE);
    assert_eq!(popup.visibility().unwrap(), Visibility::HIDDEN);
    runtime.update(SIZE).unwrap();
    popup.show(RECT).unwrap();
    assert_eq!(popup.visibility().unwrap(), Visibility::VISIBLE);
    popup.hide().unwrap();
    assert_eq!(popup.visibility().unwrap(), Visibility::HIDDEN);
    let dialog = manager
        .create_dialog(
            root.as_com_ref(),
            NcCopyFromBufferOffset::new(0, b"modal"),
            RECT,
            button(&runtime).query::<IWidgetAbi>().unwrap().as_com_ref(),
        )
        .unwrap();
    dialog.set_visibility(Visibility::VISIBLE).unwrap();
    assert_eq!(popup.show(RECT), Err(UI_E_POLICY));
    dialog.set_visibility(Visibility::HIDDEN).unwrap();
    popup.show(RECT).unwrap();
    popup.close().unwrap();
    popup.close().unwrap();
    assert_eq!(popup.visibility(), Err(UI_E_CLOSED));
    assert_eq!(popup_button.label().unwrap_err(), UI_E_CLOSED);
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
    let object = RetainingWidget::from_value(RetainingWidget {
        lifetime: button.query().unwrap(),
        button: button.clone(),
        runtime: runtime.clone(),
        probe: probe.clone(),
    });
    let window = mount(&runtime, &object.query().unwrap()).unwrap();
    runtime.update(SIZE).unwrap();
    let measured = probe.measures.get();
    probe.requested_width.set(240);
    runtime.update(SIZE).unwrap();
    assert!(probe.measures.get() > measured);
    assert_eq!(probe.width.get(), 240);

    let tab = UiInput::Key(KeyStroke {
        key: KeyCode::TAB,
        value: 0,
        pressed: true,
        repeat: false,
        modifiers: 0,
    });
    runtime.input(tab.into(), NcCopyFromBufferOffset::new(0, &[])).unwrap();
    runtime.update(SIZE).unwrap();
    let measured = probe.measures.get();
    runtime
        .input(UiInput::Text.into(), NcCopyFromBufferOffset::new(0, "hello ☃".as_bytes()))
        .unwrap();
    let (renderer, _) = renderer();
    assert_eq!(runtime.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    runtime.update(SIZE).unwrap();
    assert_eq!(&*probe.texts.borrow(), &["hello ☃"]);
    assert_eq!(probe.measures.get(), measured, "NoChange text processing must preserve measurement");

    let before = button.properties().unwrap().options;
    assert_eq!(button.set_options(WidgetOptions { bits: 8 }), Err(NC_E_INVALID_ARG));
    assert_eq!(button.properties().unwrap().options, before);
    window.set_visibility(Visibility::HIDDEN).unwrap();
    runtime.update(SIZE).unwrap();
    let updates = probe.updates.get();
    let measured = probe.measures.get();
    button.set_options(WidgetOptions::NONE).unwrap();
    runtime.update(SIZE).unwrap();
    assert_eq!(probe.updates.get(), updates, "hidden widgets skip update but retain setter invalidation");
    window.set_visibility(Visibility::VISIBLE).unwrap();
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
    let object = RetainingWidget::from_value(RetainingWidget {
        lifetime: button.query().unwrap(),
        button,
        runtime: runtime.clone(),
        probe: probe.clone(),
    });
    let window = mount(&runtime, &object.query().unwrap()).unwrap();
    runtime.update(SIZE).unwrap();
    let bad_key = UiInput::Key(KeyStroke {
        key: KeyCode::CHARACTER,
        value: 0xd800,
        pressed: true,
        repeat: false,
        modifiers: 0,
    });
    assert_eq!(runtime.input(bad_key.into(), NcCopyFromBufferOffset::new(0, &[])), Err(NC_E_INVALID_ARG));
    assert_eq!(
        runtime.input(UiInput::Text.into(), NcCopyFromBufferOffset::new(1, b"bad")),
        Err(NC_E_INVALID_ARG)
    );
    assert_eq!(
        runtime.input(UiInput::Text.into(), NcCopyFromBufferOffset::new(0, &[0xff])),
        Err(NC_E_INVALID_ARG)
    );
    assert_eq!(
        runtime.input(UiInput::Cancel.into(), NcCopyFromBufferOffset::new(0, b"unexpected")),
        Err(NC_E_INVALID_ARG)
    );
    let (renderer, _) = renderer();
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    probe.fail_close.set(true);
    assert_eq!(window.close(), Err(NC_E_INVALID_ARG));
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
    let object = RetainingWidget::from_value(RetainingWidget {
        lifetime: button.query().unwrap(),
        button,
        runtime: runtime.clone(),
        probe: probe.clone(),
    });
    let window = mount(&runtime, &object.query().unwrap()).unwrap();
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
fn child_editor_validates_before_claiming_and_closes_retained_views() {
    let runtime = runtime();
    let other_runtime = self::runtime();
    let first = button(&runtime);
    let second = button(&runtime);
    let foreign = button(&other_runtime);
    let editor = first.host().unwrap().create_children().unwrap();
    let view = editor.view().unwrap();
    assert_eq!(view.query::<IChildrenEditorAbi>().unwrap_err(), nc_com::NC_E_NOINTERFACE);
    assert_eq!(
        editor.insert(1, first.query::<IWidgetAbi>().unwrap().as_com_ref()),
        Err(nc_com::NC_E_OUT_OF_RANGE)
    );
    assert_eq!(editor.insert(0, foreign.query::<IWidgetAbi>().unwrap().as_com_ref()), Err(UI_E_OWNER));
    editor.insert(0, first.query::<IWidgetAbi>().unwrap().as_com_ref()).unwrap().finish().unwrap();
    assert_eq!(editor.insert(1, first.query::<IWidgetAbi>().unwrap().as_com_ref()), Err(UI_E_OWNER));
    assert_eq!(
        editor.replace(1, second.query::<IWidgetAbi>().unwrap().as_com_ref()),
        Err(nc_com::NC_E_OUT_OF_RANGE)
    );
    assert_eq!(view.child_count().unwrap(), 1);
    let held_child = view.child(0).unwrap();
    editor.replace(0, second.query::<IWidgetAbi>().unwrap().as_com_ref()).unwrap().finish().unwrap();
    assert_eq!(held_child.properties(), Err(UI_E_CLOSED));
    assert_eq!(view.child_count().unwrap(), 1);
    drop(editor);
    assert_eq!(view.child_count(), Err(UI_E_CLOSED));
    assert_eq!(view.child(0).unwrap_err(), UI_E_CLOSED);
    assert_eq!(second.label().unwrap_err(), UI_E_CLOSED);
    // The wrong-owner attempt did not consume the legitimate foreign mount.
    mount(&other_runtime, &foreign.query().unwrap()).unwrap().close().unwrap();
}

/// A failed old-child close reports a committed replacement, so matching metadata can still commit.
#[test]
fn child_edit_reports_committed_topology_separately_from_teardown_failure() {
    let runtime = runtime();
    let button = button(&runtime);
    let editor = button.host().unwrap().create_children().unwrap();
    let view = editor.view().unwrap();
    let probe = Rc::new(WidgetProbe::default());
    probe.fail_close.set(true);
    let object = RetainingWidget::from_value(RetainingWidget {
        lifetime: button.query().unwrap(),
        button,
        runtime: runtime.clone(),
        probe: probe.clone(),
    });
    editor.insert(0, object.query::<IWidgetAbi>().unwrap().as_com_ref()).unwrap().finish().unwrap();
    let replacement = self::button(&runtime);
    let outcome = editor.replace(0, replacement.query::<IWidgetAbi>().unwrap().as_com_ref()).unwrap();
    assert_eq!(outcome.finish(), Err(NC_E_INVALID_ARG));
    assert_eq!(probe.closes.get(), 1);
    assert_eq!(view.child_count().unwrap(), 1);
    assert_eq!(
        view.child(0).unwrap().query::<IUnknown>().unwrap().as_ptr(),
        replacement.query::<IUnknown>().unwrap().as_ptr()
    );
    editor.clear().unwrap().finish().unwrap();
    assert_eq!(view.child_count().unwrap(), 0);
    assert_eq!(replacement.label().unwrap_err(), UI_E_CLOSED);
}

/// The static facade observes one inherited state, normalized text and honest setter markers.
#[test]
fn builtin_facade_editing_preserves_normalization_geometry_and_resource_validation() {
    let runtime = runtime();
    let ui = microui_redux::com::Ui::from_interface(runtime.clone()).unwrap();
    let checkbox = ui.widgets().checkbox("Check", false).unwrap();
    let common = checkbox.node_interface().unwrap();
    common.take_change().unwrap();
    checkbox.set_checked(true).unwrap();
    assert!(checkbox.checked().unwrap());
    assert_eq!(common.take_change().unwrap(), GeometryChange::NO_CHANGE);
    checkbox.set_label("A longer checkbox").unwrap();
    assert_eq!(common.take_change().unwrap(), GeometryChange::CHANGED);
    assert_eq!(checkbox.label().unwrap(), "A longer checkbox");
    checkbox.set_font_role(TextFontRole::BODY).unwrap();
    assert_eq!(common.take_change().unwrap(), GeometryChange::NO_CHANGE);
    assert_eq!(checkbox.set_named_font("missing"), Err(nc_com::NC_E_NOT_FOUND));
    assert_eq!(common.take_change().unwrap(), GeometryChange::NO_CHANGE);
    checkbox.set_named_font("body").unwrap();
    assert_eq!(common.take_change().unwrap(), GeometryChange::CHANGED);

    let textbox = ui.widgets().textbox("é\r\n☃").unwrap();
    assert_eq!(textbox.text().unwrap(), "é☃");
    assert_eq!(textbox.cursor().unwrap(), 5);
    textbox.set_cursor(4).unwrap();
    assert_eq!(textbox.cursor().unwrap(), 2);
    let editor = textbox.node_interface().unwrap().query::<ITextEditorAbi>().unwrap();
    let text = editor.query::<ITextContentAbi>().unwrap();
    assert_eq!(editor.query::<IUnknown>().unwrap().as_ptr(), text.query::<IUnknown>().unwrap().as_ptr());
    let common = textbox.node_interface().unwrap();
    common.take_change().unwrap();
    textbox.set_text("é\n☃").unwrap();
    assert_eq!(textbox.cursor().unwrap(), 5);
    assert_eq!(common.take_change().unwrap(), GeometryChange::NO_CHANGE);
    assert_eq!(textbox.as_interface().set_text(NcCopyFromBufferOffset::new(0, &[0xff])), Err(NC_E_INVALID_ARG));
    assert_eq!(textbox.text().unwrap(), "é☃");
    textbox.clear().unwrap();
    assert_eq!(textbox.cursor().unwrap(), 0);
    assert_eq!(common.take_change().unwrap(), GeometryChange::CHANGED);

    let block = ui.widgets().text_block("one\r\ntwo", TextWrapping::NONE).unwrap();
    assert_eq!(block.text().unwrap(), "one\ntwo");
    let common = block.node_interface().unwrap();
    common.take_change().unwrap();
    block.set_wrapping(TextWrapping::WORD).unwrap();
    assert_eq!(block.wrapping().unwrap(), TextWrapping::WORD);
    assert_eq!(common.take_change().unwrap(), GeometryChange::CHANGED);
    assert_eq!(block.set_wrapping(TextWrapping::from_raw(99)), Err(NC_E_INVALID_ARG));
    assert_eq!(common.take_change().unwrap(), GeometryChange::NO_CHANGE);
    runtime.close().unwrap();
    assert_eq!(textbox.text(), Err(UI_E_CLOSED));
    assert_eq!(checkbox.set_checked(false), Err(UI_E_CLOSED));
}

/// Numeric/range setters keep validation atomic and distinguish paint-only values.
#[test]
fn builtin_numeric_and_scrollbar_facades_reject_invalid_inputs_without_losing_state() {
    let runtime = runtime();
    let ui = microui_redux::com::Ui::from_interface(runtime).unwrap();
    let slider = ui
        .widgets()
        .slider(SliderOptions {
            value: 5.0,
            low: 0.0,
            high: 10.0,
            step: 1.0,
            precision: 1,
        })
        .unwrap();
    let common = slider.node_interface().unwrap();
    common.take_change().unwrap();
    assert_eq!(slider.set_value(f32::NAN), Err(NC_E_INVALID_ARG));
    assert_eq!(slider.value().unwrap(), 5.0);
    assert_eq!(common.take_change().unwrap(), GeometryChange::NO_CHANGE);
    slider.set_value(200.0).unwrap();
    assert_eq!(slider.value().unwrap(), 10.0);
    assert_eq!(common.take_change().unwrap(), GeometryChange::CHANGED);
    assert!(
        ui.widgets()
            .slider(SliderOptions {
                value: 0.0,
                low: 2.0,
                high: 1.0,
                step: 0.0,
                precision: 0
            })
            .is_err()
    );
    assert!(ui.widgets().number(NumberOptions { value: 0.0, step: 1.0, precision: 255 }).is_err());
    let number = ui.widgets().number(NumberOptions { value: 4.0, step: 0.5, precision: 2 }).unwrap();
    number.set_value(-100.0).unwrap();
    assert_eq!(number.value().unwrap(), -100.0);
    assert!(!number.is_editing().unwrap());

    let scrollbar = ui
        .widgets()
        .scrollbar(ScrollRange {
            axis: ScrollAxis::VERTICAL,
            viewport: 50,
            content: 100,
            offset: 80,
        })
        .unwrap();
    assert_eq!(scrollbar.range().unwrap().offset, 50);
    let common = scrollbar.node_interface().unwrap();
    common.take_change().unwrap();
    assert_eq!(scrollbar.set_lengths(-1, 100), Err(NC_E_INVALID_ARG));
    assert_eq!(scrollbar.range().unwrap().viewport, 50);
    scrollbar.set_lengths(90, 100).unwrap();
    assert_eq!(scrollbar.range().unwrap().offset, 10);
    scrollbar.set_offset(i32::MAX).unwrap();
    assert_eq!(scrollbar.range().unwrap().offset, 10);
    scrollbar.set_offset(-1).unwrap();
    assert_eq!(scrollbar.range().unwrap().offset, 0);
    assert_eq!(common.take_change().unwrap(), GeometryChange::NO_CHANGE);
    scrollbar.set_lengths(100, 100).unwrap();
    assert_ne!(common.properties().unwrap().options.bits & WidgetOptions::NO_INTERACT.bits, 0);
    let swatch = ui.widgets().color_swatch(0xff102030, "RGBA").unwrap();
    let common = swatch.node_interface().unwrap();
    common.take_change().unwrap();
    swatch.set_fill(0xffaabbcc).unwrap();
    assert_eq!(swatch.fill().unwrap(), 0xffaabbcc);
    assert_eq!(common.take_change().unwrap(), GeometryChange::NO_CHANGE);
}

/// Real routing and complete painting exercise all migrated classes through the facade.
#[test]
fn builtin_facade_routes_text_and_space_and_renders_every_concrete_leaf() {
    let runtime = runtime();
    let ui = microui_redux::com::Ui::from_interface(runtime.clone()).unwrap();
    let textbox = ui.widgets().textbox("start").unwrap();
    let window = ui.window("editor", RECT, &textbox).unwrap();
    ui.input(
        UiInput::Key(KeyStroke {
            key: KeyCode::TAB,
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

    let checkbox = ui.widgets().checkbox("toggle", false).unwrap();
    let window = ui.window("checkbox", RECT, &checkbox).unwrap();
    ui.input(UiInput::Cancel, "").unwrap();
    ui.input(
        UiInput::Key(KeyStroke {
            key: KeyCode::TAB,
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
            key: KeyCode::SPACE,
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
        ui.widgets().button("button").unwrap().node_interface().unwrap(),
        ui.widgets().text_block("display\ntext", TextWrapping::WORD).unwrap().node_interface().unwrap(),
        ui.widgets()
            .slider(SliderOptions {
                value: 2.0,
                low: 0.0,
                high: 10.0,
                step: 1.0,
                precision: 0,
            })
            .unwrap()
            .node_interface()
            .unwrap(),
        ui.widgets()
            .number(NumberOptions { value: 42.0, step: 1.0, precision: 0 })
            .unwrap()
            .node_interface()
            .unwrap(),
        ui.widgets().color_swatch(0xff00ffff, "swatch").unwrap().node_interface().unwrap(),
        ui.widgets().list_item("item").unwrap().node_interface().unwrap(),
        ui.widgets().list_box("list box").unwrap().node_interface().unwrap(),
        ui.widgets()
            .scrollbar(ScrollRange {
                axis: ScrollAxis::VERTICAL,
                viewport: 40,
                content: 120,
                offset: 30,
            })
            .unwrap()
            .node_interface()
            .unwrap(),
    ];
    for root in roots {
        let window = mount(&runtime, &root).unwrap();
        ui.update(SIZE).unwrap();
        ui.render(SIZE, renderer.as_com_ref()).unwrap();
        window.close().unwrap();
        assert_eq!(root.properties(), Err(UI_E_CLOSED));
    }
}

/// Copied paint observations survive the deliberately failing recorder's call.
#[derive(Default)]
struct BuiltinPaintProbe {
    /// Exact local geometry copied by the bulk boundary.
    vertices: RefCell<Vec<RenderVertex>>,
    /// One recoverable recorder failure, used to verify workspace restoration.
    fail: Cell<bool>,
}

#[com_class(interfaces(IPainter))]
/// A foreign-style painter that reenters a read-only editing method during copying.
struct BuiltinPainter {
    /// The producer's editing projection, independent of its paint helper's storage.
    button: ComPtr<IButtonAbi>,
    /// Test-owned geometry and error observations.
    probe: Rc<BuiltinPaintProbe>,
}
impl IPainter for BuiltinPainter {
    /// This button contains no external image; receiving one would bypass the expected built-in atlas path.
    fn texture(&self, _scope: PaintScope, _rect: Rect, _rgba: u32, _texture: ComRef<'_, ITextureAbi>) -> Result<(), NcStatus> {
        panic!("unexpected external texture in button paint")
    }
    /// The complete button algorithm cannot emit application GPU work.
    fn item(&self, _scope: PaintScope, _rect: Rect, _item: ComRef<'_, IRenderItemAbi>) -> Result<(), NcStatus> {
        panic!("unexpected custom item in button paint")
    }
    /// Confirms no semantic RefCell guard spans this outward COM call.
    fn triangles(&self, _scope: PaintScope, vertices: NcCopyFromBufferOffset<'_, RenderVertex>) -> Result<(), NcStatus> {
        assert_eq!(self.button.label()?.to_string()?, "AB");
        *self.probe.vertices.borrow_mut() = vertices.as_slice().to_vec();
        if self.probe.fail.replace(false) { Err(UI_E_RENDER) } else { Ok(()) }
    }
    /// Built-in painting uses the bulk path after resolving its complete theme state.
    fn fill(&self, _scope: PaintScope, _rect: Rect, _rgba: u32) -> Result<(), NcStatus> {
        panic!("expected one complete bulk batch")
    }
    /// Text expansion stays with the UI implementation and its immutable atlas.
    fn text(&self, _scope: PaintScope, _rect: Rect, _text: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        panic!("expected UI-owned text expansion")
    }
}

/// Button style changes retain the full algorithm and failed copying does not poison reuse.
#[test]
fn builtin_bulk_paint_preserves_alignment_icons_and_releases_semantic_borrows() {
    let ui = microui_redux::com::Ui::from_interface(runtime()).unwrap();
    let button = ui.widgets().button("AB").unwrap();
    button.set_options(WidgetOptions::NONE).unwrap();
    button.set_fill_states(FillStates::NONE).unwrap();
    let common = button.node_interface().unwrap();
    common.take_change().unwrap();
    assert_eq!(button.set_fill_states(FillStates { bits: 8 }), Err(NC_E_INVALID_ARG));
    assert_eq!(button.fill_states().unwrap(), FillStates::NONE);
    assert_eq!(common.take_change().unwrap(), GeometryChange::NO_CHANGE);
    button.set_icon_role(ContentIcon::CHECK).unwrap();
    assert_eq!(common.take_change().unwrap(), GeometryChange::CHANGED);
    button.set_icon_role(ContentIcon::CHECK).unwrap();
    assert_eq!(common.take_change().unwrap(), GeometryChange::NO_CHANGE);
    assert_eq!(button.set_named_icon("missing"), Err(nc_com::NC_E_NOT_FOUND));
    button.set_icon_role(ContentIcon::NONE).unwrap();
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
    let scope = PaintScope { invocation: 1 };
    probe.fail.set(true);
    assert_eq!(common.paint(scope, paint, painter.as_com_ref()), Err(UI_E_RENDER));
    common.paint(scope, paint, painter.as_com_ref()).unwrap();
    let left = probe.vertices.borrow().iter().map(|v| v.x).reduce(f32::min).unwrap();
    button.set_options(WidgetOptions::ALIGN_RIGHT).unwrap();
    common.paint(scope, paint, painter.as_com_ref()).unwrap();
    let right = probe.vertices.borrow().iter().map(|v| v.x).reduce(f32::min).unwrap();
    assert!(right > left + 40.0, "right alignment must reach the semantic paint algorithm");
    assert!(probe.vertices.borrow().iter().all(|v| v.x >= 0.0 && v.x <= 100.0 && v.y >= 0.0 && v.y <= 30.0));
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

mod events;

mod combo;
mod menus;
