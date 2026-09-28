//! COM reconstruction and explicit authored snapshots, independent from file transport.
use crate::{*, build::Construction};
use neocogi_com::{
    binary, ComPtr, IUnknown, NcStatus, NcGuid, NcBytes, INcBytesAbi, NcCopyFromBufferOffset, NC_E_INVALID_ARG, NC_E_NOT_FOUND, NC_E_OUT_OF_RANGE,
};
use neocogi_com::persistence::{ObjectRecord, IPersistable};
use neocogi_ui::{Ui, Widget, IWidgetAbi, IWindowAbi, ITextureAbi};
use std::collections::BTreeMap;
use neocogi_ui::Mountable;

/// Recipe identity describes the authored component schema, never a runtime instance.
pub const DOCUMENT_TYPE: NcGuid = neocogi_com::guid_from_qualified_name("neocogi.ui.authored-component");

/// Caller-supplied renderer-compatible image capabilities, indexed by portable logical keys.
#[derive(Clone, Default)]
pub struct Resources {
    /// Resources are retained while loading and by widgets that actually use them.
    pub(crate) images: BTreeMap<String, ComPtr<ITextureAbi>>,
}
impl Resources {
    /// Starts with no external images; fonts and icons come from the UI asset bundle.
    pub fn new() -> Self {
        Self::default()
    }
    /// Adds an image without silently replacing a previous key.
    pub fn insert_image(&mut self, key: impl Into<String>, image: ComPtr<ITextureAbi>) -> Result<(), NcStatus> {
        let key = key.into();
        if key.is_empty() || self.images.contains_key(&key) {
            return Err(NC_E_INVALID_ARG);
        }
        self.images.insert(key, image);
        Ok(())
    }
}
/// Binary limits permit a complete typed document inside one COM initialization record.
fn limits() -> neocogi_com::persistence::Limits {
    neocogi_com::persistence::Limits {
        binary: binary::Limits {
            bytes: MAX_BYTES,
            elements: MAX_BYTES,
            depth: 192,
            values: 1_000_000,
        },
        objects: 1,
        depth: 1,
    }
}
impl Document {
    /// Wraps this typed initialization in the existing COM persistence envelope.
    pub fn to_binary(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let state = binary::to_vec_with_limits(self, limits().binary).map_err(|error| error.to_string())?;
        binary::to_vec_with_limits(
            &ObjectRecord {
                type_id: DOCUMENT_TYPE.to_uuid_bytes(),
                schema_version: VERSION,
                state,
                children: vec![],
            },
            limits().binary,
        )
        .map_err(|error| error.to_string())
    }
    /// Reads portable bytes independently of widget creation or renderer services.
    pub fn from_binary(bytes: &[u8]) -> Result<Self, String> {
        let record: ObjectRecord = binary::from_slice_with_limits(bytes, limits().binary).map_err(|error| error.to_string())?;
        if record.type_id != DOCUMENT_TYPE.to_uuid_bytes() || record.schema_version != VERSION || !record.children.is_empty() {
            return Err("Unsupported UI component recipe or revision".into());
        }
        let document: Self = binary::from_slice_with_limits(&record.state, limits().binary).map_err(|error| error.to_string())?;
        document.validate()?;
        Ok(document)
    }
}

/// Context-bound loader uses existing COM reconstruction and trusted widget factories.
pub struct Loader {
    /// Immutable construction capabilities; receivers belong to individual load attempts.
    services: Services,
}
impl Loader {
    /// Binds construction services; application receivers are supplied separately to each construction attempt.
    /// The parent window is needed only for controls that own popups.
    pub fn new(ui: Ui, parent: Option<ComPtr<IWindowAbi>>, resources: Resources) -> Result<Self, NcStatus> {
        Self::new_with_custom(ui, parent, resources, crate::CustomWidgetRegistry::new())
    }
    /// Binds trusted custom COM factories alongside the ordinary built-in constructors.
    pub fn new_with_custom(ui: Ui, parent: Option<ComPtr<IWindowAbi>>, resources: Resources, custom: crate::CustomWidgetRegistry) -> Result<Self, NcStatus> {
        Ok(Self {
            services: Services { ui, parent, resources, custom },
        })
    }
    /// Validates typed source and constructs directly, without a binary persistence round trip.
    pub fn compile(&self, document: &Document) -> Result<Component, String> {
        self.compile_with_events(document, EventBindings::new())
    }
    /// Validates all constructor receivers before creating the complete candidate tree.
    pub fn compile_with_events(&self, document: &Document, events: EventBindings) -> Result<Component, String> {
        self.services.custom.validate_document(document)?;
        let object = self.services.construct(document.clone(), events).map_err(|error| error.to_string())?;
        Ok(Component {
            interface: object.query().map_err(|error| error.to_string())?,
            object,
        })
    }
    /// Decodes external or embedded binary data before using the same typed construction routine.
    pub fn load(&self, bytes: &[u8]) -> Result<Component, NcStatus> {
        self.load_with_events(bytes, EventBindings::new())
    }
    /// Decodes persistence data separately from this attempt's live application receivers.
    pub fn load_with_events(&self, bytes: &[u8], events: EventBindings) -> Result<Component, NcStatus> {
        let document = Document::from_binary(bytes).map_err(|_| NC_E_INVALID_ARG)?;
        self.services.custom.validate_document(&document).map_err(|_| NC_E_INVALID_ARG)?;
        let object = self.services.construct(document, events)?;
        Ok(Component { interface: object.query()?, object })
    }
}

