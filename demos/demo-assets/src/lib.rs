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
//! Demo resources use adjacent native files by default; `embed-assets` explicitly selects compiled resources.

/// Native loading and host construction share one catalog recipe.
#[cfg(any(not(feature = "embed-assets"), test))]
#[cfg_attr(
    feature = "embed-assets",
    allow(dead_code, reason = "compiled metadata tests do not open the white tile source path")
)]
mod recipe_data;

/// Generated metadata and compressed pixels have no target dependency on the UI implementation.
#[cfg(feature = "embed-assets")]
mod prebuilt {
    include!(concat!(env!("OUT_DIR"), "/prebuilt_bundle.rs"));
}
#[cfg(feature = "embed-assets")]
pub use prebuilt::{load_bundle, bundle_data};

/// Native construction forwards file paths to the separately loaded asset provider.
#[cfg(not(feature = "embed-assets"))]
mod files;
#[cfg(not(feature = "embed-assets"))]
pub use files::load_bundle;

/// Theme selection preserves the original catalog regardless of resource storage.
#[cfg(feature = "full-demo")]
pub mod themes;

/// Reads the demonstration photo; only the explicit embedding feature includes bytes in the host.
#[cfg(feature = "full-demo")]
pub fn photo_png() -> std::io::Result<std::borrow::Cow<'static, [u8]>> {
    #[cfg(feature = "embed-assets")]
    {
        Ok(std::borrow::Cow::Borrowed(include_bytes!("../../demo-full/assets/FACEPALM.png")))
    }
    #[cfg(not(feature = "embed-assets"))]
    {
        std::fs::read(files::asset_path("assets/FACEPALM.png")?).map(std::borrow::Cow::Owned)
    }
}

/// Reads the mesh from the same package location as the image and atlas inputs.
#[cfg(feature = "full-demo")]
pub fn suzanne_obj() -> std::io::Result<std::borrow::Cow<'static, [u8]>> {
    #[cfg(feature = "embed-assets")]
    {
        Ok(std::borrow::Cow::Borrowed(include_bytes!("../../../assets/suzanne.obj")))
    }
    #[cfg(not(feature = "embed-assets"))]
    {
        std::fs::read(files::asset_path("assets/suzanne.obj")?).map(std::borrow::Cow::Owned)
    }
}

#[cfg(all(test, feature = "embed-assets"))]
mod tests {
    use super::*;

    /// Generated source retains every recipe name and font metric before COM publication.
    #[test]
    fn compiled_bundle_contains_the_complete_recipe() {
        let data = bundle_data().unwrap();
        data.validate().unwrap();
        assert_eq!((data.tables.size.width as u32, data.tables.size.height as u32), recipe_data::SIZE);
        assert_eq!(data.tables.fonts.len(), recipe_data::FONTS.len());
        assert_eq!(data.tables.icons.len(), recipe_data::ICONS.len() + 1);
        for (font, &(name, _, size)) in data.tables.fonts.iter().zip(recipe_data::FONTS) {
            assert_eq!(data.name(font.name).unwrap(), name);
            assert_eq!(font.font_size, size as i32);
            assert_eq!(font.glyphs.count, 95);
        }
        // The white tile precedes named recipe artwork in both the direct and COM builders.
        assert_eq!(data.name(data.tables.icons[0].name).unwrap(), "white");
        for (icon, &(name, _)) in data.tables.icons[1..].iter().zip(recipe_data::ICONS) {
            assert_eq!(data.name(icon.name).unwrap(), name);
        }
        data.into_bundle().unwrap();
    }
}
