//! Native acceptance uses the same extension protocol in dynamic assemblies and static monoliths.
use super::*;
use neocogi_ui_samples::abi::*;
use neocogi_ui::events;

/// Host-local controller owns its labels and the extension's editing projection.
struct Model {
    /// Activation markers and captured counter values establish deferred delivery order.
    labels: Vec<String>,
    /// Typed extension interface acquired once during construction.
    counter: neocogi_com::WeakComBox<CounterAbi>,
}
impl events::MessageProcessor for Model {
    type Message = neocogi_ui::events::EventMessage;
    /// Handles the independent plugin's concrete public event protocol.
    fn handle(&mut self, message: Self::Message) -> Result<(), NcStatus> {
        let Some(origin) = message.origin.widget::<CounterAbi>()? else {
            return Ok(());
        };
        if !origin.is_alive() {
            return Ok(());
        }
        if !origin.same_object(&self.counter) {
            return Ok(());
        }
        if message.query::<IActivatedEventAbi>()?.is_some() {
            self.labels.push("activated".to_owned());
            self.counter.set_count(100)?;
        } else if let Some(event) = message.query::<ICounterChangedEventAbi>()? {
            self.labels.push(format!("count={}", event.value()?));
        }
        Ok(())
    }
}

/// Runs real routing, layout, GPU recording and lifecycle through three independently built assemblies.
pub(super) fn smoke(runtime: neocogi_com::Runtime, ui: &Ui, size: Size, renderer: ComRef<'_, IRendererAbi>) -> Result<(), NcStatus> {
    let provider = runtime.acquire_plugin_root::<ExtensionProviderAbi>(neocogi_com::guid_from_qualified_name("neocogi.example.ui-extension"))?;
    let host = ui.widgets().host()?;
    // Exercise rejection through the independent extension and runtime DLLs.
    // B belongs to A, so accepting A under B would form an owning cycle. Every
    // forwarding layer must return A before the caller can safely destroy it.
    let a = provider.inset(host.as_com_ref())?;
    let b = provider.inset(host.as_com_ref())?;
    let mut a_view = a.downgrade();
    let mut b_view = b.downgrade();
    let b = b.query::<IWidgetAbi>().map_err(|(status, _candidate)| status)?;
    a_view.set_child(b).map_err(|(status, _candidate)| status)?;
    let a = a.query::<IWidgetAbi>().map_err(|(status, _candidate)| status)?;
    let original = a.as_raw();
    let (status, a) = b_view.set_child(a).unwrap_err();
    assert_eq!(status, UI_E_OWNER);
    assert_eq!(a.as_raw(), original, "rejection returns exactly the original owner");
    assert_eq!(a_view.children()?.child_count()?, 1);
    assert_eq!(b_view.children()?.child_count()?, 0);
    drop(a);
    assert_eq!(a_view.children().unwrap_err(), neocogi_com::NC_E_DROPPED);
    assert_eq!(b_view.children().unwrap_err(), neocogi_com::NC_E_DROPPED);

    let (queue, events) = neocogi_ui::events::channel();
    let binding = neocogi_ui::events::EventBinding::from_queue(Some(queue.as_com_ref()))?;
    let counter_owner = provider.counter(host.as_com_ref(), neocogi_com::NcValueRef::new(&binding)?)?;
    let mut counter = counter_owner.downgrade();
    let probe: neocogi_com::WeakComBox<IWidgetProbeAbi> = counter.query()?;
    // These construction-only error paths abandon fresh, idle candidates after
    // the provider has returned; ordinary forwarding code must return the owner.
    let widget = counter_owner.query::<IWidgetAbi>().map_err(|(status, _candidate)| status)?;
    // Both derived and inherited projections identify the same COM allocation.
    let derived_identity: neocogi_com::WeakComBox<neocogi_com::IUnknown> = counter.query()?;
    let base_identity: neocogi_com::WeakComBox<neocogi_com::IUnknown> = widget.downgrade().query()?;
    assert_eq!(derived_identity.as_raw(), base_identity.as_raw());
    let inset_owner = provider.inset(host.as_com_ref())?;
    let mut inset = inset_owner.downgrade();
    let parent_probe: neocogi_com::WeakComBox<IWidgetProbeAbi> = inset.query()?;
    inset.set_child(widget).map_err(|(status, _candidate)| status)?;
    // The second parent cannot acquire the same owner: set_child consumes it.
    let root = neocogi_ui::OwnedWidget::from_interface(inset_owner);
    // Capability discovery happens once while wiring the application, never during UI phases.
    let window = ui
        .window("Independent extension", Rect { x: 10, y: 10, width: 400, height: 200 }, root)
        .map_err(|(status, _candidate)| status)?;
    let app = events::Dispatcher::into_local(events::Dispatcher::new(
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
    assert_eq!(&app.try_borrow()?.processor.labels, &["activated".to_owned(), "count=1".to_owned()]);
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
    assert_eq!(counter.count(), Err(neocogi_com::NC_E_DROPPED));
    assert_eq!(probe.calls(), Err(neocogi_com::NC_E_DROPPED));
    assert_eq!(inset.children()?.child_count()?, 0);
    ui.update_with_events(size, owner.as_com_ref())?;
    ui.render(size, renderer)?;
    let replacement = provider.counter(host.as_com_ref(), neocogi_com::NcValueRef::new(&neocogi_ui::events::EventBinding::Unobserved)?)?;
    let replacement_view = replacement.downgrade();
    let replacement_probe = replacement_view.query::<IWidgetProbeAbi>()?;
    let replacement = replacement.query::<IWidgetAbi>().map_err(|(status, _candidate)| status)?;
    inset.set_child(replacement).map_err(|(status, _candidate)| status)?;
    ui.update_with_events(size, owner.as_com_ref())?;
    window.close()?;
    assert_eq!(replacement_view.count(), Err(neocogi_com::NC_E_DROPPED));
    assert_eq!(replacement_probe.calls(), Err(neocogi_com::NC_E_DROPPED));
    assert_eq!(parent_probe.calls(), Err(neocogi_com::NC_E_DROPPED));
    assert_eq!(probe.calls(), Err(neocogi_com::NC_E_DROPPED));
    println!("Independent extension passed: inherited counter, shared events, one-child layout, caches, resize, rejected ownership, mutation and teardown");
    Ok(())
}
