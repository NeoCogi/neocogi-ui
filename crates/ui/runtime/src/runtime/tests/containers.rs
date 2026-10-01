//! Independent container policy exercises the same boundary available to a separate plugin.
use super::*;
mod grid;
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
    /// Releases the private writer during placement to verify synchronous subtree destruction.
    abandon: Cell<bool>,
    /// Optional reentrant edit attempted inside mount validation's foreign call.
    mount_edit: RefCell<Option<WeakComBox<IChildrenEditorAbi>>>,
}

#[com_object(interfaces(IWidget, IContainerLayout), ownership = exclusive)]
/// A custom container owns its common state, private editor and concrete layout policy inline.
struct TestContainer {
    /// Exactly the same static lifetime helper available to independent component authors.
    state: WidgetState,
    /// Private topology authority; IContainerLayout only publishes the distinct read-only view.
    editor: Option<ComBox<IChildrenEditorAbi>>,
    /// Test-only state with no parent or runtime ownership cycle.
    probe: Rc<ContainerProbe>,
}
impl TestContainer {
    /// Borrows no editor cell across a COM call.
    fn editor(&self) -> Result<WeakComBox<IChildrenEditorAbi>, NcStatus> {
        self.editor.as_ref().map(ComBox::downgrade).ok_or(UI_E_CLOSED)
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
    /// This widget forwards outputs and has no private inbox.
    fn dispatch_next(&mut self) -> Result<bool, NcStatus> {
        self.state.writable()?;
        Ok(false)
    }

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
    fn update(&mut self, _state: WidgetUpdate, _event: WidgetEvent, _text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
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
            assert!(self.editor()?.view()?.child(index).is_ok());
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
            let view = self.editor()?.view()?;
            let child = view.child(0)?;
            let editor = self.editor.take();
            drop(editor);
            assert_eq!(view.child_count(), Err(UI_E_CLOSED));
            assert_eq!(child.properties(), Err(UI_E_CLOSED));
        }
        if self.probe.fail_place.get() {
            return Err(NC_E_INVALID_ARG);
        }
        Ok(ContainerGeometry::new(rect))
    }
    /// Concrete application policy can filter an event after geometric target selection.
    fn accepts_event(&self, _event: WidgetEvent, _text: NcCopyFromBufferOffset<'_, u8>) -> Result<bool, NcStatus> {
        Ok(false)
    }
}

/// Creates a custom container without any access to retained Node or runtime storage.
fn container(runtime: &ComPtr<UiRuntimeAbi>, scopes: RetainedScopes) -> (ComBox<IWidgetAbi>, WeakComBox<IChildrenEditorAbi>, Rc<ContainerProbe>) {
    let host = button(runtime).host().unwrap();
    let editor = host.create_children().unwrap();
    let probe = Rc::new(ContainerProbe { scopes, ..ContainerProbe::default() });
    let editor_view = editor.downgrade();
    let widget = TestContainer::into_box(TestContainer {
        state: WidgetState::new(host, WidgetOptions::NO_INTERACT),
        editor: Some(editor),
        probe: probe.clone(),
    })
    .query()
    .unwrap();
    (widget, editor_view, probe)
}

