//! Private Serde input adapter for statically typed COM decoding.
//!
//! Serde requires visitors as its format-facing control-flow mechanism, so this
//! module contains a small set of generic private visitors. Contract types never
//! implement or expose them: their generated [`neocogi_com::ComDeserialize`]
//! implementations recursively request concrete member types through the COM
//! decoding traits.

use std::{collections::BTreeMap, fmt, marker::PhantomData};

use neocogi_com::{ComDeserialize, ComDeserializer, ComEnum, ComMap, ComMapAccess, ComRecord, ComRecordAccess, ComSequence, ComSequenceAccess, ComVariantAccess};
use serde::Deserialize as SerdeDeserialize;
use serde::de::{self, DeserializeSeed, Deserializer, EnumAccess, Error as _, IgnoredAny, MapAccess, SeqAccess, VariantAccess, Visitor};

/// Maximum elements accepted by one recursively decoded sequence.
const MAX_SEQUENCE_ELEMENTS: usize = 1_048_576;
/// Maximum entries accepted by one recursively decoded map.
const MAX_MAP_ENTRIES: usize = 65_536;
/// Maximum UTF-8 bytes accepted by one recursively decoded string.
const MAX_STRING_BYTES: usize = 4 * 1024 * 1024;

/// Resource limits copied into every nested adapter value.
#[derive(Clone, Copy)]
struct DecodeLimits {
    /// Bound for sequence iteration and eager sequence allocation.
    sequence_elements: usize,
    /// Bound for map iteration.
    map_entries: usize,
    /// Bound for an owned UTF-8 string.
    string_bytes: usize,
}

impl DecodeLimits {
    /// Conservative limits shared by all theme document entry points.
    const THEME: Self = Self {
        sequence_elements: MAX_SEQUENCE_ELEMENTS,
        map_entries: MAX_MAP_ENTRIES,
        string_bytes: MAX_STRING_BYTES,
    };
}

/// Decodes one COM-typed value in a Serde-derived containing document.
pub(super) fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: ComDeserialize,
{
    // The concrete type controls recursive construction; Serde only supplies syntax.
    T::com_deserialize(SerdeDecoder::new(deserializer, DecodeLimits::THEME))
}

/// Decodes a closed document-key domain while leaving asset-private values to Serde.
pub(super) fn deserialize_map<'de, D, K, V>(deserializer: D) -> Result<BTreeMap<K, V>, D::Error>
where
    D: Deserializer<'de>,
    K: DocumentKey + Ord,
    V: SerdeDeserialize<'de>,
{
    // This bridge is intentionally mixed: IDL owns role values and order, this
    // asset plugin owns their JSON spellings, and values remain private documents.
    deserializer.deserialize_map(MixedMapVisitor {
        limits: DecodeLimits::THEME,
        marker: PhantomData,
    })
}

/// Asset-document spelling for one closed IDL role used as a map key.
pub(super) trait DocumentKey: Sized {
    /// Accepted JSON names in the same order used by `from_index`.
    const NAMES: &'static [&'static str];

    /// Converts one checked name-table index into its concrete IDL role.
    fn from_index(index: u32) -> Option<Self>;
}

/// Serde-backed implementation of the format-neutral COM decoder.
struct SerdeDecoder<D> {
    /// One Serde deserializer value consumed by the requested operation.
    input: D,
    /// Bounds propagated unchanged into nested seeds and access objects.
    limits: DecodeLimits,
}

impl<D> SerdeDecoder<D> {
    /// Couples one Serde input with explicit resource limits.
    fn new(input: D, limits: DecodeLimits) -> Self {
        Self { input, limits }
    }
}

macro_rules! scalar_decoder {
    ($method:ident, $ty:ty) => {
        fn $method(self) -> Result<$ty, Self::Error> {
            // Reuse Serde's exact scalar range and syntax validation.
            <$ty as SerdeDeserialize>::deserialize(self.input)
        }
    };
}

