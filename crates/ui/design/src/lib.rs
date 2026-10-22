//! Owned authoring metadata copied from generated IDL or a plugin COM catalog.
//!
//! The JSON encoding is a versioned payload of `IDesignWidgetCatalog::describe`.
//! It crosses COM as bytes, then is parsed and validated before the designer
//! exposes a widget kind or calls its generic constructor. Rust static pointers
//! and descriptor references never cross the plugin boundary.
#![deny(missing_docs)]

pub mod bounded_json;

use neocogi_com::idl_metadata::{InterfaceMetadata, MethodType, TypeShape};
use neocogi_ui_core_contracts::{EventSourceMetadata, IEventAbi};
use serde::{Deserialize, Serialize};
use neocogi_com::{ComSerialize, ComDeserialize};

/// Maximum encoded bytes accepted for one plugin-provided widget description.
pub const MAX_DESIGN_DESCRIPTOR_BYTES: usize = 64 * 1024;
/// Maximum NCB bytes for one plugin-owned settings snapshot. The plugin owns
/// its persistent schema (including its revision) and semantic validation.
pub const MAX_SETTINGS_BYTES: usize = 64 * 1024;

/// Rejects excessive opaque settings before cloning, transport, or plugin calls.
/// Only the provider can validate the concrete settings type and its revision.
pub fn validate_settings_size(settings: &[u8]) -> Result<(), String> {
    match settings.len() <= MAX_SETTINGS_BYTES {
        true => Ok(()),
        false => Err("Custom widget settings exceed byte limit".into()),
    }
}

/// Shared symmetric NCB policy for plugin settings; other budgets retain COM defaults.
fn settings_limits() -> neocogi_com::binary::Limits {
    neocogi_com::binary::Limits {
        bytes: MAX_SETTINGS_BYTES,
        ..Default::default()
    }
}

/// Encodes a provider's concrete persistent settings without a reflective property model.
pub fn encode_settings<T: ComSerialize + ?Sized>(settings: &T) -> Result<Vec<u8>, String> {
    neocogi_com::binary::to_vec_with_limits(settings, settings_limits()).map_err(|error| error.to_string())
}

/// Decodes a provider's concrete settings with the encoder's exact limits.
/// Providers must additionally reject unsupported revisions and invalid values.
pub fn decode_settings<T: ComDeserialize>(bytes: &[u8]) -> Result<T, String> {
    validate_settings_size(bytes)?;
    neocogi_com::binary::from_slice_with_limits(bytes, settings_limits()).map_err(|error| error.to_string())
}

/// Revision 12 describes concrete COM event interfaces and their read-only getters.
/// Provider-owned NCB settings retain their independent schema.
pub const DESIGN_DESCRIPTOR_VERSION: u32 = 12;

/// Owned structural field expression copied through the COM design catalog.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum DesignValueType {
    /// Empty value.
    Unit,
    /// Built-in scalar and its exact width.
    Scalar(String),
    /// Owned UTF-8 text.
    Text,
    /// Canonical edge to one entry in the finite value-definition table.
    Named(String),
    /// Explicit owned COM capability; its interface is inspected separately.
    Object(String),
    /// Shared weak capability; displaying or copying it must not imply a strong owner.
    WeakObject(String),
    /// Exclusive weak capability; does not retain the widget or hold a borrow.
    WeakBoxObject(String),
    /// Explicitly indirect sequence of recursive values.
    Sequence(Box<DesignValueType>),
    /// Inline fixed-size array.
    Array(Box<DesignValueType>, u32),
}
impl From<&neocogi_com::idl_metadata::ValueType> for DesignValueType {
    /// Copies structure without recursively expanding named definitions.
    fn from(source: &neocogi_com::idl_metadata::ValueType) -> Self {
        use neocogi_com::idl_metadata::ValueType as I;
        match source {
            I::Unit => Self::Unit,
            I::Text => Self::Text,
            I::Scalar(name) => Self::Scalar((*name).into()),
            I::Named(name) => Self::Named((*name).into()),
            I::Object(name) => Self::Object((*name).into()),
            I::WeakObject(name) => Self::WeakObject((*name).into()),
            I::WeakBoxObject(name) => Self::WeakBoxObject((*name).into()),
            I::Sequence(value) => Self::Sequence(Box::new((*value).into())),
            I::Array(value, len) => Self::Array(Box::new((*value).into()), *len),
        }
    }
}
impl std::fmt::Display for DesignValueType {
    /// Presents readable type spelling without making strings the metadata model.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unit => formatter.write_str("()"),
            Self::Text => formatter.write_str("String"),
            Self::Scalar(name) | Self::Named(name) | Self::Object(name) => formatter.write_str(name),
            Self::WeakObject(name) => write!(formatter, "WeakComPtr<{name}>"),
            Self::WeakBoxObject(name) => write!(formatter, "WeakComBox<{name}>"),
            Self::Sequence(value) => write!(formatter, "Vec<{value}>"),
            Self::Array(value, len) => write!(formatter, "[{value}; {len}]"),
        }
    }
}

