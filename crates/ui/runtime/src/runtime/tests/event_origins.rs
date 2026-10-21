//! Real host posting supplies sender identity without a producer retaining itself.
use super::*;
use neocogi_ui::{DispatchingLeafContent, LeafContent, LeafDispatchContext, Painter};
use neocogi_com::events::{channel, EventMessage, IEventReceiverAbi};

/// Checks idle authority over the real COM host and preserves the rejected allocation.
fn assert_idle(host: ComRef<'_, IWidgetHostAbi>, queue: ComRef<'_, IEventReceiverAbi>) {
    let event = neocogi_ui::events::activated().unwrap();
    let raw = event.as_raw();
    let (status, returned) = host.post_event(queue, event).unwrap_err();
    assert_eq!(status, NC_E_INVALID_ARG);
    assert_eq!(returned.as_raw(), raw);
}

/// A leaf owns only its host and output receiver; the scheduler supplies its identity.
struct Producer {
    /// Posting authority cannot be queried for origin-stack mutation.
    host: ComPtr<IWidgetHostAbi>,
    /// Output uses the same receiver as ordinary controls and other runtimes.
    queue: ComPtr<IEventReceiverAbi>,
    /// One private pending fact, consumed even if the handler subsequently fails.
    pending: bool,
    /// Exercises scope restoration after a committed post and failed dispatch.
    fail: bool,
}
impl LeafContent for Producer {
    fn measure(&self, _: LayoutLimits, _: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        Ok(Size { width: 40, height: 24 })
    }
    fn paint(&mut self, _: WidgetPaint, _: &mut Painter<'_>) -> Result<(), NcStatus> {
        Ok(())
    }
}
impl DispatchingLeafContent for Producer {
    fn dispatch_next(&mut self, _: &mut LeafDispatchContext<'_>) -> Result<bool, NcStatus> {
        if !std::mem::replace(&mut self.pending, false) {
            return Ok(false);
        }
        self.host
            .post_event(self.queue.as_com_ref(), neocogi_ui::events::activated()?)
            .map_err(|(status, _event)| status)?;
        if self.fail { Err(NC_E_INVALID_ARG) } else { Ok(true) }
    }
}

#[test]
fn dispatch_scopes_are_per_runtime_and_restore_after_success_and_failure() {
    let (queue, inbox) = channel();
    for fail in [false, true] {
        let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
        let other = runtime();
        let host = ui.widgets().host().unwrap();
        let other_host = other.construction_host().unwrap().host().unwrap();
        let leaf = ui
            .widgets()
            .dispatching_leaf(
                Producer {
                    host: host.clone(),
                    queue: queue.clone(),
                    pending: true,
                    fail,
                },
                WidgetOptions::NONE,
            )
            .unwrap();
        let observer = leaf.node_interface().unwrap();
        let window = ui.window("scoped output", RECT, leaf).unwrap();
        assert_idle(host.as_com_ref(), queue.as_com_ref());
        assert_idle(other_host.as_com_ref(), queue.as_com_ref());
        assert_eq!(ui.update(SIZE), if fail { Err(NC_E_INVALID_ARG) } else { Ok(()) });
        let message = inbox.try_recv().unwrap();
        assert!(message.origin.widget::<IWidgetAbi>().unwrap().unwrap().same_object(&observer));
        assert_idle(host.as_com_ref(), queue.as_com_ref());
        assert_idle(other_host.as_com_ref(), queue.as_com_ref());
        ui.update(SIZE).unwrap();
        assert!(inbox.try_recv().is_err());
        window.close().unwrap();
        assert!(!message.origin.widget::<IWidgetAbi>().unwrap().unwrap().is_alive());
        assert!(message.query::<IActivatedEventAbi>().unwrap().is_some());
    }
}

/// Loaded form and Rust form handlers both receive the same public composite origin.
#[test]
fn text_area_captures_public_identity_and_order_from_its_private_editor() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let (queue, inbox) = channel();
    let area = ui
        .widgets()
        .text_area_with_queue("", TextWrapping::None, false, Some(queue.as_com_ref()))
        .unwrap();
    let observer = area.handle();
    let window = ui.window("area", RECT, area).unwrap();
    ui.input(
        UiInput::Key(KeyStroke {
            key: KeyCode::Tab,
            value: 0,
            pressed: true,
            repeat: false,
            modifiers: 0,
        }),
        "",
    )
    .unwrap();
    ui.input(UiInput::Text, "first").unwrap();
    ui.input(UiInput::Text, " second").unwrap();
    ui.update(SIZE).unwrap();
    let messages: Vec<EventMessage> = inbox.try_iter().collect();
    assert_eq!(messages.len(), 2);
    for (message, text) in messages.iter().zip(["first", "first second"]) {
        assert!(message.origin.widget::<TextAreaAbi>().unwrap().unwrap().same_object(observer.as_interface()));
        assert_eq!(
            message.query::<ITextChangedEventAbi>().unwrap().unwrap().text().unwrap().to_string().unwrap(),
            text
        );
    }
    window.close().unwrap();
    assert!(!messages[0].origin.widget::<TextAreaAbi>().unwrap().unwrap().is_alive());
    assert_eq!(
        messages[0]
            .query::<ITextChangedEventAbi>()
            .unwrap()
            .unwrap()
            .text()
            .unwrap()
            .to_string()
            .unwrap(),
        "first"
    );
}
