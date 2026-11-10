//! Compiler hook for normal Cargo artifact dependencies in native demo compositions.
//!
//! Cargo exposes normal artifact paths to the consuming compiler, not its build script.
//! Moving providers to build dependencies changes their optimization/panic profiles.
//! This hook calls Platform's authoritative staging API from the correct compilation
//! boundary and emits only a filename. Resource staging stays in ordinary build scripts.
#![feature(proc_macro_tracked_path, proc_macro_tracked_env)]
#![deny(missing_docs)]

use proc_macro::TokenStream;
use quote::quote;
use std::{env::VarError, io, path::PathBuf};
use syn::LitStr;

/// Reads an optional tracked compiler variable without hiding invalid Unicode.
fn optional_environment(name: &str) -> io::Result<Option<String>> {
    match proc_macro::tracked::env_var(name) {
        Ok(value) => Ok(Some(value)),
        Err(VarError::NotPresent) => Ok(None),
        Err(error) => Err(io::Error::other(error)),
    }
}

/// Stages Cargo's selected cdylib and emits its filename for executable-relative loading.
///
/// The literal argument names a `CARGO_CDYLIB_FILE_*` variable. Both the source and
/// copied output are compiler-tracked, so cached backend switches and deleted outputs
/// rerun staging. No source path, native layout policy or copy implementation is emitted.
#[proc_macro]
pub fn stage_library(input: TokenStream) -> TokenStream {
    let variable = syn::parse_macro_input!(input as LitStr);
    let result = (|| -> io::Result<String> {
        use neocogi_platform_contracts::build::layout;
        let source = PathBuf::from(proc_macro::tracked::env_var(variable.value()).map_err(io::Error::other)?);
        let anchor = optional_environment("OUT_DIR")?.map(PathBuf::from);
        let explicit = optional_environment("NEOCOGI_NATIVE_OUTPUT_DIR")?.map(PathBuf::from);
        let root = layout::output_root(anchor.as_deref(), explicit)?;
        let filename = layout::stage_library(&source, &root)?;
        proc_macro::tracked::path(&source);
        proc_macro::tracked::path(root.join("lib").join(&filename));
        Ok(filename)
    })();
    match result {
        Ok(filename) => quote!(#filename).into(),
        Err(error) => syn::Error::new(variable.span(), error.to_string()).into_compile_error().into(),
    }
}
