//
// Copyright 2022-Present (c) Raja Lehtihet & Wael El Oraiby
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
// this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
// this list of conditions and the following disclaimer in the documentation
// and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
// may be used to endorse or promote products derived from this software without
// specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
// LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
// CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
// SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
// CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGE.
//
//! Generates the embedded atlas through ordinary Cargo host build dependencies.

use std::{env, error::Error, path::PathBuf};
#[cfg(feature = "embed-assets")]
use neocogi_gfx_impl::{
    atlas::{self, Builder},
    export::write_rust,
};
#[cfg(all(feature = "embed-assets", feature = "full-demo"))]
use neocogi_skin_impl::theme;
#[cfg(all(feature = "embed-assets", feature = "full-demo"))]
use neocogi_gfx_support::AssetData;

#[allow(dead_code, reason = "embedded recipes do not stage adjacent inputs")]
mod build_paths;
/// The file list and embedded generator consume the same authored recipe constants.
#[allow(dead_code, reason = "some recipe constants are used only in one storage mode")]
#[path = "src/recipe_data.rs"]
mod recipe_data;

/// Compile exactly the same recipe used by runtime loading and the command-line exporter.
#[cfg(feature = "embed-assets")]
#[path = "src/recipe.rs"]
mod recipe;

/// Tracks the recipe's actual inputs and writes validated embedded data only when requested.
#[cfg(feature = "embed-assets")]
fn build_embedded() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=src/recipe.rs");
    println!("cargo:rerun-if-changed=src/recipe_data.rs");
    let config = recipe::atlas_config();
    // Derive file tracking from the recipe itself so a new font/icon cannot silently omit a
    // build dependency. Repeated font paths are harmless repeated Cargo tracking directives.
    for path in std::iter::once(&config.white_icon)
        .chain(config.icons.iter().map(|icon| &icon.path))
        .chain(config.fonts.iter().map(|font| &font.path))
    {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo provides OUT_DIR")).join("prebuilt_bundle.rs");
    let bundle = Builder::from_config(&config)?.build()?;
    // One atomic generated artifact replaces the native atlas and COM alternative paths.
    write_rust(&bundle, &output)?;
    #[cfg(feature = "full-demo")]
    full_demo_assets(&bundle)?;
    Ok(())
}

/// Compiles complete theme choices on Cargo's host, keeping target theme/font parsers out.
#[cfg(all(feature = "embed-assets", feature = "full-demo"))]
fn full_demo_assets(base: &AssetData) -> Result<(), Box<dyn Error>> {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo provides OUT_DIR"));
    // Tracking the complete authored directories includes every JSON-relative PNG/font dependency.
    println!("cargo:rerun-if-changed={}", workspace.join("themes").display());
    println!("cargo:rerun-if-changed={}", workspace.join("assets").display());
    for &(generated, definition) in recipe_data::THEMES {
        let loaded = theme::load(&workspace.join(definition), base)?;
        write_rust(loaded.data(), &output.join(format!("{generated}.rs")))?;
    }
    Ok(())
}

