//! Product-boundary regressions for documents wrapped in COM persistence records.

use super::*;

/// A leaf ObjectRecord has 21 values besides its state bytes: the record itself,
/// the UUID array and 16 elements, the schema scalar, and both vector containers.
const ENVELOPE_VALUES: usize = 21;

/// Encodes a validated inner document independently of the envelope's byte vector.
/// This allows tests to distinguish valid text from an outer collection that is
/// too large, without making the document writer expose its partial output.
fn record(document: &Document) -> ObjectRecord {
    document.validate().unwrap();
    ObjectRecord {
        type_id: DOCUMENT_TYPE.to_uuid_bytes(),
        schema_version: VERSION,
        state: binary::to_vec_with_limits(document, limits().binary).unwrap(),
        children: vec![],
    }
}

/// The review's large-text case is valid inner data, but its byte-vector envelope
/// must fail on save as well as load under the document's cumulative value cap.
#[test]
fn oversized_binary_document_is_rejected_before_returning_saved_bytes() {
    let document = Document::new("x".repeat(1_048_576));
    let initialization = record(&document);
    assert!(initialization.state.len() < MAX_BYTES);
    assert!(initialization.state.len() > limits().binary.values);
    assert!(document.to_binary().is_err());

    // Construct the former unloadable artifact under a deliberately larger
    // writer policy and prove the ordinary document reader still rejects it.
    let permissive = binary::Limits { values: 2_000_000, ..limits().binary };
    let encoded = binary::to_vec_with_limits(&initialization, permissive).unwrap();
    assert!(Document::from_binary(&encoded).is_err());
}

/// A successfully saved document at the exact cumulative boundary reopens; one
/// additional byte of authored text must fail before a file can be published.
#[test]
fn binary_document_round_trips_at_its_exact_value_boundary() {
    let mut document = Document::new("");
    let base_len = record(&document).state.len();
    // String framing has fixed width, so each added ASCII character adds one
    // state byte and consequently one value in the outer Vec<u8> collection.
    let name_len = limits().binary.values - ENVELOPE_VALUES - base_len;
    document.name = "x".repeat(name_len);
    assert_eq!(record(&document).state.len() + ENVELOPE_VALUES, limits().binary.values);
    let encoded = document.to_binary().unwrap();
    assert_eq!(Document::from_binary(&encoded).unwrap(), document);

    document.name.push('x');
    assert!(document.to_binary().is_err());
    let permissive = binary::Limits {
        values: limits().binary.values + 1,
        ..limits().binary
    };
    let oversized = binary::to_vec_with_limits(&record(&document), permissive).unwrap();
    assert!(Document::from_binary(&oversized).is_err());
}
