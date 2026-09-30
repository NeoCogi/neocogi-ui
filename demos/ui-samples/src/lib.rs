//! Concrete layout samples shared by the full demo and the theme editor, with no application model.
#![deny(missing_docs)]
use neocogi_com::{ComBox, ComInterface, ComRef, NcStatus, ThreadLocal};
use neocogi_ui::*;

/// One built sample and the real buttons created with the caller's queue.
pub struct Sample {
    /// Mount this root exactly once using the caller's own parent/window relationship.
    pub content: OwnedWidget<LinearAbi>,
    /// Retained handles allow optional property edits; the tree owns all widget state.
    pub buttons: Vec<ButtonWidget>,
}
/// An explicit expanding relationship used by both extracted full-demo examples.
const FLEX: LayoutTrack = LayoutTrack::Flex { weight: 1.0 };

/// Preserves the full demo's two opposing vertical stacks and their original dimensions.
pub fn directions(ui: &Ui, queue: Option<ComRef<'_, ButtonEventQueueAbi>>) -> Result<Sample, NcStatus> {
    let mut content = ui.widgets().linear(FlowDirection::LeftToRight, CrossSize::Content)?;
    let mut buttons = Vec::new();
    for (name, direction) in [("Top to bottom", FlowDirection::TopToBottom), ("Bottom to top", FlowDirection::BottomToTop)] {
        let mut column = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch)?;
        append(&mut column, ui.widgets().text_block(name, TextWrapping::None)?, LayoutTrack::Content)?;
        let mut stack = ui.widgets().linear(direction, CrossSize::Stretch)?;
        for text in ["First", "Second", "Third"] {
            let control = button(ui, text, queue)?;
            buttons.push(control.handle());
            append(&mut stack, control, LayoutTrack::Fixed { extent: 28 })?;
        }
        append(&mut column, stack, LayoutTrack::Fixed { extent: 120 })?;
        append(&mut content, column, FLEX)?;
    }
    Ok(Sample { content, buttons })
}

/// Preserves the full demo's weighted row and grid; preview and demo use identical retained nodes.
pub fn weights(ui: &Ui, queue: Option<ComRef<'_, ButtonEventQueueAbi>>) -> Result<Sample, NcStatus> {
    let mut content = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch)?;
    let mut buttons = Vec::new();
    append(
        &mut content,
        ui.widgets().text_block("Row weights: 1 : 2 : 3", TextWrapping::None)?,
        LayoutTrack::Content,
    )?;
    let mut line = ui.widgets().linear(FlowDirection::LeftToRight, CrossSize::Content)?;
    for (text, weight) in [("1x", 1.0), ("2x", 2.0), ("3x", 3.0)] {
        let control = button(ui, text, queue)?;
        buttons.push(control.handle());
        append(&mut line, control, LayoutTrack::Flex { weight })?;
    }
    append(&mut content, line, LayoutTrack::Fixed { extent: 28 })?;
    append(
        &mut content,
        ui.widgets().text_block("Grid row weights: 1 : 2", TextWrapping::None)?,
        LayoutTrack::Content,
    )?;
    let mut grid = ui.widgets().grid(&[FLEX; 3], &[FLEX, LayoutTrack::Flex { weight: 2.0 }])?;
    for index in 0..6 {
        let control = button(ui, &format!("Cell {}", index + 1), queue)?;
        buttons.push(control.handle());
        let control = control.into_interface().query::<IWidgetAbi>().map_err(|(status, _candidate)| status)?;
        grid.as_interface_mut().insert(index, control, GridExtent { columns: 1, rows: 1 })
            .map_err(|(status, _candidate)| status)?;
    }
    append(&mut content, grid, FLEX)?;
    Ok(Sample { content, buttons })
}

/// Creates the same framed centered control as the full demo without prescribing its application action.
fn button(ui: &Ui, text: &str, queue: Option<ComRef<'_, ButtonEventQueueAbi>>) -> Result<OwnedWidget<ButtonAbi>, NcStatus> {
    let mut button = ui.widgets().button_with_queue(text, queue)?;
    button.replace_flags(WidgetOptions::FRAME | WidgetOptions::ALIGN_CENTER)?;
    Ok(button)
}
/// Appends a fresh construction candidate, abandoning it if construction fails.
///
/// This builder intentionally returns only the status. Applications that need to
/// retry a rejected typed owner use `Widget::insert` instead. Call it outside the
/// candidate's active methods: it may destroy the candidate on any failure.
/// The generic boundary selects IWidget once; all parent storage and child types
/// then share the same insertion body without converting a rejected owner back.
#[inline]
pub fn append<I: ComInterface, P>(parent: &mut Widget<LinearAbi, P>, child: OwnedWidget<I>, track: LayoutTrack) -> Result<(), NcStatus>
where P: WidgetStorage<LinearAbi> + core::ops::DerefMut<Target = nodes::LinearClient<ThreadLocal>> {
    let child = child.into_interface().query::<IWidgetAbi>().map_err(|(status, _candidate)| status)?;
    append_widget(parent.as_interface_mut(), child, track)
}

/// One construction implementation for every child interface and parent storage.
/// Keep this out of generic callers so their failure handling is not duplicated.
#[inline(never)]
fn append_widget(parent: &mut nodes::LinearClient<ThreadLocal>, child: ComBox<IWidgetAbi>, track: LayoutTrack) -> Result<(), NcStatus> {
    // The client returns any rejected owner after the provider call has ended.
    // This builder then drops it through IWidget; no reverse interface query is
    // needed to destroy an object through its provider's common lifetime header.
    let index = parent.children()?.child_count()?;
    parent.insert(index, child, LinearEdge::new(track)).map_err(|(status, _candidate)| status)
}