/// One source-level field, including the exact IDL type spelling.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesignField {
    /// Authored name or decimal tuple position.
    pub name: String,
    /// Authored type expression.
    pub ty: DesignValueType,
}

/// One tagged enum case and its ordered fields.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesignVariant {
    /// Authored variant name.
    pub name: String,
    /// Exact declared integer discriminant.
    pub tag: i64,
    /// Complete payload field list.
    pub fields: Vec<DesignField>,
}

/// One named bit flag and its explicit value.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesignFlag {
    /// Authored flag name.
    pub name: String,
    /// Nonnegative wire value.
    pub value: u64,
}

/// Complete source-level aggregate shape, independent of Rust memory layout.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum DesignShape {
    /// Struct fields in declaration order.
    Struct(Vec<DesignField>),
    /// Tagged cases in declaration order.
    Enum(Vec<DesignVariant>),
    /// Bit flags in declaration order.
    BitEnum(Vec<DesignFlag>),
}

/// One generated IDL payload, including its application Rust import path.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesignType {
    /// Dotted IDL module and declaration name.
    pub qualified_name: String,
    /// Public Rust type path used in generated handlers; checked by [`neocogi_com::validate_rust_path`].
    pub rust_type: String,
    /// Complete struct, enum, or bit-enum shape.
    pub shape: DesignShape,
}

impl From<&neocogi_com::idl_metadata::TypeMetadata> for DesignType {
    /// Copies a single named definition; recursive named edges remain references.
    fn from(source: &neocogi_com::idl_metadata::TypeMetadata) -> Self {
        fn fields(values: &[neocogi_com::idl_metadata::FieldMetadata]) -> Vec<DesignField> {
            values
                .iter()
                .map(|field| DesignField {
                    name: field.name.into(),
                    ty: (&field.ty).into(),
                })
                .collect()
        }
        Self {
            qualified_name: source.qualified_name.into(),
            rust_type: source.rust_type.into(),
            shape: match source.shape {
                TypeShape::Struct(values) => DesignShape::Struct(fields(values)),
                TypeShape::Enum(values) => DesignShape::Enum(
                    values
                        .iter()
                        .map(|value| DesignVariant {
                            name: value.name.into(),
                            tag: value.tag,
                            fields: fields(value.fields),
                        })
                        .collect(),
                ),
                TypeShape::BitEnum(values) => DesignShape::BitEnum(
                    values
                        .iter()
                        .map(|value| DesignFlag {
                            name: value.name.into(),
                            value: value.value,
                        })
                        .collect(),
                ),
            },
        }
    }
}

/// One emitted COM interface, whose getters describe the immutable event snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesignEvent {
    /// Canonical IDL name of the concrete event interface.
    pub qualified_name: String,
    /// Interface identity queried on the owned IEvent value.
    pub iid: [u8; 16],
    /// Full transitive event ABI signature, including inherited getters.
    pub signature: [u8; 32],
    /// Zero-argument read-only getters in declaration order, including bases.
    pub fields: Vec<DesignField>,
}
impl DesignEvent {
    /// Admits an ordinary interface as a UI snapshot and derives its inherited getter fields.
    pub fn from_idl(source: &InterfaceMetadata) -> Result<Self, String> {
        // Typed local identities preserve exact contract bytes without raw borrowing.
        if !source.chain.contains(&IEventAbi::IDL_INTERFACE.identity) {
            return Err("UI snapshot must derive from neocogi.ui.core.IEvent".into());
        }
        let mut fields = Vec::new();
        let mut names = std::collections::BTreeSet::new();
        for method in source.methods {
            let MethodType::Value(ty) = method.returns else {
                return Err(format!("Event getter {} must return a repeatable value", method.name));
            };
            if method.receiver != neocogi_com::NcAbiReceiver::Shared || !method.parameters.is_empty() {
                return Err(format!("Event getter {} must be immutable and parameterless", method.name));
            }
            if !names.insert(method.name) {
                return Err(format!("Event getter {} is ambiguous across inherited interfaces", method.name));
            }
            fields.push(DesignField {
                name: method.name.into(),
                ty: (&ty).into(),
            });
        }
        Ok(Self {
            qualified_name: source.qualified_name.into(),
            iid: source.identity.iid.to_uuid_bytes(),
            signature: *source.identity.abi_signature,
            fields,
        })
    }
}

