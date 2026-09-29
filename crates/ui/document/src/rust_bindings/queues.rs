//! Shared application queue generation over COM-owned binding metadata.
//!
//! Keep the existing widget-specific Event/Handlers contract. Captured messages
//! first resolve their weak origin against the current Form, then become Event.
//! This separates lifetime/membership checks from ordinary mutable application behavior.
use super::*;

/// One concrete receiver implementation shared by every instance of a class.
struct Queue {
    /// Indices into the already ordered widget-event list.
    members: Vec<usize>,
    /// Exact public producer projection used by weak origins and form fields.
    interface: String,
    /// Owned event value used by the concrete receiver ABI.
    payload: String,
    /// Checked COM-supplied Rust implementation trait path.
    implementation: String,
    /// Exact revision needed when grouping a nominal queue identity.
    signature: [u8; 32],
    /// Built-in domain check, after generic wire decoding and before enqueueing.
    validate: Option<String>,
}

/// Groups canonical queue identities without guessing a custom widget's Rust trait.
/// Rust reexports do not create new COM identities; the first spelling supplies
/// the implementation and application compilation checks alias compatibility.
fn contracts(events: &[(usize, &Node, String)], custom: &CustomWidgetRegistry) -> Result<Vec<Queue>, String> {
    let mut queues: Vec<Queue> = Vec::new();
    let mut indices = std::collections::BTreeMap::new();
    for (member, (_, node, payload)) in events.iter().enumerate() {
        let (iid, signature, implementation, validate) = match &node.widget {
            WidgetSpec::Custom { kind, .. } => {
                let queue = custom.interface(kind)?.event_queue.as_ref().ok_or("Missing constructor queue")?;
                (queue.iid, queue.signature, queue.rust_trait.clone(), None)
            }
            widget => {
                let queue = widget.event_stream().ok_or("Missing built-in constructor queue")?;
                // The facade reexports the entire generated node namespace. Use
                // that supported import path for this same generated declaration.
                let name = queue.rust_trait.rsplit("::").next().ok_or("Missing queue trait name")?;
                let implementation = format!("neocogi_ui::nodes::{name}");
                // SAFETY: built-in descriptor/signature storage is generated static data.
                let signature = unsafe { queue.abi.abi_signature.as_bytes() }
                    .map_err(|error| error.to_string())?
                    .try_into()
                    .map_err(|_| "Invalid built-in queue signature")?;
                let validate = Some(format!(
                    "{}::validate",
                    payload.strip_suffix("::Event").ok_or("Missing built-in event binding")?
                ));
                (queue.abi.interface_id.to_uuid_bytes(), signature, implementation, validate)
            }
        };
        let interface = node.widget.rust_interface(custom)?.to_owned();
        match indices.get(&iid).copied() {
            Some(index) => {
                let queue: &mut Queue = &mut queues[index];
                if queue.signature != signature {
                    return Err("One COM queue identity has conflicting ABI revisions".into());
                }
                // A custom catalog may reexport a built-in under another Rust path.
                // Preserve the built-in domain check regardless of member ordering.
                queue.validate = queue.validate.take().or(validate);
                queue.members.push(member);
            }
            None => {
                indices.insert(iid, queues.len());
                queues.push(Queue {
                    members: vec![member],
                    interface,
                    payload: payload.clone(),
                    implementation,
                    signature,
                    validate,
                });
            }
        }
    }
    Ok(queues)
}

