//! Native acceptance uses the same extension protocol in dynamic assemblies and static monoliths.
use super::*;
use neocogi_demo_contracts::*;

/// Host-local facts prove plugins never need the consumer's message enum.
enum Message {
    /// Complete event from the counter's one physical port.
    Counter(neocogi_com::WeakComBox<CounterAbi>, CounterEvent),
}
#[neocogi_com::com_object(neocogi_demo_contracts::CounterEventQueue)]
/// The host implements the independently generated custom-widget queue directly.
struct CounterQueue {
    /// Producer lifetimes remain weak while messages wait in the host inbox.
    sender: std::sync::mpsc::Sender<Message>,
}
impl CounterEventQueue for CounterQueue {
    /// Acquire the custom payload through its checked ABI before the producer returns.
    fn post(&mut self, origin: ComRef<'_, CounterAbi>, event: neocogi_com::NcValueRef<'_, CounterEvent>) -> Result<(), NcStatus> {
        let _ = self.sender.send(Message::Counter(origin.downgrade_box()?, event.to_owned()?));
        Ok(())
    }
}
/// Host-local controller owns its labels and the extension's editing projection.
struct Model {
    /// Captured values prove emission order independently of later producer edits.
    labels: Vec<String>,
    /// Typed extension interface acquired once during construction.
    counter: neocogi_com::WeakComBox<CounterAbi>,
}
impl events::EventModel for Model {
    type Message = Message;
    /// Handles the independent plugin's concrete public event protocol.
    fn handle(&mut self, message: Message) -> Result<(), NcStatus> {
        let Message::Counter(origin, event) = message;
        let Ok(_call) = origin.borrow() else {
            return Ok(());
        };
        if !origin.same_object(&self.counter) {
            return Ok(());
        }
        match event {
            CounterEvent::Activated(event) => {
                self.labels.push(event.label);
                self.counter.set_count(100)?;
            }
            CounterEvent::Changed(event) => self.labels.push(format!("count={}", event.value)),
        }
        Ok(())
    }
}

/// Runs real routing, layout, GPU recording and lifecycle through three independently built assemblies.
pub(super) fn smoke(ui: &Ui, size: Size, renderer: ComRef<'_, IRendererAbi>) -> Result<(), NcStatus> {
    let provider = neocogi_com::Runtime::global()
        .activate_plugin_factory::<ExtensionProviderAbi>(neocogi_com::guid_from_qualified_name("neocogi.example.ui-extension"))?;
    let host = ui.widgets().host()?;
    let (events_tx, events) = std::sync::mpsc::channel();
    let queue = CounterQueue::from_value(CounterQueue { sender: events_tx }).query::<CounterEventQueueAbi>()?;
    let binding = neocogi_demo_contracts::CounterEventBinding::from_queue(Some(queue.as_com_ref()));
    let counter_owner = provider.counter(host.as_com_ref(), neocogi_com::NcValueRef::new(&binding)?)?;
    let mut counter = counter_owner.downgrade();
    let probe: neocogi_com::WeakComBox<IWidgetProbeAbi> = counter.query()?;
    let widget = counter_owner.query::<IWidgetAbi>()?;
    // Both derived and inherited projections identify the same COM allocation.
    let derived_identity: neocogi_com::WeakComBox<neocogi_com::IUnknown> = counter.query()?;
    let base_identity: neocogi_com::WeakComBox<neocogi_com::IUnknown> = widget.downgrade().query()?;
    assert_eq!(derived_identity.as_raw(), base_identity.as_raw());
    let inset_owner = provider.inset(host.as_com_ref())?;
    let mut inset = inset_owner.downgrade();
    let parent_probe: neocogi_com::WeakComBox<IWidgetProbeAbi> = inset.query()?;
    inset.set_child(widget)?;
    // The second parent cannot acquire the same owner: set_child consumes it.
    let root = neocogi_ui::OwnedWidget::from_interface(inset_owner);
    // Capability discovery happens once while wiring the application, never during UI phases.
    let window = ui.window("Independent extension", Rect { x: 10, y: 10, width: 400, height: 200 }, root)?;
    let app = events::Controller::into_local(events::Controller::new(
        Model {
            labels: Vec::new(),
            counter: counter.clone(),
        },
        events,
    ));
    let owner = app.query::<IApplicationDispatcherAbi>()?;

    ui.update_with_events(size, owner.as_com_ref())?;
    ui.render(size, renderer)?;
    let first = probe.calls()?;
    assert!(first.measures > 0 && first.updates > 0 && first.paints > 0);
    assert!(parent_probe.calls()?.placements > 0);
    ui.update_with_events(size, owner.as_com_ref())?;
    assert_eq!(
        probe.calls()?.measures,
        first.measures,
        "NO_CHANGE retains measurement caches across ordinary updates"
    );
    assert_eq!(inset.set_padding(-1), Err(neocogi_com::NC_E_INVALID_ARG));
    ui.render(size, renderer)?; // Rejected edits cannot invalidate a committed frame.

    key(ui, KeyCode::Tab)?;
    key(ui, KeyCode::Space)?;
    ui.update_with_events(size, owner.as_com_ref())?;
    assert_eq!(&app.try_borrow()?.model.labels, &["1".to_owned(), "count=1".to_owned()]);
    assert_eq!(counter.count()?, 100);
    ui.render(size, renderer)?;
    let activated = probe.calls()?;
    assert!(activated.measures > first.measures, "handler edits are measured before update returns");

    counter.set_count(1_000_000)?;
    assert_eq!(ui.render(size, renderer), Err(UI_E_UPDATE_REQUIRED));
    ui.update_with_events(size, owner.as_com_ref())?;
    let edited = probe.calls()?;
    assert!(edited.measures > activated.measures);
    counter.set_count(1_000_000)?;
    ui.render(size, renderer)?; // An identical setter retains the commit.
    inset.set_padding(20)?;
    assert_eq!(ui.render(size, renderer), Err(UI_E_UPDATE_REQUIRED));
    ui.update_with_events(size, owner.as_com_ref())?;
    let padded = probe.calls()?;
    assert!(padded.measures > edited.measures, "container constraints invalidate descendants");
    window.set_rect(Rect { x: 10, y: 10, width: 450, height: 220 })?;
    ui.update_with_events(size, owner.as_com_ref())?;
    assert!(probe.calls()?.measures > padded.measures, "resizing remeasures under the new bounds");
    ui.render(size, renderer)?;

    inset.clear()?;
    assert_eq!(counter.count(), Err(UI_E_CLOSED));
    assert_eq!(probe.calls(), Err(UI_E_CLOSED));
    assert_eq!(inset.children()?.child_count()?, 0);
    ui.update_with_events(size, owner.as_com_ref())?;
    ui.render(size, renderer)?;
    let replacement = provider.counter(host.as_com_ref(), neocogi_com::NcValueRef::new(&CounterEventBinding::Unobserved)?)?;
    let replacement_view = replacement.downgrade();
    let replacement_probe = replacement_view.query::<IWidgetProbeAbi>()?;
    inset.set_child(replacement.query::<IWidgetAbi>()?)?;
    ui.update_with_events(size, owner.as_com_ref())?;
    window.close()?;
    assert_eq!(replacement_view.count(), Err(UI_E_CLOSED));
    assert_eq!(replacement_probe.calls(), Err(UI_E_CLOSED));
    assert_eq!(parent_probe.calls(), Err(UI_E_CLOSED));
    assert_eq!(probe.calls(), Err(UI_E_CLOSED));
    println!("Independent extension passed: inherited counter, shared events, one-child layout, caches, resize, mutation and teardown");
    Ok(())
}
