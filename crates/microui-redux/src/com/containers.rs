//! Concrete container conveniences hold no tree or edge metadata outside the plugin.
use super::*;
use nc_com::NcCopyToBufferOffset;

/// Static directional Linear proxy; inherited container and widget state remain one COM object.
pub type Linear = Widget<ILinearAbi>;
/// Static Grid proxy; ordered child ownership, spans and track policies live in the plugin.
pub type Grid = Widget<IGridAbi>;
/// Static scroll-composite proxy; its private structural children stay owned by the plugin.
pub type ScrollArea = Widget<IScrollAreaAbi>;

/// Static disclosure proxy; the header and Linear body are actual child COM objects.
pub type Disclosure = Widget<IDisclosureAbi>;

impl Widget<IDisclosureAbi> {
    /// Reads expansion shared by the containing layout and its interactive header.
    pub fn expanded(&self) -> Result<bool, NcStatus> {
        self.interface.expanded()
    }
    /// Retains geometry invalidation when expansion actually changes.
    pub fn set_expanded(&self, expanded: bool) -> Result<(), NcStatus> {
        self.interface.set_expanded(expanded)
    }
    /// Returns the actual header for text and common widget-option edits.
    pub fn header(&self) -> Result<Widget<ITextContentAbi>, NcStatus> {
        Ok(Widget::from_interface(self.interface.header()?))
    }
    /// Returns the actual body for ordinary typed Linear edits.
    pub fn body(&self) -> Result<Linear, NcStatus> {
        Ok(Linear::from_interface(self.interface.body()?))
    }
}

/// Forwards scrolling without creating a local model or exposing content replacement on fixed editors.
macro_rules! scrolling_proxy {
    ($interface:ty) => {
        impl Widget<$interface> {
            /// Reads actual offset requests and the latest committed viewport/range summary.
            pub fn info(&self) -> Result<ScrollInfo, NcStatus> {
                self.interface.info()
            }
            /// Requests offsets that the next placement clamps against freshly measured content.
            pub fn set_offset(&self, offset: Point) -> Result<(), NcStatus> {
                self.interface.set_offset(offset)
            }
            /// Enables or disables scrolling and both actual scrollbar children.
            pub fn set_enabled(&self, enabled: bool) -> Result<(), NcStatus> {
                self.interface.set_enabled(enabled)
            }
            /// Requests the next measured vertical end without losing the horizontal request.
            pub fn scroll_to_end(&self) -> Result<(), NcStatus> {
                self.interface.scroll_to_end()
            }
            /// Requests the nearest offsets revealing the given content-local rectangle.
            pub fn reveal(&self, rect: Rect) -> Result<(), NcStatus> {
                self.interface.reveal(rect)
            }
        }
    };
}
scrolling_proxy!(IScrollAreaAbi);
scrolling_proxy!(IScrollingAbi);

impl Widget<IScrollAreaAbi> {
    /// Returns the actual content capability without exposing edit authority over structural roles.
    pub fn content(&self) -> Result<Widget<IWidgetAbi>, NcStatus> {
        Ok(Widget::from_interface(self.interface.content()?))
    }
    /// Replaces and closes application content while retaining the same outer scroll object and model.
    pub fn replace_content<I: ComInterface>(&self, content: &Widget<I>) -> Result<ChildEditResult, NcStatus> {
        self.interface.replace_content(content.node_interface()?.as_com_ref())
    }
}

/// Common concrete proxy methods forward edits without hiding whether topology committed.
macro_rules! container_proxy {
    ($interface:ty, $edge:ty) => {
        impl Widget<$interface> {
            /// Returns the read-only topology capability, which cannot be queried for a private editor.
            pub fn children(&self) -> Result<ComPtr<IChildrenAbi>, NcStatus> {
                self.interface.children()
            }
            /// Reads one parent-owned relationship without retaining a local metadata cache.
            pub fn edge(&self, index: u32) -> Result<$edge, NcStatus> {
                self.interface.edge(index)
            }
            /// Replaces sizing metadata without replacing the child's identity.
            pub fn set_edge(&self, index: u32, edge: $edge) -> Result<(), NcStatus> {
                self.interface.set_edge(index, edge)
            }
            /// Inserts the actual child object after validating the complete relationship.
            /// Err leaves topology unchanged; Ok requires inspecting its separate teardown status when relevant.
            pub fn insert<I: ComInterface>(&self, index: u32, child: &Widget<I>, edge: $edge) -> Result<ChildEditResult, NcStatus> {
                self.interface.insert(index, child.node_interface()?.as_com_ref(), edge)
            }
            /// Replaces one subtree and relationship together. Ok reports committed replacement even if old-child teardown failed.
            pub fn replace<I: ComInterface>(&self, index: u32, child: &Widget<I>, edge: $edge) -> Result<ChildEditResult, NcStatus> {
                self.interface.replace(index, child.node_interface()?.as_com_ref(), edge)
            }
            /// Removes and closes one subtree; call the returned outcome's finish after observing the committed edit.
            pub fn remove(&self, index: u32) -> Result<ChildEditResult, NcStatus> {
                self.interface.remove(index)
            }
            /// Removes all descendants and matching edge metadata while preserving the container object.
            pub fn clear(&self) -> Result<ChildEditResult, NcStatus> {
                self.interface.clear()
            }
        }
    };
}
container_proxy!(ILinearAbi, LinearEdge);
container_proxy!(IGridAbi, GridExtent);

