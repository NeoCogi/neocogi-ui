//! Fixed scroll composition uses real COM children and explicit shared scalar data within this plugin.
use super::*;
use super::widget::WidgetCore;

#[com_object(interfaces(IWidget, IContainerLayout))]
/// Transform-only COM container owning the ordinary application content widget.
struct ScrollSurface {
    /// The existing transform/layout algorithm reads the same scalar axes as the actual bar objects.
    widget: WidgetCore<crate::ui_node::ScrollSurface>,
    /// Fixed one-child topology; replacement authority is retained by the outer scroll object.
    editor: ComPtr<IChildrenEditorAbi>,
}
fixed_phases!(ScrollSurface);

#[com_object(microui_contracts::ScrollArea)]
/// One scroll-area COM identity owns its three structural nodes and shares no parent-widget backpointer.
pub struct ScrollAreaObject {
    /// Existing convergence, wheel routing, padding and panel paint algorithms remain authoritative.
    widget: WidgetCore<crate::ScrollArea>,
    /// Exactly three owned children: transform surface, horizontal bar and vertical bar.
    editor: ComPtr<IChildrenEditorAbi>,
    /// Private authority for the surface's one replaceable application child, not a duplicate tree.
    content: ComPtr<IChildrenEditorAbi>,
}
fixed_phases!(ScrollAreaObject);

/// Prepared fixed scroll tree, shared by ordinary ScrollArea and TextArea construction.
/// This value is consumed into one final object; it never becomes a second live owner or widget.
pub(super) struct ScrollParts {
    /// Existing scalar scroll/layout state for the final outer COM identity.
    pub(super) widget: WidgetCore<crate::ScrollArea>,
    /// Fixed surface and actual bar topology.
    pub(super) editor: ComPtr<IChildrenEditorAbi>,
    /// Private insertion authority for the prepared surface's single content child.
    pub(super) content: ComPtr<IChildrenEditorAbi>,
    /// Explicit scalar model supplied to an editor before its content attachment.
    pub(super) model: Rc<crate::ui_node::ScrollModel>,
}
impl ScrollParts {
    /// Builds every structural role before attaching application content or the internal editor.
    pub(super) fn prepare(host: ComPtr<IWidgetHostAbi>, enabled: bool) -> Result<Self, NcStatus> {
        host.check_mutation()?;
        let horizontal_parameters = crate::ScrollbarParameters::new(crate::ScrollbarAxis::Horizontal);
        let horizontal_options = horizontal_parameters.opt;
        let horizontal = crate::Scrollbar::from(horizontal_parameters);
        let vertical_parameters = crate::ScrollbarParameters::new(crate::ScrollbarAxis::Vertical);
        let vertical_options = vertical_parameters.opt;
        let vertical = crate::Scrollbar::from(vertical_parameters);
        let model = crate::ui_node::ScrollModel::new(horizontal.axis_state.clone(), vertical.axis_state.clone(), enabled);
        let horizontal = super::leaves::ScrollbarObject::from_core(host.clone(), horizontal_options, horizontal)?;
        let vertical = super::leaves::ScrollbarObject::from_core(host.clone(), vertical_options, vertical)?;
        let content = host.create_children()?;
        let surface: ComPtr<IWidgetAbi> = ScrollSurface::from_value(ScrollSurface {
            widget: WidgetCore::new(
                host.clone(),
                crate::WidgetOption::NO_INTERACT,
                crate::ui_node::ScrollSurface { model: model.clone() },
            )?,
            editor: content.clone(),
        })
        .query()
        .expect("surface declares IWidget");
        let editor = host.create_children()?;
        editor.insert(0, surface.as_com_ref())?.finish()?;
        editor.insert(1, horizontal.query::<IWidgetAbi>()?.as_com_ref())?.finish()?;
        editor.insert(2, vertical.query::<IWidgetAbi>()?.as_com_ref())?.finish()?;
        let widget = WidgetCore::new(host, crate::WidgetOption::FRAME, crate::ScrollArea::from_model(model.clone()))?;
        Ok(Self { widget, editor, content, model })
    }
}
impl ScrollAreaObject {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create(
        host: ComPtr<IWidgetHostAbi>,
        content: ComRef<'_, IWidgetAbi>,
        enabled: bool,
    ) -> Result<ComPtr<ScrollAreaAbi>, NcStatus> {
        let value = <Self as ScrollArea>::new(host.as_com_ref(), content, enabled)?;
        Ok(Self::from_value(value)
            .query()
            .expect("ScrollAreaObject declares the ScrollArea class contract"))
    }
}

impl ScrollArea for ScrollAreaObject {
    /// Completes internal construction before consuming the application's sole content attachment.
    fn new(host: ComRef<'_, IWidgetHostAbi>, content: ComRef<'_, IWidgetAbi>, enabled: bool) -> Result<Self, NcStatus> {
        let parts = ScrollParts::prepare(host.add_ref(), enabled)?;
        parts.content.insert(0, content)?.finish()?;
        Ok(Self {
            widget: parts.widget,
            editor: parts.editor,
            content: parts.content,
        })
    }
    /// Reads the actual content identity through its fixed one-child editor view.
    fn content(&self) -> Result<ComPtr<IWidgetAbi>, NcStatus> {
        self.widget.state.readable()?;
        self.content.view()?.child(0)
    }
    /// Guards the parent state before replacing the actual content subtree, preserving fixed internal roles.
    fn replace_content(&self, content: ComRef<'_, IWidgetAbi>) -> Result<ChildEditResult, NcStatus> {
        self.widget.state.writable()?;
        let _core = self.widget.take()?;
        self.content.replace(0, content)
    }
}

scrolling_impl!(ScrollAreaObject);
