//! Typed composite fixture exercising the same IWidget gate for behavior and closure.

use super::*;
use neocogi_ui::events::EventModel;
use std::sync::mpsc::{Receiver, TryRecvError};

/// Test behavior adds observable close to an ordinary concrete message handler.
pub(super) trait Behavior: EventModel {
    /// Runs after descendants close and the private receiving endpoint is dropped.
    fn close(&mut self) -> Result<(), NcStatus> {
        Ok(())
    }
}

#[com_object(interfaces(IWidget, IContainerLayout))]
/// A private inbox and model belong to the mounted widget itself.
/// The fixture delegates geometry to a real Linear; no dispatcher capability exists.
pub(super) struct Composite<Model: Behavior> {
    /// Actual widget policy and child writer supplied by the production provider.
    shell: neocogi_ui::LinearWidget,
    /// Cached layout projection of that same test geometry implementation.
    layout: ComPtr<IContainerLayoutAbi>,
    /// Concrete mutable test model; LocalCom assertions use the widget's own gate.
    pub(super) model: Model,
    /// Closure disconnects this endpoint even while test handles retain the composite.
    messages: Option<Receiver<Model::Message>>,
}
impl<Model: Behavior> Composite<Model> {
    /// Builds an unmounted composite and exposes its ordinary child-editing capability.
    pub(super) fn build(ui: &neocogi_ui::Ui, model: Model, messages: Receiver<Model::Message>) -> (neocogi_com::LocalCom<Self>, neocogi_ui::LinearWidget) {
        let shell = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
        let layout = shell.node_interface().unwrap().query().unwrap();
        let local = Self::into_local(Self {
            shell: shell.clone(),
            layout,
            model,
            messages: Some(messages),
        });
        (local, shell)
    }
}
impl<Model: Behavior> IWidget for Composite<Model> {
    /// Shares the actual runtime authority with its test geometry.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        self.shell.node_interface()?.host()
    }
    /// Claims exactly one tree attachment.
    fn mount(&mut self) -> Result<(), NcStatus> {
        self.shell.node_interface()?.mount()
    }
    /// The runtime must enter this only after any active dispatch has returned.
    fn close(&mut self) -> Result<(), NcStatus> {
        match self.messages {
            None => Ok(()),
            Some(_) => {
                self.shell.node_interface()?.close()?;
                self.messages.take();
                self.model.close()
            }
        }
    }
    /// Attempts one message with the same mutable COM gate used by close and editing.
    fn dispatch_next(&mut self) -> Result<bool, NcStatus> {
        self.shell.node_interface()?.properties()?;
        match self.messages.as_ref().ok_or(UI_E_CLOSED)?.try_recv() {
            Ok(message) => {
                self.model.handle(message)?;
                Ok(true)
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => Ok(false),
        }
    }
    /// Geometry policy remains the production layout's policy.
    fn properties(&self) -> Result<WidgetProperties, NcStatus> {
        self.shell.node_interface()?.properties()
    }
    /// Changes the same requested flags used for layout and paint.
    fn replace_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.shell.node_interface()?.replace_flags(flags)
    }
    /// Preserves unselected requested flags.
    fn set_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.shell.node_interface()?.set_flags(flags)
    }
    /// Clears only selected requested flags.
    fn clear_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.shell.node_interface()?.clear_flags(flags)
    }
    /// Uses the real widget input boundary rather than a fake runtime shortcut.
    fn update(&mut self, state: WidgetUpdate, event: WidgetEvent, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
        self.shell.node_interface()?.update(state, event, text)
    }
    /// Drains ordinary sticky geometry invalidation.
    fn take_change(&mut self) -> Result<GeometryChange, NcStatus> {
        self.shell.node_interface()?.take_change()
    }
    /// Exercises production scoped drawing.
    fn paint(&mut self, scope: PaintScope, state: WidgetPaint, painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
        self.shell.node_interface()?.paint(scope, state, painter)
    }
}
impl<Model: Behavior> IContainerLayout for Composite<Model> {
    /// Publishes the exact collection edited through the fixture's shell handle.
    fn children(&self) -> Result<ComPtr<IChildrenAbi>, NcStatus> {
        self.layout.children()
    }
    /// Uses production measurement under the runtime's scoped authority.
    fn measure(&mut self, scope: LayoutScope, bounds: LayoutLimits, service: ComRef<'_, ILayoutServiceAbi>) -> Result<Size, NcStatus> {
        self.layout.measure(scope, bounds, service)
    }
    /// Uses production placement and its child ownership validation.
    fn place(&mut self, scope: LayoutScope, rect: Rect, service: ComRef<'_, ILayoutServiceAbi>) -> Result<ContainerGeometry, NcStatus> {
        self.layout.place(scope, rect, service)
    }
    /// Retains ordinary filtering behavior for the real input traversal.
    fn accepts_event(&self, event: WidgetEvent, text: NcCopyFromBufferOffset<'_, u8>) -> Result<bool, NcStatus> {
        self.layout.accepts_event(event, text)
    }
}
impl<Model: Behavior> Drop for Composite<Model> {
    /// An unpublished test candidate follows the same idempotent close path.
    fn drop(&mut self) {
        let _ = IWidget::close(self);
    }
}
