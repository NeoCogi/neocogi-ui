//! Shared construction authority, independent of any root forest or runtime transaction.
use super::HostState;
use neocogi_ui_contracts::*;
use neocogi_com::{ComPtr, ComRef, IUnknown, NcStatus, NC_E_INVALID_ARG};
use std::{cell::{Cell, RefCell}, rc::Rc};
use super::{widget_object::SharedAssets, paint_service::PaintService};
use super::{child_store::ChildStore, mounted_node::{ComNode, NodeLayoutKind}, layout_service::LayoutService};

/// Runtime-bound services shared by root attachment and owned child collections.
/// Nothing in this allocation retains a runtime, root tree or application handler.
pub(crate) struct NodeFactory {
    /// Attachment validation calls foreign interfaces; nested topology edits must wait for it to finish.
    pub(crate) attaching: Cell<bool>,
    /// Weak authentication map for the read-only views issued by this exact host.
    pub(crate) children: RefCell<super::child_store::ChildRegistry>,
    /// Common phase, lifetime and first-error authority.
    pub(crate) state: Rc<HostState>,
    /// Canonical host identity used to reject objects from another UI instance.
    pub(crate) host: ComPtr<IWidgetHostAbi>,
    /// One current imported asset value observed by built-in state and layout services.
    pub(crate) resources: SharedAssets,
    /// Borrowed measurement service supplied to independent leaf implementations.
    pub(crate) metrics: ComPtr<ITextMetricsAbi>,
    /// Reusable scope-checked owned painter service.
    pub(crate) painter: PaintService,
    /// One owned nested layout service, with no pointer to the root forest.
    pub(crate) layout: LayoutService,
}

impl NodeFactory {
    /// Validates every fallible capability query before claiming the sole attachment.
    pub(crate) fn mount_node(&self, root: ComRef<'_, IWidgetAbi>, parent: Option<&Rc<ChildStore>>) -> Result<crate::Node, NcStatus> {
        self.check_edit()?;
        self.attaching.set(true);
        let _attachment = Attachment(&self.attaching);
        let mut widget = root.add_ref();
        let owner: ComPtr<IUnknown> = widget.host()?.query()?;
        if owner.as_ptr() != self.host.query::<IUnknown>()?.as_ptr() {
            return Err(UI_E_OWNER);
        }
        // Only NOINTERFACE means an absent capability. ABI mismatches and other
        // query failures must remain errors, never select a different layout path.
        let leaf = widget.query::<ILeafLayoutAbi>();
        let branch = widget.query::<IContainerLayoutAbi>();
        let layout = match (leaf, branch) {
            (Ok(leaf), Err(neocogi_com::NC_E_NOINTERFACE)) => NodeLayoutKind::Leaf(leaf),
            (Err(neocogi_com::NC_E_NOINTERFACE), Ok(policy)) => {
                let view = policy.children()?;
                let identity = view.query::<IUnknown>()?.as_ptr() as usize;
                let children = self
                    .children
                    .try_borrow()
                    .map_err(|_| UI_E_BUSY)?
                    .get(&identity)
                    .and_then(std::rc::Weak::upgrade)
                    .ok_or(UI_E_OWNER)?;
                children.validate_attachment(parent)?;
                NodeLayoutKind::Branch { policy, _view: view, children }
            }
            (Ok(_), Ok(_)) => return Err(NC_E_INVALID_ARG),
            (Err(error), _) if error != neocogi_com::NC_E_NOINTERFACE => return Err(error),
            (_, Err(error)) | (Err(error), Ok(_)) => return Err(error),
        };
        let properties = widget.properties()?;
        properties.validate()?;
        widget.mount()?;
        if let NodeLayoutKind::Branch { children, .. } = &layout {
            // The host-wide attachment guard prevents another edit between the
            // capability/cycle validation and this one-way collection claim.
            children.claimed.set(true);
        }
        Ok(crate::Node::com_widget(ComNode {
            widget,
            dirty: Cell::new(false),
            layout,
            properties,
            metrics: self.metrics.clone(),
            painter: self.painter.clone(),
            layout_service: self.layout.clone(),
            host: self.state.clone(),
        }))
    }

    /// Rejects reentrant topology changes while another attachment is validating foreign policy.
    pub(super) fn check_edit(&self) -> Result<(), NcStatus> {
        if self.attaching.get() { Err(UI_E_BUSY) } else { Ok(()) }
    }
}

/// One host-wide attachment transaction forbids reentrant edge edits without blocking WidgetState::mount.
struct Attachment<'a>(&'a Cell<bool>);
impl Drop for Attachment<'_> {
    /// Restores topology authority after success or any fallible capability query.
    fn drop(&mut self) {
        self.0.set(false);
    }
}
