//! Grid admission regressions cross generated COM thunks and the actual owning child store.
use super::*;

/// Small policy makes exact capacity and cumulative implicit rows easy to observe.
const LIMITS: GridLimits = GridLimits { max_columns: 2, max_rows: 1 };
/// Ordinary one-cell relationship reused without hiding the exceptional spans under test.
const CELL: GridExtent = GridExtent { columns: 1, rows: 1 };

/// Invalid policies and explicit track counts fail before constructing a Grid owner.
#[test]
fn grid_construction_validates_limits_and_explicit_tracks() {
    let runtime = runtime();
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    for limits in [GridLimits { max_columns: 0, max_rows: 1 }, GridLimits { max_columns: 1, max_rows: 0 }] {
        assert_eq!(ui.widgets().grid(&[], &[], limits).err(), Some(NC_E_INVALID_ARG));
    }
    assert_eq!(
        ui.widgets().grid(&[], &[], GridLimits { max_columns: u32::MAX, max_rows: 1 }).err(),
        Some(neocogi_com::NC_E_OUT_OF_RANGE)
    );
    assert_eq!(
        ui.widgets().grid(&[LayoutTrack::Content; 3], &[], LIMITS).err(),
        Some(neocogi_com::NC_E_OUT_OF_RANGE)
    );
    assert_eq!(
        ui.widgets().grid(&[], &[LayoutTrack::Content; 2], LIMITS).err(),
        Some(neocogi_com::NC_E_OUT_OF_RANGE)
    );
    for weight in [1e30, f32::MIN_POSITIVE] {
        assert_eq!(
            ui.widgets().grid(&[LayoutTrack::Flex { weight }], &[], LIMITS).err(),
            Some(neocogi_com::NC_E_OUT_OF_RANGE)
        );
    }
    let mut grid = ui.widgets().grid(&[], &[], LIMITS).unwrap();
    assert_eq!(grid.limits().unwrap(), LIMITS);
    let (child, probe) = recorded_leaf(&runtime);
    // Empty policies imply one track, not zero capacity. Width still clamps.
    grid.insert(0, child, GridExtent { columns: u32::MAX, rows: 1 }).unwrap();
    assert_eq!(grid.children().unwrap().child_count().unwrap(), 1);
    drop(grid);
    assert_eq!(probe.closes.get(), 1);
}

/// Every rejection preserves the exact candidate, old subtree, and committed metadata.
#[test]
fn grid_rejected_edits_preserve_owners_and_support_retry() {
    let runtime = runtime();
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let columns = [LayoutTrack::Fixed { extent: 30 }; 2];
    let rows = [LayoutTrack::Fixed { extent: 20 }];
    let owner = ui.widgets().grid(&columns, &rows, LIMITS).unwrap();
    let mut grid = owner.handle();
    let (first, a) = recorded_leaf(&runtime);
    let first_handle = first.handle();
    let (second, b) = recorded_leaf(&runtime);
    grid.insert(0, first, CELL).unwrap();
    grid.insert(1, second, CELL).unwrap();
    let window = ui.window("Grid admission", RECT, owner).unwrap();
    runtime.update(SIZE).unwrap();
    let before = (a.latest.get().unwrap().screen, b.latest.get().unwrap().screen);
    let (candidate, candidate_probe) = recorded_leaf(&runtime);
    let identity = candidate.node_interface().unwrap();
    // Each span is individually legal, but a third child needs a second implicit row.
    let (status, candidate) = grid.insert(2, candidate, CELL).unwrap_err();
    assert_eq!(status, neocogi_com::NC_E_OUT_OF_RANGE);
    assert!(identity.same_object(&candidate.node_interface().unwrap()));
    let (status, candidate) = grid.replace(0, candidate, GridExtent { columns: 1, rows: u32::MAX }).unwrap_err();
    assert_eq!(status, neocogi_com::NC_E_OUT_OF_RANGE);
    assert!(identity.same_object(&candidate.node_interface().unwrap()));
    let (status, candidate) = grid.insert(3, candidate, CELL).unwrap_err();
    assert_eq!(status, neocogi_com::NC_E_OUT_OF_RANGE);
    let (status, candidate) = grid.replace(2, candidate, CELL).unwrap_err();
    assert_eq!(status, neocogi_com::NC_E_OUT_OF_RANGE);
    let (status, candidate) = grid.insert(2, candidate, GridExtent { columns: 0, rows: 1 }).unwrap_err();
    assert_eq!(status, NC_E_INVALID_ARG);
    assert_eq!(grid.set_edge(0, GridExtent { columns: 2, rows: 1 }), Err(neocogi_com::NC_E_OUT_OF_RANGE));
    assert_eq!(grid.set_columns(&columns[..1]), Err(neocogi_com::NC_E_OUT_OF_RANGE));
    assert_eq!(grid.set_rows(&[LayoutTrack::Content; 2]), Err(neocogi_com::NC_E_OUT_OF_RANGE));
    assert_eq!(grid.remove(2), Err(neocogi_com::NC_E_OUT_OF_RANGE));
    assert_eq!(grid.edge(0).unwrap(), CELL);
    assert_eq!(grid.edge(1).unwrap(), CELL);
    assert_eq!(grid.columns().unwrap(), columns);
    assert_eq!(grid.rows().unwrap(), rows);
    assert_eq!(grid.children().unwrap().child_count().unwrap(), 2);
    assert_eq!((a.closes.get(), b.closes.get(), candidate_probe.closes.get()), (0, 0, 0));
    runtime.update(SIZE).unwrap();
    assert_eq!((a.latest.get().unwrap().screen, b.latest.get().unwrap().screen), before);
    // Retrying the same returned owner proves rejection did not consume its mount-once claim.
    grid.replace(0, candidate, CELL).unwrap();
    assert_eq!(a.closes.get(), 1);
    assert!(!first_handle.as_interface().is_alive());
    grid.remove(1).unwrap();
    assert_eq!(b.closes.get(), 1);
    grid.set_edge(0, GridExtent { columns: 2, rows: 1 }).unwrap();
    grid.set_columns(&columns[..1]).unwrap(); // Clamp remains legal at the new right edge.
    grid.set_rows(&[]).unwrap(); // Implicit row remains bounded by the same ceiling.
    runtime.update(SIZE).unwrap();
    grid.clear().unwrap();
    grid.clear().unwrap();
    assert_eq!(candidate_probe.closes.get(), 1);
    assert_eq!(grid.children().unwrap().child_count().unwrap(), 0);
    assert_eq!(grid.edge(0), Err(neocogi_com::NC_E_OUT_OF_RANGE));
    window.close().unwrap();
}