impl Widget<ILinearAbi> {
    /// Returns the complete axis and leading-edge choice.
    pub fn direction(&self) -> Result<FlowDirection, NcStatus> {
        self.interface.direction()
    }
    /// Changes direction without changing child ownership order.
    pub fn set_direction(&self, direction: FlowDirection) -> Result<(), NcStatus> {
        self.interface.set_direction(direction)
    }
    /// Returns the typed cross policy, rejecting any unknown external ABI tag.
    pub fn cross_size(&self) -> Result<CrossSize, NcStatus> {
        CrossSize::try_from(self.interface.cross_size()?)
    }
    /// Replaces the line's complete typed cross policy.
    pub fn set_cross_size(&self, cross: CrossSize) -> Result<(), NcStatus> {
        self.interface.set_cross_size(cross.into())
    }
}

/// Selects one Grid track axis explicitly, avoiding callback-based bulk access.
enum Axis {
    Columns,
    Rows,
}
impl Axis {
    /// Executes the selected concrete copied-policy read.
    fn copy(&self, grid: &ComPtr<IGridAbi>, output: NcCopyToBufferOffset<'_, LayoutTrackAbi>) -> Result<u32, NcStatus> {
        match self {
            Self::Columns => grid.columns(output),
            Self::Rows => grid.rows(output),
        }
    }
    /// Allocates an ordinary application snapshot without retaining plugin state.
    fn read(&self, grid: &ComPtr<IGridAbi>) -> Result<Vec<LayoutTrack>, NcStatus> {
        let count = self.copy(grid, NcCopyToBufferOffset::new(0, &mut []))?;
        let mut values = vec![LayoutTrack::Content.into(); count as usize];
        if self.copy(grid, NcCopyToBufferOffset::new(0, &mut values))? != count {
            return Err(UI_E_BUSY);
        }
        values.into_iter().map(LayoutTrack::try_from).collect()
    }
}
impl Widget<IGridAbi> {
    /// Copies the current column policies into application-owned typed values.
    pub fn columns(&self) -> Result<Vec<LayoutTrack>, NcStatus> {
        Axis::Columns.read(&self.interface)
    }
    /// Replaces every column policy after the plugin validates the complete sequence.
    pub fn set_columns(&self, tracks: &[LayoutTrack]) -> Result<(), NcStatus> {
        let tracks = encoded_tracks(tracks);
        self.interface.set_columns(NcCopyFromBufferOffset::new(0, &tracks))
    }
    /// Copies the current row policies into application-owned typed values.
    pub fn rows(&self) -> Result<Vec<LayoutTrack>, NcStatus> {
        Axis::Rows.read(&self.interface)
    }
    /// Replaces every row policy while preserving child identities and Grid spans.
    pub fn set_rows(&self, tracks: &[LayoutTrack]) -> Result<(), NcStatus> {
        let tracks = encoded_tracks(tracks);
        self.interface.set_rows(NcCopyFromBufferOffset::new(0, &tracks))
    }
}

/// Converts a complete local typed sequence to plain wire values for one synchronous bulk call.
fn encoded_tracks(tracks: &[LayoutTrack]) -> Vec<LayoutTrackAbi> {
    tracks.iter().copied().map(Into::into).collect()
}

impl WidgetFactory {
    /// Creates a disclosure whose body starts empty and whose header owns the supplied label.
    pub fn disclosure(&self, label: &str, style: DisclosureStyle, expanded: bool) -> Result<Disclosure, NcStatus> {
        Ok(Disclosure::from_interface(self.interface.create_disclosure(
            NcCopyFromBufferOffset::new(0, label.as_bytes()),
            style,
            expanded,
        )?))
    }
    /// Constructs a framed scroll composite around one unmounted application widget.
    pub fn scroll_area<I: ComInterface>(&self, content: &Widget<I>, enabled: bool) -> Result<ScrollArea, NcStatus> {
        Ok(ScrollArea::from_interface(
            self.interface.create_scroll_area(content.node_interface()?.as_com_ref(), enabled)?,
        ))
    }
    /// Creates an empty directional container with one complete axis/cross configuration.
    pub fn linear(&self, direction: FlowDirection, cross: CrossSize) -> Result<Linear, NcStatus> {
        Ok(Linear::from_interface(self.interface.create_linear(direction, cross.into())?))
    }
    /// Creates an empty Grid with complete validated track policies on both axes.
    pub fn grid(&self, columns: &[LayoutTrack], rows: &[LayoutTrack]) -> Result<Grid, NcStatus> {
        let columns = encoded_tracks(columns);
        let rows = encoded_tracks(rows);
        Ok(Grid::from_interface(self.interface.create_grid(
            NcCopyFromBufferOffset::new(0, &columns),
            NcCopyFromBufferOffset::new(0, &rows),
        )?))
    }
}