/// Exact widget interface identity and its authoring-level output contracts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesignInterface {
    /// Canonical UUID bytes of the concrete COM projection.
    pub iid: [u8; 16],
    /// Full transitive ABI signature of that projection.
    pub signature: [u8; 32],
    /// Public Rust `*Abi` projection used by generated forms; checked by [`neocogi_com::validate_rust_path`].
    pub rust_abi: String,
    /// Concrete event interfaces advertised by this widget.
    pub events: Vec<DesignEvent>,
    /// Complete finite value graph reachable from the widget and event getters.
    pub values: Vec<DesignType>,
}
/// One constructible widget kind and its provider-owned constructor defaults.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesignWidgetManifest {
    /// Version of this owned authoring descriptor schema.
    pub version: u32,
    /// Stable kind used in authored UI documents.
    pub kind: String,
    /// Opaque NCB defaults; only the provider knows their type and schema revision.
    pub defaults: Vec<u8>,
    /// Exact COM and IDL metadata for the constructed widget.
    pub interface: DesignInterface,
}

impl DesignWidgetManifest {
    /// Validates UI event publication against ordinary IDL methods and copies an owned catalog.
    /// Invalid snapshot contracts fail before publication; no producer pointers escape.
    pub fn from_source(kind: impl Into<String>, defaults: Vec<u8>, publication: &EventSourceMetadata) -> Result<Self, String> {
        let source = publication.interface;
        let primary = source.chain.first().ok_or("IDL interface has no COM chain")?;
        if *primary != source.identity {
            return Err("IDL interface has an inconsistent COM chain".into());
        }
        let events = publication
            .events
            .iter()
            .map(|event| DesignEvent::from_idl(event))
            .collect::<Result<Vec<_>, _>>()?;
        // Prefer producer-local Rust projections, then each event's supplying projection.
        // Canonical shapes must agree even when equivalent generic paths differ.
        let mut values: Vec<DesignType> = Vec::new();
        for definition in core::iter::once(source)
            .chain(publication.events.iter().copied())
            .flat_map(|interface| interface.values)
        {
            let value = DesignType::from(definition);
            if let Some(existing) = values.iter().find(|existing| existing.qualified_name == value.qualified_name) {
                if existing.shape != value.shape {
                    return Err(format!("Conflicting value definition {}", value.qualified_name));
                }
            } else {
                values.push(value);
            }
        }
        let manifest = Self {
            version: DESIGN_DESCRIPTOR_VERSION,
            kind: kind.into(),
            defaults,
            interface: DesignInterface {
                iid: source.identity.iid.to_uuid_bytes(),
                signature: *source.identity.abi_signature,
                rust_abi: source.rust_abi.to_owned(),
                events,
                values,
            },
        };
        manifest.validate()?;
        Ok(manifest)
    }

    /// Rejects malformed metadata before it can affect construction or code generation.
    /// COM owns Rust binding syntax; graph names and references are checked separately.
    /// Failures in binding strings include the widget kind and exact metadata field.
    pub fn validate(&self) -> Result<(), String> {
        if self.version != DESIGN_DESCRIPTOR_VERSION || self.kind.is_empty() || self.interface.iid == [0; 16] {
            return Err("Invalid design widget descriptor identity or version".into());
        }
        neocogi_com::validate_rust_path(&self.interface.rust_abi).map_err(|error| format!("Widget {:?}, interface.rust_abi: {error}", self.kind))?;
        validate_settings_size(&self.defaults)?;
        let mut names = std::collections::BTreeSet::new();
        let mut identities = std::collections::BTreeSet::new();
        if self.interface.events.len() > 1024 {
            return Err("Too many event interfaces".into());
        }
        for event in &self.interface.events {
            if !qualified_name(&event.qualified_name)
                || event.iid == [0; 16]
                || event.signature == [0; 32]
                || !names.insert(&event.qualified_name)
                || !identities.insert(event.iid)
            {
                return Err("Invalid or duplicate event interface".into());
            }
        }
        validate_value_graph(&self.interface.values, &self.interface.events).map_err(|error| format!("Widget {:?}: {error}", self.kind))?;
        Ok(())
    }