/// Right-edge clamping can make removal widen a later footprint and require extra rows.
#[test]
fn grid_removal_prepares_reflow_before_dropping_any_child() {
    let runtime = runtime();
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let mut grid = ui
        .widgets()
        .grid(&[LayoutTrack::Content; 3], &[], GridLimits { max_columns: 3, max_rows: 3 })
        .unwrap();
    let mut probes = Vec::new();
    for (index, edge) in [CELL, CELL, GridExtent { columns: 2, rows: 2 }, GridExtent { columns: 2, rows: 2 }]
        .into_iter()
        .enumerate()
    {
        let (child, probe) = recorded_leaf(&runtime);
        grid.insert(index as u32, child, edge).unwrap();
        probes.push(probe);
    }
    assert_eq!(grid.remove(0), Err(neocogi_com::NC_E_OUT_OF_RANGE));
    assert_eq!(grid.children().unwrap().child_count().unwrap(), 4);
    assert_eq!(grid.edge(0).unwrap(), CELL);
    assert!(probes.iter().all(|probe| probe.closes.get() == 0));
    // Clearing needs no replacement placement and can always discard all owned children.
    grid.clear().unwrap();
    assert!(probes.iter().all(|probe| probe.closes.get() == 1));
}

/// Host rejection after successful placement preparation must discard the unpublished plan.
#[test]
fn grid_host_rejection_keeps_metadata_and_returns_reusable_foreign_owner() {
    let runtime = runtime();
    let ui = neocogi_ui::nodes::bind_builtin_nodes(runtime.clone()).unwrap();
    let other_runtime = super::runtime();
    let other_ui = neocogi_ui::nodes::bind_builtin_nodes(other_runtime.clone()).unwrap();
    let mut grid = ui.widgets().grid(&[LayoutTrack::Content; 2], &[], LIMITS).unwrap();
    let (foreign, probe) = recorded_leaf(&other_runtime);
    let identity = foreign.node_interface().unwrap();
    let (status, foreign) = grid.insert(0, foreign, CELL).unwrap_err();
    assert_eq!(status, UI_E_OWNER);
    assert!(identity.same_object(&foreign.node_interface().unwrap()));
    assert_eq!(grid.children().unwrap().child_count().unwrap(), 0);
    assert_eq!(grid.edge(0), Err(neocogi_com::NC_E_OUT_OF_RANGE));
    let mut other = other_ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Content).unwrap();
    other.insert(0, foreign, LinearEdge::new(LayoutTrack::Content)).unwrap();
    assert_eq!(probe.closes.get(), 0);
    let (local, _) = recorded_leaf(&runtime);
    grid.insert(0, local, CELL).unwrap();
    assert_eq!(grid.edge(0).unwrap(), CELL);
}
