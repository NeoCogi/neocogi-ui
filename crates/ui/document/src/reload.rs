//! Single-slot publication keeps failed candidates from replacing a live UI.
use crate::{Component, Document, Loader, Resources};
use neocogi_com::{ComPtr, NcStatus};
use neocogi_ui::*;

/// Attempt identity used to discard messages queued by retired or rejected components.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Generation(u64);

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
    /// Every attempt consumes an identity, including rejected binding attempts.
    next: u64,
    /// Trusted custom widget factories shared across candidate attempts.
    custom: crate::CustomWidgetRegistry,
}
impl ReloadSlot {
    /// Creates an empty unmounted slot; each candidate is bound before publication.
    /// Insert take_widget()? into the application's normal layout and drain the
    /// matching receiver after updates, including across component reloads.
    pub fn new(ui: Ui, parent: ComPtr<IWindowAbi>) -> Result<Self, NcStatus> {
        Self::new_with_custom(ui, parent, crate::CustomWidgetRegistry::new())
    }
    /// Creates a reload slot with application-installed custom COM widget factories.
    pub fn new_with_custom(ui: Ui, parent: ComPtr<IWindowAbi>, custom: crate::CustomWidgetRegistry) -> Result<Self, NcStatus> {
        let slot = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch)?;
        Ok(Self {
            ui,
            parent,
            slot: slot.handle(),
            slot_owner: Some(slot),
            active: None,
            next: 0,
            custom,
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
    /// Tests an application message's generation before applying its effects.
    pub fn accepts(&self, generation: Generation) -> bool {
        self.active.as_ref().is_some_and(|(active, _)| *active == generation)
    }
    /// Builds and binds a candidate, then publishes it only if every preparation step succeeds.
    /// Receivers are prepared first; the second callback resolves fields on the complete candidate.
    pub fn reload(
        &mut self,
        document: &Document,
        resources: Resources,
        events: impl FnOnce(Generation) -> Result<crate::EventBindings, NcStatus>,
        bind: impl FnOnce(&Component, Generation) -> Result<(), NcStatus>,
    ) -> Result<Reloaded, String> {
        self.next = self.next.checked_add(1).ok_or("Reload generation limit reached")?;
        let generation = Generation(self.next);
        let loader = Loader::new_with_custom(self.ui.clone(), Some(self.parent.clone()), resources, self.custom.clone()).map_err(|error| error.to_string())?;
        let receivers = events(generation).map_err(|error| error.to_string())?;
        let mut candidate = loader.compile_with_events(document, receivers)?;
        bind(&candidate, generation).map_err(|error| error.to_string())?;
        let edge = LinearEdge::new(LayoutTrack::Flex { weight: 1.0 });
        match self.active {
            None => self.slot.insert(0, &mut candidate, edge),
            Some(_) => self.slot.replace(0, &mut candidate, edge),
        }.map_err(|error| error.to_string())?;
        self.active = Some((generation, candidate));
        Ok(Reloaded { generation })
    }
}
