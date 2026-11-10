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
// -----------------------------------------------------------------------------
// Ported to rust from https://github.com/rxi/microui/ and the original license
//
// Copyright (c) 2020 rxi
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to
// deal in the Software without restriction, including without limitation the
// rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
// sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
// IN THE SOFTWARE.
//
//! Command-line helper that converts the default atlas assets into generated Rust source.
//!
//! Uses the shared demo recipe and implementation-library builder/export APIs. The demo asset package's
//! generation calls those APIs directly through a host build dependency.

use std::{
    env,
    error::Error,
    path::{Path, PathBuf},
};

use neocogi_ui::gfx::atlas;
/// The command-line and Cargo exporters consume the same concrete file recipe.
#[path = "../../../../examples/assets/src/recipe.rs"]
mod recipe;
/// Compile the shared recipe without linking the demo application's runtime loader.
#[path = "../../../../examples/assets/src/recipe_data.rs"]
mod recipe_data;

/// Parses the output path and writes the generated atlas source.
fn main() -> Result<(), Box<dyn Error>> {
    let output = parse_output_arg()?;
    export_atlas(&output)?;
    Ok(())
}

/// Extracts `--output <path>` from the helper's small argument surface.
fn parse_output_arg() -> Result<PathBuf, Box<dyn Error>> {
    let mut args = env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--output" {
            // Keep the parser explicit so missing values report the user-facing flag name.
            if let Some(path) = args.next() {
                return Ok(PathBuf::from(path));
            } else {
                return Err("--output requires a path".into());
            }
        }
    }
    Err("missing --output <path>".into())
}

/// Builds the default recipe and emits an immutable COM bundle with compressed pixels and compiled metadata.
fn export_atlas(path: &Path) -> Result<(), Box<dyn Error>> {
    let atlas = neocogi_ui::gfx::atlas::Builder::from_config(&recipe::atlas_config())?.build()?;
    // Compiled runtime tables depend only on the facade/contracts; all construction stays in this tool.
    neocogi_ui::gfx::export::write_rust(&atlas, path)?;
    Ok(())
}