impl<'de, D> ComDeserializer<'de> for SerdeDecoder<D>
where
    D: Deserializer<'de>,
{
    type Error = D::Error;

    scalar_decoder!(deserialize_unit, ());
    scalar_decoder!(deserialize_bool, bool);
    scalar_decoder!(deserialize_u8, u8);
    scalar_decoder!(deserialize_u16, u16);
    scalar_decoder!(deserialize_u32, u32);
    scalar_decoder!(deserialize_u64, u64);
    scalar_decoder!(deserialize_usize, usize);
    scalar_decoder!(deserialize_i8, i8);
    scalar_decoder!(deserialize_i16, i16);
    scalar_decoder!(deserialize_i32, i32);
    scalar_decoder!(deserialize_i64, i64);
    scalar_decoder!(deserialize_isize, isize);
    scalar_decoder!(deserialize_f32, f32);
    scalar_decoder!(deserialize_f64, f64);

    fn deserialize_string(self) -> Result<String, Self::Error> {
        // A dedicated visitor checks byte length before retaining owned input.
        self.input.deserialize_string(StringVisitor { maximum_bytes: self.limits.string_bytes })
    }

    fn deserialize_option<T>(self) -> Result<Option<T>, Self::Error>
    where
        T: ComDeserialize,
    {
        // The private visitor keeps `T` concrete for both the null and present paths.
        self.input.deserialize_option(OptionVisitor { limits: self.limits, marker: PhantomData })
    }

    fn deserialize_record<T>(self) -> Result<T, Self::Error>
    where
        T: ComRecord,
    {
        // Serde supplies field names; `T` supplies the only construction recipe.
        self.input
            .deserialize_struct(T::NAME, T::FIELDS, RecordVisitor { limits: self.limits, marker: PhantomData })
    }

    fn deserialize_sequence<T>(self) -> Result<T, Self::Error>
    where
        T: ComSequence,
    {
        // The collection or positional record decides how concrete elements are stored.
        self.input.deserialize_seq(SequenceVisitor { limits: self.limits, marker: PhantomData })
    }

    fn deserialize_enum<T>(self) -> Result<T, Self::Error>
    where
        T: ComEnum,
    {
        // Only the generic adapter sees Serde's enum visitor protocol.
        self.input
            .deserialize_enum(T::NAME, T::VARIANTS, EnumVisitor { limits: self.limits, marker: PhantomData })
    }

    fn deserialize_map<T>(self) -> Result<T, Self::Error>
    where
        T: ComMap,
    {
        // The concrete COM map builder owns duplicate-key semantics.
        self.input.deserialize_map(MapVisitor { limits: self.limits, marker: PhantomData })
    }
}

/// Recursive seed that replaces one handwritten Serde implementation per type.
struct ComSeed<T> {
    /// Resource limits inherited from the containing decoder.
    limits: DecodeLimits,
    /// Concrete output type retained at compile time.
    marker: PhantomData<fn() -> T>,
}

impl<'de, T> DeserializeSeed<'de> for ComSeed<T>
where
    T: ComDeserialize,
{
    type Value = T;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Recursion returns immediately to the concrete member implementation.
        T::com_deserialize(SerdeDecoder::new(deserializer, self.limits))
    }
}

/// Private Serde visitor for absent and present COM values.
struct OptionVisitor<T> {
    /// Resource limits inherited by a present value.
    limits: DecodeLimits,
    /// Concrete optional element type.
    marker: PhantomData<fn() -> T>,
}

impl<'de, T> Visitor<'de> for OptionVisitor<T>
where
    T: ComDeserialize,
{
    type Value = Option<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an absent or present typed COM value")
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        // Null has no nested value to decode.
        Ok(None)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        // Formats may report their null token through the unit callback.
        Ok(None)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Preserve `T` while delegating its complete construction recursively.
        ComSeed::<T> { limits: self.limits, marker: PhantomData }.deserialize(deserializer).map(Some)
    }
}

/// Private Serde visitor for owned strings with an explicit byte bound.
struct StringVisitor {
    /// Largest accepted UTF-8 byte length.
    maximum_bytes: usize,
}

