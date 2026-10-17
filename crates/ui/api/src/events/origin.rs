//! Owner-controlled synchronous event origins, independent of posting handles.

use std::cell::RefCell;
use neocogi_com::{ComBox, ComRef, NcStatus, NcValueRef, NC_E_INVALID_ARG, NC_E_OUT_OF_MEMORY};
use super::{EventOrigin, IEventAbi, IEventReceiverAbi};

/// Origin authority held by a dispatcher or the owner of a private event queue.
///
/// Posting capabilities never expose this value through QueryInterface. A scope
/// lends no mutable stack borrow across foreign code, so nested child calls can
/// install their own origin and restore the enclosing sender on return.
#[derive(Default)]
pub struct EventOriginOwner {
    /// Only synchronous scopes modify this stack; queued messages own snapshots.
    origins: RefCell<Vec<EventOrigin>>,
}

impl EventOriginOwner {
    /// Creates an idle origin authority with no producer selected.
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs one owned dispatch under its sender identity, restoring every exit.
    ///
    /// The closure is synchronous. A returned future or deferred callback does
    /// not retain the scope and must be dispatched under a new owner scope.
    pub fn with_origin<R>(&self, origin: EventOrigin, dispatch: impl FnOnce() -> R) -> Result<R, NcStatus> {
        {
            let mut origins = self.origins.borrow_mut();
            origins.try_reserve(1).map_err(|_| NC_E_OUT_OF_MEMORY)?;
            origins.push(origin);
        }
        let _scope = OriginScope(self);
        Ok(dispatch())
    }

    /// Captures the current sender without extending the producer's value life.
    /// No active dispatch is an ordinary invalid operation, never a stale sender.
    pub fn current(&self) -> Result<EventOrigin, NcStatus> {
        self.origins.borrow().last().cloned().ok_or(NC_E_INVALID_ARG)
    }

    /// Delivers a snapshot with the active origin, preserving rejected ownership.
    /// The stack borrow ends before receiver code can run or reenter the owner.
    pub fn post(&self, mut receiver: ComRef<'_, IEventReceiverAbi>, event: ComBox<IEventAbi>) -> Result<(), (NcStatus, ComBox<IEventAbi>)> {
        let origin = match self.current() {
            Ok(origin) => origin,
            Err(status) => return Err((status, event)),
        };
        let origin = match NcValueRef::new(&origin) {
            Ok(origin) => origin,
            Err(status) => return Err((status, event)),
        };
        receiver.post(origin, event)
    }
}

/// Lexical rollback: only with_origin creates a guard and it never escapes.
struct OriginScope<'a>(&'a EventOriginOwner);
impl Drop for OriginScope<'_> {
    /// Removes exactly this frame during normal return and Rust unwinding.
    fn drop(&mut self) {
        self.0.origins.borrow_mut().pop();
    }
}
