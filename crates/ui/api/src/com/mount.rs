//! Typed attachment descriptions keep behavior ownership out of widget proxies.

use super::*;

/// A value that can describe one widget or composed view at its attachment boundary.
///
/// Constructing a description retains capabilities but does not mount anything.
/// The runtime validates and commits the one-way attachment in the normal
/// window creation or container edit transaction.
pub trait Mountable {
    /// Produces concrete view/controller capabilities, with no erased Rust state.
    fn mount_description(&self) -> Result<Mount, NcStatus>;
}

impl<I: ComInterface> Mountable for Widget<I> {
    /// Projects the actual widget once when preparing its attachment.
    fn mount_description(&self) -> Result<Mount, NcStatus> {
        Ok(Mount::Widget { view: self.node_interface()? })
    }
}

impl Mountable for Mount {
    /// Retains a caller-authored description without interpreting or copying its model.
    fn mount_description(&self) -> Result<Mount, NcStatus> {
        Ok(self.clone())
    }
}

/// A composed view with one persistent event controller, ready for one attachment.
///
/// The view does not retain the controller. Its child outputs retain only
/// senders to the controller's inbox, so the controller may retain view handles
/// without creating an ownership cycle through this description.
pub struct Component<I: ComInterface> {
    /// Actual widget projection used for layout, input, painting, and logical closure.
    pub view: Widget<I>,
    /// Behavior retained by the runtime for exactly this view's mounted lifetime.
    owner: ComPtr<IEventOwnerAbi>,
}

impl<I: ComInterface> Component<I> {
    /// Associates capabilities before they are atomically claimed by an attachment.
    pub fn new(view: Widget<I>, owner: ComPtr<IEventOwnerAbi>) -> Self {
        Self { view, owner }
    }
}

impl<I: ComInterface> Clone for Component<I> {
    /// Retains the same pending or mounted component; cloning never permits reattachment.
    fn clone(&self) -> Self {
        Self {
            view: self.view.clone(),
            owner: self.owner.clone(),
        }
    }
}

impl<I: ComInterface> Mountable for Component<I> {
    /// Supplies both capabilities directly, avoiding a forwarding widget allocation.
    fn mount_description(&self) -> Result<Mount, NcStatus> {
        Ok(Mount::Component {
            view: self.view.node_interface()?,
            owner: self.owner.clone(),
        })
    }
}
