//! Trusted construction authority for authored custom COM leaf widgets.
use crate::{
    CustomProperty, Document, Node, WidgetSpec,
    design::{DesignInterface, DesignWidgetManifest},
};
use neocogi_com::{ComPtr, IUnknown, NcGuid, NcStatus, NC_E_INVALID_ARG};
use neocogi_ui::{IWidgetAbi, Ui};
use std::{collections::BTreeMap, rc::Rc};

/// A provider supplies one owned authoring description and a COM constructor.
/// Static hosts can build the description from generated `IdlInterface` data;
/// loaded plugins implement this trait with a queried COM catalog.
pub trait CustomWidgetFactory {
    /// Returns complete, versioned metadata for this constructible widget kind.
    fn manifest(&self) -> Result<DesignWidgetManifest, String>;
    /// Constructs one concrete COM identity using the runtime's widget host.
    /// The provider validates properties and the supplied typed sink before returning.
    /// `None` constructs a permanently unobserved widget; no later attachment is allowed.
    fn create(&self, ui: &Ui, properties: &[CustomProperty], sink: Option<ComPtr<IUnknown>>) -> Result<neocogi_ui::OwnedWidget<IWidgetAbi>, NcStatus>;
}

/// One installed provider and the immutable metadata validated at registration.
#[derive(Clone)]
struct Entry {
    /// Constructor used only after document property validation succeeds.
    factory: Rc<dyn CustomWidgetFactory>,
    /// Owned metadata copied from generated IDL or across the COM catalog.
    manifest: DesignWidgetManifest,
}

/// Explicit local factory catalog shared by loading and design-time authoring.
#[derive(Clone, Default)]
pub struct CustomWidgetRegistry {
    /// Installed providers are keyed by stable authored kind.
    entries: BTreeMap<String, Entry>,
}
impl CustomWidgetRegistry {
    /// Starts an empty registry; built-in widgets need no entries.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one provider after validating its complete IDL-derived description.
    pub fn register(&mut self, factory: Rc<dyn CustomWidgetFactory>) -> Result<(), String> {
        let manifest = factory.manifest()?;
        manifest.validate()?;
        if self.entries.contains_key(&manifest.kind) {
            return Err(format!("Duplicate custom widget kind: {}", manifest.kind));
        }
        self.entries.insert(manifest.kind.clone(), Entry { factory, manifest });
        Ok(())
    }

    /// Builds a palette-ready constructor from the installed provider's defaults.
    pub fn default_spec(&self, kind: &str) -> Option<WidgetSpec> {
        let entry = self.entries.get(kind)?;
        Some(WidgetSpec::Custom {
            kind: kind.to_owned(),
            properties: entry.manifest.defaults.clone(),
        })
    }

    /// Returns registered kinds in deterministic palette order.
    pub fn kinds(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    /// Returns owned COM and IDL metadata for a registered custom kind.
    pub fn interface(&self, kind: &str) -> Result<&DesignInterface, String> {
        self.entries
            .get(kind)
            .map(|entry| &entry.manifest.interface)
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
            let entry = self.entries.get(kind).ok_or_else(|| format!("Custom widget kind {kind} is not registered"))?;
            let defaults = &entry.manifest.defaults;
            if properties.len() != defaults.len()
                || !properties
                    .iter()
                    .zip(defaults)
                    .all(|(value, expected)| value.name == expected.name && value.value.same_kind(&expected.value) && value.value.valid())
            {
                return Err(format!("Custom widget property schema mismatch on {}", node.key));
            }
        }
        for child in &node.children {
            self.validate_node(&child.node)?;
        }
        Ok(())
    }

    /// Calls the installed provider and verifies its advertised concrete identity.
    pub(crate) fn create(&self, kind: &str, ui: &Ui, properties: &[CustomProperty], sink: Option<ComPtr<IUnknown>>) -> Result<neocogi_ui::OwnedWidget<IWidgetAbi>, NcStatus> {
        let entry = self.entries.get(kind).ok_or(NC_E_INVALID_ARG)?;
        let widget = entry.factory.create(ui, properties, sink)?;
        let interface = &entry.manifest.interface;
        let iid = NcGuid::from_uuid_bytes(interface.iid);
        widget.as_interface().downgrade().supports_interface_bytes(iid, &interface.signature)?;
        Ok(widget)
    }
}
