//! Unit-test driver for native algorithms; no application-state parameter remains in production Context.
use crate::{Context, Dimensioni, Ui, WidgetEventPortHandle, render::RendererBackend};
use super::events::WidgetEventDispatcher;
use neocogi_ui::events::test_support::SubscribeError;
use std::ops::{Deref, DerefMut};

/// Tests can supply borrowed state while exercising semantic algorithms before COM serialization.
pub(crate) struct TestContext<B: RendererBackend, State: 'static = ()> {
    /// Actual retained engine and renderer under test.
    context: Context<B>,
    /// Test-only callback bindings; production uses application-owned COM sessions instead.
    events: WidgetEventDispatcher<State>,
}
impl<B: RendererBackend, State: 'static> TestContext<B, State> {
    /// Builds the same engine with an initially disconnected test callback driver.
    pub(crate) fn new(backend: B) -> Self
    where
        B: super::TestAtlas,
    {
        Self {
            context: Context::new(backend),
            events: WidgetEventDispatcher::new(),
        }
    }
    /// Uses explicit dimensions for the existing terse frame fixtures.
    pub(crate) fn new_test_state(backend: B, dimensions: Dimensioni) -> Self
    where
        B: super::TestAtlas,
    {
        Self {
            context: Context::new_test(backend, dimensions),
            events: WidgetEventDispatcher::new(),
        }
    }
    /// Runs legacy semantic expectations against the shared queue without exposing a production callback API.
    pub(crate) fn update_ui_state(&mut self, dimensions: Dimensioni, state: &mut State) {
        let events = &mut self.events;
        self.context
            .window_manager
            .update_with(dimensions, state, |manager, state| events.dispatch_with_context(state, &mut Ui::new(manager)));
    }
    /// Test callbacks keep their actual captures; production handler ownership is tested through EventSession.
    pub(crate) fn subscribe<E: 'static>(
        &mut self,
        port: WidgetEventPortHandle<E>,
        handler: impl FnMut(&mut State, &mut Ui<'_>, &E) + 'static,
    ) -> Result<(), SubscribeError> {
        self.events.subscribe(port, handler)
    }
}
impl<B: RendererBackend, State: 'static> Deref for TestContext<B, State> {
    type Target = Context<B>;
    /// Observations target the actual engine rather than a second test model.
    fn deref(&self) -> &Self::Target {
        &self.context
    }
}
impl<B: RendererBackend, State: 'static> DerefMut for TestContext<B, State> {
    /// Frame and input helpers retain the production engine's normal borrowing rules.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.context
    }
}
