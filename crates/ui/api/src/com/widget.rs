//! Shared state for concrete Rust widget implementations.
//!
//! This helper lives inside a widget's COM allocation. Its Rust layout never
//! crosses an interface boundary. It owns no children, runtime tree, or application model.

use super::{GeometryChange, IWidgetHostAbi, UI_E_OWNER, WidgetOptions};
use neocogi_com::{ComPtr, ComRef, NcStatus};

/// One-way tree membership. Object lifetime belongs exclusively to ComBox;
/// this state records only whether the widget has already consumed its attachment.
enum Attachment {
    /// Construction completed but no tree has claimed this allocation.
    Unmounted,
    /// Exactly one tree attachment has been consumed.
    Mounted,
}

/// Common attachment and sticky geometry state for one concrete widget.
///
/// Compose this value into each widget class. The host capability must contain
/// only authority state, without a strong reference back to the owning tree.
/// Setters call `writable` before mutation and `record_change` after committing
/// changed measurement or placement inputs. Hidden widgets retain that marker.
pub struct WidgetState {
    /// Runtime authority shared across widgets without an ownership cycle.
    host: ComPtr<IWidgetHostAbi>,
    /// Constructor-selected posting capability; no self-reference or origin authority.
    receiver: Option<ComPtr<neocogi_com::events::IEventReceiverAbi>>,
    /// Authoritative requested flags; effective behavior-derived flags are never stored here.
    requested_flags: WidgetOptions,
    /// Geometry changes retained until the runtime collects them before layout.
    dirty: bool,
    /// One irreversible attachment claim; value lifetime is enforced by ComBox.
    attachment: Attachment,
}

impl WidgetState {
    /// Creates unmounted state; the first node measurement starts uncached.
    pub fn new(host: ComPtr<IWidgetHostAbi>, options: WidgetOptions) -> Self {
        Self {
            host,
            receiver: None,
            requested_flags: options,
            dirty: false,
            attachment: Attachment::Unmounted,
        }
    }

    /// Selects event delivery while the ordinary Rust value is being constructed.
    /// Consuming self avoids a receiver-installation operation on a published widget.
    pub fn with_events(mut self, receiver: Option<ComPtr<neocogi_com::events::IEventReceiverAbi>>) -> Self {
        self.receiver = receiver;
        self
    }

    /// Posts a captured fact using the dispatch owner's active source identity.
    /// An absent destination drops the supplied event immediately. A present
    /// destination receives the same sole owner; rejection drops that owner and
    /// returns its status. Committed producer state is never rolled back.
    pub fn post(&mut self, event: neocogi_com::ComBox<neocogi_com::events::IEventAbi>) -> Result<(), NcStatus> {
        match &self.receiver {
            Some(receiver) => self.host.post_event(receiver.as_com_ref(), event).map_err(|(status, _event)| status),
            None => Ok(()),
        }
    }

    /// Reads current requested flags without adding behavior-derived input policy.
    /// Widget algorithms use this value to compute effective properties; callers
    /// change individual bits through the in-object operations below.
    pub fn requested_flags(&self) -> Result<WidgetOptions, NcStatus> {
        self.readable()?;
        Ok(self.requested_flags)
    }

    /// Replaces the complete requested flag set after authority and bit validation.
    /// Flags omitted from the replacement are intentionally cleared.
    pub fn replace_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.writable()?;
        flags.validate()?;
        self.apply_flags(flags);
        Ok(())
    }

    /// Sets selected requested bits while preserving every unrelated application edit.
    /// Reads and writes the authoritative value during this exclusive widget call.
    pub fn set_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.writable()?;
        flags.validate()?;
        self.apply_flags(self.requested_flags | flags);
        Ok(())
    }

    /// Clears selected requested bits without capturing transient effective flags.
    /// Validate the mask before combining it: an unknown bit cannot become a
    /// silently accepted no-op merely because it was absent from the stored value.
    pub fn clear_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.writable()?;
        flags.validate()?;
        self.apply_flags(WidgetOptions::from_raw(self.requested_flags.raw() & !flags.raw()));
        Ok(())
    }

    /// Stores an authorized, validated replacement and marks actual changes once.
    /// Public mutations perform all fallible checks first. No-op operations leave
    /// an earlier geometry marker intact, including on hidden widgets.
    fn apply_flags(&mut self, flags: WidgetOptions) {
        if self.requested_flags != flags {
            self.requested_flags = flags;
            self.record_change(GeometryChange::Changed);
        }
    }

    /// Borrows the immutable owner capability for attachment authentication.
    pub fn host(&self) -> ComRef<'_, IWidgetHostAbi> {
        self.host.as_com_ref()
    }

    /// Allows observations during layout and paint while rejecting runtime shutdown.
    pub fn readable(&self) -> Result<(), NcStatus> {
        self.host.check_open()
    }

    /// Authorizes an ordinary mutation before the implementation borrows state.
    pub fn writable(&self) -> Result<(), NcStatus> {
        self.host.check_mutation()
    }

    /// Records an authorized, committed geometry change and conservatively notifies the host.
    /// Successful writable admission guarantees notification succeeds unless a callback closed
    /// the runtime; such a runtime cannot publish any later frame. Local stickiness is retained.
    ///
    /// `NO_CHANGE` preserves an earlier setter's pending marker. Event-only or
    /// visual-only changes need no measurement invalidation. External state must
    /// be synchronized through an authorized setter or a truthful update result.
    pub fn record_change(&mut self, change: GeometryChange) {
        if change == GeometryChange::Changed {
            self.dirty = true;
            let _ = self.host.mark_geometry_changed();
        }
    }

    /// Drains the marker once while the owning runtime already holds phase authority.
    ///
    /// This is the runtime's mutable collection protocol, not an application
    /// observation. The generated COM entry already protects the live value;
    /// owner expiration needs no duplicate terminal flag here.
    pub fn take_change(&mut self) -> Result<GeometryChange, NcStatus> {
        let dirty = self.dirty;
        self.dirty = false;
        Ok(if dirty { GeometryChange::Changed } else { GeometryChange::NoChange })
    }

    /// Commits attachment after owner, capability, and topology checks pass.
    pub fn mount(&mut self) -> Result<(), NcStatus> {
        self.writable()?;
        match self.attachment {
            Attachment::Unmounted => {
                self.attachment = Attachment::Mounted;
                Ok(())
            }
            Attachment::Mounted => Err(UI_E_OWNER),
        }
    }

}
