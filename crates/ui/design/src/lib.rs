//! Owned authoring metadata copied from generated IDL or a plugin COM catalog.
//!
//! The JSON encoding is a versioned payload of `IDesignWidgetCatalog::describe`.
//! It crosses COM as bytes, then is parsed and validated before the designer
//! exposes a widget kind or calls its generic constructor. Rust static pointers
//! and descriptor references never cross the plugin boundary.
#![deny(missing_docs)]

use neocogi_com::design::{IdlInterface, IdlShape};
use serde::{Deserialize, Serialize};
use neocogi_com::{ComSerialize, ComDeserialize};

/// One named, typed custom constructor value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ComSerialize, ComDeserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomProperty {
    /// Stable property identifier from the trusted widget contract.
    pub name: String,
    /// Strongly typed authored value.
    pub value: CustomValue,
}

/// Closed scalar property kinds supported by the generic designer inspector.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ComSerialize, ComDeserialize)]
#[serde(deny_unknown_fields)]
pub enum CustomValue {
    /// UTF-8 text copied into the custom factory at construction.
    Text(String),
    /// Boolean toggle.
    Boolean(bool),
    /// Signed integral value.
    Integer(i32),
    /// Finite single-precision numeric value.
    Number(f32),
}
impl CustomValue {
    /// Compares property kinds independently of their authored values.
    pub fn same_kind(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Text(_), Self::Text(_)) | (Self::Boolean(_), Self::Boolean(_)) | (Self::Integer(_), Self::Integer(_)) | (Self::Number(_), Self::Number(_))
        )
    }
    /// Rejects nonfinite authored numeric values before calling a plugin factory.
    pub fn valid(&self) -> bool {
        match self {
            Self::Number(value) => value.is_finite(),
            Self::Text(_) | Self::Boolean(_) | Self::Integer(_) => true,
        }
    }
}

/// Maximum encoded bytes accepted for one plugin-provided widget description.
pub const MAX_DESIGN_DESCRIPTOR_BYTES: usize = 64 * 1024;
/// Current authoring descriptor schema revision.
pub const DESIGN_DESCRIPTOR_VERSION: u32 = 3;

/// One source-level field, including the exact IDL type spelling.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesignField {
    /// Authored name or decimal tuple position.
    pub name: String,
    /// Authored type expression.
    pub ty: String,
}

/// One tagged enum case and its ordered fields.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesignVariant {
    /// Authored variant name.
    pub name: String,
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
    /// Public Rust type path used in generated handlers.
    pub rust_type: String,
    /// Complete struct, enum, or bit-enum shape.
    pub shape: DesignShape,
}

/// The one complete event stream resolved through the class ancestry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesignStream {
    /// Closed generic COM stream identity.
    pub iid: [u8; 16],
    /// Full stream ABI signature, including its event layout.
    pub signature: [u8; 32],
    /// Exact generated message payload.
    pub message: DesignType,
}

/// Exact widget interface identity and its authoring-level output contracts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesignInterface {
    /// Canonical UUID bytes of the concrete COM projection.
    pub iid: [u8; 16],
    /// Full transitive ABI signature of that projection.
    pub signature: [u8; 32],
    /// Public Rust `*Abi` projection used by generated forms.
    pub rust_abi: String,
    /// Optional single stream, including inherited generic projections.
    pub stream: Option<DesignStream>,
}

/// One constructible widget kind and its typed constructor defaults.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesignWidgetManifest {
    /// Version of this owned authoring descriptor schema.
    pub version: u32,
    /// Stable kind used in authored UI documents.
    pub kind: String,
    /// Typed constructor defaults in stable order.
    pub defaults: Vec<CustomProperty>,
    /// Exact COM and IDL metadata for the constructed widget.
    pub interface: DesignInterface,
}

impl DesignWidgetManifest {
    /// Copies generated static IDL metadata into an owned COM transport record.
    pub fn from_idl(kind: impl Into<String>, defaults: Vec<CustomProperty>, source: &IdlInterface) -> Result<Self, String> {
        let primary = source.chain.first().ok_or("IDL interface has no COM chain")?;
        if primary.interface_id != source.abi.interface_id || primary.abi_signature != source.abi.abi_signature {
            return Err("IDL interface has an inconsistent COM chain".into());
        }
        if let Some(stream) = &source.stream {
            if !source
                .chain
                .iter()
                .any(|interface| interface.interface_id == stream.abi.interface_id && interface.abi_signature == stream.abi.abi_signature)
            {
                return Err("Resolved event stream is absent from the COM projection chain".into());
            }
        }
        // SAFETY: the generated ABI signature points to immutable static bytes.
        let digest: [u8; 32] = unsafe { source.abi.abi_signature.as_bytes() }
            .map_err(|error| error.to_string())?
            .try_into()
            .map_err(|_| "IDL interface has a non-SHA-256 signature")?;
        let stream = source
            .stream
            .as_ref()
            .map(|output| -> Result<DesignStream, String> {
                Ok(DesignStream {
                    iid: output.abi.interface_id.to_uuid_bytes(),
                    // SAFETY: IDL owns this immutable static descriptor.
                    signature: unsafe { output.abi.abi_signature.as_bytes() }
                        .map_err(|error| error.to_string())?
                        .try_into()
                        .map_err(|_| "Stream signature is not SHA-256")?,
                    message: DesignType {
                        qualified_name: output.message.qualified_name.to_owned(),
                        rust_type: output.message.rust_type.to_owned(),
                        shape: match output.message.shape {
                            IdlShape::Struct(fields) => DesignShape::Struct(
                                fields
                                    .iter()
                                    .map(|field| DesignField {
                                        name: field.name.to_owned(),
                                        ty: field.ty.to_owned(),
                                    })
                                    .collect(),
                            ),
                            IdlShape::Enum(variants) => DesignShape::Enum(
                                variants
                                    .iter()
                                    .map(|variant| DesignVariant {
                                        name: variant.name.to_owned(),
                                        fields: variant
                                            .fields
                                            .iter()
                                            .map(|field| DesignField {
                                                name: field.name.to_owned(),
                                                ty: field.ty.to_owned(),
                                            })
                                            .collect(),
                                    })
                                    .collect(),
                            ),
                            IdlShape::BitEnum(flags) => DesignShape::BitEnum(
                                flags
                                    .iter()
                                    .map(|flag| DesignFlag {
                                        name: flag.name.to_owned(),
                                        value: flag.value,
                                    })
                                    .collect(),
                            ),
                        },
                    },
                })
            })
            .transpose()?;
        let manifest = Self {
            version: DESIGN_DESCRIPTOR_VERSION,
            kind: kind.into(),
            defaults,
            interface: DesignInterface {
                iid: source.abi.interface_id.to_uuid_bytes(),
                signature: digest,
                rust_abi: source.rust_abi.to_owned(),
                stream,
            },
        };
        manifest.validate()?;
        Ok(manifest)
    }

