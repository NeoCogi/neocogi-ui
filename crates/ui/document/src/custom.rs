//! Trusted construction authority for authored custom COM leaf widgets.
use crate::{
    Document, Node, WidgetSpec,
    design::{DesignInterface, DesignWidgetManifest},
};
use neocogi_com::{ComPtr, NcGuid, NcStatus, NC_E_INVALID_ARG};
use neocogi_ui::{IWidgetAbi, Ui};
use std::{collections::BTreeMap, rc::Rc};

/// A provider supplies one owned authoring description and a COM constructor.
/// Static hosts can build the description from generated `InterfaceMetadata` data;
/// loaded plugins implement this trait with a queried COM catalog.
pub trait CustomWidgetFactory {
    /// Returns complete, versioned metadata for this constructible widget kind.
    fn manifest(&self) -> Result<DesignWidgetManifest, String>;
    /// Optional plugin-owned editor. Absence leaves settings read-only in the designer.
    fn create_editor(&self, _ui: &Ui, _settings: &[u8]) -> Result<Option<neocogi_com::ComBox<neocogi_ui::abi::document::IDesignSettingsEditorAbi>>, NcStatus> {
        Ok(None)
    }
    /// Constructs one concrete COM identity using the runtime's widget host.
    /// The provider validates settings and the supplied typed sink before returning.
    /// `None` constructs a permanently unobserved widget; no later attachment is allowed.
    fn create(
        &self,
        ui: &Ui,
        settings: &[u8],
        sink: Option<ComPtr<neocogi_ui::events::IEventReceiverAbi>>,
    ) -> Result<neocogi_ui::OwnedWidget<IWidgetAbi>, NcStatus>;
}

/// One installed provider and the immutable metadata validated at registration.
#[derive(Clone)]
struct Entry {
    /// Constructor used after document admission; it owns semantic settings validation.
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
            settings: entry.manifest.defaults.clone(),
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

    /// Admits the complete persistent recipe and checks custom provider availability.
    /// Binary and settings budgets are checked before recipe cloning or any provider
    /// call; local and independently loaded factories follow the same policy.
    pub fn validate_document(&self, document: &Document) -> Result<(), String> {
        document.validate_binary()?;
        self.validate_node(&document.root)?;
        document.validate_output_eligibility(|key| {
            let node = document.root.find(key).ok_or_else(|| format!("Missing display node {key}"))?;
            self.has_events(&node.widget)
        })
    }

    /// Resolves immutable event eligibility from the admitted kind, without constructing/probing it.
    pub(crate) fn has_events(&self, widget: &WidgetSpec) -> Result<bool, String> {
        match widget {
            WidgetSpec::Custom { kind, .. } => Ok(!self.interface(kind)?.events.is_empty()),
            builtin => Ok(!builtin.event_interfaces().is_empty()),
        }
    }

    /// Validates one subtree without rebuilding the document name index.
    fn validate_node(&self, node: &Node) -> Result<(), String> {
        if let WidgetSpec::Custom { kind, .. } = &node.widget {
            self.entries.get(kind).ok_or_else(|| format!("Custom widget kind {kind} is not registered"))?;
            // Document admission already checked snapshot sizes. The provider
            // performs semantic validation during construction and editor Apply.
        }
        for child in &node.children {
            self.validate_node(&child.node)?;
        }
        Ok(())
    }

    /// Opens the provider's concrete draft editor; the caller owns its lifetime and view.
    pub fn create_editor(
        &self,
        kind: &str,
        ui: &Ui,
        settings: &[u8],
    ) -> Result<Option<neocogi_com::ComBox<neocogi_ui::abi::document::IDesignSettingsEditorAbi>>, NcStatus> {
        neocogi_ui_design::validate_settings_size(settings).map_err(|_| NC_E_INVALID_ARG)?;
        self.entries.get(kind).ok_or(NC_E_INVALID_ARG)?.factory.create_editor(ui, settings)
    }

    /// Calls the installed provider and verifies its advertised concrete identity.
    pub(crate) fn create(
        &self,
        kind: &str,
        ui: &Ui,
        settings: &[u8],
        sink: Option<ComPtr<neocogi_ui::events::IEventReceiverAbi>>,
    ) -> Result<neocogi_ui::OwnedWidget<IWidgetAbi>, NcStatus> {
        let entry = self.entries.get(kind).ok_or(NC_E_INVALID_ARG)?;
        let widget = entry.factory.create(ui, settings, sink)?;
        let interface = &entry.manifest.interface;
        let iid = NcGuid::from_uuid_bytes(interface.iid);
        widget.as_interface().downgrade().supports_interface_bytes(iid, &interface.signature)?;
        Ok(widget)
    }
}
