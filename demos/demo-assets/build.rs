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
use microui_ui::{atlas::builder::Builder, SourceFormat};

/// Compile exactly the same recipe used by runtime loading and the command-line exporter.
#[path = "src/recipe.rs"]
mod recipe;

/// Tracks the recipe's actual inputs and writes validated embedded data only when requested.
fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=src/recipe.rs");
    if env::var_os("CARGO_FEATURE_PREBUILT_ATLAS").is_none() && env::var_os("CARGO_FEATURE_PREBUILT_COM").is_none() {
        return Ok(());
    }
    let config = recipe::atlas_config();
    // Derive file tracking from the recipe itself so a new font/icon cannot silently omit a
    // build dependency. Repeated font paths are harmless repeated Cargo tracking directives.
    for path in std::iter::once(&config.white_icon)
        .chain(config.icons.iter().map(|icon| &icon.path))
        .chain(config.fonts.iter().map(|font| &font.path))
    {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo provides OUT_DIR")).join("prebuilt_atlas.rs");
    let atlas = Builder::from_config(&config)?.build()?;
    // The core exporter writes the PNG and its metadata together; no nested Cargo process,
    // separate target directory, or copied serialization format is needed.
    if env::var_os("CARGO_FEATURE_PREBUILT_ATLAS").is_some() {
        atlas.to_rust_files("PREBUILT_ATLAS", SourceFormat::Png, output)?;
    }
    if env::var_os("CARGO_FEATURE_PREBUILT_COM").is_some() {
        let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo provides OUT_DIR")).join("prebuilt_bundle.rs");
        let bundle = microui_ui::SkinBundle::from_atlas(atlas);
        bundle.to_com_rust_files(output)?;
        if env::var_os("CARGO_FEATURE_FULL_DEMO").is_some() {
            full_demo_assets(&bundle)?;
        }
    }
    Ok(())
}

/// Compiles complete theme choices on Cargo's host, keeping target theme/font parsers out.
fn full_demo_assets(base: &microui_ui::SkinBundle) -> Result<(), Box<dyn Error>> {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo provides OUT_DIR"));
    // Tracking the complete authored directories includes every JSON-relative PNG/font dependency.
    println!("cargo:rerun-if-changed={}", workspace.join("themes").display());
    println!("cargo:rerun-if-changed={}", workspace.join("assets").display());
    let source = microui_ui::assets::export_bundle(base)?;
    let imported = microui_ui::assets::ImportedAssets::read(source.as_com_ref(), None)?;
    for (directory, generated) in [("windows-3.11", "windows_311"), ("windows-95", "windows_95"), ("mac-os-9", "mac_os_9")] {
        imported.load_theme(&workspace.join("themes").join(directory).join("theme.json"))?
            .bundle().to_com_rust_files(output.join(format!("{generated}.rs")))?;
    }
    Ok(())
}
