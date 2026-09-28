//! Concrete container conveniences hold no tree or edge metadata outside the plugin.
use super::*;
use neocogi_com::NcCopyToBufferOffset;

/// Static convenience wrapper around the dynamically constructed Linear class projection.
pub type LinearWidget = Widget<LinearAbi>;
/// Static convenience wrapper around the dynamically constructed Grid class projection.
pub type GridWidget = Widget<GridAbi>;
/// Static convenience wrapper around the dynamically constructed ScrollArea class projection.
pub type ScrollAreaWidget = Widget<ScrollAreaAbi>;

/// Static client for the node-owned two-pane layout and interactive divider.
pub type SplitViewWidget = Widget<SplitViewAbi>;

impl Widget<SplitViewAbi> {
    /// Observes pane identities without exposing the container's topology writer.
    pub fn children(&self) -> Result<ComPtr<IChildrenAbi>, NcStatus> {
        self.interface.children()
    }
    /// Inserts a first or second pane; a third pane is rejected before attachment.
    pub fn insert(&mut self, index: u32, pane: &impl Mountable) -> Result<ChildEditResult, NcStatus> {
        self.interface.insert(index, pane.mount_widget()?.as_com_ref())
    }
    /// Replaces and closes one pane while preserving the split container.
    pub fn replace(&mut self, index: u32, pane: &impl Mountable) -> Result<ChildEditResult, NcStatus> {
        self.interface.replace(index, pane.mount_widget()?.as_com_ref())
    }
    /// Removes one pane; the surviving pane fills the allocation after layout.
    pub fn remove(&mut self, index: u32) -> Result<ChildEditResult, NcStatus> {
        self.interface.remove(index)
    }
    /// Retires both panes without retiring the split itself.
    pub fn clear(&mut self) -> Result<ChildEditResult, NcStatus> {
        self.interface.clear()
    }
    /// Returns the requested split settings, unaffected by temporary narrow allocations.
    pub fn settings(&self) -> Result<SplitOptions, NcStatus> {
        self.interface.settings()
    }
    /// Replaces validated sizing and cancels a gesture without publishing a user event.
    pub fn set_settings(&mut self, settings: SplitOptions) -> Result<(), NcStatus> {
        self.interface.set_settings(settings)
    }
}

/// Static convenience wrapper around the dynamically constructed Disclosure class projection.
pub type DisclosureWidget = Widget<DisclosureAbi>;

impl Widget<DisclosureAbi> {
    /// Reads expansion shared by the containing layout and its interactive header.
    pub fn expanded(&self) -> Result<bool, NcStatus> {
        self.interface.expanded()
    }
    /// Retains geometry invalidation when expansion actually changes.
    pub fn set_expanded(&mut self, expanded: bool) -> Result<(), NcStatus> {
        self.interface.set_expanded(expanded)
    }
    /// Returns the actual header for text and common widget-option edits.
    pub fn header(&self) -> Result<Widget<ITextContentAbi>, NcStatus> {
        Ok(Widget::from_interface(self.interface.header()?))
    }
    /// Returns the actual body for ordinary typed Linear edits.
    pub fn body(&self) -> Result<LinearWidget, NcStatus> {
        Ok(LinearWidget::from_interface(self.interface.body()?))
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
            pub fn set_offset(&mut self, offset: Point) -> Result<(), NcStatus> {
                self.interface.set_offset(offset)
            }
            /// Enables or disables scrolling and both actual scrollbar children.
            pub fn set_enabled(&mut self, enabled: bool) -> Result<(), NcStatus> {
                self.interface.set_enabled(enabled)
            }
            /// Requests the next measured vertical end without losing the horizontal request.
            pub fn scroll_to_end(&mut self) -> Result<(), NcStatus> {
                self.interface.scroll_to_end()
            }
            /// Requests the nearest offsets revealing the given content-local rectangle.
            pub fn reveal(&mut self, rect: Rect) -> Result<(), NcStatus> {
                self.interface.reveal(rect)
            }
        }
    };
}
scrolling_proxy!(IScrollingAbi);

