//! Fixed scroll composition uses real COM children and explicit shared scalar data within this plugin.
use super::*;
use super::widget::{HostedWidget, WidgetRuntime};

direct_widget_phase_impl!(crate::ui_node::ScrollSurface);
direct_container_layout_impl!(crate::ui_node::ScrollSurface);

direct_widget_phase_impl!(crate::ScrollArea);
direct_container_layout_impl!(crate::ScrollArea);
direct_scrolling_impl!(crate::ScrollArea);

/// Prepared fixed scroll tree, shared by ordinary ScrollArea and TextArea construction.
/// This value is consumed into one final object; it never becomes a second live owner or widget.
pub(crate) struct ScrollParts {
    /// Fixed surface and actual bar topology.
    pub(crate) editor: ComPtr<IChildrenEditorAbi>,
    /// Private insertion authority for the prepared surface's single content child.
    pub(crate) content: ComPtr<IChildrenEditorAbi>,
    /// Explicit scalar model supplied to an editor before its content attachment.
    pub(crate) model: Rc<crate::ui_node::ScrollModel>,
}
impl ScrollParts {
    /// Builds every structural role before attaching application content or the internal editor.
    pub(crate) fn prepare(host: ComPtr<IWidgetHostAbi>, enabled: bool) -> Result<Self, NcStatus> {
        host.check_mutation()?;
        let horizontal_parameters = crate::ScrollbarParameters::new(crate::ScrollbarAxis::Horizontal);
        let horizontal_options = horizontal_parameters.opt;
        let horizontal_runtime = super::widget::WidgetRuntime::new(host.clone(), horizontal_options)?;
        let horizontal = crate::Scrollbar::from_runtime(horizontal_runtime, horizontal_parameters);
        let vertical_parameters = crate::ScrollbarParameters::new(crate::ScrollbarAxis::Vertical);
        let vertical_options = vertical_parameters.opt;
        let vertical_runtime = super::widget::WidgetRuntime::new(host.clone(), vertical_options)?;
        let vertical = crate::Scrollbar::from_runtime(vertical_runtime, vertical_parameters);
        let model = crate::ui_node::ScrollModel::new(horizontal.axis_state.clone(), vertical.axis_state.clone(), enabled);
        let horizontal = horizontal.into_com();
        let vertical = vertical.into_com();
        let content = host.create_children()?;
        let surface_runtime = super::widget::WidgetRuntime::new(host.clone(), crate::WidgetOption::NO_INTERACT)?;
        let surface = crate::ui_node::ScrollSurface::from_runtime(surface_runtime, content.clone(), model.clone());
        let surface: ComPtr<IWidgetAbi> = crate::ui_node::ScrollSurface::from_value(surface)
        .query()
        .expect("ScrollSurface declares its widget and container-layout capabilities");
        let editor = host.create_children()?;
        editor.insert(0, surface.as_com_ref())?.finish()?;
        editor.insert(1, horizontal.query::<IWidgetAbi>()?.as_com_ref())?.finish()?;
        editor.insert(2, vertical.query::<IWidgetAbi>()?.as_com_ref())?.finish()?;
        Ok(Self { editor, content, model })
    }
}
impl crate::ScrollArea {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create_com(host: ComPtr<IWidgetHostAbi>, content: ComRef<'_, IWidgetAbi>, enabled: bool) -> Result<ComPtr<ScrollAreaAbi>, NcStatus> {
        let value = <Self as microui_contracts::ScrollArea>::new(host.as_com_ref(), content, enabled)?;
        Ok(Self::from_value(value)
            .query()
            .expect("ScrollArea declares its callable class contract"))
    }
}

impl microui_contracts::ScrollArea for crate::ScrollArea {
    /// Completes internal construction before consuming the application's sole content attachment.
    fn new(host: ComRef<'_, IWidgetHostAbi>, content: ComRef<'_, IWidgetAbi>, enabled: bool) -> Result<Self, NcStatus> {
        let parts = ScrollParts::prepare(host.add_ref(), enabled)?;
        parts.content.insert(0, content)?.finish()?;
        let runtime = WidgetRuntime::new(host.add_ref(), crate::WidgetOption::FRAME)?;
        Ok(Self::from_runtime(runtime, parts.editor, parts.content, parts.model))
    }
    /// Reads the actual content identity through its fixed one-child editor view.
    fn content(&self) -> Result<ComPtr<IWidgetAbi>, NcStatus> {
        self.runtime().state.readable()?;
        self.content.view()?.child(0)
    }
    /// Guards the parent state before replacing the actual content subtree, preserving fixed internal roles.
    fn replace_content(&mut self, content: ComRef<'_, IWidgetAbi>) -> Result<ChildEditResult, NcStatus> {
        self.runtime().state.writable()?;
        self.content.replace(0, content)
    }
}