/// Adds the capture/resolve boundary and one concrete application queue object.
/// Existing Event variants, required handlers, typed fields and bind() are unchanged.
pub(super) fn emit(source: &mut String, events: &[(usize, &Node, String)], custom: &CustomWidgetRegistry) -> Result<(), String> {
    let queues = contracts(events, custom)?;
    source.push_str("/// Captured facts retain weak origins; resolve against current fields before dispatch.\npub enum QueuedEvent {\n");
    for (index, queue) in queues.iter().enumerate() {
        writeln!(source, "    /// Complete stream for instances of {}.\n    Source{index} {{\n        /// Does not retain a removed widget.\n        origin: neocogi_com::WeakComPtr<{}>,\n        /// Owned snapshot acquired before post returned.\n        event: {},\n    }},", queue.interface, queue.interface, queue.payload).unwrap();
    }
    source.push_str("}\nimpl QueuedEvent {\n    /// Resolve only after generation validation; expired or foreign origins return None.\n    /// Inspect the captured payload directly when its origin lifetime is irrelevant.\n    pub fn resolve(self, form: &Form) -> Result<Option<Event>, neocogi_com::NcStatus> {\n        let _ = &form;\n        match self {\n");
    for (index, queue) in queues.iter().enumerate() {
        writeln!(source, "            Self::Source{index} {{ origin, event }} => {{\n                let origin = match origin.upgrade()? {{ Some(origin) => origin, None => return Ok(None) }};\n                Ok(match origin.as_com_ref() {{").unwrap();
        for member in &queue.members {
            let (ordinal, node, _) = &events[*member];
            writeln!(
                source,
                "                    origin if origin.same_object(form.{}.as_interface()) => Some(Event::Widget{ordinal}(event)),",
                symbol("widget", &node.key, *ordinal)
            )
            .unwrap();
        }
        source.push_str("                    _ => None,\n                })\n            }\n");
    }
    source.push_str("        }\n    }\n}\n");
    if !queues.is_empty() {
        writeln!(
            source,
            "#[neocogi_com::com_object(interfaces({}))]",
            queues.iter().map(|queue| queue.implementation.as_str()).collect::<Vec<_>>().join(", ")
        )
        .unwrap();
        source.push_str("/// One application queue allocation serves every widget in this construction attempt.\nstruct FormQueue {\n    /// Fixed reload identity, independent of widget lifetime.\n    generation: neocogi_ui_document::Generation,\n    /// Retains the channel, never the form, model or widgets.\n    send: std::sync::mpsc::Sender<Message>,\n}\n");
        for (index, queue) in queues.iter().enumerate() {
            writeln!(source, "impl {} for FormQueue {{\n    /// Acquires the snapshot and weak origin, then enqueues without running handlers.\n    fn post(&mut self, origin: neocogi_com::ComRef<'_, {}>, event: neocogi_com::NcValueRef<'_, {}>) -> Result<(), neocogi_com::NcStatus> {{\n        let event = event.to_owned()?;", queue.implementation, queue.interface, queue.payload).unwrap();
            if let Some(validate) = &queue.validate {
                writeln!(source, "        {validate}(&event)?;").unwrap();
            }
            writeln!(source, "        let event = QueuedEvent::Source{index} {{ origin: origin.downgrade()?, event }};\n        let _ = self.send.send(Message {{ generation: self.generation, event }});\n        Ok(())\n    }}\n}}").unwrap();
        }
    }
    source.push_str("/// Supplies shared queue projections before Loader or ReloadSlot creates widgets.\npub fn events(generation: neocogi_ui_document::Generation, send: std::sync::mpsc::Sender<Message>) -> Result<neocogi_ui_document::EventBindings, neocogi_com::NcStatus> {\n    let mut bindings = neocogi_ui_document::EventBindings::new();\n");
    match queues.is_empty() {
        true => source.push_str("    let _ = (generation, send, &mut bindings);\n"),
        false => {
            source.push_str("    let queue = FormQueue::from_value(FormQueue { generation, send });\n");
            for (index, queue) in queues.iter().enumerate() {
                writeln!(
                    source,
                    "    let queue_{index} = queue.query::<neocogi_com::events::QueueOf<{}>>()?;",
                    queue.interface
                )
                .unwrap();
                for member in &queue.members {
                    let (_, node, _) = &events[*member];
                    writeln!(
                        source,
                        "    bindings.insert_queue::<{}>({:?}, queue_{index}.as_com_ref())?;",
                        queue.interface, node.key
                    )
                    .unwrap();
                }
            }
        }
    }
    source.push_str("    Ok(bindings)\n}\n");
    Ok(())
}
