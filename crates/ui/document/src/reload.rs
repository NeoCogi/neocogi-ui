//! Single-slot publication keeps failed candidates from replacing a live UI.
use crate::{Component, Document, Loader, Resources};
use neocogi_com::{ComPtr, NcStatus};
use std::sync::mpsc::SyncSender;
use neocogi_ui::*;

/// Attempt identity used to discard messages queued by retired or rejected components.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Generation(u64);

/// Successful publication remains successful even if old-subtree teardown reports an error.
pub struct Reloaded {
    /// Identity to associate with application messages from the new component.
    pub generation: Generation,
    /// Original subtree cleanup status; the replacement is already committed.
    pub teardown: NcStatus,
}

/// Owns one ordinary container slot and the component currently mounted inside it.
pub struct ReloadSlot {
    /// Existing runtime owns rendering and input; reload owns no extra event loop.
    ui: Ui,
    /// Stable host window required by controls with child popups.
    parent: ComPtr<IWindowAbi>,
    /// One child is replaced atomically through the existing container operation.
    slot: LinearWidget,
    /// Explicit empty/loaded state, with no independent loaded flag.
    active: Option<(Generation, Component)>,
    /// Every attempt consumes an identity, including rejected binding attempts.
    next: u64,
    /// Bounded application-owned channel shared with each newly loaded form.
    diagnostics: SyncSender<crate::RouteDiagnostic>,
}
impl ReloadSlot {
    /// Creates an empty unmounted slot with a bounded route-diagnostic sender.
    /// Insert widget() into the application's normal layout and drain the
    /// matching receiver after updates, including across component reloads.
    pub fn new(ui: Ui, parent: ComPtr<IWindowAbi>, diagnostics: SyncSender<crate::RouteDiagnostic>) -> Result<Self, NcStatus> {
        let slot = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch)?;
        Ok(Self { ui, parent, slot, active: None, next: 0, diagnostics })
    }
    /// Returns the stable slot root, whose identity does not change during reload.
    pub fn widget(&self) -> &LinearWidget {
        &self.slot
    }
    /// Borrows the current component for typed inputs, outputs, or authored saving.
    pub fn component(&self) -> Option<&Component> {
        self.active.as_ref().map(|(_, component)| component)
    }
    /// Tests an application message's generation before applying its effects.
    pub fn accepts(&self, generation: Generation) -> bool {
        self.active.as_ref().is_some_and(|(active, _)| *active == generation)
    }
    /// Builds and binds a candidate, then publishes it only if every preparation step succeeds.
    /// The binding callback receives only the opaque component and its attempt identity.
    pub fn reload(
        &mut self,
        document: &Document,
        resources: Resources,
        bind: impl FnOnce(&Component, Generation) -> Result<(), NcStatus>,
    ) -> Result<Reloaded, String> {
        self.next = self.next.checked_add(1).ok_or("Reload generation limit reached")?;
        let generation = Generation(self.next);
        let loader = Loader::new(self.ui.clone(), Some(self.parent.clone()), resources, self.diagnostics.clone())
            .map_err(|error| error.to_string())?;
        let candidate = loader.prepare(document)?;
        bind(&candidate, generation).map_err(|error| error.to_string())?;
        let edge = LinearEdge::new(LayoutTrack::Flex { weight: 1.0 });
        let committed = match self.active {
            None => self.slot.insert(0, &candidate, edge),
            Some(_) => self.slot.replace(0, &candidate, edge),
        }
        .map_err(|error| error.to_string())?;
        // The topology operation has committed. Store the matching component even
        // when closing the old subtree reported a post-commit cleanup diagnostic.
        self.active = Some((generation, candidate));
        Ok(Reloaded { generation, teardown: committed.teardown })
    }
}