impl<'de> Visitor<'de> for StringVisitor {
    type Value = String;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "a UTF-8 string no longer than {} bytes", self.maximum_bytes)
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        // Check borrowed text before allocating its owned result.
        if value.len() > self.maximum_bytes {
            return Err(E::custom("string exceeds the configured COM decoding limit"));
        }
        Ok(value.to_owned())
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        // Retain Serde's allocation only after validating its final size.
        if value.len() > self.maximum_bytes {
            return Err(E::custom("string exceeds the configured COM decoding limit"));
        }
        Ok(value)
    }
}

/// Private Serde visitor that delegates named record construction to `T`.
struct RecordVisitor<T> {
    /// Resource limits inherited by record members.
    limits: DecodeLimits,
    /// Concrete record type.
    marker: PhantomData<fn() -> T>,
}

impl<'de, T> Visitor<'de> for RecordVisitor<T>
where
    T: ComRecord,
{
    type Value = T;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "record {}", T::NAME)
    }

    fn visit_map<A>(self, mut input: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        // The generated record switch requests every field as its declared type.
        T::read_record(&mut SerdeRecordAccess { input: &mut input, limits: self.limits })
    }
}

/// Serde map cursor implementing the COM named-record access contract.
struct SerdeRecordAccess<'a, A> {
    /// Borrowed Serde record cursor; no cursor or input data escapes this call.
    input: &'a mut A,
    /// Resource limits inherited by field values.
    limits: DecodeLimits,
}

impl<'de, A> ComRecordAccess<'de> for SerdeRecordAccess<'_, A>
where
    A: MapAccess<'de>,
{
    type Error = A::Error;

    fn next_field(&mut self, fields: &'static [&'static str]) -> Result<Option<u32>, Self::Error> {
        // Unknown names are rejected by the typed name seed before a value is read.
        self.input.next_key_seed(NameSeed { expected: fields, kind: NameKind::Field })
    }

    fn next_value<T>(&mut self) -> Result<T, Self::Error>
    where
        T: ComDeserialize,
    {
        // Decode the field directly into its declared Rust type.
        self.input.next_value_seed(ComSeed { limits: self.limits, marker: PhantomData })
    }

    fn duplicate_field(&self, field: &'static str) -> Self::Error {
        A::Error::duplicate_field(field)
    }

    fn missing_field(&self, field: &'static str) -> Self::Error {
        A::Error::missing_field(field)
    }

    fn invalid_field_index(&self, index: u32) -> Self::Error {
        A::Error::custom(format_args!("COM decoder returned invalid field index {index}"))
    }
}

/// Serde seed that maps a source name to its stable declaration-order index.
struct NameSeed {
    /// Complete accepted name list supplied by the concrete record or enum.
    expected: &'static [&'static str],
    /// Diagnostic category used when lookup fails.
    kind: NameKind,
}

/// Schema-name category retained only for precise Serde diagnostics.
#[derive(Clone, Copy)]
enum NameKind {
    /// Name selects a record field.
    Field,
    /// Name selects an enum variant.
    Variant,
}

impl<'de> DeserializeSeed<'de> for NameSeed {
    type Value = u32;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        // The visitor borrows the static expected list and allocates no lookup table.
        deserializer.deserialize_identifier(NameVisitor { expected: self.expected, kind: self.kind })
    }
}

/// Private visitor implementing strict record-field and enum-variant lookup.
struct NameVisitor {
    /// Accepted source names in declaration order.
    expected: &'static [&'static str],
    /// Diagnostic category used when lookup fails.
    kind: NameKind,
}

impl<'de> Visitor<'de> for NameVisitor {
    type Value = u32;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a known statically typed COM field or variant name")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        // Linear lookup is compile-time-small schema dispatch, never per-frame UI work.
        if let Some(index) = self.expected.iter().position(|expected| *expected == value) {
            return Ok(index as u32);
        }
        match self.kind {
            NameKind::Field => Err(E::unknown_field(value, self.expected)),
            NameKind::Variant => Err(E::unknown_variant(value, self.expected)),
        }
    }
}

/// Private Serde visitor that delegates sequence construction to `T`.
struct SequenceVisitor<T> {
    /// Resource limits inherited by sequence elements.
    limits: DecodeLimits,
    /// Concrete output sequence or positional record type.
    marker: PhantomData<fn() -> T>,
}