impl Widget<ScrollAreaAbi> {
    /// Reads actual offset requests through this class's shared scrolling protocol.
    pub fn info(&self) -> Result<ScrollInfo, NcStatus> {
        self.interface.query::<IScrollingAbi>()?.info()
    }
    /// Requests offsets through this class's shared scrolling protocol.
    pub fn set_offset(&mut self, offset: Point) -> Result<(), NcStatus> {
        self.interface.query::<IScrollingAbi>()?.set_offset(offset)
    }
    /// Enables or disables scrolling and both actual scrollbar children.
    pub fn set_enabled(&mut self, enabled: bool) -> Result<(), NcStatus> {
        self.interface.query::<IScrollingAbi>()?.set_enabled(enabled)
    }
    /// Requests the next measured vertical end without losing the horizontal request.
    pub fn scroll_to_end(&mut self) -> Result<(), NcStatus> {
        self.interface.query::<IScrollingAbi>()?.scroll_to_end()
    }
    /// Requests the nearest offsets revealing the given content-local rectangle.
    pub fn reveal(&mut self, rect: Rect) -> Result<(), NcStatus> {
        self.interface.query::<IScrollingAbi>()?.reveal(rect)
    }
    /// Returns the actual content capability without exposing edit authority over structural roles.
    pub fn content(&self) -> Result<Widget<IWidgetAbi>, NcStatus> {
        Ok(Widget::from_interface(self.interface.content()?))
    }
    /// Replaces and closes application content while retaining the same outer scroll object and model.
    pub fn replace_content(&mut self, content: &impl Mountable) -> Result<ChildEditResult, NcStatus> {
        self.interface.replace_content(content.mount_widget()?.as_com_ref())
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
            pub fn set_edge(&mut self, index: u32, edge: $edge) -> Result<(), NcStatus> {
                self.interface.set_edge(index, edge)
            }
            /// Inserts the actual child object after validating the complete relationship.
            /// Err leaves topology unchanged; Ok requires inspecting its separate teardown status when relevant.
            pub fn insert(&mut self, index: u32, child: &impl Mountable, edge: $edge) -> Result<ChildEditResult, NcStatus> {
                self.interface
                    .insert(index, child.mount_widget()?.as_com_ref(), edge)
            }
            /// Replaces one subtree and relationship together. Ok reports committed replacement even if old-child teardown failed.
            pub fn replace(&mut self, index: u32, child: &impl Mountable, edge: $edge) -> Result<ChildEditResult, NcStatus> {
                self.interface
                    .replace(index, child.mount_widget()?.as_com_ref(), edge)
            }
            /// Removes and closes one subtree; call the returned outcome's finish after observing the committed edit.
            pub fn remove(&mut self, index: u32) -> Result<ChildEditResult, NcStatus> {
                self.interface.remove(index)
            }
            /// Removes all descendants and matching edge metadata while preserving the container object.
            pub fn clear(&mut self) -> Result<ChildEditResult, NcStatus> {
                self.interface.clear()
            }
        }
    };
}
container_proxy!(LinearAbi, LinearEdge);
container_proxy!(GridAbi, GridExtent);

