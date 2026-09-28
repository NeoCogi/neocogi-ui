//! COM reconstruction and explicit authored snapshots, independent from file transport.
use crate::{*, build::Construction};
use neocogi_com::{
    binary, ComPtr, ComRef, IUnknown, NcStatus, NcGuid, NcBytes, INcBytesAbi, NcCopyFromBufferOffset, NC_E_INVALID_ARG, NC_E_NOT_FOUND, NC_E_OUT_OF_RANGE,
};
use neocogi_com::persistence::{ObjectRecord, IPersistable};
use neocogi_ui::{Ui, Widget, IWidgetAbi, IWindowAbi, ITextureAbi};
use std::collections::BTreeMap;
use neocogi_ui::{IEventOwner, IEventOwnerAbi, IEventOwnerVtblFor, Mount, Mountable};

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
    pub fn prepare(&self, document: &Document) -> Result<Component, String> {
        self.prepare_with_events(document, EventBindings::new())
    }
    /// Validates all constructor receivers before creating the complete candidate tree.
    pub fn prepare_with_events(&self, document: &Document, events: EventBindings) -> Result<Component, String> {
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

/// Application-facing component that resolves authored widget keys to checked typed handles.
pub struct Component {
    /// Canonical owner retained for authored snapshots and optional capabilities.
    object: ComPtr<IUnknown>,
    /// Cached root/input projection avoids repeated discovery.
    interface: ComPtr<contracts::IUiComponentAbi>,
}
impl Component {
    /// Returns the root for inspection. Mount the Component itself to retain its event owner.
    pub fn root(&self) -> Result<Widget<IWidgetAbi>, NcStatus> {
        Ok(Widget::from_interface(self.interface.root()?))
    }
    /// Resolves an authored child and checks its requested concrete COM interface.
    pub fn widget<I: neocogi_com::ComInterface>(&self, name: &str) -> Result<Widget<I>, NcStatus> {
        let base = self.interface.widget(NcCopyFromBufferOffset::new(0, name.as_bytes()))?;
        Ok(Widget::from_interface(base.query()?))
    }
    /// Resolves a concrete input once and rejects an incompatible message contract immediately.
    pub fn input<C: Command>(&self, name: &str) -> Result<InputHandle<C>, NcStatus> {
        let receiver = self.interface.input(NcCopyFromBufferOffset::new(0, name.as_bytes()))?.query()?;
        Ok(InputHandle { receiver })
    }
    /// Saves authored defaults; preview interaction and application commands never rewrite them.
    pub fn save(&self) -> Result<Vec<u8>, NcStatus> {
        neocogi_com::persistence::snapshot_with_limits(self.object.as_com_ref(), limits())
    }
}

impl Mountable for Component {
    /// Attaches the view and retains its publication/retirement guard for the subtree lifetime.
    fn mount_description(&self) -> Result<Mount, NcStatus> {
        Ok(Mount::Component {
            view: self.interface.root()?,
            owner: self.object.query()?,
        })
    }
}

/// One persistent composition owns its initialization and the concrete binding capabilities.
#[neocogi_com::com_object(interfaces(contracts::IUiComponent, neocogi_com::persistence::IPersistable, IEventOwner))]
struct Loaded {
    /// Exact authored values; intentionally independent of live interactive state.
    document: Document,
    /// Retains construction cleanup authority until the runtime claims the subtree.
    phase: LoadedPhase,
    /// Real tree root, attached only when the host commits the candidate.
    root: Widget<IWidgetAbi>,
    /// Authored widget keys retain each concrete object for checked application queries.
    nodes: BTreeMap<String, Widget<IWidgetAbi>>,
    /// Typed receivers retain only the nodes needed by public commands.
    inputs: BTreeMap<String, ComPtr<IUnknown>>,
}
/// Publication lifecycle only; the application owns all event queues and handlers.
enum LoadedPhase {
    /// Failed candidates must close every resource they created.
    Prepared,
    /// The runtime owns subtree retirement and retains this metadata allocation.
    Mounted,
    /// Logical subtree removal is permanent even when clients retain handles.
    Retired,
}
impl IEventOwner for Loaded {
    /// Keeps the recipe and cleanup guard alive for the mounted subtree, without an event relay.
    fn mount(&mut self, mut view: ComRef<'_, IWidgetAbi>) -> Result<(), NcStatus> {
        match self.phase {
            LoadedPhase::Prepared => {
                // Reject a different view before transferring this candidate's lifetime.
                if view.add_ref().query::<IUnknown>()?.as_raw().object != self.root.node_interface()?.query::<IUnknown>()?.as_raw().object {
                    return Err(neocogi_ui::UI_E_OWNER);
                }
                view.mount()?;
                self.phase = LoadedPhase::Mounted;
                Ok(())
            }
            LoadedPhase::Mounted => Err(neocogi_ui::UI_E_OWNER),
            LoadedPhase::Retired => Err(neocogi_ui::UI_E_CLOSED),
        }
    }
    /// There is no intermediate inbox: widgets deliver directly into the application's queue.
    fn dispatch_next(&mut self) -> Result<bool, NcStatus> {
        match self.phase {
            LoadedPhase::Mounted => Ok(false),
            LoadedPhase::Prepared => Err(neocogi_ui::UI_E_OWNER),
            LoadedPhase::Retired => Err(neocogi_ui::UI_E_CLOSED),
        }
    }
    /// Records the runtime's irreversible retirement without consuming application messages.
    fn retire(&mut self) -> Result<(), NcStatus> {
        self.phase = LoadedPhase::Retired;
        Ok(())
    }
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
    /// Resolves only widgets authored in this component; concrete type checks follow by QueryInterface.
    fn widget(&self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<IWidgetAbi>, NcStatus> {
        if name.offset() != 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let name = std::str::from_utf8(name.as_slice()).map_err(|_| NC_E_INVALID_ARG)?;
        self.nodes.get(name).ok_or(NC_E_NOT_FOUND)?.node_interface()
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
    /// Builds the checked tree and concrete endpoints with one cleanup guard until ownership transfers.
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
        let inputs = crate::bindings::inputs(&document, &construction.nodes)?;
        // Success transfers the complete typed tree to Loaded; the failed-build guard is disarmed.
        let nodes = std::mem::take(&mut construction.nodes);
        drop(construction);
        Ok(Loaded::from_value(Loaded {
            document,
            root,
            inputs,
            nodes,
            phase: LoadedPhase::Prepared,
        }))
    }
}

/// Validates binary persistence limits through the public document API.
#[cfg(test)]
mod tests;