impl<'de, T> Visitor<'de> for SequenceVisitor<T>
where
    T: ComSequence,
{
    type Value = T;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a statically typed COM sequence")
    }

    fn visit_seq<A>(self, mut input: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        // `T` owns storage and element order; the adapter owns limits and syntax.
        T::read_sequence(&mut SerdeSequenceAccess {
            input: &mut input,
            limits: self.limits,
            decoded: 0,
        })
    }
}

/// Serde cursor implementing bounded COM sequence access.
struct SerdeSequenceAccess<'a, A> {
    /// Borrowed Serde sequence cursor.
    input: &'a mut A,
    /// Resource limits inherited by elements.
    limits: DecodeLimits,
    /// Elements successfully decoded so far.
    decoded: usize,
}

impl<'de, A> ComSequenceAccess<'de> for SerdeSequenceAccess<'_, A>
where
    A: SeqAccess<'de>,
{
    type Error = A::Error;

    fn next_element<T>(&mut self) -> Result<Option<T>, Self::Error>
    where
        T: ComDeserialize,
    {
        // Probe only after reaching the limit, so oversized input fails before
        // another typed element can allocate nested storage.
        if self.decoded == self.limits.sequence_elements {
            if self.input.next_element::<IgnoredAny>()?.is_some() {
                return Err(A::Error::custom("sequence exceeds the configured COM decoding limit"));
            }
            return Ok(None);
        }
        let value = self.input.next_element_seed(ComSeed { limits: self.limits, marker: PhantomData })?;
        if value.is_some() {
            self.decoded += 1;
        }
        Ok(value)
    }

    fn size_hint(&self) -> Option<usize> {
        self.input.size_hint()
    }

    fn element_limit(&self) -> usize {
        self.limits.sequence_elements
    }

    fn finish(&mut self, actual: usize) -> Result<(), Self::Error> {
        // Discard at most one extra syntax value solely to report exact-length failure.
        if self.input.next_element::<IgnoredAny>()?.is_some() {
            Err(self.invalid_length(actual.saturating_add(1), actual))
        } else {
            Ok(())
        }
    }

    fn invalid_length(&self, actual: usize, expected: usize) -> Self::Error {
        A::Error::invalid_length(actual, &ExactLength(expected))
    }
}

/// Formatting helper for Serde's fixed-length diagnostics.
struct ExactLength(usize);

impl de::Expected for ExactLength {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "exactly {} elements", self.0)
    }
}

/// Private Serde visitor that delegates enum construction to `T`.
struct EnumVisitor<T> {
    /// Resource limits inherited by a variant payload.
    limits: DecodeLimits,
    /// Concrete enum output type.
    marker: PhantomData<fn() -> T>,
}

impl<'de, T> Visitor<'de> for EnumVisitor<T>
where
    T: ComEnum,
{
    type Value = T;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "enum {}", T::NAME)
    }

    fn visit_enum<A>(self, input: A) -> Result<Self::Value, A::Error>
    where
        A: EnumAccess<'de>,
    {
        // Resolve the variant name once, then enter the concrete generated switch.
        let (variant, value) = input.variant_seed(NameSeed {
            expected: T::VARIANTS,
            kind: NameKind::Variant,
        })?;
        T::read_variant(variant, SerdeVariantAccess { input: value, limits: self.limits })
    }
}

/// Serde enum payload cursor implementing the COM variant access contract.
struct SerdeVariantAccess<A> {
    /// Serde payload cursor consumed by exactly one variant shape.
    input: A,
    /// Resource limits inherited by payload members.
    limits: DecodeLimits,
}

