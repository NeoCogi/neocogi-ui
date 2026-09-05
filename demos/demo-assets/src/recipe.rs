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
//! One source of demo atlas names, paths, font sizes, and packing dimensions.

use std::path::{Path, PathBuf};
use microui_ui::atlas::builder;

/// Builds the shared icon/font recipe used by runtime loading, Cargo generation, and the exporter.
pub fn atlas_config() -> builder::Config {
    // Config owns its recipes and platform paths, so this factory returns one self-contained value
    // without static borrowed arrays or UTF-8-only path storage.
    builder::Config {
        texture_height: 256,
        texture_width: 512,
        white_icon: workspace_path("assets/WHITE.png"),
        icons: vec![
            builder::IconAsset {
                name: "close".into(),
                path: workspace_path("assets/CLOSE.png"),
            },
            builder::IconAsset {
                name: "expand".into(),
                path: workspace_path("assets/PLUS.png"),
            },
            builder::IconAsset {
                name: "collapse".into(),
                path: workspace_path("assets/MINUS.png"),
            },
            builder::IconAsset {
                name: "check".into(),
                path: workspace_path("assets/CHECK.png"),
            },
            builder::IconAsset {
                name: "expand_down".into(),
                path: workspace_path("assets/EXPAND_DOWN.png"),
            },
            builder::IconAsset {
                name: "open_folder".into(),
                path: workspace_path("assets/OPEN_FOLDER_16.png"),
            },
            builder::IconAsset {
                name: "closed_folder".into(),
                path: workspace_path("assets/CLOSED_FOLDER_16.png"),
            },
            builder::IconAsset {
                name: "file".into(),
                path: workspace_path("assets/FILE_16.png"),
            },
        ],
        fonts: vec![
            builder::FontAsset {
                name: "body".into(),
                path: workspace_path("assets/NORMAL.ttf"),
                size: 12,
            },
            builder::FontAsset {
                name: "small".into(),
                path: workspace_path("assets/NORMAL.ttf"),
                size: 10,
            },
            builder::FontAsset {
                name: "title".into(),
                path: workspace_path("assets/BOLD.ttf"),
                size: 12,
            },
            builder::FontAsset {
                name: "heading".into(),
                path: workspace_path("assets/NORMAL.ttf"),
                size: 18,
            },
            builder::FontAsset {
                name: "mono".into(),
                path: workspace_path("assets/CONSOLE.ttf"),
                size: 14,
            },
            builder::FontAsset {
                name: "calculator-display".into(),
                path: workspace_path("assets/CONSOLE.ttf"),
                size: 28,
            },
        ],
    }
}

/// Resolves repository assets relative to this package for both its library and host build script.
fn workspace_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(relative)
}
