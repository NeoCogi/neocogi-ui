//! Single-slot publication keeps failed candidates from replacing a live UI.
use crate::{Component, Document, Loader, Resources};
use neocogi_com::{ComPtr, NcStatus};
use neocogi_ui::*;

/// Application-owned slot-ID allocator for one message-dispatch domain.
/// Every slot whose generations can meet in a queue must use this same allocator,
/// including slots in different UI runtimes and replacements for destroyed slots.
/// Keep it across slot recreation; never reset/replace it while older messages can
/// enter that domain. Independent allocators define independent, incomparable domains.
///
/// Exclusive construction needs only &mut self. Concurrent callers must serialize
/// access to this one allocator rather than creating a counter per thread/library.
/// This Rust application helper is not a COM wire value or a process-global registry.
/// It intentionally implements neither Clone nor Copy: allocation history is owned.
#[derive(Debug, Default)]
pub struct ReloadIds {
    /// Last reserved slot ID; zero precedes the first reservation and is never issued.
    /// IDs remain consumed after slot destruction or a later construction failure.
    last_slot: u64,
}
impl ReloadIds {
    /// Starts a new dispatch domain. Do not create a fresh allocator per slot.
    pub const fn new() -> Self {
        Self { last_slot: 0 }
    }

    /// Reserves a non-recycled slot ID before invoking a concrete container factory.
    /// Attempt zero is private counter state; only reload() issues application tokens.
    fn reserve_slot(&mut self) -> Result<Generation, NcStatus> {
        self.last_slot = self.last_slot.checked_add(1).ok_or(neocogi_com::NC_E_OUT_OF_RANGE)?;
        Ok(Generation { slot_id: self.last_slot, attempt: 0 })
    }
}

/// Identifies a construction attempt by (slot ID, slot-local attempt number).
/// Equality is meaningful only within the shared ReloadIds dispatch domain.
/// Tokens are plain copied values: no allocation, widget ownership or reference
/// counting. Keep the complete pair in messages; an attempt number alone is not
/// unique across slots. Generated events additionally check weak widget origins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Generation {
    /// Stable reservation from the application's non-recycled slot-ID sequence.
    slot_id: u64,
    /// Nonzero reload attempt; rejected attempts are consumed just like successful ones.
    attempt: u64,
}
impl Generation {
    /// Advances the private per-slot counter before receivers can capture its token.
    /// On exhaustion leave the counter unchanged; never wrap into an earlier attempt.
    fn reserve_next(&mut self) -> Result<Self, NcStatus> {
        self.attempt = self.attempt.checked_add(1).ok_or(neocogi_com::NC_E_OUT_OF_RANGE)?;
        Ok(*self)
    }
}

/// Identity of a fully committed replacement; dropping old ownership has no status.
pub struct Reloaded {
    /// Identity to associate with application messages from the new component.
    pub generation: Generation,
}

