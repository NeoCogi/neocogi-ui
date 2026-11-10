//! Fixed-coordinate placement for authored forms, independent of editor interaction.
use crate::nodes::{
    Change, Constraints, ContainerLayoutCtx, ContainerWidget, Dimensioni, MeasureCtx, Recti, UiInputEvent, Widget, WidgetOption, WidgetPaintCtx,
    WidgetUpdateCtx,
};
use crate::nodes::widget_object::{HostedWidget, WidgetRuntime, direct_container_layout_impl, direct_widget_phase_impl};
use neocogi_com::{ComPtr, NcStatus, NC_E_INVALID_ARG};
use neocogi_ui::abi::core::{GeometryChange, IChildrenEditorAbi, IWidgetHostAbi, Rect, Size};
use neocogi_ui::abi::nodes::FormAbi;

/// Validates coordinates and their sums before any topology or metadata mutation.
fn rectangle(edge: Rect) -> Result<Rect, NcStatus> {
    if edge.x < 0 || edge.y < 0 || edge.width <= 0 || edge.height <= 0 || edge.x.checked_add(edge.width).is_none() || edge.y.checked_add(edge.height).is_none()
    {
        return Err(NC_E_INVALID_ARG);
    }
    neocogi_ui::geometry::rectangle(edge)?;
    Ok(edge)
}
/// Copies the retained ABI value; the form has no second geometry representation.
fn encoded(edge: Rect) -> Rect {
    edge
}
/// Rejects empty or negative preferred form sizes before construction or mutation.
fn dimensions(size: Size) -> Result<Size, NcStatus> {
    if size.width <= 0 || size.height <= 0 {
        return Err(NC_E_INVALID_ARG);
    }
    Ok(size)
}

#[neocogi_com::com_object(neocogi_ui::abi::nodes::Form, ownership = exclusive)]
/// Owns fixed child rectangles; core still owns clipping, traversal, capture and focus.
pub struct Form {
    /// Common lifecycle, resource access and layout invalidation.
    runtime: WidgetRuntime,
    /// Exclusive child topology writer; external callers receive its immutable view.
    editor: neocogi_com::ComBox<IChildrenEditorAbi>,
    /// Preferred content extent; allocation may be smaller and clip the form.
    size: Size,
    /// Validated content-relative rectangles in exact child ownership order.
    edges: Vec<Rect>,
}
impl Form {
    /// Creates a host-bound form after validating its preferred dimensions.
    /// Initial requested flags are size-independent; size changes content geometry
    /// only. Authoring tools can inspect one fresh Form for its frame policy.
    pub(crate) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: crate::nodes::widget_object::SharedAssets,
        size: Size,
    ) -> Result<neocogi_com::ComBox<FormAbi>, NcStatus> {
        let size = dimensions(size)?;
        let editor = host.create_children()?;
        let runtime = WidgetRuntime::for_resources(host, WidgetOption::NO_INTERACT, Some(&resources))?;
        Ok(Self::into_box(Self { runtime, editor, size, edges: Vec::new() })
            .query()
            .expect("Form declares its class contract"))
    }
}
impl HostedWidget for Form {
    /// Borrows lifetime state without a shared mutable geometry model.
    fn runtime(&self) -> &WidgetRuntime {
        &self.runtime
    }
    /// Mutates lifetime state exclusively beside authored geometry.
    fn runtime_mut(&mut self) -> &mut WidgetRuntime {
        &mut self.runtime
    }
}
impl crate::nodes::widget_object::WidgetAlgorithm for Form {}
impl super::ContainerAlgorithm for Form {
    type Edge = Rect;
    /// Copies one parent-owned rectangle.
    fn edge(&self, index: usize) -> Option<Rect> {
        self.edges.get(index).copied()
    }
    type Prepared = super::EdgeEdit<Rect>;

    /// Fixed rectangles need only validated indices and reserved relationship storage.
    fn prepare_edges(&mut self, edit: Self::Prepared) -> Result<Self::Prepared, NcStatus> {
        let len = edit.resulting_len(self.edges.len())?;
        self.edges
            .try_reserve(len.saturating_sub(self.edges.len()))
            .map_err(|_| neocogi_com::NC_E_OUT_OF_MEMORY)?;
        Ok(edit)
    }

    /// Publishes the prepared edit without allocation after ownership has committed.
    fn commit_edges(&mut self, prepared: Self::Prepared) {
        prepared.apply(&mut self.edges);
    }
}
direct_widget_phase_impl!(Form);
direct_container_layout_impl!(Form);
impl neocogi_ui::abi::nodes::Form for Form {
    container_methods!();
    container_edges!(Rect, rectangle, encoded);
    /// Returns preferred dimensions rather than allocation-dependent bounds.
    fn size(&self) -> Result<Size, NcStatus> {
        self.runtime.state.readable()?;
        Ok(self.size)
    }
    /// Invalidates layout when the validated preferred size changes.
    fn set_size(&mut self, size: Size) -> Result<(), NcStatus> {
        self.runtime.state.writable()?;
        let size = dimensions(size)?;
        if self.size != size {
            self.size = size;
            self.runtime.state.record_change(GeometryChange::Changed);
        }
        Ok(())
    }
}
impl ContainerWidget for Form {
    /// Child positions are authored pixels, independent of intrinsic measurement.
    fn measure(&self, _: &mut MeasureCtx<'_>, _: Constraints) -> Result<Dimensioni, NcStatus> {
        Ok(Dimensioni::new(self.size.width, self.size.height))
    }
    /// Places children exactly once using the same rectangles exposed by the typed API.
    fn place(&mut self, ctx: &mut ContainerLayoutCtx<'_>, rect: Recti) -> Result<(), NcStatus> {
        for (index, edge) in self.edges.iter().enumerate() {
            ctx.layout_child(
                index,
                Recti::new(rect.x.saturating_add(edge.x), rect.y.saturating_add(edge.y), edge.width, edge.height),
            )?;
        }
        ctx.set_content_size(Dimensioni::new(self.size.width, self.size.height));
        ctx.set_child_overflow_propagation(false);
        Ok(())
    }
}
impl Widget for Form {
    /// Interaction belongs to descendants; visual authoring is a tool concern.
    fn update(&mut self, _: &mut WidgetUpdateCtx<'_>, _: Option<&UiInputEvent>) -> Result<Change, neocogi_com::NcStatus> {
        Ok(Change::NoChange)
    }
    /// The normal optional frame and descendants are painted by the existing runtime.
    fn paint(&self, _: &mut WidgetPaintCtx<'_>) {}
}
