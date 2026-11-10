//! Private CPU-frame lifetime shared by traversal and its separately allocated paint facades.
use neocogi_com::{ComBox, ComPtr, ComRef, NcStatus, NC_E_BUSY, NC_E_NOT_FOUND};
use neocogi_platform_contracts::render::abi::*;
use std::{cell::RefCell, rc::Rc};

/// Short checkout slots bridge runtime frame boundaries and sequential widget paint calls.
#[derive(Default)]
pub(super) struct Frames {
    /// Installed by the trusted host; never returned to a widget.
    broker: RefCell<Option<ComPtr<IRenderViewBrokerAbi>>>,
    /// Taken across foreign calls so no RefCell guard spans generated dispatch.
    frame: RefCell<Option<ComBox<IRenderRecordingFrameAbi>>>,
}

impl Frames {
    /// Runtime binding preflight occurs before any fallible foreign graphics preparation.
    pub(super) fn can_install(&self) -> Result<(), NcStatus> {
        if self.frame.borrow().is_some() {
            return Err(NC_E_BUSY);
        }
        Ok(())
    }

    /// The runtime owns the context throughout preparation, so no paint frame can start after preflight.
    pub(super) fn install(&self, broker: ComPtr<IRenderViewBrokerAbi>) {
        debug_assert!(self.frame.borrow().is_none());
        let old = self.broker.replace(Some(broker));
        drop(old);
    }

    /// Begins exactly once for the entire UI traversal; headless painting remains supported.
    pub(super) fn begin(self: &Rc<Self>) -> Result<Guard, NcStatus> {
        if self.frame.borrow().is_some() {
            return Err(NC_E_BUSY);
        }
        let broker = self.broker.borrow().clone();
        if let Some(broker) = broker {
            let frame = broker.begin_frame()?;
            *self.frame.borrow_mut() = Some(frame);
        }
        Ok(Guard { frames: self.clone() })
    }

    /// Issues a host-controlled session while its controller is outside the shared slot.
    pub(super) fn view(&self, root: RenderViewRoot, resources: ComRef<'_, IRenderResourcesAbi>) -> Result<ComBox<IRenderViewSessionAbi>, NcStatus> {
        let mut frame = self.frame.take().ok_or(NC_E_NOT_FOUND)?;
        let result = frame.begin_view(root, resources);
        *self.frame.borrow_mut() = Some(frame);
        result
    }
}

/// Owned paint-frame transaction; Drop only aborts and can never submit native work.
pub(crate) struct Guard {
    /// Does not retain the UI tree or expose the broker through public COM identity.
    frames: Rc<Frames>,
}
impl Guard {
    /// Completes CPU recording after all widget sessions have been harvested.
    pub(crate) fn finish(self) -> Result<(), NcStatus> {
        match self.frames.frame.take() {
            Some(mut frame) => frame.finish(),
            None => Ok(()),
        }
    }
}
impl Drop for Guard {
    /// Every early return retires pending payload before any later frame can begin.
    fn drop(&mut self) {
        if let Some(mut frame) = self.frames.frame.take() {
            let _ = frame.abort();
        }
    }
}
