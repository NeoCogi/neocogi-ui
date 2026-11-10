//! Concrete container conveniences hold no tree or edge metadata outside the plugin.
use super::*;

/// Static convenience wrapper around the dynamically constructed Linear class projection.
pub type LinearWidget = Widget<LinearAbi>;
/// Static convenience wrapper around the dynamically constructed Grid class projection.
pub type GridWidget = Widget<GridAbi>;
/// Static convenience wrapper around the dynamically constructed ScrollArea class projection.
pub type ScrollAreaWidget = Widget<ScrollAreaAbi>;

/// Static client for the node-owned two-pane layout and interactive divider.
pub type SplitViewWidget = Widget<SplitViewAbi>;

impl<P> Widget<SplitViewAbi, P>
where
    P: WidgetStorage<SplitViewAbi> + core::ops::DerefMut<Target = <SplitViewAbi as neocogi_com::ComClientInterface>::Client<neocogi_com::ThreadLocal>>,
{
    /// Observes pane identities without exposing the container's topology writer.
    pub fn children(&self) -> Result<ComPtr<IChildrenAbi>, NcStatus> {
        self.interface.children()
    }
    /// Inserts a first or second pane; a third pane is rejected before attachment.
    pub fn insert<W: Mountable>(&mut self, index: u32, pane: W) -> Result<(), (NcStatus, W)> {
        pane.with_mount(|pane| self.interface.insert(index, pane))
    }
    /// Replaces and destroys one pane while preserving the split container.
    pub fn replace<W: Mountable>(&mut self, index: u32, pane: W) -> Result<(), (NcStatus, W)> {
        pane.with_mount(|pane| self.interface.replace(index, pane))
    }
    /// Removes one pane; the surviving pane fills the allocation after layout.
    pub fn remove(&mut self, index: u32) -> Result<(), NcStatus> {
        self.interface.remove(index)
    }
    /// Retires both panes without retiring the split itself.
    pub fn clear(&mut self) -> Result<(), NcStatus> {
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
    /// Changes only the requested position in one checked COM call.
    /// Preserves current sizing policy, rejects negative positions, and emits no user event.
    pub fn set_position(&mut self, position: i32) -> Result<(), NcStatus> {
        self.interface.set_position(position)
    }
}

/// Static convenience wrapper around the dynamically constructed Disclosure class projection.
pub type DisclosureWidget = Widget<DisclosureAbi>;

impl<P> Widget<DisclosureAbi, P>
where
    P: WidgetStorage<DisclosureAbi> + core::ops::DerefMut<Target = <DisclosureAbi as neocogi_com::ComClientInterface>::Client<neocogi_com::ThreadLocal>>,
{
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
        impl<P> Widget<$interface, P>
        where
            P: WidgetStorage<$interface> + core::ops::DerefMut<Target = <$interface as neocogi_com::ComClientInterface>::Client<neocogi_com::ThreadLocal>>,
        {
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

impl<P> Widget<ScrollAreaAbi, P>
where
    P: WidgetStorage<ScrollAreaAbi> + core::ops::DerefMut<Target = <ScrollAreaAbi as neocogi_com::ComClientInterface>::Client<neocogi_com::ThreadLocal>>,
{
    /// Reads actual offset requests through this class's shared scrolling protocol.
    pub fn info(&self) -> Result<ScrollInfo, NcStatus> {
        self.interface.observe::<IScrollingAbi>()?.info()
    }
    /// Requests offsets through this class's shared scrolling protocol.
    pub fn set_offset(&mut self, offset: Point) -> Result<(), NcStatus> {
        self.interface.observe::<IScrollingAbi>()?.set_offset(offset)
    }
    /// Enables or disables scrolling and both actual scrollbar children.
    pub fn set_enabled(&mut self, enabled: bool) -> Result<(), NcStatus> {
        self.interface.observe::<IScrollingAbi>()?.set_enabled(enabled)
    }
    /// Requests the next measured vertical end without losing the horizontal request.
    pub fn scroll_to_end(&mut self) -> Result<(), NcStatus> {
        self.interface.observe::<IScrollingAbi>()?.scroll_to_end()
    }
    /// Requests the nearest offsets revealing the given content-local rectangle.
    pub fn reveal(&mut self, rect: Rect) -> Result<(), NcStatus> {
        self.interface.observe::<IScrollingAbi>()?.reveal(rect)
    }
    /// Returns the actual content capability without exposing edit authority over structural roles.
    pub fn content(&self) -> Result<Widget<IWidgetAbi>, NcStatus> {
        Ok(Widget::from_interface(self.interface.content()?))
    }
    /// Replaces and destroys application content while retaining the same outer scroll object and model.
    pub fn replace_content<W: Mountable>(&mut self, content: W) -> Result<(), (NcStatus, W)> {
        content.with_mount(|content| self.interface.replace_content(content))
    }
}

/// Common concrete proxy methods forward edits without hiding whether topology committed.
macro_rules! container_proxy {
    ($interface:ty, $edge:ty) => {
        impl<P> Widget<$interface, P>
        where
            P: WidgetStorage<$interface> + core::ops::DerefMut<Target = <$interface as neocogi_com::ComClientInterface>::Client<neocogi_com::ThreadLocal>>,
        {
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
            /// Rejection returns the unchanged typed candidate and preserves existing topology.
            pub fn insert<W: Mountable>(&mut self, index: u32, child: W, edge: $edge) -> Result<(), (NcStatus, W)> {
                child.with_mount(|child| self.interface.insert(index, child, edge))
            }
            /// Replaces one subtree and relationship together, dropping the previous child owner on success.
            pub fn replace<W: Mountable>(&mut self, index: u32, child: W, edge: $edge) -> Result<(), (NcStatus, W)> {
                child.with_mount(|child| self.interface.replace(index, child, edge))
            }
            /// Removes one subtree and drops its exclusive owner.
            pub fn remove(&mut self, index: u32) -> Result<(), NcStatus> {
                self.interface.remove(index)
            }
            /// Removes all descendants and matching edge metadata while preserving the container object.
            pub fn clear(&mut self) -> Result<(), NcStatus> {
                self.interface.clear()
            }
        }
    };
}
container_proxy!(LinearAbi, LinearEdge);
container_proxy!(GridAbi, GridExtent);

impl<P> Widget<LinearAbi, P>
where
    P: WidgetStorage<LinearAbi> + core::ops::DerefMut<Target = <LinearAbi as neocogi_com::ComClientInterface>::Client<neocogi_com::ThreadLocal>>,
{
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
        self.interface.cross_size()
    }
    /// Replaces the line's complete typed cross policy.
    pub fn set_cross_size(&mut self, cross: CrossSize) -> Result<(), NcStatus> {
        self.interface.set_cross_size(cross)
    }
}

/// Selects one Grid track axis explicitly, avoiding callback-based bulk access.
enum Axis {
    Columns,
    Rows,
}
impl Axis {
    /// Executes the selected concrete copied-policy read.
    fn copy(
        &self,
        grid: &neocogi_ui_contracts::abi::nodes::GridClient<neocogi_com::ThreadLocal>,
        output: neocogi_com::NcOutput<'_, LayoutTrack>,
    ) -> Result<u32, NcStatus> {
        match self {
            Self::Columns => grid.columns(output),
            Self::Rows => grid.rows(output),
        }
    }
    /// Allocates an ordinary application snapshot without retaining plugin state.
    fn read(&self, grid: &neocogi_ui_contracts::abi::nodes::GridClient<neocogi_com::ThreadLocal>) -> Result<Vec<LayoutTrack>, NcStatus> {
        let mut values = neocogi_com::NcOutputBuffer::new();
        let count = self.copy(grid, values.output(0, 0))?;
        if self.copy(grid, values.output(0, count as usize))? != count || values.len() != count as usize {
            return Err(UI_E_BUSY);
        }
        Ok(values.into_vec())
    }
}
impl<P> Widget<GridAbi, P>
where
    P: WidgetStorage<GridAbi> + core::ops::DerefMut<Target = <GridAbi as neocogi_com::ComClientInterface>::Client<neocogi_com::ThreadLocal>>,
{
    /// Returns the immutable construction policy enforced by all Grid edits.
    pub fn limits(&self) -> Result<GridLimits, NcStatus> {
        self.interface.limits()
    }
    /// Copies the current column policies into application-owned typed values.
    pub fn columns(&self) -> Result<Vec<LayoutTrack>, NcStatus> {
        Axis::Columns.read(&self.interface)
    }
    /// Replaces every column policy after the plugin validates the complete sequence.
    pub fn set_columns(&mut self, tracks: &[LayoutTrack]) -> Result<(), NcStatus> {
        self.interface.set_columns(NcCopyFromBufferOffset::new(0, tracks))
    }
    /// Copies the current row policies into application-owned typed values.
    pub fn rows(&self) -> Result<Vec<LayoutTrack>, NcStatus> {
        Axis::Rows.read(&self.interface)
    }
    /// Replaces every row policy while preserving child identities and Grid spans.
    pub fn set_rows(&mut self, tracks: &[LayoutTrack]) -> Result<(), NcStatus> {
        self.interface.set_rows(NcCopyFromBufferOffset::new(0, tracks))
    }
}

impl WidgetFactory {
    /// Creates an empty split; insert panes in first/second ownership order.
    pub fn split_view_with_queue(
        &self,
        settings: SplitOptions,
        queue: Option<ComRef<'_, crate::events::IEventReceiverAbi>>,
    ) -> Result<OwnedWidget<SplitViewAbi>, NcStatus> {
        Ok(OwnedWidget::from_interface(self.interface.create_split_view(
            settings,
            neocogi_com::NcValueRef::new(&crate::events::EventBinding::from_queue(queue)?)?,
        )?))
    }
    /// Creates a disclosure whose body starts empty and whose header owns the supplied label.
    pub fn disclosure(&self, label: &str, style: DisclosureStyle, expanded: bool) -> Result<OwnedWidget<DisclosureAbi>, NcStatus> {
        Ok(OwnedWidget::from_interface(self.interface.create_disclosure(
            NcCopyFromBufferOffset::new(0, label.as_bytes()),
            style,
            expanded,
        )?))
    }
    /// Constructs a framed scroll composite around one unmounted application widget.
    pub fn scroll_area<W: Mountable>(&self, content: W, enabled: bool) -> Result<OwnedWidget<ScrollAreaAbi>, (NcStatus, W)> {
        content.with_mount(|content| Ok(OwnedWidget::from_interface(self.interface.create_scroll_area(content, enabled)?)))
    }
    /// Creates an empty directional container with one complete axis/cross configuration.
    pub fn linear(&self, direction: FlowDirection, cross: CrossSize) -> Result<OwnedWidget<LinearAbi>, NcStatus> {
        Ok(OwnedWidget::from_interface(self.interface.create_linear(direction, cross)?))
    }
    /// Creates an empty Grid with immutable application-selected track/placement ceilings.
    /// Limits bound explicit columns and all explicit/implicit rows; no maximum-sized
    /// storage is allocated up front. Rejected edits preserve their incoming owner.
    pub fn grid(&self, columns: &[LayoutTrack], rows: &[LayoutTrack], limits: GridLimits) -> Result<OwnedWidget<GridAbi>, NcStatus> {
        Ok(OwnedWidget::from_interface(self.interface.create_grid(
            NcCopyFromBufferOffset::new(0, columns),
            NcCopyFromBufferOffset::new(0, rows),
            limits,
        )?))
    }
}

/// Typed client for runtime-owned fixed-coordinate child geometry.
pub type FormWidget = Widget<FormAbi>;
impl<P> Widget<FormAbi, P>
where
    P: WidgetStorage<FormAbi> + core::ops::DerefMut<Target = <FormAbi as neocogi_com::ComClientInterface>::Client<neocogi_com::ThreadLocal>>,
{
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
    pub fn insert<W: Mountable>(&mut self, index: u32, child: W, edge: Rect) -> Result<(), (NcStatus, W)> {
        child.with_mount(|child| self.interface.insert(index, child, edge))
    }
    /// Replaces a child with the supplied geometry.
    pub fn replace<W: Mountable>(&mut self, index: u32, child: W, edge: Rect) -> Result<(), (NcStatus, W)> {
        child.with_mount(|child| self.interface.replace(index, child, edge))
    }
    /// Drops one owned subtree.
    pub fn remove(&mut self, index: u32) -> Result<(), NcStatus> {
        self.interface.remove(index)
    }
    /// Drops all child owners while retaining this form.
    pub fn clear(&mut self) -> Result<(), NcStatus> {
        self.interface.clear()
    }
}
impl WidgetFactory {
    /// Creates a form whose child rectangles are interpreted by the runtime node.
    pub fn form(&self, size: Size) -> Result<OwnedWidget<FormAbi>, NcStatus> {
        Ok(OwnedWidget::from_interface(self.interface.create_form(size)?))
    }
}