impl<'de, A> ComVariantAccess<'de> for SerdeVariantAccess<A>
where
    A: VariantAccess<'de>,
{
    type Error = A::Error;

    fn unit(self) -> Result<(), Self::Error> {
        self.input.unit_variant()
    }

    fn newtype<T>(self) -> Result<T, Self::Error>
    where
        T: ComDeserialize,
    {
        self.input.newtype_variant_seed(ComSeed { limits: self.limits, marker: PhantomData })
    }

    fn tuple<T>(self, length: usize) -> Result<T, Self::Error>
    where
        T: ComSequence,
    {
        self.input.tuple_variant(length, SequenceVisitor { limits: self.limits, marker: PhantomData })
    }

    fn record<T>(self) -> Result<T, Self::Error>
    where
        T: ComRecord,
    {
        self.input.struct_variant(T::FIELDS, RecordVisitor { limits: self.limits, marker: PhantomData })
    }

    fn invalid_variant(self, variant: u32, expected: &'static [&'static str]) -> Self::Error {
        A::Error::custom(format_args!(
            "COM decoder returned invalid variant index {variant}; expected one of {expected:?}"
        ))
    }
}

/// Private Serde visitor that delegates homogeneous map construction to `T`.
struct MapVisitor<T> {
    /// Resource limits inherited by keys and values.
    limits: DecodeLimits,
    /// Concrete map output type.
    marker: PhantomData<fn() -> T>,
}

impl<'de, T> Visitor<'de> for MapVisitor<T>
where
    T: ComMap,
{
    type Value = T;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a statically typed COM map")
    }

    fn visit_map<A>(self, mut input: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        // `T` owns key ordering and duplicate policy; the adapter owns syntax and limits.
        T::read_map(&mut SerdeMapAccess {
            input: &mut input,
            limits: self.limits,
            decoded: 0,
        })
    }
}

/// Serde map cursor implementing bounded COM map access.
struct SerdeMapAccess<'a, A> {
    /// Borrowed Serde map cursor.
    input: &'a mut A,
    /// Resource limits inherited by entries.
    limits: DecodeLimits,
    /// Keys successfully decoded so far.
    decoded: usize,
}

impl<'de, A> ComMapAccess<'de> for SerdeMapAccess<'_, A>
where
    A: MapAccess<'de>,
{
    type Error = A::Error;

    fn next_key<K>(&mut self) -> Result<Option<K>, Self::Error>
    where
        K: ComDeserialize,
    {
        // Reject an oversized map before decoding another potentially allocating key.
        if self.decoded == self.limits.map_entries {
            if self.input.next_key::<IgnoredAny>()?.is_some() {
                return Err(A::Error::custom("map exceeds the configured COM decoding limit"));
            }
            return Ok(None);
        }
        let key = self.input.next_key_seed(ComSeed { limits: self.limits, marker: PhantomData })?;
        if key.is_some() {
            self.decoded += 1;
        }
        Ok(key)
    }

    fn next_value<V>(&mut self) -> Result<V, Self::Error>
    where
        V: ComDeserialize,
    {
        self.input.next_value_seed(ComSeed { limits: self.limits, marker: PhantomData })
    }

    fn duplicate_key(&self) -> Self::Error {
        A::Error::custom("duplicate key in statically typed COM map")
    }
}

/// Private visitor for maps with COM keys and asset-private Serde values.
struct MixedMapVisitor<K, V> {
    /// Resource limits inherited by role keys.
    limits: DecodeLimits,
    /// Concrete key/value types retained at compile time.
    marker: PhantomData<fn() -> (K, V)>,
}

impl<'de, K, V> Visitor<'de> for MixedMapVisitor<K, V>
where
    K: DocumentKey + Ord,
    V: SerdeDeserialize<'de>,
{
    type Value = BTreeMap<K, V>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded map with statically typed COM keys")
    }

    fn visit_map<A>(self, mut input: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        // Values deliberately remain private Serde documents; no duplicate schema type is made.
        let mut values = BTreeMap::new();
        let mut decoded = 0usize;
        while let Some(key) = input.next_key_seed(DocumentKeySeed::<K> { marker: PhantomData })? {
            if decoded == self.limits.map_entries {
                return Err(A::Error::custom("map exceeds the configured COM decoding limit"));
            }
            decoded += 1;
            let value = input.next_value::<V>()?;
            if values.insert(key, value).is_some() {
                return Err(A::Error::custom("duplicate key in statically typed COM map"));
            }
        }
        Ok(values)
    }
}

