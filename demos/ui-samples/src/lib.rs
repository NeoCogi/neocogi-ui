//! Concrete layout samples shared by the full demo and the theme editor, with no application model.
#![deny(missing_docs)]
use neocogi_com::{ComInterface, NcStatus};
use neocogi_ui::*;

/// One built sample and the real buttons whose typed activation endpoints a caller may subscribe to.
pub struct Sample {
    /// Mount this root exactly once using the caller's own parent/window relationship.
    pub content: LinearWidget,
    /// Optional caller wiring uses existing activation endpoints; the tree owns all widget state.
    pub buttons: Vec<ButtonWidget>,
}
/// An explicit expanding relationship used by both extracted full-demo examples.
const FLEX: LayoutTrack = LayoutTrack::Flex { weight: 1.0 };

/// Preserves the full demo's two opposing vertical stacks and their original dimensions.
pub fn directions(ui: &Ui) -> Result<Sample, NcStatus> {
    let mut content = ui.widgets().linear(FlowDirection::LeftToRight, CrossSize::Content)?;
    let mut buttons = Vec::new();
    for (name, direction) in [("Top to bottom", FlowDirection::TopToBottom), ("Bottom to top", FlowDirection::BottomToTop)] {
        let mut column = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch)?;
        append(&mut column, &ui.widgets().text_block(name, TextWrapping::None)?, LayoutTrack::Content)?;
        let mut stack = ui.widgets().linear(direction, CrossSize::Stretch)?;
        for text in ["First", "Second", "Third"] {
            let control = button(ui, text)?;
            append(&mut stack, &control, LayoutTrack::Fixed { extent: 28 })?;
            buttons.push(control);
        }
        append(&mut column, &stack, LayoutTrack::Fixed { extent: 120 })?;
        append(&mut content, &column, FLEX)?;
    }
    Ok(Sample { content, buttons })
}

/// Preserves the full demo's weighted row and grid; preview and demo use identical retained nodes.
pub fn weights(ui: &Ui) -> Result<Sample, NcStatus> {
    let mut content = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch)?;
    let mut buttons = Vec::new();
    append(
        &mut content,
        &ui.widgets().text_block("Row weights: 1 : 2 : 3", TextWrapping::None)?,
        LayoutTrack::Content,
    )?;
    let mut line = ui.widgets().linear(FlowDirection::LeftToRight, CrossSize::Content)?;
    for (text, weight) in [("1x", 1.0), ("2x", 2.0), ("3x", 3.0)] {
        let control = button(ui, text)?;
        append(&mut line, &control, LayoutTrack::Flex { weight })?;
        buttons.push(control);
    }
    append(&mut content, &line, LayoutTrack::Fixed { extent: 28 })?;
    append(
        &mut content,
        &ui.widgets().text_block("Grid row weights: 1 : 2", TextWrapping::None)?,
        LayoutTrack::Content,
    )?;
    let mut grid = ui.widgets().grid(&[FLEX; 3], &[FLEX, LayoutTrack::Flex { weight: 2.0 }])?;
    for index in 0..6 {
        let control = button(ui, &format!("Cell {}", index + 1))?;
        grid.insert(index, &control, GridExtent { columns: 1, rows: 1 })?.finish()?;
        buttons.push(control);
    }
    append(&mut content, &grid, FLEX)?;
    Ok(Sample { content, buttons })
}

/// Creates the same framed centered control as the full demo without prescribing its application action.
fn button(ui: &Ui, text: &str) -> Result<ButtonWidget, NcStatus> {
    let mut button = ui.widgets().button(text)?;
    button.set_options(WidgetOptions::FRAME | WidgetOptions::ALIGN_CENTER)?;
    Ok(button)
}
/// Appends one explicit relationship using authoritative topology, with no parallel child-state model.
fn append<I: ComInterface>(parent: &mut LinearWidget, child: &Widget<I>, track: LayoutTrack) -> Result<(), NcStatus> {
    parent.insert(parent.children()?.child_count()?, child, LinearEdge::new(track))?.finish()
}
