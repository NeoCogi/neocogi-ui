//! Independent container policy exercises the same boundary available to a separate plugin.
use super::*;
use neocogi_ui::com::widget::WidgetState;

/// Deliberately retained service/token pairs probe expiry after their real nested invocation.
type RetainedScopes = Rc<RefCell<Vec<(ComPtr<ILayoutServiceAbi>, LayoutScope)>>>;

/// Shared observations deliberately retain services and both active and expired authority tokens.
#[derive(Default)]
struct ContainerProbe {
    /// Optional direct leaf probe receives each active parent scope for deliberate reentry.
    leaf_probe: RefCell<Option<Rc<WidgetProbe>>>,
    /// All scopes from every nested container, used to test restoration and stale-token rejection.
    scopes: RetainedScopes,
    /// Actual policy invocations, independent of node-cache hits.
    measures: Cell<usize>,
    /// Actual placement invocations, including a failed partial pass.
    placements: Cell<usize>,
    /// Injects a fallible preferred-size call.
    fail_measure: Cell<bool>,
    /// Injects failure after placing children, before publishing parent geometry.
    fail_place: Cell<bool>,
    /// Demonstrates responsive parent-controlled subtree participation.
    hidden: Cell<bool>,
    /// Releases the private writer during placement to exercise deferred subtree destruction.
    abandon: Cell<bool>,
    /// Optional reentrant edit attempted inside mount validation's foreign call.
    mount_edit: RefCell<Option<ComPtr<IChildrenEditorAbi>>>,
}

#[com_object(interfaces(IWidget, IContainerLayout))]
/// A custom container owns its common state, private editor and concrete layout policy inline.
struct TestContainer {
    /// Exactly the same static lifetime helper available to independent component authors.
    state: WidgetState,
    /// Private topology authority; IContainerLayout only publishes the distinct read-only view.
    editor: RefCell<Option<ComPtr<IChildrenEditorAbi>>>,
    /// Test-only state with no parent or runtime ownership cycle.
    probe: Rc<ContainerProbe>,
}
impl TestContainer {
    /// Borrows no editor cell across a COM call.
    fn editor(&self) -> Result<ComPtr<IChildrenEditorAbi>, NcStatus> {
        self.editor.borrow().clone().ok_or(UI_E_CLOSED)
    }
    /// Checks retained tokens before recording the current invocation for later rejection.
    fn observe(&self, scope: LayoutScope, layout: ComRef<'_, ILayoutServiceAbi>) {
        let old_scopes = self.probe.scopes.borrow().clone();
        for (service, old) in old_scopes {
            assert_eq!(service.child_count(old), Err(UI_E_EXPIRED_SCOPE));
        }
        self.probe.scopes.borrow_mut().push((layout.add_ref(), scope));
        let leaf = self.probe.leaf_probe.borrow().clone();
        if let Some(leaf) = leaf {
            *leaf.parent_layout.borrow_mut() = Some((layout.add_ref(), scope));
        }
    }
}
impl IWidget for TestContainer {
    /// Common authority carries no runtime tree pointer.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        Ok(self.state.host().add_ref())
    }
    /// Attachment must reject nested edge edits before committing the mount-once claim.
    fn mount(&mut self) -> Result<(), NcStatus> {
        let editor = self.probe.mount_edit.borrow().clone();
        if let Some(editor) = editor {
            assert_eq!(editor.clear(), Err(UI_E_BUSY));
        }
        self.state.mount()
    }
    /// Explicit container closure closes its private editor, while retained public views remain safe.
    fn close(&mut self) -> Result<(), NcStatus> {
        if self.state.close()? {
            let editor = self.editor.borrow().clone();
            if let Some(editor) = editor {
                editor.close()?.finish()?;
            }
        }
        Ok(())
    }
    /// This geometry-only container leaves pointer targeting to enabled descendants.
    fn properties(&self) -> Result<WidgetProperties, NcStatus> {
        Ok(WidgetProperties {
            options: self.state.requested_flags()?,
            keyboard: 0,
            frame: WidgetFrame::Panel,
        })
    }
    /// Common option changes share the same sticky invalidation as built-in widgets.
    fn replace_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.state.replace_flags(flags)
    }
    /// Sets current requested bits through the same widget-owned mutation authority.
    fn set_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.state.set_flags(flags)
    }
    /// Clears requested bits without copying effective properties back into settings.
    fn clear_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.state.clear_flags(flags)
    }
    /// This fixture's layout inputs change through setters only.
    fn update(&mut self, _state: WidgetUpdate, _event: WidgetEventAbi, _text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
        self.state.writable()?;
        Ok(GeometryChange::NoChange)
    }
    /// Returns sticky setter changes without exporting its concrete state.
    fn take_change(&mut self) -> Result<GeometryChange, NcStatus> {
        self.state.take_change()
    }
    /// The fixture has no surface geometry; actual leaf children exercise the renderer.
    fn paint(&mut self, _scope: PaintScope, _state: WidgetPaint, _painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
        self.state.readable()
    }
}
impl IContainerLayout for TestContainer {
    /// The runtime authenticates this stable host-issued identity before mounting the container.
    fn children(&self) -> Result<ComPtr<IChildrenAbi>, NcStatus> {
        self.editor()?.view()
    }
    /// A simple test column probes read-only phases, nested authority and measured leaf reuse.
    fn measure(&mut self, scope: LayoutScope, bounds: LayoutLimits, layout: ComRef<'_, ILayoutServiceAbi>) -> Result<Size, NcStatus> {
        self.state.readable()?;
        self.observe(scope, layout);
        self.probe.measures.set(self.probe.measures.get() + 1);
        assert_eq!(self.editor()?.clear(), Err(UI_E_BUSY));
        assert_eq!(layout.set_participation(scope, 0, LayoutParticipation::Active), Err(UI_E_BUSY));
        assert_eq!(layout.child_content_size(scope, 0), Err(UI_E_BUSY));
        if self.probe.fail_measure.get() {
            return Err(NC_E_INVALID_ARG);
        }
        let count = layout.child_count(scope)?;
        assert_eq!(self.editor()?.view()?.child_count()?, count);
        let mut size = Size::default();
        for index in 0..count {
            assert_eq!(self.editor()?.view()?.child(index).unwrap_err(), UI_E_BUSY);
            let child = layout.measure_child(scope, index, bounds)?;
            // A nested container's newer token must restore this parent token on return.
            assert_eq!(layout.child_count(scope)?, count);
            size.width = size.width.max(child.width);
            size.height += child.height;
        }
        Ok(size)
    }
    /// Places children through exact copied rectangles, then publishes one complete viewport result.
    fn place(&mut self, scope: LayoutScope, rect: Rect, layout: ComRef<'_, ILayoutServiceAbi>) -> Result<ContainerGeometry, NcStatus> {
        self.observe(scope, layout);
        self.probe.placements.set(self.probe.placements.get() + 1);
        let count = layout.child_count(scope)?;
        for index in 0..count {
            let participation = if self.probe.hidden.get() {
                LayoutParticipation::Hidden
            } else {
                LayoutParticipation::Active
            };
            layout.set_participation(scope, index, participation)?;
            layout.place_child(
                scope,
                index,
                Rect {
                    x: rect.x,
                    y: rect.y + index as i32 * 30,
                    width: rect.width,
                    height: 30,
                },
            )?;
            assert_eq!(layout.child_count(scope)?, count);
            assert!(layout.child_content_size(scope, index)?.height >= 0);
        }
        if self.probe.abandon.get() {
            let editor = self.editor.borrow_mut().take();
            drop(editor);
        }
        if self.probe.fail_place.get() {
            return Err(NC_E_INVALID_ARG);
        }
        Ok(ContainerGeometry::new(rect))
    }
    /// Concrete application policy can filter an event after geometric target selection.
    fn accepts_event(&self, _event: WidgetEventAbi, _text: NcCopyFromBufferOffset<'_, u8>) -> Result<bool, NcStatus> {
        Ok(false)
    }
}