/// Compiled document recipe and typed lookups, independent of mounted widget lifetime.
/// Dropping this result releases its references. A retained root remains usable;
/// successful attachment transfers logical closure responsibility to the tree.
pub struct Component {
    /// Canonical owner retained for authored snapshots and optional capabilities.
    object: ComPtr<IUnknown>,
    /// Cached root/widget lookup projection avoids repeated discovery.
    interface: ComPtr<contracts::IUiComponentAbi>,
}
impl Component {
    /// Retains the actual root. The runtime owns closure after successful attachment.
    pub fn root(&self) -> Result<Widget<IWidgetAbi>, NcStatus> {
        Ok(Widget::from_interface(self.interface.root()?))
    }
    /// Resolves an authored child and checks its requested concrete COM interface.
    pub fn widget<I: neocogi_com::ComInterface>(&self, name: &str) -> Result<Widget<I>, NcStatus> {
        let base = self.interface.widget(NcCopyFromBufferOffset::new(0, name.as_bytes()))?;
        Ok(Widget::from_interface(base.query()?))
    }
    /// Saves authored defaults; preview interaction and widget setters never rewrite them.
    pub fn save(&self) -> Result<Vec<u8>, NcStatus> {
        neocogi_com::persistence::snapshot_with_limits(self.object.as_com_ref(), limits())
    }
}

impl Mountable for Component {
    /// Returns the actual root; the tree retains widgets without retaining this recipe.
    fn mount_widget(&self) -> Result<ComPtr<IWidgetAbi>, NcStatus> {
        self.interface.root()
    }
}

/// Authored recipe and typed lookup capabilities, with ordinary COM reference ownership.
/// This metadata is not a tree lifetime guard: releasing it must not close a root
/// retained by the runtime or by an application. Failed construction is closed
/// by Construction; rejected publication is closed explicitly by ReloadSlot.
#[neocogi_com::com_object(interfaces(contracts::IUiComponent, neocogi_com::persistence::IPersistable))]
struct Loaded {
    /// Exact authored values; intentionally independent of live interactive state.
    document: Document,
    /// Real tree root, attached only when the host commits the candidate.
    root: Widget<IWidgetAbi>,
    /// Authored widget keys retain each concrete object for checked application queries.
    nodes: BTreeMap<String, Widget<IWidgetAbi>>,
}
impl contracts::IUiComponent for Loaded {
    /// Shares the mountable widget identity without copying its state.
    fn root(&self) -> Result<ComPtr<IWidgetAbi>, NcStatus> {
        self.root.node_interface()
    }
    /// Resolves only widgets authored in this component; concrete type checks follow by QueryInterface.
    fn widget(&self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<IWidgetAbi>, NcStatus> {
        if name.offset() != 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let name = std::str::from_utf8(name.as_slice()).map_err(|_| NC_E_INVALID_ARG)?;
        self.nodes.get(name).ok_or(NC_E_NOT_FOUND)?.node_interface()
    }
}
impl IPersistable for Loaded {
    /// Identifies the explicit authored-component factory.
    fn type_id(&self) -> Result<NcGuid, NcStatus> {
        Ok(DOCUMENT_TYPE)
    }
    /// Reject incompatible schema revisions rather than silently reinterpreting bytes.
    fn schema_version(&self) -> Result<u32, NcStatus> {
        Ok(VERSION)
    }
    /// Saves typed constructor data instead of transient widget memory.
    fn save_state(&self) -> Result<ComPtr<INcBytesAbi>, NcStatus> {
        Ok(NcBytes::from_vec(
            binary::to_vec_with_limits(&self.document, limits().binary).map_err(|_| NC_E_INVALID_ARG)?,
        ))
    }
    /// The complete private tree belongs to this component's typed initialization.
    fn child_count(&self) -> Result<u32, NcStatus> {
        Ok(0)
    }
    /// No separately published persistent child components are defined by this schema.
    fn child(&self, _: u32) -> Result<ComPtr<IUnknown>, NcStatus> {
        Err(NC_E_OUT_OF_RANGE)
    }
}

/// One concrete construction context serves in-memory editing and persisted binary loading.
struct Services {
    /// Runtime-bound widget construction authority.
    ui: Ui,
    /// Window owner for combo popups, absent for detached standalone forms.
    parent: Option<ComPtr<IWindowAbi>>,
    /// Supplied logical image map.
    resources: Resources,
    /// Trusted custom widget construction authority, separate from serialized document data.
    custom: crate::CustomWidgetRegistry,
}
impl Services {
    /// Builds the checked widget tree with one cleanup guard until ownership transfers.
    fn construct(&self, document: Document, events: EventBindings) -> Result<ComPtr<IUnknown>, NcStatus> {
        events.validate(&document, &self.custom)?;
        let mut construction = Construction {
            ui: &self.ui,
            custom: &self.custom,
            parent: self.parent.as_ref(),
            resources: &self.resources,
            nodes: BTreeMap::new(),
            events,
        };
        let root = construction.build(&document.root)?;
        // Success transfers the complete typed tree to Loaded; the failed-build guard is disarmed.
        let nodes = std::mem::take(&mut construction.nodes);
        drop(construction);
        Ok(Loaded::from_value(Loaded {
            document,
            root,
            nodes,
        }))
    }
}

/// Validates binary persistence limits through the public document API.
#[cfg(test)]
mod tests;