/// Nested independent policies restore parent scopes; removal closes the one authoritative subtree.
#[test]
fn nested_container_layout_scopes_expire_and_teardown_closes_descendants() {
    let runtime = runtime();
    let history = Rc::default();
    let (parent, parent_editor, parent_probe) = container(&runtime, Rc::clone(&history));
    let (child, child_editor, child_probe) = container(&runtime, Rc::clone(&history));
    let leaf_owner = button(&runtime);
    let mut leaf = leaf_owner.downgrade();
    child_editor
        .insert(0, leaf_owner.query::<IWidgetAbi>().unwrap())
        .unwrap();
    parent_editor.insert(0, child).unwrap();
    let child_view = child_editor.view().unwrap();
    let window = mount(&runtime, parent).unwrap();
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
fn container_topology_rejects_cycles_and_reentrant_edits_without_retaining_owners() {
    let runtime = runtime();
    let (a, a_editor, _) = container(&runtime, Rc::default());
    let (b, b_editor, _) = container(&runtime, Rc::default());
    let observed = a.downgrade();
    a_editor.insert(0, b).unwrap();
    // B's editor is executing inside A's subtree. Rejection must return A
    // through that call before the application can decide to mount or drop it.
    let (status, a) = b_editor.insert(0, a).unwrap_err();
    assert_eq!(status, UI_E_OWNER);
    assert!(observed.is_alive());
    assert_eq!(a_editor.view().unwrap().child_count(), Ok(1));
    assert_eq!(b_editor.view().unwrap().child_count(), Ok(0));
    drop(a);
    assert!(!observed.is_alive());
    assert_eq!(a_editor.view().unwrap_err(), UI_E_CLOSED);
    assert_eq!(b_editor.view().unwrap_err(), UI_E_CLOSED);
    let (a, _, a_probe) = container(&runtime, Rc::default());
    let (b, b_editor, _) = container(&runtime, Rc::default());
    *a_probe.mount_edit.borrow_mut() = Some(b_editor);
    let window = mount(&runtime, a).unwrap();
    runtime.update(SIZE).unwrap();
    window.close().unwrap();
    drop(b);
}

/// Failed preferred sizes and partial child placement both force a fresh successful layout before rendering.
#[test]
fn failed_container_layout_retries_without_caching_failed_parent_geometry() {
    let runtime = runtime();
    let (root, editor, probe) = container(&runtime, Rc::default());
    let leaf = button(&runtime);
    editor.insert(0, leaf.query::<IWidgetAbi>().unwrap()).unwrap();
    let _window = mount(&runtime, root).unwrap();
    // A later root must not receive speculative geometry after an earlier root fails.
    let (later_root, _later_editor, later_probe) = container(&runtime, Rc::default());
    let _later_window = mount(&runtime, later_root).unwrap();
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

/// Destroying the exclusive writer expires views and destroys children when traversal unwinds.
#[test]
fn final_child_writer_release_during_layout_closes_retained_descendants() {
    let runtime = runtime();
    let (root, editor, probe) = container(&runtime, Rc::default());
    let leaf_owner = button(&runtime);
    let leaf = leaf_owner.downgrade();
    editor.insert(0, leaf_owner.query::<IWidgetAbi>().unwrap()).unwrap();
    let view = editor.view().unwrap();
    drop(editor);
    let window = mount(&runtime, root).unwrap();
    probe.abandon.set(true);
    assert_eq!(runtime.update(SIZE), Err(UI_E_CLOSED));
    assert_eq!(view.child_count(), Err(UI_E_CLOSED));
    assert_eq!(leaf.label().unwrap_err(), UI_E_CLOSED);
    // Child ownership was released; ordinary window removal remains valid.
    window.close().unwrap();
}

/// Creates a custom leaf with copied geometry observations and no frame inset to obscure its assigned rectangle.
fn recorded_leaf(runtime: &ComPtr<UiRuntimeAbi>) -> (neocogi_ui::OwnedWidget<IWidgetAbi>, Rc<WidgetProbe>) {
    let mut button = button(runtime);
    button.replace_flags(WidgetOptions::NONE).unwrap();
    let probe = Rc::new(WidgetProbe::default());
    let object = RetainingWidget::into_box(RetainingWidget {
        button,
        runtime: runtime.clone(),
        probe: probe.clone(),
    });
    (neocogi_ui::OwnedWidget::from_interface(object.query().unwrap()), probe)
}

/// An active parent token rejects direct reentry while its service method owns the frame for a leaf call.
#[test]
fn layout_service_rejects_direct_reentry_without_holding_an_interior_borrow() {
    let runtime = runtime();
    let (root, editor, probe) = container(&runtime, Rc::default());
    let (leaf, leaf_probe) = recorded_leaf(&runtime);
    *probe.leaf_probe.borrow_mut() = Some(leaf_probe.clone());
    editor.insert(0, leaf.into_interface()).unwrap();
    let window = mount(&runtime, root).unwrap();
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
    let row_owner = ui.widgets().linear(FlowDirection::LeftToRight, CrossSize::Stretch).unwrap();
    let mut row = row_owner.handle();
    let (first_owner, a) = recorded_leaf(&runtime);
    let first = first_owner.handle();
    let (second, b) = recorded_leaf(&runtime);
    let (third, c) = recorded_leaf(&runtime);
    let invalid = LinearEdge::new(LayoutTrack::Flex { weight: f32::NAN });
    let (status, first_owner) = row.insert(0, first_owner, invalid).unwrap_err();
    assert_eq!(status, NC_E_INVALID_ARG);
    assert!(first.as_interface().is_alive());
    assert_eq!(a.closes.get(), 0);
    row.insert(0, first_owner, LinearEdge::new(LayoutTrack::Fixed { extent: 40 }))
        .unwrap();
    row.insert(1, second, LinearEdge::new(LayoutTrack::Flex { weight: 1.0 }))
        .unwrap();
    row.insert(2, third, LinearEdge::new(LayoutTrack::Flex { weight: 2.0 }))
        .unwrap();
    let common = row.node_interface().unwrap();
    assert_eq!(common.query::<IContainerAbi>().unwrap_err(), neocogi_com::NC_E_NOINTERFACE);
    assert_eq!(
        common.query::<IUnknown>().unwrap().as_raw().object,
        row.as_interface().clone().query::<IUnknown>().unwrap().as_raw().object
    );
    assert_eq!(
        row.children().unwrap().query::<IChildrenEditorAbi>().unwrap_err(),
        neocogi_com::NC_E_NOINTERFACE
    );
    let window = ui.window("Linear", RECT, row_owner).unwrap();
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
    assert_eq!(row.edge(0).unwrap().main, LayoutTrack::Fixed { extent: 40 });
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
    let grid_owner = ui.widgets().grid(&columns, &rows, neocogi_ui::GridLimits { max_columns: 2, max_rows: 2 }).unwrap();
    let mut grid = grid_owner.handle();
    let (first_owner, a) = recorded_leaf(&runtime);
    let first = first_owner.handle();
    let (second, b) = recorded_leaf(&runtime);
    let (third, c) = recorded_leaf(&runtime);
    let (status, first_owner) = grid.insert(0, first_owner, GridExtent { columns: 0, rows: 1 }).unwrap_err();
    assert_eq!(status, NC_E_INVALID_ARG);
    assert!(first.as_interface().is_alive());
    assert_eq!(a.closes.get(), 0);
    grid.insert(0, first_owner, GridExtent { columns: 1, rows: 1 }).unwrap();
    grid.insert(1, second, GridExtent { columns: 1, rows: 1 }).unwrap();
    grid.insert(2, third, GridExtent { columns: 2, rows: 1 }).unwrap();
    assert_eq!(grid.columns().unwrap(), columns);
    assert_eq!(grid.rows().unwrap(), rows);
    let window = ui.window("Grid", RECT, grid_owner).unwrap();
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

/// Exercise span exclusion through generated COM calls and actual screen layout.
#[test]
fn builtin_grid_spans_never_overlap_a_tall_previous_child() {
    let runtime = runtime();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let owner = ui.widgets().grid(&[LayoutTrack::Fixed { extent: 30 }; 3], &[LayoutTrack::Fixed { extent: 20 }; 2], neocogi_ui::GridLimits { max_columns: 3, max_rows: 2 }).unwrap();
    let mut grid = owner.handle();
    let mut probes = Vec::new();
    for (index, edge) in [
        GridExtent { columns: 1, rows: 1 }, GridExtent { columns: 1, rows: 2 },
        GridExtent { columns: 1, rows: 1 }, GridExtent { columns: 2, rows: 1 },
    ].into_iter().enumerate() {
        let (child, probe) = recorded_leaf(&runtime);
        grid.insert(index as u32, child, edge).unwrap();
        probes.push(probe);
    }
    let window = ui.window("span exclusion", RECT, owner).unwrap();
    runtime.update(SIZE).unwrap();
    let tall = probes[1].latest.get().unwrap().screen;
    let last = probes[3].latest.get().unwrap().screen;
    assert!(last.x >= tall.x + tall.width);
    assert_eq!(last.x, probes[2].latest.get().unwrap().screen.x);
    assert_eq!(last.width, 30); // Requested two columns clamp to one at the right edge.
    assert!(last.y > tall.y);
    window.close().unwrap();
}

/// Replacement commits matching metadata; reentrant edits are rejected before mutation.
#[test]
fn builtin_container_replacement_preserves_metadata_and_rejects_reentry() {
    let runtime = runtime();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let row_owner = ui.widgets().linear(FlowDirection::LeftToRight, CrossSize::Stretch).unwrap();
    let mut row = row_owner.handle();
    let (first, previous) = recorded_leaf(&runtime);

    row.insert(0, first, LinearEdge::new(LayoutTrack::Fixed { extent: 40 }))
        .unwrap();
    let window = ui.window("replace", RECT, row_owner).unwrap();
    runtime.update(SIZE).unwrap();
    let (second, replacement) = recorded_leaf(&runtime);
    *replacement.clear_on_mount.borrow_mut() = Some(row.node_interface().unwrap());
    let outcome = row.replace(0, second, LinearEdge::new(LayoutTrack::Fixed { extent: 90 })).unwrap();
    assert_eq!(outcome, ());
    assert_eq!(replacement.clear_mount_result.get(), Some(Err(NC_E_BUSY)));
    assert_eq!(row.edge(0).unwrap().main, LayoutTrack::Fixed { extent: 90 });
    runtime.update(SIZE).unwrap();
    assert_eq!(replacement.latest.get().unwrap().screen.width, 90);
    assert_eq!(previous.closes.get(), 1);
    row.remove(0).unwrap();
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
    let content_owner = ui.widgets().text_block(&text, TextWrapping::None).unwrap();
    let mut content = content_owner.handle();
    let scroll_owner = ui.widgets().scroll_area(content_owner, true).unwrap();
    let mut scroll = scroll_owner.handle();
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
    let window = ui.window("scroll", RECT, scroll_owner).unwrap();
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
    let replacement_owner = ui.widgets().text_block("short replacement", TextWrapping::None).unwrap();
    let replacement = replacement_owner.handle();
    scroll.replace_content(replacement_owner).unwrap();
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
    let inner_owner = ui.widgets().scroll_area(text, true).unwrap();
    let mut inner = inner_owner.handle();
    let tail = ui.widgets().text_block(&"outer content\n".repeat(60), TextWrapping::None).unwrap();
    let mut column = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    column
        .insert(0, inner_owner, LinearEdge::new(LayoutTrack::Fixed { extent: 110 }))
        .unwrap();
    column.insert(1, tail, LinearEdge::new(LayoutTrack::Content)).unwrap();
    let outer_owner = ui.widgets().scroll_area(column, true).unwrap();
    let outer = outer_owner.handle();
    let window = ui.window("nested scroll", RECT, outer_owner).unwrap();
    runtime.update(SIZE).unwrap();
    assert!(inner.info().unwrap().maximum.y > 0);
    assert!(outer.info().unwrap().maximum.y > 0);
    runtime
        .input(UiInput::Pointer(Point { x: 100, y: 100 }), NcCopyFromBufferOffset::new(0, &[]))
        .unwrap();
    runtime
        .input(UiInput::Scroll(Point { x: 0, y: 20 }), NcCopyFromBufferOffset::new(0, &[]))
        .unwrap();
    runtime.update(SIZE).unwrap();
    assert!(inner.info().unwrap().requested.y > 0);
    assert_eq!(outer.info().unwrap().requested.y, 0);
    inner.scroll_to_end().unwrap();
    runtime.update(SIZE).unwrap();
    runtime
        .input(UiInput::Scroll(Point { x: 0, y: 20 }), NcCopyFromBufferOffset::new(0, &[]))
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
    let disclosure_owner = ui.widgets().disclosure("Tree", DisclosureStyle::Tree, false).unwrap();
    let mut disclosure = disclosure_owner.handle();
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
    body.insert(0, leaf, LinearEdge::new(LayoutTrack::Fixed { extent: 33 }))
        .unwrap();
    let window = ui.window("disclosure", RECT, disclosure_owner).unwrap();
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
                }),
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
    let concrete = area.as_interface().downgrade();
    assert_eq!(concrete.query::<IUnknown>().unwrap().as_raw().object, identity.as_raw().object);
    assert_eq!(root.query::<ITextEditorAbi>().unwrap_err(), neocogi_com::NC_E_NOINTERFACE);
    assert_eq!(root.query::<ScrollAreaAbi>().unwrap_err(), neocogi_com::NC_E_NOINTERFACE);
    let mut scroll = area.scrolling().unwrap();
    assert_eq!(
        scroll.node_interface().unwrap().query::<IUnknown>().unwrap().as_raw().object,
        identity.as_raw().object
    );
    let structural = root.query::<IContainerLayoutAbi>().unwrap().children().unwrap();
    assert_eq!(structural.child_count().unwrap(), 3);
    let surface = structural.child(0).unwrap().query::<IContainerLayoutAbi>().unwrap();
    let editor = surface.children().unwrap().child(0).unwrap().query::<IMultilineEditorAbi>().unwrap();
    assert_ne!(editor.query::<IUnknown>().unwrap().as_raw().object, identity.as_raw().object);
    assert_eq!(editor.query::<TextAreaAbi>().unwrap_err(), neocogi_com::NC_E_NOINTERFACE);
    area.set_cursor(3).unwrap();
    assert_eq!(area.cursor().unwrap(), 1); // A byte inside the snowman clamps to its preceding scalar boundary.
    assert_eq!(editor.cursor().unwrap(), 1);
    let area_owner = area;
    let mut area = area_owner.handle();
    let window = ui.window("editor", RECT, area_owner).unwrap();
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
            }),
            NcCopyFromBufferOffset::new(0, &[]),
        )
        .unwrap();
    runtime.update(SIZE).unwrap();
    runtime.input(UiInput::Text, NcCopyFromBufferOffset::new(0, "é".as_bytes())).unwrap();
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
            }),
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
        let window = ui.window("framing", RECT, disclosure).unwrap();
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

