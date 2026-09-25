//! Trusted construction authority for authored custom COM leaf widgets.
use crate::{CustomProperty, Document, Node, WidgetSpec};
use neocogi_com::{NcStatus, NC_E_INVALID_ARG, design::IdlInterface};
use neocogi_ui::{IWidgetAbi, Ui, Widget};
use std::{collections::BTreeMap, rc::Rc};

/// A provider supplies construction behavior and generated COM metadata for one widget kind.
/// The concrete interface metadata is emitted by IDL beside its ABI descriptor.
pub trait CustomWidgetFactory {
    /// Stable kind by which an authored document selects this installed provider.
    fn kind(&self) -> &str;
    /// IDL-generated description of the actual COM interface returned by `create`.
    fn interface(&self) -> &'static IdlInterface;
    /// Typed constructor property names and default values in stable order.
    fn defaults(&self) -> &[CustomProperty];
    /// Constructs one concrete COM identity using the runtime's widget host.
    /// The provider validates domain-specific property ranges before returning.
    fn create(&self, ui: &Ui, properties: &[CustomProperty]) -> Result<Widget<IWidgetAbi>, NcStatus>;
}

/// Explicit, local factory catalog shared by loading and design-time authoring.
#[derive(Clone, Default)]
pub struct CustomWidgetRegistry {
    /// Installed factories are keyed by stable kind, not dynamically loaded from document bytes.
    entries: BTreeMap<String, Rc<dyn CustomWidgetFactory>>,
}
impl CustomWidgetRegistry {
    /// Starts an empty registry; built-in widgets need no entries.
    pub fn new() -> Self { Self::default() }

    /// Registers one trusted provider after checking its IDL-derived event policy and property schema.
    pub fn register(&mut self, factory: Rc<dyn CustomWidgetFactory>) -> Result<(), String> {
        let kind = factory.kind();
        if kind.is_empty() || self.entries.contains_key(kind) {
            return Err(format!("Duplicate or empty custom widget kind: {kind}"));
        }
        let interface = factory.interface();
        if interface.outputs.len() > 1 {
            return Err(format!("Custom widget {kind} has more than one event port; use one enum message"));
        }
        // The source-level record is read through the generated COM projection.
        // Cross-check its port names against the actual physical method table so
        // a hand-built descriptor cannot hide a second consumer connection.
        let primary = interface.chain.first().ok_or_else(|| format!("Custom widget {kind} has no COM interface chain"))?;
        if primary.interface_id != interface.abi.interface_id || primary.abi_signature != interface.abi.abi_signature {
            return Err(format!("Custom widget {kind} has an inconsistent COM interface chain"));
        }
        let mut physical_ports = Vec::new();
        for descriptor in interface.chain {
            // SAFETY: registered factory metadata points to immutable generated static tables.
            let methods = unsafe { descriptor.methods() }
                .map_err(|error| format!("Invalid COM interface metadata for {kind}: {error}"))?;
            for method in methods {
                // SAFETY: the trusted generated descriptor owns static method names.
                let name = unsafe { method.name.as_str() }
                    .map_err(|error| format!("Invalid COM method metadata for {kind}: {error}"))?;
                if let Some(port) = name.strip_prefix("__map_") {
                    physical_ports.push(port.to_owned());
                }
            }
        }
        if physical_ports.len() != interface.outputs.len()
            || physical_ports.iter().zip(interface.outputs).any(|(physical, output)| physical != output.name)
        {
            return Err(format!("Custom widget {kind} output metadata disagrees with its COM method table"));
        }
        if !rust_path(interface.rust_abi) || interface.rust_outputs.is_some_and(|path| !rust_path(path)) {
            return Err(format!("Custom widget {kind} has invalid generated Rust paths"));
        }
        if let Some(output) = interface.outputs.first() {
            if interface.rust_outputs.is_none() || !rust_path(output.message.rust_type) || !identifier(output.name) {
                return Err(format!("Custom widget {kind} has invalid IDL output metadata"));
            }
        }
        let mut names = std::collections::BTreeSet::new();
        if !factory.defaults().iter().all(|property| {
            !property.name.is_empty() && names.insert(&property.name) && property.value.valid()
        }) {
            return Err(format!("Custom widget {kind} has invalid property defaults"));
        }
        self.entries.insert(kind.to_owned(), factory);
        Ok(())
    }

    /// Builds a palette-ready constructor from the installed factory's defaults.
    pub fn default_spec(&self, kind: &str) -> Option<WidgetSpec> {
        let factory = self.entries.get(kind)?;
        Some(WidgetSpec::Custom { kind: kind.to_owned(), properties: factory.defaults().to_vec() })
    }

    /// Returns registered kinds in deterministic palette order.
    pub fn kinds(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    /// Returns the generated COM description for a registered custom kind.
    pub fn interface(&self, kind: &str) -> Result<&'static IdlInterface, String> {
        self.entries.get(kind).map(|factory| factory.interface())
            .ok_or_else(|| format!("Custom widget kind {kind} is not registered"))
    }

    /// Checks every custom node against the installed constructor schema.
    pub fn validate_document(&self, document: &Document) -> Result<(), String> {
        document.validate()?;
        self.validate_node(&document.root)
    }

    /// Validates one subtree without rebuilding the document name index.
    fn validate_node(&self, node: &Node) -> Result<(), String> {
        if let WidgetSpec::Custom { kind, properties } = &node.widget {
            let factory = self.entries.get(kind).ok_or_else(|| format!("Custom widget kind {kind} is not registered"))?;
            let defaults = factory.defaults();
            if properties.len() != defaults.len() || !properties.iter().zip(defaults).all(|(value, expected)| {
                value.name == expected.name && value.value.same_kind(&expected.value) && value.value.valid()
            }) {
                return Err(format!("Custom widget property schema mismatch on {}", node.key));
            }
        }
        for child in &node.children {
            self.validate_node(&child.node)?;
        }
        Ok(())
    }

    /// Dispatches a validated leaf constructor to its installed COM provider.
    pub(crate) fn create(&self, kind: &str, ui: &Ui, properties: &[CustomProperty]) -> Result<Widget<IWidgetAbi>, NcStatus> {
        let factory = self.entries.get(kind).ok_or(NC_E_INVALID_ARG)?;
        let widget = factory.create(ui, properties)?;
        let identity = neocogi_com::NcInterfaceId::new(
            factory.interface().abi.interface_id,
            factory.interface().abi.abi_signature,
        );
        if let Err(error) = widget.as_interface().add_ref().supports_interface_id(&identity) {
            // The candidate has not entered Construction's owned-node set yet.
            // Close its common widget state here before dropping the last handle.
            if let Ok(mut node) = widget.node_interface() {
                let _ = node.close();
            }
            return Err(error);
        }
        Ok(widget)
    }
}

/// Rust identifiers are validated before metadata is interpolated into generated source.
fn identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some('a'..='z' | 'A'..='Z' | '_'))
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        && !matches!(value, "self" | "super" | "crate" | "Self" | "fn" | "mod" | "pub" | "use")
}

/// Generated Rust type locations must be simple paths, never arbitrary source snippets.
fn rust_path(path: &str) -> bool {
    !path.is_empty() && path.split("::").all(identifier)
}