/// Serde seed resolving one asset-owned name to a generated IDL role.
struct DocumentKeySeed<K> {
    /// Concrete role retained without allocating a lookup table.
    marker: PhantomData<fn() -> K>,
}

impl<'de, K> DeserializeSeed<'de> for DocumentKeySeed<K>
where
    K: DocumentKey,
{
    type Value = K;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        let index = NameSeed {
            expected: K::NAMES,
            kind: NameKind::Variant,
        }
        .deserialize(deserializer)?;
        K::from_index(index).ok_or_else(|| D::Error::custom("document key table returned an invalid index"))
    }
}

#[cfg(test)]
mod tests {
    //! Adapter tests covering every COM enum payload shape and owned containers.

    use std::collections::BTreeMap;

    use super::deserialize;

    /// Payload enum used to verify macro-generated variant construction.
    #[derive(Debug, PartialEq)]
    #[neocogi_com::com_enum(tag = u8, deserialize)]
    enum Message {
        /// Fieldless variant.
        Unit,
        /// Single-value variant.
        Count(u32),
        /// Heterogeneous positional variant.
        Offset(u8, i16),
        /// Named-field variant.
        State {
            /// Stable state identifier.
            id: u32,
            /// Whether the state is enabled.
            enabled: bool,
        },
    }

    /// Parses one complete JSON value through the private adapter entry point.
    fn parse<T>(json: &str) -> T
    where
        T: neocogi_com::ComDeserialize,
    {
        // A streaming Serde deserializer proves the adapter does not require an
        // intermediate `serde_json::Value` or another erased document tree.
        let mut input = serde_json::Deserializer::from_str(json);
        deserialize(&mut input).expect("test JSON should satisfy the typed COM schema")
    }

    /// Attempts typed parsing while retaining the adapter's structured JSON error.
    fn try_parse<T>(json: &str) -> Result<T, serde_json::Error>
    where
        T: neocogi_com::ComDeserialize,
    {
        // Tests use this fallible path to verify generated schema validation.
        let mut input = serde_json::Deserializer::from_str(json);
        deserialize(&mut input)
    }

    /// COM enums retain their concrete payload type for every supported shape.
    #[test]
    fn enum_payloads_decode_without_an_erased_value_tree() {
        assert_eq!(parse::<Message>(r#""Unit""#), Message::Unit);
        assert_eq!(parse::<Message>(r#"{"Count": 7}"#), Message::Count(7));
        assert_eq!(parse::<Message>(r#"{"Offset": [3, -9]}"#), Message::Offset(3, -9));
        assert_eq!(
            parse::<Message>(r#"{"State": {"id": 4, "enabled": true}}"#),
            Message::State { id: 4, enabled: true }
        );
    }

    /// Blanket implementations recursively decode arrays and owned containers.
    #[test]
    fn generic_owned_containers_decode_recursively() {
        assert_eq!(parse::<[u16; 3]>("[2, 4, 8]"), [2, 4, 8]);
        assert_eq!(parse::<Option<Box<[i32]>>>("[1, -2, 3]").as_deref(), Some([1, -2, 3].as_slice()));
        assert_eq!(
            parse::<BTreeMap<String, u32>>(r#"{"first": 1, "second": 2}"#),
            BTreeMap::from([(String::from("first"), 1), (String::from("second"), 2)])
        );
    }

    /// Generated record constructors enforce shape while supporting explicit defaults.
    #[test]
    fn record_generation_rejects_shape_errors_and_retains_defaults() {
        use neocogi_ui_contracts::{AssetMetrics, Insets};

        let metrics = parse::<AssetMetrics>(r#"{"padding": 11}"#);
        assert_eq!(metrics.padding, 11);
        assert_eq!(metrics.title_height, AssetMetrics::default().title_height);

        assert!(try_parse::<Insets>(r#"{"left": 1, "top": 2, "right": 3}"#).is_err());
        assert!(try_parse::<Insets>(r#"{"left": 1, "top": 2, "right": 3, "bottom": 4, "left": 5}"#).is_err());
        assert!(try_parse::<Insets>(r#"{"left": 1, "top": 2, "right": 3, "bottom": 4, "extra": 5}"#).is_err());
    }
}
