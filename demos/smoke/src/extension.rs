//! Native acceptance for the independently built extension. This host never links its implementation.
use super::*;
use neocogi_demo_contracts::*;

/// Concrete application callback retains the real editing capability and the captured event history.
struct Advance {
    /// The producer owns no reference to this handler, so this direction creates no cycle.
    counter: ComPtr<CounterAbi>,
}
impl EventHandler<Vec<String>, events::Activation> for Advance {
    /// A callback edit must participate in the same update's layout commit without reentering the producer.
    fn handle(&mut self, labels: &mut Vec<String>, event: &events::Activation) -> Result<(), NcStatus> {
        labels.push(event.label.clone());
        self.counter.set_count(100)
    }
}

/// Application handler for the independently generated counter-value event.
struct RecordCount;
impl EventHandler<Vec<String>, ICounterChangedSinkEvent> for RecordCount {
    /// Records the committed value, not the later programmatic callback edit.
    fn handle(&mut self, history: &mut Vec<String>, event: &ICounterChangedSinkEvent) -> Result<(), NcStatus> {
        history.push(format!("count={}", event.value));
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
    inset.set_child(widget.as_com_ref())?.finish()?;
    let mut another = provider.inset(host.as_com_ref())?;
    assert_eq!(another.set_child(widget.as_com_ref()).err(), Some(UI_E_OWNER));
    assert_eq!(another.children()?.child_count()?, 0);
    let root = Widget::from_interface(inset.clone());
    // Capability discovery happens once while wiring the application, never during UI phases.
    let activation: ComPtr<IActivationEventsAbi> = counter.query()?;
    let window = ui.window("Independent extension", Rect { x: 10, y: 10, width: 400, height: 200 }, &root)?;
    let events = EventSession::new(Vec::new());
    events.subscribe(activation.activated(), Advance { counter: counter.clone() })?;
    events.subscribe(counter.changed(), RecordCount)?;

    ui.update_with_events(size, &events)?;
    ui.render(size, renderer)?;
    let first = probe.calls()?;
    assert!(first.measures > 0 && first.updates > 0 && first.paints > 0);
    assert!(parent_probe.calls()?.placements > 0);
    ui.update_with_events(size, &events)?;
    assert_eq!(
        probe.calls()?.measures,
        first.measures,
        "NO_CHANGE retains measurement caches across ordinary updates"
    );
    assert_eq!(inset.set_padding(-1), Err(neocogi_com::NC_E_INVALID_ARG));
    ui.render(size, renderer)?; // Rejected edits cannot invalidate a committed frame.

    key(ui, KeyCode::Tab)?;
    key(ui, KeyCode::Space)?;
    ui.update_with_events(size, &events)?;
    assert_eq!(&*events.state()?, &["1".to_owned(), "count=1".to_owned()]);
    assert_eq!(counter.count()?, 100);
    ui.render(size, renderer)?;
    let activated = probe.calls()?;
    assert!(activated.measures > first.measures, "handler edits are measured before update returns");

    counter.set_count(1_000_000)?;
    assert_eq!(ui.render(size, renderer), Err(UI_E_UPDATE_REQUIRED));
    ui.update_with_events(size, &events)?;
    let edited = probe.calls()?;
    assert!(edited.measures > activated.measures);
    counter.set_count(1_000_000)?;
    ui.render(size, renderer)?; // An identical setter retains the commit.
    inset.set_padding(20)?;
    assert_eq!(ui.render(size, renderer), Err(UI_E_UPDATE_REQUIRED));
    ui.update_with_events(size, &events)?;
    let padded = probe.calls()?;
    assert!(padded.measures > edited.measures, "container constraints invalidate descendants");
    window.set_rect(Rect { x: 10, y: 10, width: 450, height: 220 })?;
    ui.update_with_events(size, &events)?;
    assert!(probe.calls()?.measures > padded.measures, "resizing remeasures under the new bounds");
    ui.render(size, renderer)?;

    inset.clear()?.finish()?;
    assert_eq!(counter.count(), Err(UI_E_CLOSED));
    assert_eq!(probe.calls()?.closes, 1);
    assert_eq!(inset.children()?.child_count()?, 0);
    ui.update_with_events(size, &events)?;
    ui.render(size, renderer)?;
    let replacement = provider.counter(host.as_com_ref())?;
    let replacement_widget: ComPtr<IWidgetAbi> = replacement.query()?;
    let replacement_probe: ComPtr<IWidgetProbeAbi> = replacement.query()?;
    inset.set_child(replacement_widget.as_com_ref())?.finish()?;
    ui.update_with_events(size, &events)?;
    window.close()?;
    assert_eq!(replacement.count(), Err(UI_E_CLOSED));
    assert_eq!(replacement_probe.calls()?.closes, 1);
    assert_eq!(parent_probe.calls()?.closes, 1);
    assert_eq!(probe.calls()?.closes, 1, "parent teardown cannot close an already removed child twice");
    another.close()?;
    println!("Independent extension passed: inherited counter, shared events, one-child layout, caches, resize, mutation and teardown");
    Ok(())
}