/// Owns one ordinary container slot and the component currently mounted inside it.
pub struct ReloadSlot {
    /// Existing runtime owns rendering and input; reload owns no extra event loop.
    ui: Ui,
    /// Stable host window required by controls with child popups.
    parent: ComPtr<IWindowAbi>,
    /// One child is replaced atomically through the existing container operation.
    slot: LinearWidget,
    /// Until mounted, this is the slot's only owner; taking it is explicit.
    slot_owner: Option<OwnedWidget<LinearAbi>>,
    /// Explicit empty/loaded state, with no independent loaded flag.
    active: Option<(Generation, Component)>,
    /// Slot identity and last reserved attempt, including rejected attempts.
    /// Starts at private attempt zero; active changes only after publication succeeds.
    last_attempt: Generation,
    /// Trusted custom widget factories shared across candidate attempts.
    custom: crate::CustomWidgetRegistry,
    /// Host-selected Grid ceilings remain the same across every reload attempt.
    grid_limits: GridLimits,
}
impl ReloadSlot {
    /// Creates an empty unmounted slot; each candidate is bound before publication.
    /// Insert take_widget()? into the application's normal layout and drain the
    /// matching receiver after updates, including across component reloads. IDs must
    /// come from the application's shared dispatch-domain allocator, not a fresh local.
    /// Exhausted IDs return NC_E_OUT_OF_RANGE before constructing the slot widget.
    pub fn new(ids: &mut ReloadIds, ui: Ui, parent: ComPtr<IWindowAbi>, grid_limits: GridLimits) -> Result<Self, NcStatus> {
        Self::new_with_custom(ids, ui, parent, crate::CustomWidgetRegistry::new(), grid_limits)
    }
    /// Creates a reload slot with application-installed custom COM widget factories.
    /// Uses the same shared-ID contract as new(); a reserved ID is never returned
    /// to the allocator, including when subsequent container construction fails.
    pub fn new_with_custom(
        ids: &mut ReloadIds,
        ui: Ui,
        parent: ComPtr<IWindowAbi>,
        custom: crate::CustomWidgetRegistry,
        grid_limits: GridLimits,
    ) -> Result<Self, NcStatus> {
        grid_limits.validate()?;
        let last_attempt = ids.reserve_slot()?;
        let slot = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch)?;
        Ok(Self {
            ui,
            parent,
            slot: slot.handle(),
            slot_owner: Some(slot),
            active: None,
            last_attempt,
            custom,
            grid_limits,
        })
    }
    /// Transfers the slot into the application's owning layout exactly once.
    pub fn take_widget(&mut self) -> Result<OwnedWidget<LinearAbi>, NcStatus> {
        self.slot_owner.take().ok_or(neocogi_com::NC_E_CLOSED)
    }
    /// Returns the stable slot root, whose identity does not change during reload.
    pub fn widget(&self) -> &LinearWidget {
        &self.slot
    }
    /// Borrows the current component for typed widget lookup or authored saving.
    pub fn component(&self) -> Option<&Component> {
        self.active.as_ref().map(|(_, component)| component)
    }
    /// Borrows the current component for explicit authored-metadata publication.
    /// This changes no slot/attempt identity and cannot replace the mounted subtree.
    pub fn component_mut(&mut self) -> Option<&mut Component> {
        self.active.as_mut().map(|(_, component)| component)
    }
    /// Checks both slot identity and published attempt before applying message effects.
    /// The sender and this slot must share one ReloadIds domain. Rejected attempts,
    /// previous forms and other slots in that domain cannot match the active pair.
    /// Call DocumentEvent::event_for to check the current widget's weak origin as well.
    pub fn accepts(&self, generation: Generation) -> bool {
        self.active.as_ref().is_some_and(|(active, _)| *active == generation)
    }
    /// Builds and binds a candidate, then publishes it only if every preparation step succeeds.
    /// Reserve the attempt before preparing receivers; the second callback resolves
    /// fields on the complete candidate. A failed attempt never replaces active state
    /// or returns its number for reuse. Exhaustion fails before either callback runs.
    pub fn reload(
        &mut self,
        document: &Document,
        resources: Resources,
        events: impl FnOnce(Generation) -> Result<crate::EventBindings, NcStatus>,
        bind: impl FnOnce(&Component, Generation) -> Result<(), NcStatus>,
    ) -> Result<Reloaded, String> {
        let generation = self.last_attempt.reserve_next().map_err(|error| error.to_string())?;
        let loader = Loader::new_with_custom(self.ui.clone(), Some(self.parent.clone()), resources, self.custom.clone(), self.grid_limits)
            .map_err(|error| error.to_string())?;
        let receivers = events(generation).map_err(|error| error.to_string())?;
        let mut candidate = loader.compile_with_events(document, receivers)?;
        bind(&candidate, generation).map_err(|error| error.to_string())?;
        let edge = LinearEdge::new(LayoutTrack::Flex { weight: 1.0 });
        match self.active {
            None => self.slot.insert(0, &mut candidate, edge),
            Some(_) => self.slot.replace(0, &mut candidate, edge),
        }
        .map_err(|(status, _candidate)| status.to_string())?;
        self.active = Some((generation, candidate));
        Ok(Reloaded { generation })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Equal attempt numbers from distinct slots remain different, copied queue values.
    #[test]
    fn copied_tokens_keep_slot_identity_across_threads_and_counter_progress() {
        let mut ids = ReloadIds::new();
        let mut first = ids.reserve_slot().unwrap();
        let mut second = ids.reserve_slot().unwrap();
        let first_attempt = first.reserve_next().unwrap();
        let second_attempt = second.reserve_next().unwrap();
        assert_eq!(first_attempt.attempt, second_attempt.attempt);
        assert_ne!(first_attempt, second_attempt);
        assert_eq!(core::mem::size_of::<Generation>(), 16);

        // Worker results can retain plain tokens without retaining the slot, its
        // counter or any COM allocation. UI mutation itself stays on its owner thread.
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || send.send(first_attempt).unwrap()).join().unwrap();
        assert_eq!(receive.recv().unwrap(), first_attempt);
        let next = first.reserve_next().unwrap();
        assert_eq!(next.slot_id, first_attempt.slot_id);
        assert_eq!(next.attempt, 2);
        assert_ne!(next, first_attempt);
        assert_ne!(next, second_attempt);
    }

    /// The final slot ID is usable; all later reservations fail without recycling zero.
    #[test]
    fn slot_id_exhaustion_is_permanent_and_preserves_existing_attempts() {
        let mut ids = ReloadIds { last_slot: u64::MAX - 1 };
        let mut last = ids.reserve_slot().unwrap();
        assert_eq!(last.slot_id, u64::MAX);
        for _ in 0..2 {
            assert_eq!(ids.reserve_slot(), Err(neocogi_com::NC_E_OUT_OF_RANGE));
            assert_eq!(ids.last_slot, u64::MAX);
        }
        // Exhausting slot allocation does not prevent a previously created slot reloading.
        assert_eq!(last.reserve_next().unwrap().attempt, 1);
    }

    /// The final attempt is usable; exhaustion cannot make an old generation current again.
    #[test]
    fn attempt_exhaustion_preserves_the_last_reserved_pair() {
        let mut counter = Generation { slot_id: 7, attempt: u64::MAX - 1 };
        let last = counter.reserve_next().unwrap();
        assert_eq!(last, Generation { slot_id: 7, attempt: u64::MAX });
        for _ in 0..2 {
            assert_eq!(counter.reserve_next(), Err(neocogi_com::NC_E_OUT_OF_RANGE));
            assert_eq!(counter, last);
        }
    }
}
