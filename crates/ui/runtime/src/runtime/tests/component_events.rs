//! Production runtime regressions for plugin-style private composite events.

use super::*;
use neocogi_com_events::{EventHandler, EventSession as ComEventSession, EventSource};
use neocogi_ui_core_contracts::{IActivationSinkEvent, create_i_activation_sink_event_sink};

/// Shared test observation deliberately owned outside either COM component.
type Trace = Rc<RefCell<Vec<&'static str>>>;

/// Exact private handler type, with an optional ordinary window edit after delivery.
struct PrivateRecord {
    /// Component name used to prove child/surface ordering at the boundary.
    name: &'static str,
    /// Foreign capability edited after all retained traversal borrows have ended.
    close_target: Rc<RefCell<Option<ComPtr<IWindowAbi>>>>,
}

impl EventHandler<Trace, IActivationSinkEvent> for PrivateRecord {
    /// Records the captured fact and may remove another mounted component.
    fn handle(&mut self, trace: &mut Trace, _event: &IActivationSinkEvent) -> Result<(), NcStatus> {
        trace.borrow_mut().push(self.name);
        let target = self.close_target.borrow_mut().take();
        if let Some(target) = target {
            target.close()?;
        }
        Ok(())
    }
}

/// Application-owned handler running only after all eligible private drivers.
struct AppRecord;

impl EventHandler<Trace, IActivationSinkEvent> for AppRecord {
    /// Marks the public session's position in the deterministic sweep.
    fn handle(&mut self, trace: &mut Trace, _event: &IActivationSinkEvent) -> Result<(), NcStatus> {
        trace.borrow_mut().push("application");
        Ok(())
    }
}

#[com_object(interfaces(IWidget, ILeafLayout, IComponentEventProvider))]
/// Plugin-style composite allocation with its own event source and private session.
/// Its dispatcher is the session's separate COM allocation, never this widget.
struct EventfulComposite {
    /// Real host-bound widget lifetime delegated without storing any runtime root.
    lifetime: ComPtr<IWidgetAbi>,
    /// Producer-owned captured events; the source never stores the private handler.
    _source: EventSource<IActivationSinkEvent>,
    /// Component-owned typed handler/session, entirely inside this binary.
    private_session: ComEventSession<Trace>,
}

impl EventfulComposite {
    /// Connects its native event queue before publishing one committed snapshot.
    fn create(runtime: &ComPtr<UiRuntimeAbi>, trace: Trace, name: &'static str, close_target: Rc<RefCell<Option<ComPtr<IWindowAbi>>>>) -> ComPtr<IWidgetAbi> {
        let lifetime = button(runtime).query::<IWidgetAbi>().unwrap();
        let source = EventSource::new();
        let private_session = ComEventSession::new(trace);
        private_session
            .register(|| {
                let sink = create_i_activation_sink_event_sink(&private_session, PrivateRecord { name, close_target })?;
                source.subscribe(sink.as_com_ref())
            })
            .unwrap();
        source.emit(IActivationSinkEvent { label: name.to_owned() });
        EventfulComposite::from_value(Self {
            lifetime,
            _source: source,
            private_session,
        })
        .query()
        .unwrap()
    }
}

impl ILeafLayout for EventfulComposite {
    /// Returns fixed content geometry without retaining the measurement service.
    fn measure(&mut self, _bounds: LayoutLimits, _metrics: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        Ok(Size { width: 120, height: 30 })
    }
}

impl IComponentEventProvider for EventfulComposite {
    /// Yields the separate, typed session dispatcher as an owned COM reference.
    fn private_dispatcher(&self) -> Result<ComPtr<IEventDispatcherAbi>, NcStatus> {
        Ok(self.private_session.as_dispatcher().add_ref())
    }
}

impl IWidget for EventfulComposite {
    /// Delegates host authentication to a factory-created widget lifetime.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        self.lifetime.host()
    }
    /// Claims the underlying one-way mount only after the runtime validates the driver.
    fn mount(&mut self) -> Result<(), NcStatus> {
        self.lifetime.mount()
    }
    /// Cancels private leases before closing its independently retained lifetime.
    fn close(&mut self) -> Result<(), NcStatus> {
        self.private_session.cancel()?;
        self.lifetime.close()
    }
    /// Uses the same real common routing policy as its factory-created lifetime.
    fn properties(&self) -> Result<WidgetProperties, NcStatus> {
        self.lifetime.properties()
    }
    /// Delegates ordinary option mutation without shadow state.
    fn set_options(&mut self, options: WidgetOptions) -> Result<(), NcStatus> {
        self.lifetime.set_options(options)
    }
    /// All test events were committed before mount; eventless updates are inert.
    fn update(&mut self, _state: WidgetUpdate, _event: WidgetEventAbi, _text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
        Ok(GeometryChange::NoChange)
    }
    /// No programmatic geometry edits exist on this fixture.
    fn take_change(&mut self) -> Result<GeometryChange, NcStatus> {
        Ok(GeometryChange::NoChange)
    }
    /// Paints no fixture geometry; runtime scheduling is the test subject.
    fn paint(&mut self, _scope: PaintScope, _state: WidgetPaint, _painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
        Ok(())
    }
}

/// Private dispatch precedes application dispatch and a removed later mount is skipped.
#[test]
fn mounted_component_events_dispatch_before_application_and_skip_removed_snapshots() {
    let runtime = runtime();
    let ui = crate::bind_builtin_nodes(runtime.clone()).unwrap();
    let trace: Trace = Rc::new(RefCell::new(Vec::new()));
    let close_target = Rc::new(RefCell::new(None));
    let older = EventfulComposite::create(&runtime, trace.clone(), "older", Rc::new(RefCell::new(None)));
    let older_window = ui.window("older", RECT, &neocogi_ui::Widget::from_interface(older)).unwrap();
    let newer = EventfulComposite::create(&runtime, trace.clone(), "newer", close_target.clone());
    let newer_window = ui.window("newer", RECT, &neocogi_ui::Widget::from_interface(newer)).unwrap();
    *close_target.borrow_mut() = Some(older_window.clone());

    let app_source = EventSource::new();
    let app_session = ComEventSession::new(trace.clone());
    app_session
        .register(|| {
            let sink = create_i_activation_sink_event_sink(&app_session, AppRecord)?;
            app_source.subscribe(sink.as_com_ref())
        })
        .unwrap();
    app_source.emit(IActivationSinkEvent { label: "app".to_owned() });

    ui.update_with_events(SIZE, &app_session).unwrap();
    assert_eq!(&*trace.borrow(), &["newer", "application"]);
    assert_eq!(older_window.snapshot(), Err(UI_E_CLOSED));
    newer_window.close().unwrap();
}