/// Stages recipe inputs through Platform's build API and reports its exact relative file set.
/// Packaging reads that same build's JSON event; no persistent output ownership ledger is needed.
fn stage_recipe() -> Result<(), Box<dyn Error>> {
    use std::collections::BTreeMap;
    println!("cargo:rerun-if-changed=src/recipe_data.rs");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize()?;
    let mut files = BTreeMap::<String, PathBuf>::new();
    if !cfg!(feature = "embed-assets") && env::var("CARGO_CFG_TARGET_ARCH")? != "wasm32" {
        for path in std::iter::once(recipe_data::WHITE)
            .chain(recipe_data::ICONS.iter().map(|(_, path)| *path))
            .chain(recipe_data::FONTS.iter().map(|(_, path, _)| *path))
        {
            files.insert(path.to_owned(), root.join(path));
        }
        #[cfg(feature = "full-demo")]
        {
            files.insert("assets/FACEPALM.png".into(), root.join("examples/full/assets/FACEPALM.png"));
            files.insert("assets/suzanne.obj".into(), root.join("assets/suzanne.obj"));
        }
        #[cfg(feature = "external-themes")]
        for &(name, relative) in recipe_data::THEMES {
            let definition = build_paths::source(&root, std::path::Path::new("resources"), std::path::Path::new(relative))?;
            println!("cargo:rerun-if-changed={}", definition.display());
            let mut theme = neocogi_skin_authoring::theme::ThemeDocument::from_bytes(&definition, &std::fs::read(&definition)?)?;
            let mut failure = None;
            theme.visit_resource_paths_mut(|path| {
                let result = (|| -> Result<(), Box<dyn Error>> {
                    let destination = build_paths::destination(std::path::Path::new(relative), path)?;
                    let authored = definition.parent().unwrap().join(&*path);
                    println!("cargo:rerun-if-changed={}", authored.display());
                    let source = build_paths::source(&root, std::path::Path::new(relative), path)?;
                    let destination = destination.to_str().ok_or("theme resource path is not UTF-8")?.replace('\\', "/");
                    files.insert(destination, source);
                    *path = build_paths::reference(path);
                    Ok(())
                })();
                if let Err(error) = result {
                    failure.get_or_insert(error);
                }
            });
            if let Some(error) = failure {
                return Err(error);
            }
            let staged = PathBuf::from(env::var_os("OUT_DIR").ok_or("Cargo provides OUT_DIR")?).join(format!("{name}-adjacent.json"));
            let bytes = theme.to_bytes()?;
            // This generated input is tracked by the staging build API. Preserve its
            // timestamp when the normalized recipe is unchanged so cached builds settle.
            if std::fs::read(&staged).ok().as_deref() != Some(bytes.as_slice()) {
                std::fs::write(&staged, &bytes)?;
            }
            files.insert(relative.into(), staged);
        }
    }
    let mut resources = Vec::new();
    for (destination, source) in &files {
        let output = PathBuf::from(env::var_os("OUT_DIR").ok_or("Cargo provides OUT_DIR")?);
        // Generated normalized JSON is trusted build output. Every authored input,
        // including direct fonts/icons and the definition itself, shares admission.
        let generated = source.starts_with(&output);
        let source = if generated {
            source.clone()
        } else {
            build_paths::source(&root, std::path::Path::new("resources"), source.strip_prefix(&root)?)?
        };
        if !source.is_file() {
            return Err(format!("missing declared resource: {}", source.display()).into());
        }
        if !generated {
            println!("cargo:rerun-if-changed={}", source.display());
        }
        resources.push(neocogi_platform_contracts::build::layout::Resource { source, destination: destination.into() });
    }
    if env::var("CARGO_CFG_TARGET_ARCH")? != "wasm32" {
        use neocogi_platform_contracts::build::layout::{output_root, stage_resources};
        println!("cargo::rerun-if-env-changed=NEOCOGI_NATIVE_OUTPUT_DIR");
        let anchor = PathBuf::from(env::var_os("OUT_DIR").ok_or("Cargo OUT_DIR")?);
        let output = output_root(Some(&anchor), env::var_os("NEOCOGI_NATIVE_OUTPUT_DIR").map(Into::into))?;
        let staged = stage_resources(&PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").ok_or("Cargo manifest")?), &output, &resources)?;
        for path in staged.inputs.iter().chain(&staged.outputs) {
            println!("cargo::rerun-if-changed={}", path.display());
        }
    }
    let paths: Vec<_> = files.keys().collect();
    println!("cargo:rustc-env=NEOCOGI_RESOURCE_FILES={}", serde_json::to_string(&paths)?);
    Ok(())
}

/// Tracks exact selected file inputs; embedded builds additionally generate their source bundle.
fn main() -> Result<(), Box<dyn Error>> {
    stage_recipe()?;
    #[cfg(feature = "embed-assets")]
    build_embedded()?;
    Ok(())
}
