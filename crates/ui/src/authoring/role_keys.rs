//! Closed role-name maps are the only custom visitor needed by the theme file format.
use std::{collections::BTreeMap, fmt, marker::PhantomData};
use serde::{Deserialize, Deserializer};
use serde::de::{DeserializeSeed, Error, MapAccess, Visitor};

/// Canonical asset-document names for one finite generated role domain.
pub(super) trait DocumentKey: Sized {
    /// Accepted names, in the same order used by `from_index`.
    const NAMES: &'static [&'static str];
    /// Returns a canonical spelling, rejecting unknown raw ABI enum values.
    fn name(&self) -> Option<&'static str>;
    /// Resolves a checked name index into the concrete generated role.
    fn from_index(index: u32) -> Option<Self>;
}

/// Decode role keys directly, rejecting unknown and duplicate names before reading their values.
pub(super) fn deserialize_map<'de, D, K, V>(deserializer: D) -> Result<BTreeMap<K, V>, D::Error>
where
    D: Deserializer<'de>,
    K: DocumentKey + Ord,
    V: Deserialize<'de>,
{
    deserializer.deserialize_map(Roles::<K, V>(PhantomData))
}

/// Retains only the map's statically known role and value types; no dynamic decoder state exists.
struct Roles<K, V>(
    /// Phantom type ownership introduces no allocation or runtime metadata.
    PhantomData<fn() -> (K, V)>,
);
impl<'de, K: DocumentKey + Ord, V: Deserialize<'de>> Visitor<'de> for Roles<K, V> {
    type Value = BTreeMap<K, V>;
    /// Reports the concrete file syntax when the JSON value has the wrong shape.
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an object with unique appearance role names")
    }
    /// The finite key domain bounds entries; duplicates cannot overwrite previously authored state.
    fn visit_map<A: MapAccess<'de>>(self, mut input: A) -> Result<Self::Value, A::Error> {
        let mut values = BTreeMap::new();
        while let Some(key) = input.next_key_seed(Role::<K>(PhantomData))? {
            if values.contains_key(&key) {
                return Err(A::Error::custom("duplicate appearance role"));
            }
            values.insert(key, input.next_value()?);
        }
        Ok(values)
    }
}

/// Resolves a borrowed JSON identifier directly against the generated role's closed name table.
struct Role<K>(
    /// Carries the concrete role without allocating a string or lookup table.
    PhantomData<fn() -> K>,
);
impl<'de, K: DocumentKey> DeserializeSeed<'de> for Role<K> {
    type Value = K;
    /// Ask Serde only for a name, without interpreting arbitrary records or payloads.
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<K, D::Error> {
        deserializer.deserialize_identifier(self)
    }
}
impl<'de, K: DocumentKey> Visitor<'de> for Role<K> {
    type Value = K;
    /// Describes the accepted role key syntax in JSON diagnostics.
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a declared appearance role name")
    }
    /// Unknown names fail before allocation of a corresponding appearance value.
    fn visit_str<E: Error>(self, value: &str) -> Result<K, E> {
        let index = K::NAMES
            .iter()
            .position(|name| *name == value)
            .ok_or_else(|| E::unknown_variant(value, K::NAMES))?;
        K::from_index(index as u32).ok_or_else(|| E::custom("invalid appearance role table"))
    }
}