/// Reentrant construction rejects a peer's child edits during the host transaction.
/// Rejection preserves that peer; dropping its owner later destroys it normally.
#[test]
fn reentrant_child_clear_rejection_preserves_the_parent_and_its_children() {
    let runtime = runtime();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let mut victim = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let retained = ui.widgets().button_unobserved("retained descendant").unwrap();
    let retained_owner = retained;
    let retained = retained_owner.handle();
    victim
        .insert(0, retained_owner, LinearEdge::new(LayoutTrack::Flex { weight: 1.0 }))
        .unwrap();
    let mut unrelated = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let (candidate, probe) = recorded_leaf(&runtime);
    *probe.clear_on_mount.borrow_mut() = Some(victim.node_interface().unwrap());
    unrelated
        .insert(0, candidate, LinearEdge::new(LayoutTrack::Flex { weight: 1.0 }))
        .unwrap();
    assert_eq!(probe.clear_mount_result.get(), Some(Err(UI_E_BUSY)));
    assert!(victim.node_interface().unwrap().properties().is_ok());
    assert_eq!(retained.label().unwrap(), "retained descendant");
    let observed = victim.handle();
    drop(victim);
    assert_eq!(retained.label(), Err(UI_E_CLOSED));
    assert_eq!(observed.node_interface().unwrap().properties(), Err(UI_E_CLOSED));
}

