//! Host-owned child topology and scoped container layout contracts.
use nc_com::{ComPtr, ComRef, NcStatus, NcCopyFromBufferOffset, com_class, com_interface, com_struct, com_int_enum, com_enum};
use crate::{Rect, Size, LayoutLimits, WidgetEventAbi};

#[com_struct]
/// Non-reused authority for one synchronous container measurement or placement.
pub struct LayoutScope {
    /// Zero is never active; retaining a service does not extend this invocation.
    pub invocation: u64,
}

com_int_enum! {
    /// Parent-authored participation, independent of a child's semantic options.
    pub enum LayoutParticipation: u8 {
        /// Update, paint and input are enabled.
        ACTIVE = 0,
        /// Update and paint remain enabled; the entire subtree rejects input.
        DISABLED = 1,
        /// Ownership remains; ordinary update, paint and input skip this subtree.
        HIDDEN = 2,
    }
}

#[com_struct]
/// Complete copied parent layout output, published only after a successful placement.
pub struct ContainerGeometry {
    /// Logical extent visible to the parent's overflow policy.
    pub content: Size,
    /// Descendant viewport in the container's local coordinates, clipped to its content by runtime.
    pub viewport: Rect,
    /// Horizontal descendant translation in the container's local coordinates.
    pub offset_x: i32,
    /// Vertical descendant translation in the container's local coordinates.
    pub offset_y: i32,
    /// Whether visible descendant overflow contributes to this container's extent.
    pub propagate_overflow: bool,
}
impl ContainerGeometry {
    /// Starts with the full content viewport, no translation and ordinary overflow propagation.
    pub fn new(rect: Rect) -> Self {
        Self {
            content: Size { width: rect.width, height: rect.height },
            viewport: rect,
            offset_x: 0,
            offset_y: 0,
            propagate_overflow: true,
        }
    }
    /// Rejects negative extents before the runtime publishes geometry.
    pub fn validate(self) -> Result<(), NcStatus> {
        if self.content.width < 0 || self.content.height < 0 || self.viewport.width < 0 || self.viewport.height < 0 {
            Err(nc_com::NC_E_INVALID_ARG)
        } else {
            Ok(())
        }
    }
}

#[com_interface(auto_guid = "microui.LayoutService:v1")]
/// Owned scoped child geometry service; no borrowed runtime node crosses COM.
/// Measurement permits only count and measure. Placement permits every method.
/// Calls reject expired/parent tokens and direct reentry while a child is active.
pub trait ILayoutService {
    /// Returns the direct child count for this exact active invocation.
    fn child_count(&self, scope: LayoutScope) -> Result<u32, NcStatus>;
    /// Measures a child's outer preference, including its own frame geometry.
    fn measure_child(&self, scope: LayoutScope, index: u32, limits: LayoutLimits) -> Result<Size, NcStatus>;
    /// Assigns one exact parent-content rectangle; runtime never reinterprets sizing policy.
    fn place_child(&self, scope: LayoutScope, index: u32, rect: Rect) -> Result<Size, NcStatus>;
    /// Reads the content extent produced by the child's most recent placement.
    fn child_content_size(&self, scope: LayoutScope, index: u32) -> Result<Size, NcStatus>;
    /// Sets one child's derived participation, including its entire subtree.
    fn set_participation(&self, scope: LayoutScope, index: u32, participation: LayoutParticipation) -> Result<(), NcStatus>;
    /// Invalidates placement when a shared dependency changes despite an unchanged assigned rectangle.
    fn child_placement_changed(&self, scope: LayoutScope, index: u32) -> Result<(), NcStatus>;
}

