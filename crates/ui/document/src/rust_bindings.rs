//! Deterministic Rust contract generation; exported names remain string literals, never source code.
use crate::{Document, InputKind, OutputType, MAX_BYTES};
use std::fmt::Write;

/// Builds a valid, collision-free symbol with a role prefix and declaration ordinal.
/// Public endpoint names remain exact in string literals, even when punctuation normalizes alike.
fn symbol(prefix: &str, name: &str, index: usize) -> String {
    let mut symbol = prefix.to_owned();
    let mut separator = true;
    for character in name.chars() {
        match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' => {
                // Collapse punctuation runs and omit leading/trailing empty name segments.
                // The role prefix keeps even numeric names valid Rust identifiers.
                if separator {
                    symbol.push('_');
                }
                symbol.push(character.to_ascii_lowercase());
                separator = false;
            }
            _ => separator = true,
        }
    }
    // The ordinal disambiguates distinct public names that normalize to the same stem.
    write!(symbol, "_{index}").unwrap();
    symbol
}

impl OutputType {
    /// Public Rust payload spelling shared with generated event variants and handler signatures.
    pub fn rust_type(self) -> &'static str {
        match self {
            Self::Activation => "neocogi_ui::events::Activation",
            Self::Checked => "neocogi_ui::events::CheckboxChanged",
            Self::Number => "neocogi_ui::events::NumericChanged",
            Self::TextChanged => "neocogi_ui::events::TextChanged",
            Self::TextSubmitted => "neocogi_ui::events::TextSubmitted",
            Self::Offset => "neocogi_ui::events::ScrollbarChanged",
            Self::Position => "neocogi_ui::events::SplitChanged",
            Self::Selection => "neocogi_ui::events::ChoiceSelection",
        }
    }
}
impl InputKind {
    /// Public Rust command spelling for generated, statically resolved input handles.
    pub fn rust_type(&self) -> &'static str {
        match self {
            Self::Text => "neocogi_ui_document::contracts::SetText",
            Self::Checked => "neocogi_ui_document::contracts::SetChecked",
            Self::Value => "neocogi_ui_document::contracts::SetValue",
            Self::Enabled => "neocogi_ui_document::contracts::SetEnabled",
            Self::Expanded => "neocogi_ui_document::contracts::SetExpanded",
            Self::Choices => "neocogi_ui_document::contracts::SetChoices",
            Self::Selection => "neocogi_ui_document::contracts::SetSelection",
            Self::Offset => "neocogi_ui_document::contracts::SetOffset",
            Self::Position => "neocogi_ui_document::contracts::SetPosition",
        }
    }
}
impl Document {
    /// Generates a complete Rust module containing typed events, required handlers, and input bindings.
    /// Call `bind` in a reload preparation callback and generation-filter its messages before dispatch.
    /// Application implementations of `Handlers` live in separate files and survive regeneration.
    pub fn rust_bindings(&self) -> Result<String, String> {
        self.validate()?;
        let mut source = String::from("// Generated from a validated UI document. Keep application handlers in another module.\n");
        source.push_str("/// Captured public events; private visual connections remain inside the loaded component.\npub enum Event {\n");
        for (index, output) in self.outputs.iter().enumerate() {
            let payload = self
                .root
                .find(&output.node)
                .and_then(|node| node.widget.output_type(&output.port))
                .ok_or("Missing output contract")?;
            writeln!(source, "    /// Public output {:?}.\n    Output{index}({}),", output.name, payload.rust_type()).unwrap();
        }
        source.push_str("}\n/// A captured event belongs to exactly one component reload attempt.\npub struct Message {\n    /// Reject stale messages with ReloadSlot::accepts before dispatch.\n    pub generation: neocogi_ui_document::Generation,\n    /// Concrete captured event payload.\n    pub event: Event,\n}\n");
        source.push_str("/// Implement every public handler in application-owned code; no callback bodies are overwritten.\npub trait Handlers {\n");
        for (index, output) in self.outputs.iter().enumerate() {
            let payload = self
                .root
                .find(&output.node)
                .and_then(|node| node.widget.output_type(&output.port))
                .ok_or("Missing output contract")?;
            writeln!(source, "    /// Handles public output {:?} at the application's safe update boundary.\n    fn {}(&mut self, value: {}) -> Result<(), neocogi_com::NcStatus>;", output.name, symbol("on", &output.name, index), payload.rust_type()).unwrap();
        }
        source.push_str("}\nimpl Event {\n    /// Dispatch only after checking the enclosing message's generation.\n    pub fn dispatch(self, _handlers: &mut impl Handlers) -> Result<(), neocogi_com::NcStatus> {\n        match self {\n");
        for (index, output) in self.outputs.iter().enumerate() {
            writeln!(
                source,
                "            Self::Output{index}(value) => _handlers.{}(value),",
                symbol("on", &output.name, index)
            )
            .unwrap();
        }
        source.push_str("        }\n    }\n}\n/// Reconnect these concrete input handles after each successful component replacement.\npub struct Inputs {\n");
        for (index, input) in self.inputs.iter().enumerate() {
            writeln!(
                source,
                "    /// Public input {:?}; send its exact command at an ordinary update boundary.\n    pub {}: neocogi_ui_document::InputHandle<{}>,",
                input.name,
                symbol("input", &input.name, index),
                input.kind.rust_type()
            )
            .unwrap();
        }
        source.push_str("}\n/// Resolves inputs and queues public outputs during candidate preparation.\n/// Use inside ReloadSlot::reload; failed preparation leaves the previous component published.\npub fn bind(\n    _component: &neocogi_ui_document::Component,\n    _generation: neocogi_ui_document::Generation,\n    _send: std::sync::mpsc::Sender<Message>,\n) -> Result<Inputs, neocogi_com::NcStatus> {\n    let inputs = Inputs {\n");
        for (index, input) in self.inputs.iter().enumerate() {
            writeln!(
                source,
                "        {}: _component.input::<{}>({:?})?,",
                symbol("input", &input.name, index),
                input.kind.rust_type(),
                input.name
            )
            .unwrap();
        }
        source.push_str("    };\n");
        if !self.outputs.is_empty() {
            source.push_str("    let outputs = _component.outputs()?;\n");
        }
        for (index, output) in self.outputs.iter().enumerate() {
            let payload = self
                .root
                .find(&output.node)
                .and_then(|node| node.widget.output_type(&output.port))
                .ok_or("Missing output contract")?;
            writeln!(source, "    neocogi_com::events::bind_output(outputs.as_com_ref(), {:?}, _send.clone(), move |value: {}| Message {{ generation: _generation, event: Event::Output{index}(value) }})?;", output.name, payload.rust_type()).unwrap();
        }
        source.push_str("    Ok(inputs)\n}\n");
        if source.len() > MAX_BYTES {
            return Err("Generated bindings exceed the byte limit".into());
        }
        Ok(source)
    }
}
