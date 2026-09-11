//! Shared state for concrete Rust widget implementations.
//!
//! This helper lives inside a widget's COM allocation. Its Rust layout never
//! crosses an interface boundary and it owns no children, runtime, or app sink.

use super::{GeometryChange, IWidgetHostAbi, UI_E_CLOSED, UI_E_OWNER, WidgetOptions};
use neocogi_com::{ComPtr, ComRef, NcStatus};

/// Common logical lifetime and sticky geometry state for one concrete widget.
///
/// Compose this value into each widget class. The host capability must contain
/// only authority state, without a strong reference back to the owning tree.
/// Setters call `writable` before mutation and `record_change` after committing
/// changed measurement or placement inputs. Hidden widgets retain that marker.
pub struct WidgetState {
    /// Runtime authority shared across widgets without an ownership cycle.
    host: ComPtr<IWidgetHostAbi>,
    /// Mutable common presentation and routing options.
    options: WidgetOptions,
    /// Geometry changes retained until the runtime collects them before layout.
    dirty: bool,
    /// One-way attachment state for the lifetime of this allocation.
    mounted: bool,
    /// Logical closure invalidating retained handles before final COM release.
    closed: bool,
}

impl WidgetState {
    /// Creates unmounted state; the first node measurement starts uncached.
    pub fn new(host: ComPtr<IWidgetHostAbi>, options: WidgetOptions) -> Self {
        Self {
            host,
            options,
            dirty: false,
            mounted: false,
            closed: false,
        }
    }

    /// Reads the current copied options without exposing a state reference.
    pub fn options(&self) -> Result<WidgetOptions, NcStatus> {
        self.readable()?;
        Ok(self.options)
    }

    /// Validates an option replacement before mutation and retains its geometry marker.
    pub fn set_options(&mut self, options: WidgetOptions) -> Result<(), NcStatus> {
        self.writable()?;
        options.validate()?;
        if self.options != options {
            self.options = options;
            self.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }

    /// Borrows the immutable owner capability for attachment authentication.
    pub fn host(&self) -> ComRef<'_, IWidgetHostAbi> {
        self.host.as_com_ref()
    }

    /// Allows observations during layout and paint while rejecting logical closure.
    pub fn readable(&self) -> Result<(), NcStatus> {
        self.local_open()?;
        self.host.check_open()
    }

    /// Rejects this allocation's terminal state without consulting its owner.
    fn local_open(&self) -> Result<(), NcStatus> {
        if self.closed { Err(UI_E_CLOSED) } else { Ok(()) }
    }

    /// Authorizes an ordinary mutation before the implementation borrows state.
    pub fn writable(&self) -> Result<(), NcStatus> {
        if self.closed {
            return Err(UI_E_CLOSED);
        }
        self.host.check_mutation()
    }

    /// Records an authorized, committed geometry change without a call out.
    ///
    /// `NO_CHANGE` preserves an earlier setter's pending marker. Event-only or
    /// visual-only changes need no measurement invalidation. External state must
    /// be synchronized through an authorized setter or a truthful update result.
    pub fn record_change(&mut self, change: GeometryChange) {
        if change == GeometryChange::CHANGED {
            self.dirty = true;
        }
    }

    /// Drains the marker once while the owning runtime already holds phase authority.
    ///
    /// This is the runtime's mutable collection protocol, not an application
    /// observation. Mounted widgets close directly during tree teardown, so the
    /// local terminal flag is authoritative during a tree scan.
    pub fn take_change(&mut self) -> Result<GeometryChange, NcStatus> {
        self.local_open()?;
        let dirty = self.dirty;
        self.dirty = false;
        Ok(if dirty { GeometryChange::CHANGED } else { GeometryChange::NO_CHANGE })
    }

    /// Commits attachment after owner, capability, and topology checks pass.
    pub fn mount(&mut self) -> Result<(), NcStatus> {
        self.writable()?;
        if self.mounted {
            Err(UI_E_OWNER)
        } else {
            self.mounted = true;
            Ok(())
        }
    }

    /// Closes this widget once and reports whether event queues need clearing.
    ///
    /// Runtime teardown may already have closed the host. Otherwise observational
    /// phases still reject closure, just as they reject semantic setters. The
    /// implementation clears its typed queues after this returns `true`.
    pub fn close(&mut self) -> Result<bool, NcStatus> {
        if self.closed {
            return Ok(false);
        }
        match self.host.check_mutation() {
            Ok(()) | Err(UI_E_CLOSED) => {}
            Err(error) => return Err(error),
        }
        self.closed = true;
        self.dirty = false;
        Ok(true)
    }
}