impl Widget<LinearAbi> {
    /// Returns the complete axis and leading-edge choice.
    pub fn direction(&self) -> Result<FlowDirection, NcStatus> {
        self.interface.direction()
    }
    /// Changes direction without changing child ownership order.
    pub fn set_direction(&mut self, direction: FlowDirection) -> Result<(), NcStatus> {
        self.interface.set_direction(direction)
    }
    /// Returns the typed cross policy, rejecting any unknown external ABI tag.
    pub fn cross_size(&self) -> Result<CrossSize, NcStatus> {
        CrossSize::try_from(self.interface.cross_size()?)
    }
    /// Replaces the line's complete typed cross policy.
    pub fn set_cross_size(&mut self, cross: CrossSize) -> Result<(), NcStatus> {
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
    fn copy(&self, grid: &ComPtr<GridAbi>, output: NcCopyToBufferOffset<'_, LayoutTrackAbi>) -> Result<u32, NcStatus> {
        match self {
            Self::Columns => grid.columns(output),
            Self::Rows => grid.rows(output),
        }
    }
    /// Allocates an ordinary application snapshot without retaining plugin state.
    fn read(&self, grid: &ComPtr<GridAbi>) -> Result<Vec<LayoutTrack>, NcStatus> {
        let count = self.copy(grid, NcCopyToBufferOffset::new(0, &mut []))?;
        let mut values = vec![LayoutTrack::Content.into(); count as usize];
        if self.copy(grid, NcCopyToBufferOffset::new(0, &mut values))? != count {
            return Err(UI_E_BUSY);
        }
        values.into_iter().map(LayoutTrack::try_from).collect()
    }
}
impl Widget<GridAbi> {
    /// Copies the current column policies into application-owned typed values.
    pub fn columns(&self) -> Result<Vec<LayoutTrack>, NcStatus> {
        Axis::Columns.read(&self.interface)
    }
    /// Replaces every column policy after the plugin validates the complete sequence.
    pub fn set_columns(&mut self, tracks: &[LayoutTrack]) -> Result<(), NcStatus> {
        let tracks = encoded_tracks(tracks);
        self.interface.set_columns(NcCopyFromBufferOffset::new(0, &tracks))
    }
    /// Copies the current row policies into application-owned typed values.
    pub fn rows(&self) -> Result<Vec<LayoutTrack>, NcStatus> {
        Axis::Rows.read(&self.interface)
    }
    /// Replaces every row policy while preserving child identities and Grid spans.
    pub fn set_rows(&mut self, tracks: &[LayoutTrack]) -> Result<(), NcStatus> {
        let tracks = encoded_tracks(tracks);
        self.interface.set_rows(NcCopyFromBufferOffset::new(0, &tracks))
    }
}

/// Converts a complete local typed sequence to plain wire values for one synchronous bulk call.
fn encoded_tracks(tracks: &[LayoutTrack]) -> Vec<LayoutTrackAbi> {
    tracks.iter().copied().map(Into::into).collect()
}

impl WidgetFactory {
    /// Creates an empty split; insert panes in first/second ownership order.
    pub fn split_view_with_sink(&self, settings: SplitOptions, sink: Option<ComRef<'_, SplitViewSinkAbi>>) -> Result<SplitViewWidget, NcStatus> {
        Ok(SplitViewWidget::from_interface(self.interface.create_split_view(
            settings,
            neocogi_com::NcValueRef::new(&SplitViewEventBinding::from_sink(sink))?,
        )?))
    }
    /// Creates a disclosure whose body starts empty and whose header owns the supplied label.
    pub fn disclosure(&self, label: &str, style: DisclosureStyle, expanded: bool) -> Result<DisclosureWidget, NcStatus> {
        Ok(DisclosureWidget::from_interface(self.interface.create_disclosure(
            NcCopyFromBufferOffset::new(0, label.as_bytes()),
            style,
            expanded,
        )?))
    }
    /// Constructs a framed scroll composite around one unmounted application widget.
    pub fn scroll_area(&self, content: &impl Mountable, enabled: bool) -> Result<ScrollAreaWidget, NcStatus> {
        Ok(ScrollAreaWidget::from_interface(
            self.interface
                .create_scroll_area(content.mount_widget()?.as_com_ref(), enabled)?,
        ))
    }
    /// Creates an empty directional container with one complete axis/cross configuration.
    pub fn linear(&self, direction: FlowDirection, cross: CrossSize) -> Result<LinearWidget, NcStatus> {
        Ok(LinearWidget::from_interface(self.interface.create_linear(direction, cross.into())?))
    }
    /// Creates an empty Grid with complete validated track policies on both axes.
    pub fn grid(&self, columns: &[LayoutTrack], rows: &[LayoutTrack]) -> Result<GridWidget, NcStatus> {
        let columns = encoded_tracks(columns);
        let rows = encoded_tracks(rows);
        Ok(GridWidget::from_interface(self.interface.create_grid(
            NcCopyFromBufferOffset::new(0, &columns),
            NcCopyFromBufferOffset::new(0, &rows),
        )?))
    }
}

/// Typed client for runtime-owned fixed-coordinate child geometry.
pub type FormWidget = Widget<FormAbi>;
impl Widget<FormAbi> {
    /// Reads the topology without exposing its writer.
    pub fn children(&self) -> Result<ComPtr<IChildrenAbi>, NcStatus> {
        self.interface.children()
    }
    /// Returns the authored preferred content dimensions.
    pub fn size(&self) -> Result<Size, NcStatus> {
        self.interface.size()
    }
    /// Changes the preferred dimensions without scaling children.
    pub fn set_size(&mut self, size: Size) -> Result<(), NcStatus> {
        self.interface.set_size(size)
    }
    /// Reads one exact child rectangle.
    pub fn edge(&self, index: u32) -> Result<Rect, NcStatus> {
        self.interface.edge(index)
    }
    /// Moves/resizes an existing child without replacing it.
    pub fn set_edge(&mut self, index: u32, edge: Rect) -> Result<(), NcStatus> {
        self.interface.set_edge(index, edge)
    }
    /// Attaches one child after validating its complete rectangle.
    pub fn insert(&mut self, index: u32, child: &impl Mountable, edge: Rect) -> Result<ChildEditResult, NcStatus> {
        self.interface.insert(index, child.mount_widget()?.as_com_ref(), edge)
    }
    /// Replaces a child with the supplied geometry.
    pub fn replace(&mut self, index: u32, child: &impl Mountable, edge: Rect) -> Result<ChildEditResult, NcStatus> {
        self.interface.replace(index, child.mount_widget()?.as_com_ref(), edge)
    }
    /// Closes one owned subtree.
    pub fn remove(&mut self, index: u32) -> Result<ChildEditResult, NcStatus> {
        self.interface.remove(index)
    }
    /// Closes all children while retaining this form.
    pub fn clear(&mut self) -> Result<ChildEditResult, NcStatus> {
        self.interface.clear()
    }
}
impl WidgetFactory {
    /// Creates a form whose child rectangles are interpreted by the runtime node.
    pub fn form(&self, size: Size) -> Result<FormWidget, NcStatus> {
        Ok(FormWidget::from_interface(self.interface.create_form(size)?))
    }
}