    /// Rejects malformed metadata before it can affect construction or code generation.
    pub fn validate(&self) -> Result<(), String> {
        if self.version != DESIGN_DESCRIPTOR_VERSION || self.kind.is_empty() || self.interface.iid == [0; 16] {
            return Err("Invalid design widget descriptor identity or version".into());
        }
        if !rust_path(&self.interface.rust_abi) {
            return Err("Invalid design widget interface or output policy".into());
        }
        let mut names = std::collections::BTreeSet::new();
        if !self
            .defaults
            .iter()
            .all(|property| !property.name.is_empty() && names.insert(&property.name) && property.value.valid())
        {
            return Err("Invalid design widget property defaults".into());
        }
        if let Some(output) = &self.interface.stream {
            if output.iid == [0; 16] || !rust_path(&output.message.rust_type) {
                return Err("Invalid design widget output metadata".into());
            }
            match &output.message.shape {
                DesignShape::Struct(fields) => {
                    if !fields.iter().all(|field| !field.name.is_empty() && !field.ty.is_empty()) {
                        return Err("Invalid IDL struct field".into());
                    }
                }
                DesignShape::Enum(variants) => {
                    if !variants
                        .iter()
                        .all(|variant| identifier(&variant.name) && variant.fields.iter().all(|field| !field.name.is_empty() && !field.ty.is_empty()))
                    {
                        return Err("Invalid IDL enum variant".into());
                    }
                }
                DesignShape::BitEnum(flags) => {
                    if !flags.iter().all(|flag| identifier(&flag.name)) {
                        return Err("Invalid IDL bit flag".into());
                    }
                }
            }
        }
        Ok(())
    }

    /// Serializes a validated descriptor for one synchronous COM catalog read.
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|error| error.to_string())?;
        if bytes.len() > MAX_DESIGN_DESCRIPTOR_BYTES {
            return Err("Design widget descriptor exceeds byte limit".into());
        }
        Ok(bytes)
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

/// Checks one generated Rust identifier before interpolating it into output.
fn identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some('a'..='z' | 'A'..='Z' | '_'))
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        && !matches!(value, "self" | "super" | "crate" | "Self" | "fn" | "mod" | "pub" | "use")
}

/// Allows only simple generated Rust paths, never injected source expressions.
fn rust_path(path: &str) -> bool {
    !path.is_empty() && path.split("::").all(identifier)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A copied catalog description must retain exact typed metadata and reject
    /// malformed versions, Rust paths, and nonfinite constructor defaults.
    #[test]
    fn manifest_round_trip_and_validation_guard_generated_bindings() {
        let manifest = DesignWidgetManifest {
            version: DESIGN_DESCRIPTOR_VERSION,
            kind: "example.counter".into(),
            defaults: vec![CustomProperty {
                name: "count".into(),
                value: CustomValue::Integer(0),
            }],
            interface: DesignInterface {
                iid: [1; 16],
                signature: [2; 32],
                rust_abi: "example_contracts::CounterAbi".into(),
                stream: Some(DesignStream {
                    iid: [2; 16],
                    signature: [3; 32],
                    message: DesignType {
                        qualified_name: "example.CounterEvent".into(),
                        rust_type: "example_contracts::CounterEvent".into(),
                        shape: DesignShape::Enum(vec![DesignVariant { name: "Activated".into(), fields: vec![] }]),
                    },
                }),
            },
        };
        assert_eq!(DesignWidgetManifest::decode(&manifest.encode().unwrap()).unwrap(), manifest);
        let mut invalid = manifest.clone();
        invalid.version += 1;
        assert!(invalid.encode().is_err());
        invalid = manifest.clone();
        invalid.interface.rust_abi = "example_contracts::CounterAbi; panic!()".into();
        assert!(invalid.encode().is_err());
        invalid = manifest.clone();
        invalid.defaults[0].value = CustomValue::Number(f32::NAN);
        assert!(invalid.encode().is_err());
    }
}
