//! Generated modules are compiled as ordinary Rust, not merely parsed or string-checked.
use neocogi_ui_document::Document;

#[rustfmt::skip]
#[allow(dead_code)]
#[path = "fixtures/full_bindings.rs"]
mod full;
#[rustfmt::skip]
#[allow(dead_code)]
#[path = "fixtures/empty_bindings.rs"]
mod empty;

/// Exact fixture equality keeps the compiled artifact synchronized with the generator's output.
#[test]
fn generated_bindings_compile_for_every_public_contract_and_escaped_name() {
    let document = Document::from_json(include_bytes!("fixtures/bindings.ui.json")).unwrap();
    assert_eq!(document.rust_bindings().unwrap(), include_str!("fixtures/full_bindings.rs"));
}

/// Empty public contracts still produce a valid exhaustive dispatcher and a callable binding function.
#[test]
fn empty_public_contract_generates_a_valid_module() {
    let document = Document::from_json(include_bytes!("fixtures/empty.ui.json")).unwrap();
    assert_eq!(document.rust_bindings().unwrap(), include_str!("fixtures/empty_bindings.rs"));
}
