//! Native callback fixtures for the old renderer ordering tests.
//! Production custom rendering uses owned IRenderItem objects and has no closure registry.
use std::{collections::HashMap, error::Error, fmt, marker::PhantomData};
use crate::render::{RendererBackend, RenderItemArgs};
use crate::render::RendererId;
/// Private heterogeneous storage shape for one backend-specialized callback closure.
///
/// Context registration accepts this same higher-ranked `FnMut` contract directly. Keeping only
/// the closure shape avoids exposing a named trait that applications could implement but no public
/// registration API could consume. Callbacks remain on the owning Context thread and therefore
/// deliberately have no `Send` or `Sync` bound.
type CustomRenderCallback<B> = dyn for<'frame> FnMut(&mut <B as RendererBackend>::Frame<'frame>, RenderItemArgs) + 'static;

/// Backend-neutral key retained by UI nodes and display-list operations.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct CustomRenderKey {
    /// Identity of the renderer that owns this callback registry.
    renderer: RendererId,
    /// Monotonically allocated callback slot within `renderer`.
    slot: u64,
}

/// Typed public handle for a callback registered on a particular backend type.
pub struct CustomRenderHandle<B: RendererBackend> {
    /// Fixture registry identity checked before callback execution.
    pub(crate) key: CustomRenderKey,
    /// Preserves the fixture backend type without retaining or allocating backend state.
    _backend: PhantomData<fn(&mut B)>,
}

impl<B: RendererBackend> Copy for CustomRenderHandle<B> {}

impl<B: RendererBackend> Clone for CustomRenderHandle<B> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<B: RendererBackend> fmt::Debug for CustomRenderHandle<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CustomRenderHandle").field("key", &self.key).finish()
    }
}

/// Custom-render registry mutation failure.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CustomRenderRegistryError {
    /// This registry's monotonically increasing callback slot counter was exhausted.
    SlotExhausted,
    /// The supplied handle is foreign to this registry or has already been removed.
    UnknownRenderer,
}

impl fmt::Display for CustomRenderRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SlotExhausted => f.write_str("custom-render registry slot counter exhausted"),
            Self::UnknownRenderer => f.write_str("unknown custom renderer"),
        }
    }
}

impl Error for CustomRenderRegistryError {}

/// Renderer-owned callbacks specialized for one concrete backend.
pub(crate) struct CustomRenderRegistry<B: RendererBackend> {
    /// Identity shared with the Renderer and every other renderer-owned capability.
    renderer: RendererId,
    /// Last callback slot allocated by this registry.
    next_slot: u64,
    /// Live callbacks keyed by renderer provenance and local slot.
    callbacks: HashMap<CustomRenderKey, Box<CustomRenderCallback<B>>>,
}

impl<B: RendererBackend> CustomRenderRegistry<B> {
    /// Creates the sole custom-render registry owned by `renderer`.
    pub(crate) fn new(renderer: RendererId) -> Self {
        // Renderer constructs this registry exactly once and supplies the same identity copied into
        // texture handles. No second namespace allocator or lazy initialization state is required.
        Self {
            renderer,
            next_slot: 0,
            callbacks: HashMap::new(),
        }
    }

    /// Registers one concrete backend-specialized callback under a fresh local slot.
    pub(crate) fn register<F>(&mut self, callback: F) -> Result<CustomRenderHandle<B>, CustomRenderRegistryError>
    where
        F: for<'frame> FnMut(&mut B::Frame<'frame>, RenderItemArgs) + 'static,
    {
        // next_slot = current_slot + 1.
        let slot = self.next_slot.checked_add(1).ok_or(CustomRenderRegistryError::SlotExhausted)?;
        self.next_slot = slot;
        let key = CustomRenderKey { renderer: self.renderer, slot };
        let previous = self.callbacks.insert(key, Box::new(callback));
        debug_assert!(previous.is_none(), "fresh custom-render key was already occupied");
        Ok(CustomRenderHandle { key, _backend: PhantomData })
    }

    /// Removes a live callback only when its handle belongs to this renderer.
    pub(crate) fn remove(&mut self, handle: CustomRenderHandle<B>) -> Result<(), CustomRenderRegistryError> {
        // Check provenance before touching the map so a same-slot foreign handle can never remove
        // this registry's callback.
        if handle.key.renderer != self.renderer || self.callbacks.remove(&handle.key).is_none() {
            return Err(CustomRenderRegistryError::UnknownRenderer);
        }
        Ok(())
    }

    /// Reports whether `key` identifies a live callback owned by this renderer.
    pub(crate) fn contains(&self, key: CustomRenderKey) -> bool {
        key.renderer == self.renderer && self.callbacks.contains_key(&key)
    }

    /// Borrows a live callback only when its key belongs to this renderer.
    pub(crate) fn get_mut(&mut self, key: CustomRenderKey) -> Option<&mut Box<CustomRenderCallback<B>>> {
        if key.renderer != self.renderer {
            return None;
        }
        self.callbacks.get_mut(&key)
    }
}