    /// Serializes a validated descriptor for one synchronous COM catalog read.
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        // Validate recursive type expressions before entering serde's recursive
        // traversal. Then reject oversize output without materializing a buffer.
        self.validate()?;
        bounded_json::encoded_len(self, MAX_DESIGN_DESCRIPTOR_BYTES, bounded_json::Format::Compact)?;
        bounded_json::to_vec(self, MAX_DESIGN_DESCRIPTOR_BYTES, bounded_json::Format::Compact)
    }

    /// Decodes one bounded, owned descriptor copied through COM.
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_DESIGN_DESCRIPTOR_BYTES {
            return Err("Design widget descriptor exceeds byte limit".into());
        }
        let manifest: Self = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        manifest.validate()?;
        Ok(manifest)
    }
}

/// Checks canonical logical names without accepting source-code fragments.
fn qualified_name(name: &str) -> bool {
    !name.is_empty() && name.split('.').all(identifier)
}

/// Validates a bounded finite graph, including references and inline layout cycles.
fn validate_value_graph(values: &[DesignType], events: &[DesignEvent]) -> Result<(), String> {
    use std::collections::{BTreeMap, BTreeSet};
    if values.len() > 1024 {
        return Err("Too many value definitions".into());
    }
    let mut definitions = BTreeMap::new();
    for (index, value) in values.iter().enumerate() {
        // Logical graph names and Rust source spellings have different grammars.
        // Check every reachable definition, including types nested in the event.
        neocogi_com::validate_rust_path(&value.rust_type)
            .map_err(|error| format!("interface.values[{index}].rust_type ({:?}): {error}", value.qualified_name))?;
        if !qualified_name(&value.qualified_name) || definitions.insert(value.qualified_name.as_str(), value).is_some() {
            return Err("Invalid or duplicate value definition".into());
        }
    }
    /// Field expressions are bounded independently of named graph traversal.
    fn field(ty: &DesignValueType, definitions: &BTreeMap<&str, &DesignType>, inline: bool, edges: &mut BTreeSet<String>, depth: usize) -> Result<(), String> {
        if depth == 0 {
            return Err("Value expression exceeds nesting limit".into());
        }
        match ty {
            DesignValueType::Unit | DesignValueType::Text => {}
            DesignValueType::Scalar(name) => {
                if !matches!(
                    name.as_str(),
                    "bool" | "u8" | "u16" | "u32" | "u64" | "usize" | "i8" | "i16" | "i32" | "i64" | "isize" | "f32" | "f64" | "Guid" | "Status"
                ) {
                    return Err("Unknown scalar definition".into());
                }
            }
            DesignValueType::Object(name) | DesignValueType::WeakObject(name) | DesignValueType::WeakBoxObject(name) => {
                if !qualified_name(name) {
                    return Err("Invalid object identity".into());
                }
            }
            DesignValueType::Named(name) => {
                if !definitions.contains_key(name.as_str()) {
                    return Err(format!("Missing value definition {name}"));
                }
                if inline {
                    edges.insert(name.clone());
                }
            }
            DesignValueType::Sequence(value) => field(value, definitions, false, edges, depth - 1)?,
            DesignValueType::Array(value, _) => field(value, definitions, inline, edges, depth - 1)?,
        }
        Ok(())
    }
    /// Names distinguish ordered tuple positions from ordinary named record fields.
    fn fields(values: &[DesignField], definitions: &BTreeMap<&str, &DesignType>, edges: &mut BTreeSet<String>, remaining: &mut usize) -> Result<(), String> {
        *remaining = remaining.checked_sub(values.len()).ok_or("Too many value fields")?;
        let mut names = BTreeSet::new();
        let tuple = values.iter().enumerate().all(|(index, value)| value.name == index.to_string());
        for value in values {
            if (!tuple && !identifier(&value.name)) || !names.insert(&value.name) {
                return Err("Invalid or duplicate field name".into());
            }
            field(&value.ty, definitions, true, edges, 64)?;
        }
        Ok(())
    }
    let mut pending = BTreeMap::new();
    let mut remaining = 4096;
    for event in events {
        // Getter names are identifiers; tuple positions only apply to value fields.
        if event.fields.iter().any(|field| !identifier(&field.name)) {
            return Err("Invalid event getter name".into());
        }
        fields(&event.fields, &definitions, &mut BTreeSet::new(), &mut remaining)?;
    }
    for value in values {
        let mut edges = BTreeSet::new();
        match &value.shape {
            DesignShape::Struct(values) => fields(values, &definitions, &mut edges, &mut remaining)?,
            DesignShape::Enum(variants) => {
                let mut names = BTreeSet::new();
                let mut tags = BTreeSet::new();
                if variants.is_empty() {
                    return Err("Empty enum definition".into());
                }
                for variant in variants {
                    if !identifier(&variant.name) || !names.insert(&variant.name) || !tags.insert(variant.tag) {
                        return Err("Invalid or duplicate enum case".into());
                    }
                    fields(&variant.fields, &definitions, &mut edges, &mut remaining)?;
                }
            }
            DesignShape::BitEnum(flags) => {
                let mut names = BTreeSet::new();
                if !flags.iter().all(|flag| identifier(&flag.name) && names.insert(&flag.name)) {
                    return Err("Invalid or duplicate bit flag".into());
                }
            }
        }
        pending.insert(value.qualified_name.clone(), edges);
    }
    // Iterative topological removal avoids recursion on producer-supplied names.
    while !pending.is_empty() {
        let leaves = pending
            .iter()
            .filter(|(_, edges)| edges.is_empty())
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        if leaves.is_empty() {
            return Err("Recursive inline value layout".into());
        }
        for name in &leaves {
            pending.remove(name);
        }
        for edges in pending.values_mut() {
            for name in &leaves {
                edges.remove(name);
            }
        }
    }
    Ok(())
}

