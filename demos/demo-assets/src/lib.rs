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
//! Demo atlas loading from the shared recipe or Cargo-generated embedded data.

#[cfg(any(feature = "builder", feature = "prebuilt-atlas"))]
use microui_ui::prelude::AtlasHandle;

/// Asset recipe also compiled directly into this package's Cargo build script.
#[cfg(feature = "builder")]
mod recipe;
#[cfg(feature = "builder")]
pub use recipe::atlas_config;

/// Builds and validates the shared recipe at startup when embedding was not selected.
#[cfg(all(not(feature = "prebuilt-atlas"), feature = "builder"))]
pub fn load_atlas() -> AtlasHandle {
    use microui_ui::atlas::builder::Builder;
    Builder::from_config(&atlas_config())
        .and_then(Builder::build)
        .expect("default atlas assets must load, pack, and satisfy the complete atlas contract")
}

/// Generated source stays private; callers receive the same validated AtlasHandle in either mode.
#[cfg(feature = "prebuilt-atlas")]
mod prebuilt {
    use microui_ui::prelude::AtlasHandle;
    include!(concat!(env!("OUT_DIR"), "/prebuilt_atlas.rs"));

    /// Validates generated pixels and metadata again at the runtime ownership boundary.
    pub fn load() -> AtlasHandle {
        AtlasHandle::try_from(&PREBUILT_ATLAS).expect("generated prebuilt atlas must satisfy the complete atlas contract")
    }
}

/// Loads the self-contained atlas produced by the host build script.
#[cfg(feature = "prebuilt-atlas")]
pub fn load_atlas() -> AtlasHandle {
    prebuilt::load()
}

/// Compiled COM tables have no target dependency on the retained UI or any asset parser.
#[cfg(feature = "prebuilt-com")]
mod prebuilt_com {
    include!(concat!(env!("OUT_DIR"), "/prebuilt_bundle.rs"));
}
#[cfg(feature = "prebuilt-com")]
pub use prebuilt_com::load_bundle;

#[cfg(all(not(feature = "prebuilt-atlas"), not(feature = "prebuilt-com"), not(feature = "builder")))]
compile_error!("demo assets require `builder` for runtime generation or `prebuilt-atlas` for embedded data");

/// Compare host generation with the runtime recipe after actual generated-data decoding.
#[cfg(all(test, feature = "builder", feature = "prebuilt-atlas"))]
mod tests {
    use super::*;
    use microui_ui::atlas::builder::Builder;

    /// Catch recipe/feature drift by validating both pixels and resource presentation metadata.
    #[test]
    fn prebuilt_and_runtime_recipes_produce_equivalent_atlases() {
        let built = Builder::from_config(&atlas_config()).unwrap().build().unwrap();
        let embedded = load_atlas();
        assert_eq!((built.width(), built.height()), (embedded.width(), embedded.height()));
        let built_pixels: Vec<_> = built.pixels().iter().map(|pixel| [pixel.x, pixel.y, pixel.z, pixel.w]).collect();
        let embedded_pixels: Vec<_> = embedded.pixels().iter().map(|pixel| [pixel.x, pixel.y, pixel.z, pixel.w]).collect();
        assert_eq!(built_pixels, embedded_pixels);
        let built_fonts = built.clone_font_table();
        let embedded_fonts = embedded.clone_font_table();
        assert_eq!(built_fonts.len(), embedded_fonts.len());
        for (name, font) in built_fonts {
            let other = embedded.font_id(&name).expect("generated font keeps its recipe name");
            assert_eq!(built.get_font_height(font), embedded.get_font_height(other));
            assert_eq!(built.get_font_baseline(font), embedded.get_font_baseline(other));
            assert_eq!(built.get_font_size(font), embedded.get_font_size(other));
        }
        let built_icons = built.clone_icon_table();
        assert_eq!(built_icons.len(), embedded.clone_icon_table().len());
        for (name, icon) in built_icons {
            let other = embedded.icon_id(&name).expect("generated icon keeps its recipe name");
            let a = built.get_icon_rect(icon);
            let b = embedded.get_icon_rect(other);
            assert_eq!((a.x, a.y, a.width, a.height), (b.x, b.y, b.width, b.height));
        }
    }
}
