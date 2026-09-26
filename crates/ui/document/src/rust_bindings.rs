//! Deterministic Rust contract generation; exported names remain string literals, never source code.
use crate::{CustomWidgetRegistry, Document, InputKind, Node, WidgetSpec, MAX_BYTES};
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

/// Visits every owned node once in authored preorder so generated field names are stable.
fn collect_nodes<'a>(node: &'a Node, nodes: &mut Vec<&'a Node>) {
    nodes.push(node);
    for child in &node.children {
        collect_nodes(&child.node, nodes);
    }
}

impl WidgetSpec {
    /// Complete typed stream payload used by direct generated form handlers.
    fn rust_event_type(&self) -> Option<&'static str> {
        Some(match self {
            Self::Button { .. } => "neocogi_ui::button::Event",
            Self::Checkbox { .. } => "neocogi_ui::checkbox::Event",
            Self::Textbox { .. } => "neocogi_ui::textbox::Event",
            Self::TextArea { .. } => "neocogi_ui::text_area::Event",
            Self::Slider { .. } => "neocogi_ui::slider::Event",
            Self::Number { .. } => "neocogi_ui::number::Event",
            Self::ListItem { .. } => "neocogi_ui::list_item::Event",
            Self::ListBox { .. } => "neocogi_ui::list_box::Event",
            Self::Scrollbar { .. } => "neocogi_ui::scrollbar::Event",
            Self::Split { .. } => "neocogi_ui::split_view::Event",
            Self::Combo { .. } => "neocogi_ui::combo::Event",
            _ => return None,
        })
    }

    /// Returns the concrete built-in COM projection used by generated form fields.
    /// QueryInterface checks this expectation when a candidate form is bound.
    fn rust_interface<'a>(&'a self, custom: &'a CustomWidgetRegistry) -> Result<&'a str, String> {
        Ok(match self {
            Self::Button { .. } => "neocogi_ui::ButtonAbi",
            Self::Checkbox { .. } => "neocogi_ui::CheckboxAbi",
            Self::Textbox { .. } => "neocogi_ui::TextboxAbi",
            Self::TextBlock { .. } => "neocogi_ui::TextBlockAbi",
            Self::TextArea { .. } => "neocogi_ui::TextAreaAbi",
            Self::Slider { .. } => "neocogi_ui::SliderAbi",
            Self::Number { .. } => "neocogi_ui::NumberAbi",
            Self::ColorSwatch { .. } => "neocogi_ui::ColorSwatchAbi",
            Self::ListItem { .. } => "neocogi_ui::ListItemAbi",
            Self::ListBox { .. } => "neocogi_ui::ListBoxAbi",
            Self::Scrollbar { .. } => "neocogi_ui::ScrollbarAbi",
            Self::Combo { .. } => "neocogi_ui::ComboAbi",
            Self::Form { .. } => "neocogi_ui::FormAbi",
            Self::Linear { .. } => "neocogi_ui::LinearAbi",
            Self::Grid { .. } => "neocogi_ui::GridAbi",
            Self::Split { .. } => "neocogi_ui::SplitViewAbi",
            Self::Scroll { .. } => "neocogi_ui::ScrollAreaAbi",
            Self::Disclosure { .. } => "neocogi_ui::DisclosureAbi",
            Self::Custom { kind, .. } => custom.interface(kind)?.rust_abi.as_str(),
        })
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
    /// Generates one concrete form field and one required handler for every event-producing widget.
    /// The application implements `Handlers` outside generated source and dispatches at its update boundary.
    pub fn rust_bindings(&self) -> Result<String, String> {
        self.rust_bindings_with(&CustomWidgetRegistry::new())
    }

    /// Generates bindings with custom interface and message types read from IDL-generated COM metadata.
    pub fn rust_bindings_with(&self, custom: &CustomWidgetRegistry) -> Result<String, String> {
        custom.validate_document(self)?;
        let mut nodes = Vec::new();
        collect_nodes(&self.root, &mut nodes);
        // The generated contract has one required handler per actual emitting widget.
        // Plugin streams use the same EventSource trait as built-ins.
        let mut events = Vec::new();
        for (index, node) in nodes.iter().enumerate() {
            let payload = match &node.widget {
                WidgetSpec::Custom { kind, .. } => custom.interface(kind)?.stream.as_ref().map(|stream| stream.message.rust_type.clone()),
                widget => widget.rust_event_type().map(str::to_owned),
            };
            if let Some(payload) = payload {
                events.push((index, *node, payload));
            }
        }
        let mut source = String::from("// Generated from a validated UI document. Implement Handlers in application-owned code.\n");
        source.push_str("/// Complete captured widget events, one variant per event-producing widget.\npub enum Event {\n");
        for (index, node, payload) in &events {
            writeln!(
                source,
                "    /// Complete event stream from authored widget {:?}.\n    Widget{index}({}),",
                node.key, payload
            )
            .unwrap();
        }
        source.push_str("}\n/// A captured event belongs to exactly one component reload attempt.\npub struct Message {\n    /// Reject stale messages with ReloadSlot::accepts before dispatch.\n    pub generation: neocogi_ui_document::Generation,\n    /// Complete typed widget event.\n    pub event: Event,\n}\n");
        source.push_str("/// Application-owned behavior; every emitting widget requires an implementation.\npub trait Handlers {\n");
        for (index, node, payload) in &events {
            writeln!(source, "    /// Handles the event from widget {:?} after traversal releases widget borrows.\n    fn {}(&mut self, event: {}) -> Result<(), neocogi_com::NcStatus>;", node.key, symbol("on", &node.key, *index), payload).unwrap();
        }
        source.push_str("}\nimpl Event {\n    /// Dispatch only after checking the enclosing message generation.\n    pub fn dispatch(self, handlers: &mut impl Handlers) -> Result<(), neocogi_com::NcStatus> {\n        match self {\n");
        for (index, node, _) in &events {
            writeln!(
                source,
                "            Self::Widget{index}(event) => handlers.{}(event),",
                symbol("on", &node.key, *index)
            )
            .unwrap();
        }
        source.push_str("        }\n    }\n}\n/// Reconnect concrete command handles after every successful component replacement.\npub struct Inputs {\n");
        for (index, input) in self.inputs.iter().enumerate() {
            writeln!(
                source,
                "    /// Public command {:?}; its receiver is checked when binding.\n    pub {}: neocogi_ui_document::InputHandle<{}>,",
                input.name,
                symbol("input", &input.name, index),
                input.kind.rust_type()
            )
            .unwrap();
        }
        source.push_str("}\n/// Checked concrete handles for every authored widget and the exported commands.\npub struct Form {\n    /// Public command endpoints retained for message-based integrations.\n    pub inputs: Inputs,\n");
        for (index, node) in nodes.iter().enumerate() {
            writeln!(
                source,
                "    /// Authored widget {:?}; the concrete COM interface is checked when binding.\n    pub {}: neocogi_ui::Widget<{}>,",
                node.key,
                symbol("widget", &node.key, index),
                node.widget.rust_interface(custom)?
            )
            .unwrap();
        }
        source.push_str("}\n/// Checks widget fields and binds each complete event stream to the application queue.\n/// Use inside ReloadSlot::reload; failed preparation preserves the published component.\npub fn bind(\n    component: &neocogi_ui_document::Component,\n    generation: neocogi_ui_document::Generation,\n    send: std::sync::mpsc::Sender<Message>,\n) -> Result<Form, neocogi_com::NcStatus> {\n    let inputs = Inputs {\n");
        for (index, input) in self.inputs.iter().enumerate() {
            writeln!(
                source,
                "        {}: component.input::<{}>({:?})?,",
                symbol("input", &input.name, index),
                input.kind.rust_type(),
                input.name
            )
            .unwrap();
        }
        source.push_str("    };\n");
        source.push_str("    let form = Form {\n        inputs,\n");
        for (index, node) in nodes.iter().enumerate() {
            writeln!(
                source,
                "        {}: component.widget::<{}>({:?})?,",
                symbol("widget", &node.key, index),
                node.widget.rust_interface(custom)?,
                node.key
            )
            .unwrap();
        }
        source.push_str("    };\n");
        for (index, node, payload) in &events {
            writeln!(source,
                "    neocogi_com::events::bind_source(form.{}.as_interface(), send.clone(), move |event: {}| Message {{ generation, event: Event::Widget{index}(event) }})?;",
                symbol("widget", &node.key, *index), payload).unwrap();
        }
        source.push_str("    Ok(form)\n}\n");
        if source.len() > MAX_BYTES {
            return Err("Generated bindings exceed the byte limit".into());
        }
        Ok(source)
    }
}