#[com_interface(auto_guid = "microui.ContainerLayout:v1")]
/// Container policy beside IWidget on the same object; mutually exclusive with ILeafLayout.
pub trait IContainerLayout {
    /// Returns this container's immutable, host-issued collection identity for its entire lifetime.
    /// The implementation retains its matching editor privately and publishes metadata after every committed edit.
    fn children(&self) -> Result<ComPtr<crate::IChildrenAbi>, NcStatus>;
    /// Measures content through indexed scoped access; semantic and topology mutations are forbidden.
    fn measure(&mut self, scope: LayoutScope, bounds: LayoutLimits, layout: ComRef<'_, ILayoutServiceAbi>) -> Result<Size, NcStatus>;
    /// Places direct children and returns complete copied viewport/content policy.
    fn place(&mut self, scope: LayoutScope, rect: Rect, layout: ComRef<'_, ILayoutServiceAbi>) -> Result<ContainerGeometry, NcStatus>;
    /// Filters a router-selected event; false permits ancestor bubbling without exposing covered siblings.
    /// The runtime bypasses this filter for captured drag/release continuation.
    fn accepts_event(&self, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<bool, NcStatus>;
}

#[com_interface(auto_guid = "microui.ChildrenEditor:v1")]
/// Private topology authority retained by a container implementation.
/// Its read-only view is a distinct COM identity: querying that view cannot
/// bypass a concrete container's matching Linear/Grid/domain edge metadata.
pub trait IChildrenEditor {
    /// Returns the read-only capability used by the runtime's authoritative traversal.
    fn view(&self) -> Result<ComPtr<crate::IChildrenAbi>, NcStatus>;
    /// Inserts one unmounted child at index (including count), committing only after validation.
    fn insert(&self, index: u32, widget: ComRef<'_, crate::IWidgetAbi>) -> Result<ChildEditResult, NcStatus>;
    /// Validates the replacement before permanently closing the old indexed subtree.
    fn replace(&self, index: u32, widget: ComRef<'_, crate::IWidgetAbi>) -> Result<ChildEditResult, NcStatus>;
    /// Permanently closes one indexed subtree; it cannot be detached or reparented.
    fn remove(&self, index: u32) -> Result<ChildEditResult, NcStatus>;
    /// Permanently closes all children while leaving this collection open for new content.
    fn clear(&self) -> Result<ChildEditResult, NcStatus>;
    /// Permanently closes the collection and its descendants, including retained read-only views.
    fn close(&self) -> Result<ChildEditResult, NcStatus>;
}

#[com_struct]
#[derive(Debug, Eq, PartialEq)]
/// Successful topology commit, including a separately reported teardown result.
/// An editor method's Err means no edit committed. Ok means the container must
/// publish matching edge metadata before propagating this teardown status.
pub struct ChildEditResult {
    /// Error from closing a replaced/removed subtree, or NC_OK after clean teardown.
    pub teardown: NcStatus,
}
impl ChildEditResult {
    /// Reports teardown only after the caller commits its matching relationship metadata.
    pub fn finish(self) -> Result<(), NcStatus> {
        if self.teardown.is_err() { Err(self.teardown) } else { Ok(()) }
    }
}

#[com_enum(tag = u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
/// One parent-owned track; variants carry only the data relevant to their policy.
pub enum LayoutTrack {
    /// Uses the child's intrinsic preferred extent.
    Content,
    /// Uses an exact nonnegative pixel extent.
    Fixed {
        /// Exact nonnegative pixel extent.
        extent: i32,
    },
    /// Receives a strictly positive finite weighted share of bounded remaining space.
    Flex {
        /// Strictly positive finite share of bounded remaining space.
        weight: f32,
    },
}

#[com_enum(tag = u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Shared cross-axis size of a linear container's line.
pub enum CrossSize {
    /// Fits the largest child's preferred cross extent.
    Content,
    /// Reports preferred content, then stretches to its exact placement allocation.
    Stretch,
    /// Uses an exact nonnegative pixel extent.
    Fixed {
        /// Exact nonnegative pixel extent.
        extent: i32,
    },
}

#[com_enum(tag = u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Optional exact child cross extent; Stretch uses the owning linear line's cross size.
pub enum ChildCrossSize {
    /// Uses the line's resolved cross extent.
    Stretch,
    /// Uses an exact nonnegative pixel extent.
    Fixed {
        /// Exact nonnegative pixel extent.
        extent: i32,
    },
}

com_int_enum! {
    /// Main axis and leading edge, without independent potentially inconsistent flags.
    pub enum FlowDirection: u8 {
        /// Horizontal sequence starting at the left edge.
        LEFT_TO_RIGHT = 0,
        /// Horizontal sequence starting at the right edge.
        RIGHT_TO_LEFT = 1,
        /// Vertical sequence starting at the top edge.
        TOP_TO_BOTTOM = 2,
        /// Vertical sequence starting at the bottom edge.
        BOTTOM_TO_TOP = 3,
    }
}

#[com_struct]
/// Linear-owned relationship stored beside one direct child.
pub struct LinearEdge {
    /// Main-axis content, exact or weighted-flexible policy.
    pub main: LayoutTrackAbi,
    /// Optional exact cross extent for this child only.
    pub cross: ChildCrossSizeAbi,
}
impl LinearEdge {
    /// Creates an ordinary relationship that follows the Linear line's resolved cross extent.
    pub fn new(main: LayoutTrack) -> Self {
        Self {
            main: main.into(),
            cross: ChildCrossSize::Stretch.into(),
        }
    }
    /// Requests an exact cross extent; the receiving container validates it before mutation.
    pub fn with_fixed_cross(mut self, extent: i32) -> Self {
        self.cross = ChildCrossSize::Fixed { extent }.into();
        self
    }
}

#[com_struct]
#[derive(Debug, Eq, PartialEq)]
/// Positive Grid-owned cell span; placement clamps columns at the right grid boundary.
pub struct GridExtent {
    /// Number of logical columns, greater than zero.
    pub columns: u32,
    /// Number of logical rows, greater than zero.
    pub rows: u32,
}

#[com_interface(auto_guid = "microui.Container:v1")]
/// Common editing capability inherited by built-in concrete container interfaces.
pub trait IContainer: crate::IWidget {
    /// Returns the stable read-only view; editing stays on the concrete container to preserve edge metadata.
    fn children(&self) -> Result<ComPtr<crate::IChildrenAbi>, NcStatus>;
    /// Removes one complete subtree and matching edge metadata; Ok reports a committed edit.
    fn remove(&mut self, index: u32) -> Result<ChildEditResult, NcStatus>;
    /// Removes all owned subtrees and their matching metadata while retaining reusable capacity.
    fn clear(&mut self) -> Result<ChildEditResult, NcStatus>;
}

#[com_class(auto_guid = "microui.Linear:v1")]
/// Constructible directional container whose concrete editing API is one class contract.
pub trait Linear: crate::IWidget + IContainerLayout {
    /// Creates an empty unmounted container after validating its complete layout policy.
    #[constructor]
    fn new(host: ComRef<'_, crate::IWidgetHostAbi>, direction: FlowDirection, cross: CrossSizeAbi) -> Result<Self, NcStatus>;
    /// Returns the stable read-only topology view for this container's lifetime.
    fn children(&self) -> Result<ComPtr<crate::IChildrenAbi>, NcStatus>;
    /// Removes one complete subtree and matching edge metadata.
    fn remove(&mut self, index: u32) -> Result<ChildEditResult, NcStatus>;
    /// Removes all owned subtrees and matching metadata while retaining reusable capacity.
    fn clear(&mut self) -> Result<ChildEditResult, NcStatus>;
    /// Returns its complete axis and leading-edge policy.
    fn direction(&self) -> Result<FlowDirection, NcStatus>;
    /// Replaces the direction without reordering child ownership.
    fn set_direction(&mut self, direction: FlowDirection) -> Result<(), NcStatus>;
    /// Returns the shared cross-axis size policy.
    fn cross_size(&self) -> Result<CrossSizeAbi, NcStatus>;
    /// Validates and replaces the shared cross-axis policy.
    fn set_cross_size(&mut self, cross: CrossSizeAbi) -> Result<(), NcStatus>;
    /// Returns one child's exact parent-owned sizing relationship.
    fn edge(&self, index: u32) -> Result<LinearEdge, NcStatus>;
    /// Replaces sizing metadata without replacing the child's identity.
    fn set_edge(&mut self, index: u32, edge: LinearEdge) -> Result<(), NcStatus>;
    /// Validates child and relationship before committing insertion at index, including count.
    fn insert(&mut self, index: u32, child: ComRef<'_, crate::IWidgetAbi>, edge: LinearEdge) -> Result<ChildEditResult, NcStatus>;
    /// Validates replacement before permanently closing the previous indexed subtree.
    fn replace(&mut self, index: u32, child: ComRef<'_, crate::IWidgetAbi>, edge: LinearEdge) -> Result<ChildEditResult, NcStatus>;
}

#[com_class(auto_guid = "microui.Grid:v1")]
/// Constructible Grid whose topology, spans, and both track axes share one class contract.
pub trait Grid: crate::IWidget + IContainerLayout {
    /// Creates an empty unmounted Grid after validating both complete track sequences.
    #[constructor]
    fn new(
        host: ComRef<'_, crate::IWidgetHostAbi>,
        columns: NcCopyFromBufferOffset<'_, LayoutTrackAbi>,
        rows: NcCopyFromBufferOffset<'_, LayoutTrackAbi>,
    ) -> Result<Self, NcStatus>;
    /// Returns the stable read-only topology view for this container's lifetime.
    fn children(&self) -> Result<ComPtr<crate::IChildrenAbi>, NcStatus>;
    /// Removes one complete subtree and matching span metadata.
    fn remove(&mut self, index: u32) -> Result<ChildEditResult, NcStatus>;
    /// Removes all owned subtrees and matching metadata while retaining reusable capacity.
    fn clear(&mut self) -> Result<ChildEditResult, NcStatus>;
    /// Copies a range of column policies; offset indexes tracks and the return value is total count.
    fn columns(&self, output: nc_com::NcCopyToBufferOffset<'_, LayoutTrackAbi>) -> Result<u32, NcStatus>;
    /// Replaces the complete column sequence after validating every track; input offset must be zero.
    fn set_columns(&mut self, tracks: NcCopyFromBufferOffset<'_, LayoutTrackAbi>) -> Result<(), NcStatus>;
    /// Copies a range of row policies; offset indexes tracks and the return value is total count.
    fn rows(&self, output: nc_com::NcCopyToBufferOffset<'_, LayoutTrackAbi>) -> Result<u32, NcStatus>;
    /// Replaces the complete row sequence after validating every track; input offset must be zero.
    fn set_rows(&mut self, tracks: NcCopyFromBufferOffset<'_, LayoutTrackAbi>) -> Result<(), NcStatus>;
    /// Returns one child's positive Grid-owned span.
    fn edge(&self, index: u32) -> Result<GridExtent, NcStatus>;
    /// Changes span without replacing the child's identity.
    fn set_edge(&mut self, index: u32, edge: GridExtent) -> Result<(), NcStatus>;
    /// Validates a positive span and unmounted child before committing insertion.
    fn insert(&mut self, index: u32, child: ComRef<'_, crate::IWidgetAbi>, edge: GridExtent) -> Result<ChildEditResult, NcStatus>;
    /// Validates replacement before permanently closing the previous indexed subtree.
    fn replace(&mut self, index: u32, child: ComRef<'_, crate::IWidgetAbi>, edge: GridExtent) -> Result<ChildEditResult, NcStatus>;
}

#[com_struct]
#[derive(Debug, Eq, PartialEq)]
/// Coherent scrolling observation; requested offsets are clamped against new content during placement.
pub struct ScrollInfo {
    /// Actual nonnegative offset requests shared with the scrollbar widgets.
    pub requested: crate::Point,
    /// Maximum offsets for the most recently installed scrollbar ranges.
    pub maximum: crate::Point,
    /// Most recently committed content viewport, excluding padding and visible bars.
    pub viewport: Size,
    /// Whether wheel input and responsive scrollbar participation are enabled.
    pub enabled: bool,
}

#[com_interface(auto_guid = "microui.Scrolling:v1")]
/// Shared scroll policy; this capability cannot replace the fixed content of a composed editor.
pub trait IScrolling: crate::IWidget {
    /// Returns one coherent request/range/viewport observation without a parent-widget backpointer.
    fn info(&self) -> Result<ScrollInfo, NcStatus>;
    /// Requests nonnegative offsets; next placement clamps against freshly measured content.
    fn set_offset(&mut self, offset: crate::Point) -> Result<(), NcStatus>;
    /// Enables or disables wheel input and both structural scrollbar children.
    fn set_enabled(&mut self, enabled: bool) -> Result<(), NcStatus>;
    /// Requests the next measured vertical end while preserving the horizontal request.
    fn scroll_to_end(&mut self) -> Result<(), NcStatus>;
    /// Requests the nearest offsets that reveal a nonnegative content-local rectangle.
    fn reveal(&mut self, rect: Rect) -> Result<(), NcStatus>;
}

#[com_class(auto_guid = "microui.ScrollArea:v1")]
/// Constructible scroll composite with one replaceable content root and private structural roles.
pub trait ScrollArea: crate::IWidget + IContainerLayout + IScrolling {
    /// Creates a framed unmounted scroll tree around one unmounted application content object.
    #[constructor]
    fn new(host: ComRef<'_, crate::IWidgetHostAbi>, content: ComRef<'_, crate::IWidgetAbi>, enabled: bool) -> Result<Self, NcStatus>;
    /// Returns the actual application content widget, without its surrounding transform surface.
    fn content(&self) -> Result<ComPtr<crate::IWidgetAbi>, NcStatus>;
    /// Validates replacement before closing old content; structural roles and model identity stay fixed.
    fn replace_content(&mut self, content: ComRef<'_, crate::IWidgetAbi>) -> Result<ChildEditResult, NcStatus>;
}

com_int_enum! {
    /// Immutable disclosure presentation; expansion and child ownership use one shared policy.
    pub enum DisclosureStyle: u8 {
        /// Full framed header above the body.
        HEADER = 0,
        /// Compact tree row with an indented body.
        TREE = 1,
    }
}

#[com_class(auto_guid = "microui.Disclosure:v1")]
/// Constructible fixed header/body composition exposing its actual editable child objects.
pub trait Disclosure: crate::IWidget + IContainerLayout {
    /// Creates a complete unmounted composition with immutable presentation and initial expansion.
    #[constructor]
    fn new(host: ComRef<'_, crate::IWidgetHostAbi>, label: NcCopyFromBufferOffset<'_, u8>, style: DisclosureStyle, expanded: bool) -> Result<Self, NcStatus>;
    /// Returns shared expansion state also changed by header activation and keyboard input.
    fn expanded(&self) -> Result<bool, NcStatus>;
    /// Changes expansion and retains layout invalidation even while this subtree is hidden.
    fn set_expanded(&mut self, expanded: bool) -> Result<(), NcStatus>;
    /// Returns the real header's text capability, including its inherited widget options.
    fn header(&self) -> Result<ComPtr<crate::ITextContentAbi>, NcStatus>;
    /// Returns the real body container; its child editor keeps topology and edge metadata consistent.
    fn body(&self) -> Result<ComPtr<LinearAbi>, NcStatus>;
}