/// Creates a custom container without any access to retained Node or runtime storage.
fn container(runtime: &ComPtr<UiRuntimeAbi>, scopes: RetainedScopes) -> (ComPtr<IWidgetAbi>, ComPtr<IChildrenEditorAbi>, Rc<ContainerProbe>) {
    let host = button(runtime).host().unwrap();
    let editor = host.create_children().unwrap();
    let probe = Rc::new(ContainerProbe { scopes, ..ContainerProbe::default() });
    let widget = TestContainer::from_value(TestContainer {
        state: WidgetState::new(host, WidgetOptions::NO_INTERACT),
        editor: RefCell::new(Some(editor.clone())),
        probe: probe.clone(),
    })
    .query()
    .unwrap();
    (widget, editor, probe)
}

/// Nested independent policies restore parent scopes; removal closes the one authoritative subtree.
#[test]
fn nested_container_layout_scopes_expire_and_teardown_closes_descendants() {
    let runtime = runtime();
    let history = Rc::default();
    let (parent, parent_editor, parent_probe) = container(&runtime, Rc::clone(&history));
    let (child, child_editor, child_probe) = container(&runtime, Rc::clone(&history));
    let mut leaf = button(&runtime);
    child_editor
        .insert(
            0,
            neocogi_ui_core_contracts::Mount::widget(leaf.query::<IWidgetAbi>().unwrap().as_com_ref()).as_value().unwrap(),
        )
        .unwrap()
        .finish()
        .unwrap();
    parent_editor
        .insert(0, neocogi_ui_core_contracts::Mount::widget(child.as_com_ref()).as_value().unwrap())
        .unwrap()
        .finish()
        .unwrap();
    let child_view = child_editor.view().unwrap();
    let window = mount(&runtime, &parent).unwrap();
    runtime.update(SIZE).unwrap();
    assert!(parent_probe.measures.get() > 0 && child_probe.measures.get() > 0);
    assert!(parent_probe.placements.get() > 0 && child_probe.placements.get() > 0);
    let (renderer, _) = renderer();
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    for (service, scope) in history.borrow().iter() {
        assert_eq!(service.child_count(*scope), Err(UI_E_EXPIRED_SCOPE));
        assert_eq!(service.child_placement_changed(*scope, 0), Err(UI_E_EXPIRED_SCOPE));
    }
    // Setter collection reaches descendants through the actual stored tree.
    leaf.set_label(NcCopyFromBufferOffset::new(0, b"a changed descendant")).unwrap();
    assert_eq!(runtime.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    runtime.update(SIZE).unwrap();
    window.close().unwrap();
    assert_eq!(child_view.child_count(), Err(UI_E_CLOSED));
    assert_eq!(leaf.label().unwrap_err(), UI_E_CLOSED);
}

/// Cyclic edges, aliased collections and reentrant mount edits fail before consuming legitimate ownership.
#[test]
fn container_topology_rejects_cycles_shared_collections_and_reentrant_edits() {
    let runtime = runtime();
    let (a, a_editor, a_probe) = container(&runtime, Rc::default());
    let (b, b_editor, _) = container(&runtime, Rc::default());
    a_editor
        .insert(0, neocogi_ui_core_contracts::Mount::widget(b.as_com_ref()).as_value().unwrap())
        .unwrap()
        .finish()
        .unwrap();
    assert_eq!(
        b_editor.insert(0, neocogi_ui_core_contracts::Mount::widget(a.as_com_ref()).as_value().unwrap()),
        Err(UI_E_OWNER)
    );
    assert_eq!(a_editor.view().unwrap().child_count().unwrap(), 1);
    assert_eq!(b_editor.view().unwrap().child_count().unwrap(), 0);
    *a_probe.mount_edit.borrow_mut() = Some(b_editor.clone());
    let window = mount(&runtime, &a).unwrap();
    let alias = TestContainer::from_value(TestContainer {
        state: WidgetState::new(a.host().unwrap(), WidgetOptions::NO_INTERACT),
        editor: RefCell::new(Some(a_editor.clone())),
        probe: Rc::default(),
    })
    .query()
    .unwrap();
    assert_eq!(mount(&runtime, &alias).unwrap_err(), UI_E_OWNER);
    runtime.update(SIZE).unwrap();
    window.close().unwrap();
}

/// Failed preferred sizes and partial child placement both force a fresh successful layout before rendering.
#[test]
fn failed_container_layout_retries_without_caching_failed_parent_geometry() {
    let runtime = runtime();
    let (root, editor, probe) = container(&runtime, Rc::default());
    let leaf = button(&runtime);
    editor
        .insert(
            0,
            neocogi_ui_core_contracts::Mount::widget(leaf.query::<IWidgetAbi>().unwrap().as_com_ref()).as_value().unwrap(),
        )
        .unwrap()
        .finish()
        .unwrap();
    let _window = mount(&runtime, &root).unwrap();
    // A later root must not receive speculative geometry after an earlier root fails.
    let (later_root, _later_editor, later_probe) = container(&runtime, Rc::default());
    let _later_window = mount(&runtime, &later_root).unwrap();
    probe.fail_measure.set(true);
    assert_eq!(runtime.update(SIZE), Err(NC_E_INVALID_ARG));
    assert_eq!(later_probe.measures.get(), 0);
    assert_eq!(later_probe.placements.get(), 0);
    let failed_measures = probe.measures.get();
    probe.fail_measure.set(false);
    probe.fail_place.set(true);
    assert_eq!(runtime.update(SIZE), Err(NC_E_INVALID_ARG));
    assert!(probe.measures.get() > failed_measures);
    assert_eq!(later_probe.measures.get(), 0);
    assert_eq!(later_probe.placements.get(), 0);
    let failed_placements = probe.placements.get();
    probe.fail_place.set(false);
    runtime.update(SIZE).unwrap();
    assert!(probe.placements.get() > failed_placements);
    assert!(later_probe.measures.get() > 0);
    assert!(later_probe.placements.get() > 0);
    let (renderer, _) = renderer();
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
}

/// Final writer release closes views immediately and defers actual widget close until observation ends.
#[test]
fn final_child_writer_release_during_layout_closes_retained_descendants() {
    let runtime = runtime();
    let (root, editor, probe) = container(&runtime, Rc::default());
    let leaf = button(&runtime);
    editor
        .insert(
            0,
            neocogi_ui_core_contracts::Mount::widget(leaf.query::<IWidgetAbi>().unwrap().as_com_ref()).as_value().unwrap(),
        )
        .unwrap()
        .finish()
        .unwrap();
    let view = editor.view().unwrap();
    drop(editor);
    let window = mount(&runtime, &root).unwrap();
    probe.abandon.set(true);
    assert_eq!(runtime.update(SIZE), Err(UI_E_CLOSED));
    assert_eq!(view.child_count(), Err(UI_E_CLOSED));
    assert_eq!(leaf.label().unwrap_err(), UI_E_CLOSED);
    // The retired child was closed after observation, so normal explicit root cleanup remains valid.
    window.close().unwrap();
}

/// Creates a custom leaf with copied geometry observations and no frame inset to obscure its assigned rectangle.
fn recorded_leaf(runtime: &ComPtr<UiRuntimeAbi>) -> (neocogi_ui::Widget<IWidgetAbi>, Rc<WidgetProbe>) {
    let mut button = button(runtime);
    button.replace_flags(WidgetOptions::NONE).unwrap();
    let probe = Rc::new(WidgetProbe::default());
    let object = RetainingWidget::from_value(RetainingWidget {
        lifetime: button.query().unwrap(),
        button,
        runtime: runtime.clone(),
        probe: probe.clone(),
    });
    (neocogi_ui::Widget::from_interface(object.query().unwrap()), probe)
}

/// An active parent token rejects direct reentry while its service method owns the frame for a leaf call.
#[test]
fn layout_service_rejects_direct_reentry_without_holding_an_interior_borrow() {
    let runtime = runtime();
    let (root, editor, probe) = container(&runtime, Rc::default());
    let (leaf, leaf_probe) = recorded_leaf(&runtime);
    *probe.leaf_probe.borrow_mut() = Some(leaf_probe.clone());
    editor
        .insert(0, neocogi_ui_core_contracts::Mount::widget(leaf.as_interface()).as_value().unwrap())
        .unwrap()
        .finish()
        .unwrap();
    let window = mount(&runtime, &root).unwrap();
    runtime.update(SIZE).unwrap();
    assert!(leaf_probe.measures.get() > 0);
    let (service, scope) = leaf_probe.parent_layout.borrow().clone().unwrap();
    assert_eq!(service.child_count(scope), Err(UI_E_EXPIRED_SCOPE));
    window.close().unwrap();
}

/// The production Linear class preserves exact/flexible tracks without a redundant container interface.
#[test]
fn builtin_linear_reuses_track_resolution_and_validates_edges_before_mounting() {
    let runtime = runtime();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let mut row = ui.widgets().linear(FlowDirection::LeftToRight, CrossSize::Stretch).unwrap();
    let (first, a) = recorded_leaf(&runtime);
    let (second, b) = recorded_leaf(&runtime);
    let (third, c) = recorded_leaf(&runtime);
    let invalid = LinearEdge::new(LayoutTrack::Flex { weight: f32::NAN });
    assert_eq!(row.insert(0, &first, invalid), Err(NC_E_INVALID_ARG));
    row.insert(0, &first, LinearEdge::new(LayoutTrack::Fixed { extent: 40 }))
        .unwrap()
        .finish()
        .unwrap();
    row.insert(1, &second, LinearEdge::new(LayoutTrack::Flex { weight: 1.0 }))
        .unwrap()
        .finish()
        .unwrap();
    row.insert(2, &third, LinearEdge::new(LayoutTrack::Flex { weight: 2.0 }))
        .unwrap()
        .finish()
        .unwrap();
    let common = row.node_interface().unwrap();
    assert_eq!(common.query::<IContainerAbi>().unwrap_err(), neocogi_com::NC_E_NOINTERFACE);
    assert_eq!(
        common.query::<IUnknown>().unwrap().as_raw().object,
        row.as_interface().add_ref().query::<IUnknown>().unwrap().as_raw().object
    );
    assert_eq!(
        row.children().unwrap().query::<IChildrenEditorAbi>().unwrap_err(),
        neocogi_com::NC_E_NOINTERFACE
    );
    let window = ui.window("Linear", RECT, &row).unwrap();
    runtime.update(SIZE).unwrap();
    let (first_rect, second_rect, third_rect) = (a.latest.get().unwrap().screen, b.latest.get().unwrap().screen, c.latest.get().unwrap().screen);
    assert_eq!(first_rect.width, 40);
    assert!(first_rect.x < second_rect.x && second_rect.x < third_rect.x);
    // Indivisible pixels go to the earlier flexible track; doubling that
    // rounded first share can differ from the remaining share by two pixels.
    assert!(second_rect.width > 0);
    assert!((third_rect.width - second_rect.width * 2).abs() <= 2);
    assert_eq!((first_rect.y, first_rect.height), (third_rect.y, third_rect.height));
    row.set_direction(FlowDirection::RightToLeft).unwrap();
    runtime.update(SIZE).unwrap();
    assert!(a.latest.get().unwrap().screen.x > b.latest.get().unwrap().screen.x);
    assert!(b.latest.get().unwrap().screen.x > c.latest.get().unwrap().screen.x);
    assert_eq!(a.latest.get().unwrap().screen.width, 40);
    assert_eq!(LayoutTrack::try_from(row.edge(0).unwrap().main).unwrap(), LayoutTrack::Fixed { extent: 40 });
    assert_eq!(row.set_cross_size(CrossSize::Fixed { extent: -1 }), Err(NC_E_INVALID_ARG));
    assert_eq!(row.cross_size().unwrap(), CrossSize::Stretch);
    let (renderer, _) = renderer();
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    window.close().unwrap();
    assert_eq!(a.closes.get(), 1);
    assert_eq!(b.closes.get(), 1);
    assert_eq!(c.closes.get(), 1);
}

/// The production Grid facade preserves span geometry and validates complete bulk replacements atomically.
#[test]
fn builtin_grid_reuses_span_placement_and_bulk_track_validation() {
    let runtime = runtime();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let columns = [LayoutTrack::Fixed { extent: 40 }, LayoutTrack::Fixed { extent: 60 }];
    let rows = [LayoutTrack::Fixed { extent: 20 }, LayoutTrack::Fixed { extent: 30 }];
    let mut grid = ui.widgets().grid(&columns, &rows).unwrap();
    let (first, a) = recorded_leaf(&runtime);
    let (second, b) = recorded_leaf(&runtime);
    let (third, c) = recorded_leaf(&runtime);
    assert_eq!(grid.insert(0, &first, GridExtent { columns: 0, rows: 1 }), Err(NC_E_INVALID_ARG));
    grid.insert(0, &first, GridExtent { columns: 1, rows: 1 }).unwrap().finish().unwrap();
    grid.insert(1, &second, GridExtent { columns: 1, rows: 1 }).unwrap().finish().unwrap();
    grid.insert(2, &third, GridExtent { columns: 2, rows: 1 }).unwrap().finish().unwrap();
    assert_eq!(grid.columns().unwrap(), columns);
    assert_eq!(grid.rows().unwrap(), rows);
    let window = ui.window("Grid", RECT, &grid).unwrap();
    runtime.update(SIZE).unwrap();
    let (first_rect, second_rect, third_rect) = (a.latest.get().unwrap().screen, b.latest.get().unwrap().screen, c.latest.get().unwrap().screen);
    assert_eq!((first_rect.width, first_rect.height), (40, 20));
    assert_eq!((second_rect.width, second_rect.height), (60, 20));
    let gap = second_rect.x - first_rect.x - 40;
    assert!(gap >= 0);
    assert_eq!(
        (third_rect.x, third_rect.y, third_rect.width, third_rect.height),
        (first_rect.x, first_rect.y + 20 + gap, 100 + gap, 30)
    );
    assert_eq!(
        grid.set_columns(&[LayoutTrack::Content, LayoutTrack::Flex { weight: f32::INFINITY }]),
        Err(NC_E_INVALID_ARG)
    );
    assert_eq!(grid.columns().unwrap(), columns);
    grid.set_columns(&[LayoutTrack::Fixed { extent: 70 }, LayoutTrack::Fixed { extent: 20 }])
        .unwrap();
    runtime.update(SIZE).unwrap();
    assert_eq!(a.latest.get().unwrap().screen.width, 70);
    assert_eq!(c.latest.get().unwrap().screen.width, 90 + gap);
    let (renderer, _) = renderer();
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    window.close().unwrap();
    assert_eq!(a.closes.get(), 1);
}

/// A failed old-child teardown cannot leave Linear metadata describing the replaced subtree.
#[test]
fn builtin_container_commits_replacement_metadata_before_reporting_failed_teardown() {
    let runtime = runtime();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let mut row = ui.widgets().linear(FlowDirection::LeftToRight, CrossSize::Stretch).unwrap();
    let (first, previous) = recorded_leaf(&runtime);
    previous.fail_close.set(true);
    row.insert(0, &first, LinearEdge::new(LayoutTrack::Fixed { extent: 40 }))
        .unwrap()
        .finish()
        .unwrap();
    let window = ui.window("replace", RECT, &row).unwrap();
    runtime.update(SIZE).unwrap();
    let (second, replacement) = recorded_leaf(&runtime);
    *replacement.close_on_mount.borrow_mut() = Some(row.node_interface().unwrap());
    let outcome = row.replace(0, &second, LinearEdge::new(LayoutTrack::Fixed { extent: 90 })).unwrap();
    assert_eq!(outcome.finish(), Err(NC_E_INVALID_ARG));
    assert_eq!(replacement.close_mount_result.get(), Some(Err(NC_E_BUSY)));
    assert_eq!(LayoutTrack::try_from(row.edge(0).unwrap().main).unwrap(), LayoutTrack::Fixed { extent: 90 });
    runtime.update(SIZE).unwrap();
    assert_eq!(replacement.latest.get().unwrap().screen.width, 90);
    assert_eq!(previous.closes.get(), 1);
    row.remove(0).unwrap().finish().unwrap();
    assert_eq!(row.children().unwrap().child_count().unwrap(), 0);
    runtime.update(SIZE).unwrap();
    window.close().unwrap();
}

/// Composed scrolling retains real bar identities and observes shared requests without relying on ordinary update callbacks.
#[test]
fn builtin_scroll_area_uses_real_children_and_shared_sticky_invalidation() {
    let runtime = runtime();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let text = format!("{}\n{}", "wide".repeat(80), "another line\n".repeat(50));
    let mut content = ui.widgets().text_block(&text, TextWrapping::None).unwrap();
    let mut scroll = ui.widgets().scroll_area(&content, true).unwrap();
    let root = scroll.node_interface().unwrap();
    assert_eq!(root.query::<IContainerAbi>().unwrap_err(), neocogi_com::NC_E_NOINTERFACE);
    let structural = root.query::<IContainerLayoutAbi>().unwrap().children().unwrap();
    assert_eq!(structural.child_count().unwrap(), 3);
    let mut horizontal = structural.child(1).unwrap().query::<ScrollbarAbi>().unwrap();
    let vertical = structural.child(2).unwrap().query::<ScrollbarAbi>().unwrap();
    assert_eq!(horizontal.range().unwrap().axis, ScrollAxis::Horizontal);
    assert_eq!(vertical.range().unwrap().axis, ScrollAxis::Vertical);
    let content_identity = content.node_interface().unwrap().query::<IUnknown>().unwrap();
    assert_eq!(
        scroll.content().unwrap().node_interface().unwrap().query::<IUnknown>().unwrap().as_raw().object,
        content_identity.as_raw().object
    );
    let window = ui.window("scroll", RECT, &scroll).unwrap();
    runtime.update(SIZE).unwrap();
    let info = scroll.info().unwrap();
    assert!(info.maximum.x > 0 && info.maximum.y > 0);
    assert!(info.viewport.width > 0 && info.viewport.height > 0);
    let (renderer, _) = renderer();
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();

    // A request through a retained actual bar affects its containing viewport;
    // the parent collects its shared marker even before an eventless update.
    horizontal.set_offset(info.maximum.x / 2).unwrap();
    assert_eq!(scroll.info().unwrap().requested.x, info.maximum.x / 2);
    assert_eq!(runtime.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    runtime.update(SIZE).unwrap();
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    scroll.scroll_to_end().unwrap();
    content.set_text(&format!("{text}{}", "extra line\n".repeat(50))).unwrap();
    runtime.update(SIZE).unwrap();
    let next = scroll.info().unwrap();
    assert_eq!(next.requested.y, next.maximum.y);
    assert!(next.maximum.y > info.maximum.y);
    assert_eq!(next.requested.x, info.maximum.x / 2);
    scroll.set_enabled(false).unwrap();
    runtime.update(SIZE).unwrap();
    let disabled = scroll.info().unwrap();
    assert!(!disabled.enabled);
    assert_eq!(disabled.requested, Point::default());
    assert_eq!(disabled.maximum, Point::default());
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    scroll.set_enabled(true).unwrap();
    let replacement = ui.widgets().text_block("short replacement", TextWrapping::None).unwrap();
    scroll.replace_content(&replacement).unwrap().finish().unwrap();
    assert_eq!(content.text(), Err(UI_E_CLOSED));
    runtime.update(SIZE).unwrap();
    assert_eq!(scroll.info().unwrap().maximum, Point::default());
    assert_eq!(structural.child_count().unwrap(), 3);
    window.close().unwrap();
    assert_eq!(horizontal.range().unwrap_err(), UI_E_CLOSED);
    assert_eq!(vertical.range().unwrap_err(), UI_E_CLOSED);
    assert_eq!(replacement.text(), Err(UI_E_CLOSED));
}

/// Nested wheel routing consumes movable inner scrolling and bubbles only after its range is exhausted.
#[test]
fn nested_com_scroll_areas_preserve_boundary_bubbling() {
    let runtime = runtime();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let text = ui.widgets().text_block(&"nested content\n".repeat(60), TextWrapping::None).unwrap();
    let mut inner = ui.widgets().scroll_area(&text, true).unwrap();
    let tail = ui.widgets().text_block(&"outer content\n".repeat(60), TextWrapping::None).unwrap();
    let mut column = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    column
        .insert(0, &inner, LinearEdge::new(LayoutTrack::Fixed { extent: 110 }))
        .unwrap()
        .finish()
        .unwrap();
    column.insert(1, &tail, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    let outer = ui.widgets().scroll_area(&column, true).unwrap();
    let window = ui.window("nested scroll", RECT, &outer).unwrap();
    runtime.update(SIZE).unwrap();
    assert!(inner.info().unwrap().maximum.y > 0);
    assert!(outer.info().unwrap().maximum.y > 0);
    runtime
        .input(UiInput::Pointer(Point { x: 100, y: 100 }).into(), NcCopyFromBufferOffset::new(0, &[]))
        .unwrap();
    runtime
        .input(UiInput::Scroll(Point { x: 0, y: 20 }).into(), NcCopyFromBufferOffset::new(0, &[]))
        .unwrap();
    runtime.update(SIZE).unwrap();
    assert!(inner.info().unwrap().requested.y > 0);
    assert_eq!(outer.info().unwrap().requested.y, 0);
    inner.scroll_to_end().unwrap();
    runtime.update(SIZE).unwrap();
    runtime
        .input(UiInput::Scroll(Point { x: 0, y: 20 }).into(), NcCopyFromBufferOffset::new(0, &[]))
        .unwrap();
    runtime.update(SIZE).unwrap();
    let end = inner.info().unwrap();
    assert_eq!(end.requested.y, end.maximum.y);
    assert!(outer.info().unwrap().requested.y > 0);
    let (renderer, _) = renderer();
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    window.close().unwrap();
}

/// Expansion input and retained child edits share real COM identities, including while the body is hidden.
#[test]
fn disclosure_com_header_controls_the_actual_body_without_a_parent_backpointer() {
    let runtime = runtime();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let mut disclosure = ui.widgets().disclosure("Tree", DisclosureStyle::Tree, false).unwrap();
    let mut header = disclosure.header().unwrap();
    let mut body = disclosure.body().unwrap();
    let root = disclosure.node_interface().unwrap();
    assert_eq!(root.query::<IContainerAbi>().unwrap_err(), neocogi_com::NC_E_NOINTERFACE);
    let children = root.query::<IContainerLayoutAbi>().unwrap().children().unwrap();
    assert_eq!(children.child_count().unwrap(), 2);
    assert_eq!(
        children.child(0).unwrap().query::<IUnknown>().unwrap().as_raw().object,
        header.node_interface().unwrap().query::<IUnknown>().unwrap().as_raw().object
    );
    assert_eq!(
        children.child(1).unwrap().query::<IUnknown>().unwrap().as_raw().object,
        body.node_interface().unwrap().query::<IUnknown>().unwrap().as_raw().object
    );
    let (leaf, probe) = recorded_leaf(&runtime);
    body.insert(0, &leaf, LinearEdge::new(LayoutTrack::Fixed { extent: 33 }))
        .unwrap()
        .finish()
        .unwrap();
    let window = ui.window("disclosure", RECT, &disclosure).unwrap();
    runtime.update(SIZE).unwrap();
    assert!(probe.latest.get().is_none());
    let (renderer, _) = renderer();
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    // A single Tab reaches the real header. Right expands a tree even though
    // the header's preferred size and its ordinary update result do not change.
    for key in [KeyCode::Tab, KeyCode::ArrowRight] {
        runtime
            .input(
                UiInput::Key(KeyStroke {
                    key,
                    value: 0,
                    pressed: true,
                    repeat: false,
                    modifiers: 0,
                })
                .into(),
                NcCopyFromBufferOffset::new(0, &[]),
            )
            .unwrap();
        runtime.update(SIZE).unwrap();
    }
    assert!(disclosure.expanded().unwrap());
    runtime.update(SIZE).unwrap();
    assert_eq!(probe.latest.get().unwrap().screen.height, 33);
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    disclosure.set_expanded(false).unwrap();
    assert_eq!(runtime.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    runtime.update(SIZE).unwrap();
    body.set_edge(0, LinearEdge::new(LayoutTrack::Fixed { extent: 61 })).unwrap();
    header.set_text("Longer tree label").unwrap();
    disclosure.set_expanded(true).unwrap();
    runtime.update(SIZE).unwrap();
    assert_eq!(probe.latest.get().unwrap().screen.height, 61);
    assert_eq!(header.text().unwrap(), "Longer tree label");
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    window.close().unwrap();
    assert_eq!(header.text(), Err(UI_E_CLOSED));
    assert_eq!(body.children().unwrap_err(), UI_E_CLOSED);
    assert_eq!(probe.closes.get(), 1);
}

/// Multiline editing uses one real text leaf; its scroll/root capabilities cannot replace that fixed content.
#[test]
fn textarea_com_preserves_inherited_editing_caret_reveal_and_fixed_content_identity() {
    let runtime = runtime();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let mut area = ui.widgets().text_area_unobserved("a☃\r\nb\r", TextWrapping::None, true).unwrap();
    assert_eq!(area.text().unwrap(), "a☃\nb\n");
    let root = area.node_interface().unwrap();
    let identity = root.query::<IUnknown>().unwrap();
    let concrete = area.as_interface().add_ref();
    assert_eq!(concrete.query::<IUnknown>().unwrap().as_raw().object, identity.as_raw().object);
    assert_eq!(root.query::<ITextEditorAbi>().unwrap_err(), neocogi_com::NC_E_NOINTERFACE);
    assert_eq!(root.query::<ScrollAreaAbi>().unwrap_err(), neocogi_com::NC_E_NOINTERFACE);
    let mut scroll = area.scrolling().unwrap();
    assert_eq!(scroll.node_interface().unwrap().query::<IUnknown>().unwrap().as_raw().object, identity.as_raw().object);
    let structural = root.query::<IContainerLayoutAbi>().unwrap().children().unwrap();
    assert_eq!(structural.child_count().unwrap(), 3);
    let surface = structural.child(0).unwrap().query::<IContainerLayoutAbi>().unwrap();
    let editor = surface.children().unwrap().child(0).unwrap().query::<IMultilineEditorAbi>().unwrap();
    assert_ne!(editor.query::<IUnknown>().unwrap().as_raw().object, identity.as_raw().object);
    assert_eq!(editor.query::<TextAreaAbi>().unwrap_err(), neocogi_com::NC_E_NOINTERFACE);
    area.set_cursor(3).unwrap();
    assert_eq!(area.cursor().unwrap(), 1); // A byte inside the snowman clamps to its preceding scalar boundary.
    assert_eq!(editor.cursor().unwrap(), 1);
    let window = ui.window("editor", RECT, &area).unwrap();
    runtime.update(SIZE).unwrap();
    let (renderer, probe) = renderer();
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    assert!(probe.calls.get() > 0);
    runtime
        .input(
            UiInput::Key(KeyStroke {
                key: KeyCode::Tab,
                value: 0,
                pressed: true,
                repeat: false,
                modifiers: 0,
            })
            .into(),
            NcCopyFromBufferOffset::new(0, &[]),
        )
        .unwrap();
    runtime.update(SIZE).unwrap();
    runtime.input(UiInput::Text.into(), NcCopyFromBufferOffset::new(0, "é".as_bytes())).unwrap();
    runtime.update(SIZE).unwrap();
    assert_eq!(area.text().unwrap(), "aé☃\nb\n");
    runtime
        .input(
            UiInput::Key(KeyStroke {
                key: KeyCode::Enter,
                value: 0,
                pressed: true,
                repeat: false,
                modifiers: 0,
            })
            .into(),
            NcCopyFromBufferOffset::new(0, &[]),
        )
        .unwrap();
    runtime.update(SIZE).unwrap();
    assert_eq!(area.text().unwrap(), "aé\n☃\nb\n");

    let document = "a moderately long line of text\n".repeat(90);
    area.set_text(&document).unwrap();
    assert_eq!(runtime.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    runtime.update(SIZE).unwrap();
    assert!(scroll.info().unwrap().maximum.y > 0);
    assert_eq!(scroll.info().unwrap().requested.y, scroll.info().unwrap().maximum.y);
    // Cursor-only editing returns NoChange from the text algorithm, but caret
    // reveal marks the shared scroll model and produces current parent placement.
    area.set_cursor(0).unwrap();
    assert_eq!(runtime.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    runtime.update(SIZE).unwrap();
    assert_eq!(scroll.info().unwrap().requested.y, 0);
    assert_eq!(area.text().unwrap(), document);
    area.set_cursor(u64::MAX).unwrap();
    runtime.update(SIZE).unwrap();
    assert_eq!(area.cursor().unwrap(), document.len() as u64);
    assert_eq!(scroll.info().unwrap().requested.y, scroll.info().unwrap().maximum.y);
    area.set_wrapping(TextWrapping::Word).unwrap();
    area.set_font_role(FontRole::Mono).unwrap();
    runtime.update(SIZE).unwrap();
    assert_eq!(editor.wrapping().unwrap(), TextWrapping::Word);
    // The composed text area forwards the same policy to its editable child.
    area.set_wrapping(TextWrapping::WordOrCharacter).unwrap();
    assert_eq!(editor.wrapping().unwrap(), TextWrapping::WordOrCharacter);
    runtime.update(SIZE).unwrap();
    assert_eq!(area.set_wrapping(TextWrapping::from_raw(255)), Err(neocogi_com::NC_E_INVALID_TAG));
    runtime.render(SIZE, renderer.as_com_ref()).unwrap();
    scroll.set_enabled(false).unwrap();
    runtime.update(SIZE).unwrap();
    assert_eq!(scroll.info().unwrap().maximum, Point::default());
    window.close().unwrap();
    assert_eq!(area.text(), Err(UI_E_CLOSED));
    assert_eq!(editor.cursor(), Err(UI_E_CLOSED));
    assert_eq!(scroll.info().unwrap_err(), UI_E_CLOSED);
}

/// Common frame policy remains observable on the actual header object and invalidates only on edits.
#[test]
fn disclosure_headers_use_the_common_frame_policy_for_both_variants() {
    for (style, frame, defaults) in [
        (DisclosureStyle::Header, WidgetFrame::Button, WidgetOptions::FRAME),
        (DisclosureStyle::Tree, WidgetFrame::Item, WidgetOptions::NONE),
    ] {
        let runtime = runtime();
        let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
        let disclosure = ui.widgets().disclosure("Header", style, false).unwrap();
        let mut header = disclosure.header().unwrap().node_interface().unwrap();
        assert_eq!(header.properties().unwrap().frame, frame);
        assert_eq!(header.properties().unwrap().options, defaults);
        let window = ui.window("framing", RECT, &disclosure).unwrap();
        let (renderer, _) = renderer();
        // The common traversal supplies the frame once; the content algorithm consumes its
        // inset rectangle. A policy change must make render wait for the new measurement.
        header.replace_flags(WidgetOptions::NONE).unwrap();
        runtime.update(SIZE).unwrap();
        runtime.render(SIZE, renderer.as_com_ref()).unwrap();
        header.replace_flags(WidgetOptions::FRAME).unwrap();
        assert_eq!(header.properties().unwrap().options, WidgetOptions::FRAME);
        assert_eq!(runtime.render(SIZE, renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
        runtime.update(SIZE).unwrap();
        runtime.render(SIZE, renderer.as_com_ref()).unwrap();
        header.replace_flags(WidgetOptions::FRAME).unwrap();
        assert_eq!(header.take_change().unwrap(), GeometryChange::NoChange);
        window.close().unwrap();
    }
}

/// Reentrant construction can reject a different container's child-close
/// preflight after its widget authority check succeeds. That refusal must not
/// poison the parent terminal flag or skip descendants on the later legal call.
#[test]
fn refused_descendant_close_leaves_parent_retryable() {
    let runtime = runtime();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let mut victim = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let retained = ui.widgets().button_unobserved("retained descendant").unwrap();
    victim
        .insert(0, &retained, LinearEdge::new(LayoutTrack::Flex { weight: 1.0 }))
        .unwrap()
        .finish()
        .unwrap();
    let mut unrelated = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let (candidate, probe) = recorded_leaf(&runtime);
    *probe.close_on_mount.borrow_mut() = Some(victim.node_interface().unwrap());
    unrelated
        .insert(0, &candidate, LinearEdge::new(LayoutTrack::Flex { weight: 1.0 }))
        .unwrap()
        .finish()
        .unwrap();
    assert_eq!(probe.close_mount_result.get(), Some(Err(UI_E_BUSY)));
    assert!(victim.node_interface().unwrap().properties().is_ok());
    assert_eq!(retained.label().unwrap(), "retained descendant");
    victim.node_interface().unwrap().close().unwrap();
    assert_eq!(retained.label(), Err(UI_E_CLOSED));
    assert_eq!(victim.node_interface().unwrap().properties(), Err(UI_E_CLOSED));
    victim.node_interface().unwrap().close().unwrap();
}

/// A diagnostic after child retirement must not skip local terminal cleanup or
/// cause another child close on retry. It cannot undo the committed removal.
#[test]
fn descendant_cleanup_diagnostic_still_completes_parent_close() {
    let runtime = runtime();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let mut parent = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let (child, probe) = recorded_leaf(&runtime);
    probe.fail_close.set(true);
    parent
        .insert(0, &child, LinearEdge::new(LayoutTrack::Flex { weight: 1.0 }))
        .unwrap()
        .finish()
        .unwrap();
    let mut parent = parent.node_interface().unwrap();
    assert_eq!(parent.close(), Err(NC_E_INVALID_ARG));
    assert_eq!(parent.properties(), Err(UI_E_CLOSED));
    assert_eq!(probe.closes.get(), 1);
    parent.close().unwrap();
    assert_eq!(probe.closes.get(), 1);
}

/// A captured lifetime makes COM receiver release observable without invoking a
/// mapper or application handler during terminal cleanup.
struct MappingLifetime {
    /// Independent test log retained after every widget and sink is destroyed.
    released: Rc<RefCell<Vec<&'static str>>>,
    /// Which widget owned this receiver's last reference.
    label: &'static str,
}
impl Drop for MappingLifetime {
    /// Captures must be released in descendant-first order exactly once.
    fn drop(&mut self) {
        self.released.borrow_mut().push(self.label);
    }
}

/// Explicit closure and final allocation release share descendant-first cleanup;
/// dropping a mere cloned handle must never close a live widget.
#[test]
fn mappings_release_children_before_parent_on_close_and_final_drop() {
    use neocogi_com::events::MessageSinkInterface;
    for explicit in [false, true] {
        let runtime = runtime();
        let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
        let released = Rc::new(RefCell::new(Vec::new()));
        let lifetime = MappingLifetime {
            released: released.clone(),
            label: "parent",
        };
        let sink = SplitViewSinkAbi::receiver(move |_| {
            let _ = &lifetime;
            panic!("close must not invoke mapping")
        })
        .unwrap();
        let mut parent = ui
            .widgets()
            .split_view_with_sink(
                SplitOptions {
                    axis: SplitAxis::Horizontal,
                    position: 30,
                    minimum_first: 0,
                    minimum_second: 0,
                    handle_width: 4,
                },
                Some(sink.as_com_ref()),
            )
            .unwrap();
        drop(sink);
        let mut children = Vec::new();
        for label in ["first", "second"] {
            let lifetime = MappingLifetime { released: released.clone(), label };
            let sink = ButtonSinkAbi::receiver(move |_| {
                let _ = &lifetime;
                panic!("close must not invoke mapping")
            })
            .unwrap();
            let child = ui.widgets().button_with_sink(label, Some(sink.as_com_ref())).unwrap();
            drop(sink);
            parent.insert(children.len() as u32, &child).unwrap().finish().unwrap();
            children.push(child);
        }
        drop(parent.clone());
        assert!(released.borrow().is_empty());
        if explicit {
            parent.node_interface().unwrap().close().unwrap();
            parent.node_interface().unwrap().close().unwrap();
            assert_eq!(&*released.borrow(), &["first", "second", "parent"]);
        }
        drop(parent);
        assert_eq!(&*released.borrow(), &["first", "second", "parent"]);
        for child in &children {
            assert_eq!(child.label(), Err(UI_E_CLOSED));
        }
        drop(children);
        assert_eq!(&*released.borrow(), &["first", "second", "parent"]);
    }
}
