//! Optional native form resolution over the same common COM inbox used by dynamic hosts.
use super::*;

/// Emits one receiver for the construction attempt, independent of widget/event kinds.
/// Payload decoding happens after generation and current-form identity checks.
pub(super) fn emit(source: &mut String, events: &[(usize, &Node, String)], _custom: &CustomWidgetRegistry) -> Result<(), String> {
    source.push_str("/// A captured exclusive event and weak source, still readable after source removal.\npub struct QueuedEvent(pub neocogi_com::events::EventMessage);\nimpl QueuedEvent {\n    /// Call only after generation validation. Removed or foreign sources return None.\n    /// Native decoding keeps the COM event owned by this message throughout the call.\n    pub fn resolve(&self, form: &Form) -> Result<Option<Event>, neocogi_com::NcStatus> {\n        let _ = form;\n        let neocogi_com::events::EventOrigin::Widget(origin) = &self.0.origin else { return Ok(None); };\n        if !origin.is_alive() { return Ok(None); }\n");
    for (ordinal, node, payload) in events {
        writeln!(source,
            "        if origin.same_object(form.{}.as_interface()) {{\n            return Ok(self.0.payload::<{payload}>()?.map(Event::Widget{ordinal}));\n        }}",
            symbol("widget", &node.key, *ordinal)).unwrap();
    }
    source.push_str("        Ok(None)\n    }\n}\n");
    if !events.is_empty() {
        source.push_str("#[neocogi_com::com_object(interfaces(neocogi_com::events::IEventReceiver))]\n/// One receiver serves every widget in this construction attempt.\nstruct FormQueue {\n    /// Slot/attempt identity is checked before interpreting an event.\n    generation: neocogi_ui_document::Generation,\n    /// Posting access owns no application state or widgets.\n    send: std::sync::mpsc::Sender<Message>,\n}\nimpl neocogi_com::events::IEventReceiver for FormQueue {\n    /// Move the event into the inbox; rejection restores exactly the original owner.\n    fn post(&mut self, origin: neocogi_com::NcValueRef<'_, neocogi_com::events::EventOrigin>, event: neocogi_com::ComBox<neocogi_com::events::IEventAbi>) -> Result<(), (neocogi_com::NcStatus, neocogi_com::ComBox<neocogi_com::events::IEventAbi>)> {\n        let event = QueuedEvent(neocogi_com::events::EventMessage::capture(origin, event)?);\n        self.send.send(Message { generation: self.generation, event }).map_err(|error| (neocogi_com::NC_E_CLOSED, error.0.event.0.event))\n    }\n}\n");
    }
    source.push_str("/// Supplies the common receiver before Loader or ReloadSlot creates widgets.\n/// Use the token issued by a slot in the receiving application's ReloadIds domain.\npub fn events(generation: neocogi_ui_document::Generation, send: std::sync::mpsc::Sender<Message>) -> Result<neocogi_ui_document::EventBindings, neocogi_com::NcStatus> {\n    let mut bindings = neocogi_ui_document::EventBindings::new();\n");
    if events.is_empty() {
        source.push_str("    let _ = (generation, send, &mut bindings);\n");
    } else {
        source.push_str("    let queue = FormQueue::from_value(FormQueue { generation, send }).query::<neocogi_com::events::IEventReceiverAbi>()?;\n");
        for (_, node, _) in events {
            writeln!(source, "    bindings.insert_queue({:?}, queue.as_com_ref())?;", node.key).unwrap();
        }
    }
    source.push_str("    Ok(bindings)\n}\n");
    Ok(())
}
