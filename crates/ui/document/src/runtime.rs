//! COM reconstruction and explicit authored snapshots, independent from file transport.
use crate::{*, build::Construction};
use neocogi_com::{
    binary, ComPtr, ComRef, IUnknown, NcStatus, NcGuid, NcBytes, INcBytesAbi, NcCopyFromBufferOffset, NC_E_INVALID_ARG, NC_E_NOT_FOUND, NC_E_ABI_VERSION,
    NC_E_OUT_OF_RANGE,
};
use neocogi_com_persistence::{FactoryRegistry, ObjectRecord, IPersistable, IPersistenceFactory, IObjectChildrenAbi};
use neocogi_com_events::{NamedOutputs, INamedOutputsAbi};
use neocogi_com_events_contracts::INamedOutputs;
use neocogi_ui::{Ui, Widget, IWidgetAbi, IWindowAbi, ITextureAbi};
use std::collections::BTreeMap;

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
fn limits() -> neocogi_com_persistence::Limits {
    neocogi_com_persistence::Limits {
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
    /// No globals or dynamic code selection can be driven by document bytes.
    registry: FactoryRegistry,
}
impl Loader {
    /// Binds services; parent is needed only for controls that own popups.
    pub fn new(ui: Ui, parent: Option<ComPtr<IWindowAbi>>, resources: Resources) -> Result<Self, NcStatus> {
        let mut registry = FactoryRegistry::new();
        registry.register(DOCUMENT_TYPE, Factory::from_value(Factory { ui, parent, resources }).query()?)?;
        Ok(Self { registry })
    }
    /// Validates authored values before producing an unmounted component.
    pub fn prepare(&self, document: &Document) -> Result<Component, String> {
        self.load(&document.to_binary()?).map_err(|error| error.to_string())
    }
    /// Loads either external-file bytes or include_bytes! data through the exact same path.
    pub fn load(&self, bytes: &[u8]) -> Result<Component, NcStatus> {
        let object = self.registry.restore_with_limits(bytes, limits())?;
        Ok(Component { interface: object.query()?, object })
    }
}

/// Application-facing opaque component; private child names are not an application API.
pub struct Component {
    /// Canonical owner retained for authored snapshots and optional capabilities.
    object: ComPtr<IUnknown>,
    /// Cached root/input projection avoids repeated discovery.
    interface: ComPtr<contracts::IUiComponentAbi>,
}
impl Component {
    /// Mounts the real root without exposing its internal concrete control types.
    pub fn root(&self) -> Result<Widget<IWidgetAbi>, NcStatus> {
        Ok(Widget::from_interface(self.interface.root()?))
    }
    /// The existing typed named-output binding API accepts this capability.
    pub fn outputs(&self) -> Result<ComPtr<INamedOutputsAbi>, NcStatus> {
        self.object.query()
    }
    /// Resolves a concrete input once and rejects an incompatible message contract immediately.
    pub fn input<C: Command>(&self, name: &str) -> Result<InputHandle<C>, NcStatus> {
        let receiver = self.interface.input(NcCopyFromBufferOffset::new(0, name.as_bytes()))?.query()?;
        Ok(InputHandle { receiver })
    }
    /// Saves authored defaults; preview interaction and application commands never rewrite them.
    pub fn save(&self) -> Result<Vec<u8>, NcStatus> {
        neocogi_com_persistence::snapshot_with_limits(self.object.as_com_ref(), limits())
    }
}

/// One persistent composition owns its initialization and the concrete binding capabilities.
#[neocogi_com::com_object(interfaces(contracts::IUiComponent, neocogi_com_events_contracts::INamedOutputs, neocogi_com_persistence::IPersistable))]
struct Loaded {
    /// Exact authored values; intentionally independent of live interactive state.
    document: Document,
    /// Real tree root, attached only when the host commits the candidate.
    root: Widget<IWidgetAbi>,
    /// Public output names resolve only during connection.
    outputs: NamedOutputs,
    /// Typed receivers retain only the nodes needed by public commands.
    inputs: BTreeMap<String, ComPtr<IUnknown>>,
}
impl Drop for Loaded {
    /// Retires a rejected unmounted candidate; normal mounted retirement is idempotent.
    fn drop(&mut self) {
        if let Ok(mut root) = self.root.node_interface() {
            let _ = root.close();
        }
    }
}
impl contracts::IUiComponent for Loaded {
    /// Shares the mountable widget identity without copying its state.
    fn root(&self) -> Result<ComPtr<IWidgetAbi>, NcStatus> {
        self.root.node_interface()
    }
    /// Uses checked names only during application connection.
    fn input(&self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<IUnknown>, NcStatus> {
        if name.offset() != 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let name = std::str::from_utf8(name.as_slice()).map_err(|_| NC_E_INVALID_ARG)?;
        self.inputs.get(name).cloned().ok_or(NC_E_NOT_FOUND)
    }
}
impl INamedOutputs for Loaded {
    /// Installs the exact typed destination selected by the public output declaration.
    fn bind_output(&mut self, name: NcCopyFromBufferOffset<'_, u8>, receiver: ComRef<'_, IUnknown>) -> Result<(), NcStatus> {
        self.outputs.bind_com(name, receiver)
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
    /// No separately published persistent child components are defined by revision one.
    fn child(&self, _: u32) -> Result<ComPtr<IUnknown>, NcStatus> {
        Err(NC_E_OUT_OF_RANGE)
    }
}

/// Factory services are supplied by the application and never decoded from a file.
#[neocogi_com::com_object(interfaces(neocogi_com_persistence::IPersistenceFactory))]
struct Factory {
    /// Runtime-bound widget construction authority.
    ui: Ui,
    /// Window owner for combo popups, absent for detached standalone forms.
    parent: Option<ComPtr<IWindowAbi>>,
    /// Supplied logical image map.
    resources: Resources,
}
/// Parses the exact initialization revision and validates the entire document.
fn decode(version: u32, state: NcCopyFromBufferOffset<'_, u8>, count: u32) -> Result<Document, NcStatus> {
    if version != VERSION {
        return Err(NC_E_ABI_VERSION);
    }
    if state.offset() != 0 || count != 0 {
        return Err(NC_E_INVALID_ARG);
    }
    let document: Document = binary::from_slice_with_limits(state.as_slice(), limits().binary).map_err(|_| NC_E_INVALID_ARG)?;
    document.validate().map_err(|_| NC_E_INVALID_ARG)?;
    Ok(document)
}
impl IPersistenceFactory for Factory {
    /// Pure preflight performs no widget construction or topology changes.
    fn validate(&self, version: u32, state: NcCopyFromBufferOffset<'_, u8>, children: u32) -> Result<(), NcStatus> {
        decode(version, state, children).map(|_| ())
    }
    /// Constructs one candidate and transfers ownership only after all bindings exist.
    fn restore(&self, version: u32, state: NcCopyFromBufferOffset<'_, u8>, children: ComRef<'_, IObjectChildrenAbi>) -> Result<ComPtr<IUnknown>, NcStatus> {
        let document = decode(version, state, children.count()?)?;
        let mut construction = Construction {
            ui: &self.ui,
            parent: self.parent.as_ref(),
            resources: &self.resources,
            nodes: BTreeMap::new(),
        };
        let root = construction.build(&document.root)?;
        let outputs = crate::bindings::outputs(&document, &construction.nodes)?;
        let inputs = crate::bindings::inputs(&document, &construction.nodes)?;
        // Success transfers tree ownership to Loaded; the construction guard no longer closes it.
        construction.nodes.clear();
        Ok(Loaded::from_value(Loaded { document, root, outputs, inputs }))
    }
}
