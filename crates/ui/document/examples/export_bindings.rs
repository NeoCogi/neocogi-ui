//! Generates a Rust binding module from an authored JSON document for application integration.
use neocogi_ui_document::Document;

/// Accepts one source path and writes only generated code to stdout for explicit redirection.
fn main() -> Result<(), String> {
    let path = std::env::args_os().nth(1).ok_or("Usage: export_bindings document.ui.json")?;
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    print!("{}", Document::from_json(&bytes)?.rust_bindings()?);
    Ok(())
}
