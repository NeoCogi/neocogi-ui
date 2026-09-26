//! Native acceptance uses the same extension protocol in dynamic assemblies and static monoliths.
use super::*;
use neocogi_demo_contracts::*;

/// Host-local facts prove plugins never need the consumer's message enum.
enum Message {
    /// Complete event from the counter's one physical port.
    Counter(CounterEvent),
}
/// Host-local controller owns its labels and the extension's editing projection.
struct Model {
    /// Captured values prove emission order independently of later producer edits.
    labels: Vec<String>,
    /// Typed extension interface acquired once during construction.
    counter: ComPtr<CounterAbi>,
}
impl events::EventModel for Model {
    type Message = Message;
    /// Handles the independent plugin's concrete public event protocol.
    fn handle(&mut self, message: Message) -> Result<(), NcStatus> {
        match message {
            Message::Counter(CounterEvent::Activated(event)) => {
                self.labels.push(event.label);
                self.counter.set_count(100)?;
            }
            Message::Counter(CounterEvent::Changed(event)) => self.labels.push(format!("count={}", event.value)),
        }
        Ok(())
    }
}

/// Runs real routing, layout, GPU recording and lifecycle through three independently built assemblies.
pub(super) fn smoke(ui: &Ui, size: Size, renderer: ComRef<'_, IRendererAbi>) -> Result<(), NcStatus> {
    let provider = neocogi_com::Runtime::global()
        .activate_plugin_factory::<ExtensionProviderAbi>(neocogi_com::guid_from_qualified_name("neocogi.example.ui-extension"))?;
    let host = ui.widgets().host()?;
    let mut counter = provider.counter(host.as_com_ref())?;
    let probe: ComPtr<IWidgetProbeAbi> = counter.query()?;
    let widget: ComPtr<IWidgetAbi> = counter.query()?;
    // Both derived and inherited projections identify the same COM allocation.
    let derived_identity: ComPtr<neocogi_com::IUnknown> = counter.query()?;
    let base_identity: ComPtr<neocogi_com::IUnknown> = widget.query()?;
    assert_eq!(derived_identity.as_ptr(), base_identity.as_ptr());
    let mut inset = provider.inset(host.as_com_ref())?;
    let parent_probe: ComPtr<IWidgetProbeAbi> = inset.query()?;
    inset.set_child(Mount::widget(widget.as_com_ref()).as_message())?.finish()?;
    let mut another = provider.inset(host.as_com_ref())?;
    assert_eq!(another.set_child(Mount::widget(widget.as_com_ref()).as_message()).err(), Some(UI_E_OWNER));
    assert_eq!(another.children()?.child_count()?, 0);
    let root = Widget::from_interface(inset.clone());
    // Capability discovery happens once while wiring the application, never during UI phases.
    let window = ui.window("Independent extension", Rect { x: 10, y: 10, width: 400, height: 200 }, &root)?;
    let (events_tx, events) = std::sync::mpsc::channel();
    neocogi_com::events::bind_source(counter.as_com_ref(), events_tx.clone(), Message::Counter)?;
    let app = events::Controller::into_local(events::Controller::root(
        Model {
            labels: Vec::new(),
            counter: counter.clone(),
        },
        events,
    ));
    let owner = app.query::<IEventOwnerAbi>()?;

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

    inset.clear()?.finish()?;
    assert_eq!(counter.count(), Err(UI_E_CLOSED));
    assert_eq!(probe.calls()?.closes, 1);
    assert_eq!(inset.children()?.child_count()?, 0);
    ui.update_with_events(size, owner.as_com_ref())?;
    ui.render(size, renderer)?;
    let replacement = provider.counter(host.as_com_ref())?;
    let replacement_widget: ComPtr<IWidgetAbi> = replacement.query()?;
    let replacement_probe: ComPtr<IWidgetProbeAbi> = replacement.query()?;
    inset.set_child(Mount::widget(replacement_widget.as_com_ref()).as_message())?.finish()?;
    ui.update_with_events(size, owner.as_com_ref())?;
    window.close()?;
    assert_eq!(replacement.count(), Err(UI_E_CLOSED));
    assert_eq!(replacement_probe.calls()?.closes, 1);
    assert_eq!(parent_probe.calls()?.closes, 1);
    assert_eq!(probe.calls()?.closes, 1, "parent teardown cannot close an already removed child twice");
    another.close()?;
    println!("Independent extension passed: inherited counter, shared events, one-child layout, caches, resize, mutation and teardown");
    Ok(())
}