/// Retained weak views cannot postpone or repeat the child destructor.
#[test]
fn dropping_a_parent_destroys_its_child_once_despite_retained_weak_views() {
    let runtime = runtime();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let mut parent = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let (child, probe) = recorded_leaf(&runtime);
    parent
        .insert(0, child, LinearEdge::new(LayoutTrack::Flex { weight: 1.0 }))
        .unwrap();
    let observed = parent.node_interface().unwrap();
    drop(parent);
    assert_eq!(observed.properties(), Err(UI_E_CLOSED));
    assert_eq!(probe.closes.get(), 1);
    drop(observed);
    assert_eq!(probe.closes.get(), 1);
}

/// A captured lifetime makes COM receiver release observable without invoking a
/// mapper or application handler during terminal cleanup.
struct QueueLifetime {
    /// Independent test log retained after every widget and queue is destroyed.
    released: std::sync::mpsc::Sender<&'static str>,
    /// Which widget owned this receiver's last reference.
    label: &'static str,
}
impl Drop for QueueLifetime {
    /// Captures must be released in descendant-first order exactly once.
    fn drop(&mut self) {
        let _ = self.released.send(self.label);
    }
}

/// Idle composite fields release child queues before the parent queue exactly once;
/// cloning or dropping weak observations never affects the unique owner.
#[test]
fn idle_composite_destruction_releases_child_and_parent_queues_once() {
    for explicit in [false, true] {
        let runtime = runtime();
        let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
        let (released, observations) = std::sync::mpsc::channel();
        let mut observed = Vec::new();
        let lifetime = QueueLifetime {
            released: released.clone(),
            label: "parent",
        };
        let (send, _receive) = std::sync::mpsc::channel::<()>();
        let queue = neocogi_ui::test_event_queue!(split_view, (), send,
            |queue, _origin, _| { queue.lifetime.released.send("unexpected post").unwrap(); }, { lifetime: QueueLifetime = lifetime })
        .unwrap();
        let mut parent = ui
            .widgets()
            .split_view_with_queue(
                SplitOptions {
                    axis: SplitAxis::Horizontal,
                    position: 30,
                    minimum_first: 0,
                    minimum_second: 0,
                    handle_width: 4,
                },
                Some(queue.as_com_ref()),
            )
            .unwrap();
        drop(queue);
        let mut children = Vec::new();
        for label in ["first", "second"] {
            let lifetime = QueueLifetime { released: released.clone(), label };
            let (send, _receive) = std::sync::mpsc::channel::<()>();
            let queue = neocogi_ui::test_event_queue!(button, (), send,
            |queue, _origin, _| { queue.lifetime.released.send("unexpected post").unwrap(); }, { lifetime: QueueLifetime = lifetime })
            .unwrap();
            let child = ui.widgets().button_with_queue(label, Some(queue.as_com_ref())).unwrap();
            drop(queue);
            let observed = child.handle();
            parent.insert(children.len() as u32, child).unwrap();
            children.push(observed);
        }
        drop(parent.handle());
        assert!(observations.try_recv().is_err());
        if explicit {
            let window = ui.window("owned subtree", RECT, parent).unwrap();
            window.close().unwrap();
        } else {
            drop(parent);
        }
        observed.extend(observations.try_iter());
        assert_eq!(observed, ["first", "second", "parent"]);
        for child in &children {
            assert_eq!(child.label(), Err(UI_E_CLOSED));
        }
        drop(children);
        observed.extend(observations.try_iter());
        assert_eq!(observed, ["first", "second", "parent"]);
    }
}

/// A real built-in container forwards rejection through its editor and restores
/// the typed facade. Neither ABI layer may destroy the offered ancestor.
#[test]
fn forwarded_cycle_rejection_returns_a_reusable_complete_tree() {
    let runtime = runtime();
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let mut a = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let b = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    let mut b_observer = b.handle();
    let a_observer = a.handle();
    let edge = LinearEdge::new(LayoutTrack::Content);
    a.insert(0, b, edge).unwrap();
    let (status, a) = b_observer.insert(0, a, edge).unwrap_err();
    assert_eq!(status, UI_E_OWNER);
    assert_eq!(a.children().unwrap().child_count(), Ok(1));
    assert_eq!(b_observer.children().unwrap().child_count(), Ok(0));
    // Rejection has not spent A's mount-once claim. Publish the unchanged tree.
    let window = ui.window("returned ancestor", RECT, a).unwrap();
    runtime.update(SIZE).unwrap();
    window.close().unwrap();
    assert!(!a_observer.as_interface().is_alive());
    assert!(!b_observer.as_interface().is_alive());
}