/// Checks one logical IDL graph name, independently of Rust binding spelling.
/// These names are not interpolated source; keyword rules apply only to binding paths.
fn identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some('a'..='z' | 'A'..='Z' | '_')) && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Transport, display and catalog validation must retain weak field ownership.
    #[test]
    fn weak_capability_metadata_round_trips_without_becoming_a_strong_object() {
        let weak = DesignValueType::from(&neocogi_com::idl_metadata::ValueType::WeakObject("example.Service"));
        assert_eq!(weak.to_string(), "WeakComPtr<example.Service>");
        assert_ne!(weak, DesignValueType::Object("example.Service".into()));
        let json = serde_json::to_string(&weak).unwrap();
        assert_eq!(serde_json::from_str::<DesignValueType>(&json).unwrap(), weak);
        let widget = DesignValueType::from(&neocogi_com::idl_metadata::ValueType::WeakBoxObject("example.Widget"));
        assert_eq!(widget.to_string(), "WeakComBox<example.Widget>");
        assert_ne!(widget, DesignValueType::WeakObject("example.Widget".into()));
        let json = serde_json::to_string(&widget).unwrap();
        assert_eq!(serde_json::from_str::<DesignValueType>(&json).unwrap(), widget);
        let mut manifest = counter_manifest();
        let DesignShape::Enum(variants) = &mut manifest.interface.values[0].shape else {
            panic!("event enum")
        };
        variants[0].fields.push(DesignField { name: "source".into(), ty: weak });
        variants[0].fields.push(DesignField { name: "widget".into(), ty: widget });
        manifest.validate().unwrap();
        manifest.version = 10;
        assert!(manifest.validate().is_err(), "the old catalog cannot describe exclusive weak ownership");
    }

    /// Supplies one complete stream for validation-boundary tests.
    fn counter_manifest() -> DesignWidgetManifest {
        let mut manifest = DesignWidgetManifest {
            version: DESIGN_DESCRIPTOR_VERSION,
            kind: "example.counter".into(),
            defaults: encode_settings(&0i32).unwrap(),
            interface: DesignInterface {
                iid: [1; 16],
                signature: [2; 32],
                rust_abi: "example_contracts::CounterAbi".into(),
                values: vec![],
                events: vec![DesignEvent {
                    qualified_name: "example.ICounterEvent".into(),
                    iid: [2; 16],
                    signature: [3; 32],
                    fields: vec![DesignField {
                        name: "value".into(),
                        ty: DesignValueType::Named("example.CounterEvent".into()),
                    }],
                }],
            },
        };
        manifest.interface.values.push(DesignType {
            qualified_name: "example.CounterEvent".into(),
            rust_type: "example_contracts::CounterEvent".into(),
            shape: DesignShape::Enum(vec![DesignVariant {
                name: "Activated".into(),
                tag: 0,
                fields: vec![],
            }]),
        });
        manifest
    }

    /// Event identities and getter graph edges are checked before editor use.
    #[test]
    fn event_interfaces_validate_identities_and_getters() {
        let valid = counter_manifest();
        for corrupt in 0..7 {
            let mut manifest = valid.clone();
            match corrupt {
                0 => manifest.interface.events[0].qualified_name = "bad::event".into(),
                1 => manifest.interface.events[0].iid = [0; 16],
                2 => manifest.interface.events[0].signature = [0; 32],
                3 => manifest.interface.events.push(manifest.interface.events[0].clone()),
                4 => manifest.interface.events[0].fields[0].name = "0".into(),
                5 => {
                    let field = manifest.interface.events[0].fields[0].clone();
                    manifest.interface.events[0].fields.push(field);
                }
                6 => manifest.interface.events[0].fields[0].ty = DesignValueType::Named("example.Missing".into()),
                _ => unreachable!(),
            }
            assert!(manifest.encode().is_err(), "corruption {corrupt}");
            assert!(DesignWidgetManifest::decode(&serde_json::to_vec(&manifest).unwrap()).is_err());
        }
    }

    /// A copied catalog description retains typed metadata and checks defaults.
    #[test]
    fn manifest_round_trip_and_validation_guard_generated_bindings() {
        let manifest = counter_manifest();
        assert_eq!(DesignWidgetManifest::decode(&manifest.encode().unwrap()).unwrap(), manifest);
        let mut invalid = manifest.clone();
        invalid.version += 1;
        assert!(invalid.encode().is_err());
        invalid = manifest.clone();
        invalid.interface.rust_abi = "example_contracts::CounterAbi; panic!()".into();
        assert!(invalid.encode().is_err());
        invalid = manifest.clone();
        invalid.defaults.resize(MAX_SETTINGS_BYTES + 1, 0);
        assert!(invalid.encode().is_err());
    }
    /// Producers and COM consumers must apply the same complete binding-path grammar.
    #[test]
    fn rust_binding_paths_share_com_policy_and_report_the_metadata_field() {
        for prefix in ["example_contracts", "example_contracts::r#type", "::example_contracts::r#gen"] {
            let mut manifest = counter_manifest();
            manifest.interface.rust_abi = format!("{prefix}::CounterAbi");
            manifest.interface.values[0].rust_type = format!("{prefix}::CounterEvent");
            assert_eq!(DesignWidgetManifest::decode(&manifest.encode().unwrap()).unwrap(), manifest);
        }
        for path in [
            "api::type::Event",
            "api::match",
            "api::async",
            "api::gen",
            "api::",
            "api::::Event",
            "crate::Event",
            "api::r#self",
            "api::Event<u32>",
            "api::Event; panic!()",
            "api::Évent",
            "api ::Event",
            &"a".repeat(neocogi_com::NC_DESCRIPTOR_RUST_PATH_MAX_LEN + 1),
        ] {
            let reason = neocogi_com::validate_rust_path(path).unwrap_err().to_string();
            for (field, is_interface) in [("interface.rust_abi", true), ("interface.values[0].rust_type", false)] {
                let mut manifest = counter_manifest();
                match is_interface {
                    true => manifest.interface.rust_abi = path.into(),
                    false => manifest.interface.values[0].rust_type = path.into(),
                }
                let error = manifest.encode().unwrap_err();
                assert!(error.contains("example.counter") && error.contains(field) && error.contains(&reason), "{error}");
                // An external producer can bypass encode, so decoding must enforce
                // the identical rule on structurally valid, owned JSON storage.
                let forged = serde_json::to_vec(&manifest).unwrap();
                assert_eq!(DesignWidgetManifest::decode(&forged).unwrap_err(), error);
            }
        }
    }

    /// Logical names are graph identities; only explicit Rust bindings become source.
    #[test]
    fn logical_names_do_not_use_rust_keyword_rules() {
        let mut manifest = counter_manifest();
        manifest.interface.events[0].fields[0].ty = DesignValueType::Named("example.type".into());
        manifest.interface.values[0].qualified_name = "example.type".into();
        manifest.interface.values[0].rust_type = "example_contracts::r#type".into();
        manifest.validate().unwrap();
    }

    /// Recursive named edges are finite metadata, while inline cycles are invalid layouts.
    #[test]
    fn recursive_graph_requires_complete_definitions_and_real_indirection() {
        let tree = DesignType {
            qualified_name: "example.Tree".into(),
            rust_type: "example_contracts::Tree".into(),
            shape: DesignShape::Struct(vec![DesignField {
                name: "children".into(),
                ty: DesignValueType::Sequence(Box::new(DesignValueType::Named("example.Tree".into()))),
            }]),
        };
        assert!(validate_value_graph(&[tree.clone()], &[]).is_ok());
        let encoded = serde_json::to_vec(&tree).unwrap();
        assert_eq!(serde_json::from_slice::<DesignType>(&encoded).unwrap(), tree);
        let mut invalid = tree.clone();
        let DesignShape::Struct(fields) = &mut invalid.shape else { unreachable!() };
        fields[0].ty = DesignValueType::Named("example.OptionalTree".into());
        // Optional storage is an ordinary enum edge. Its active payload is
        // inline, so the Tree -> Option<Tree> -> Tree cycle remains invalid.
        let optional = DesignType {
            qualified_name: "example.OptionalTree".into(),
            rust_type: "example_contracts::OptionalTree".into(),
            shape: DesignShape::Enum(vec![
                DesignVariant {
                    name: "None".into(),
                    tag: 0,
                    fields: vec![],
                },
                DesignVariant {
                    name: "Some".into(),
                    tag: 1,
                    fields: vec![DesignField {
                        name: "0".into(),
                        ty: DesignValueType::Named("example.Tree".into()),
                    }],
                },
            ]),
        };
        assert_eq!(validate_value_graph(&[invalid, optional], &[]).unwrap_err(), "Recursive inline value layout");
        let mut missing = tree.clone();
        let DesignShape::Struct(fields) = &mut missing.shape else { unreachable!() };
        fields[0].ty = DesignValueType::Named("example.Missing".into());
        assert!(validate_value_graph(&[missing], &[]).unwrap_err().contains("Missing value definition"));
        assert!(validate_value_graph(&[tree.clone(), tree], &[]).unwrap_err().contains("duplicate"));
    }

    /// Tagged cases must identify an unambiguous payload at every named definition.
    #[test]
    fn nested_enum_duplicate_tags_are_rejected() {
        let value = DesignType {
            qualified_name: "example.Choice".into(),
            rust_type: "example_contracts::Choice".into(),
            shape: DesignShape::Enum(vec![
                DesignVariant {
                    name: "First".into(),
                    tag: 4,
                    fields: vec![],
                },
                DesignVariant {
                    name: "Second".into(),
                    tag: 4,
                    fields: vec![],
                },
            ]),
        };
        assert!(validate_value_graph(&[value], &[]).unwrap_err().contains("duplicate enum case"));
    }

    /// Both NCB directions admit exactly the same byte boundary and reject trailing data.
    #[test]
    fn settings_encoding_and_decoding_share_the_exact_byte_boundary() {
        let overhead = encode_settings("").unwrap().len();
        let text = "x".repeat(MAX_SETTINGS_BYTES - overhead);
        let bytes = encode_settings(text.as_str()).unwrap();
        assert_eq!(bytes.len(), MAX_SETTINGS_BYTES);
        assert_eq!(decode_settings::<String>(&bytes).unwrap(), text);
        assert!(encode_settings((text + "x").as_str()).is_err());
        let mut excessive = bytes;
        excessive.push(0);
        assert!(decode_settings::<String>(&excessive).is_err());
        assert!(decode_settings::<bool>(&[2]).is_err());
    }

    /// Catalog JSON has its own output ceiling, including expansion of opaque bytes.
    #[test]
    fn oversized_descriptor_is_rejected_before_output_materialization() {
        let mut manifest = counter_manifest();
        manifest.defaults = vec![255; MAX_DESIGN_DESCRIPTOR_BYTES];
        assert!(manifest.encode().is_err());
        assert!(DesignWidgetManifest::decode(&serde_json::to_vec(&manifest).unwrap()).is_err());
    }
}
