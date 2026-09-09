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

#[cfg(feature = "embed-assets")]
use std::{env, error::Error, path::PathBuf};
#[cfg(feature = "embed-assets")]
use microui_plugin_assets::{atlas::{self, Builder}, export::write_rust, theme};
#[cfg(feature = "embed-assets")]
use neocogi_ui::assets::AssetData;

/// Shared source paths are compiled only for explicitly embedded builds.
#[cfg(feature = "embed-assets")]
#[path = "src/recipe_data.rs"]
mod recipe_data;

/// Compile exactly the same recipe used by runtime loading and the command-line exporter.
#[cfg(feature = "embed-assets")]
#[path = "src/recipe.rs"]
mod recipe;

/// Tracks the recipe's actual inputs and writes validated embedded data only when requested.
#[cfg(feature = "embed-assets")]
fn main() -> Result<(), Box<dyn Error>> {
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
    if env::var_os("CARGO_FEATURE_FULL_DEMO").is_some() {
        full_demo_assets(&bundle)?;
    }
    Ok(())
}

/// Compiles complete theme choices on Cargo's host, keeping target theme/font parsers out.
#[cfg(feature = "embed-assets")]
fn full_demo_assets(base: &AssetData) -> Result<(), Box<dyn Error>> {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo provides OUT_DIR"));
    // Tracking the complete authored directories includes every JSON-relative PNG/font dependency.
    println!("cargo:rerun-if-changed={}", workspace.join("themes").display());
    println!("cargo:rerun-if-changed={}", workspace.join("assets").display());
    for (directory, generated) in [("windows-3.11", "windows_311"), ("windows-95", "windows_95"), ("mac-os-9", "mac_os_9")] {
        let loaded = theme::load(&workspace.join("themes").join(directory).join("theme.json"), base)?;
        write_rust(loaded.data(), &output.join(format!("{generated}.rs")))?;
    }
    Ok(())
}

/// File-based deployments generate no embedded resource code.
#[cfg(not(feature = "embed-assets"))]
fn main() {}
