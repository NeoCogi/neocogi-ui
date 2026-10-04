//! Deterministic Rust contract generation; exported names remain string literals, never source code.
use crate::{CustomWidgetRegistry, Document, Node, WidgetSpec, MAX_BYTES};
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

impl Document {
    /// Generates optional typed widget fields; event dispatch uses the common COM inbox.
    pub fn rust_bindings(&self) -> Result<String, String> {
        self.rust_bindings_with(&CustomWidgetRegistry::new())
    }

    /// Generates fields with custom interface types read from IDL-generated COM metadata.
    /// Registry entries are immutable after COM path validation at registration.
    /// Rust compilation still checks crate availability, types and trait implementations.
    pub fn rust_bindings_with(&self, custom: &CustomWidgetRegistry) -> Result<String, String> {
        custom.validate_document(self)?;
        let mut nodes = Vec::new();
        collect_nodes(&self.root, &mut nodes);
        let mut source = String::from("// Generated optional widget fields from a validated UI document.\n");
        source.push_str("/// Checked widget fields expose their own getters and setters.\n/// Resolve a new Form after replacement; old handles observe widget closure.\npub struct Form {\n");
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
        source.push_str("}\n/// Resolves concrete fields after construction has installed every receiver.\npub fn bind(\n    component: &neocogi_ui_document::Component,\n) -> Result<Form, neocogi_com::NcStatus> {\n    let form = Form {\n");
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
        source.push_str("    Ok(form)\n}\n");
        if source.len() > MAX_BYTES {
            return Err("Generated bindings exceed the byte limit".into());
        }
        Ok(source)
    }
}
