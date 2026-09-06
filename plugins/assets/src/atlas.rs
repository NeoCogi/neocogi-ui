//
// Copyright 2023-Present (c) Raja Lehtihet & Wael El Oraiby
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

//! PNG packing and printable-ASCII font rasterization into portable asset tables.
//!
//! Construction owns CPU pixels and file input. Runtime font identities and UI skins
//! are deliberately absent: consumers create their local lookup state from the result.
use microui_contracts::*;
use microui_redux::assets::{AssetData, AssetTables};
use fontdue::{Font as RasterFont, FontSettings, Metrics};
use std::{
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};
use crate::{png::RgbaImage, style};

mod packer;
use packer::Packer;

/// Named font recipe shared by build-time generation and native loading.
#[derive(Clone)]
pub struct FontAsset {
    /// Resource name retained across derived bundles.
    pub name: String,
    /// Font file read only by the asset constructor.
    pub path: PathBuf,
    /// Positive rasterization size in pixels.
    pub size: usize,
}

/// Named PNG recipe shared by build-time generation and native loading.
#[derive(Clone)]
pub struct IconAsset {
    /// Resource name retained across derived bundles.
    pub name: String,
    /// PNG file decoded before any atlas mutation.
    pub path: PathBuf,
}

/// Complete ordered recipe for constructing the default application resources.
pub struct Config {
    /// Positive atlas width, subject to the shared RGBA budget.
    pub texture_width: usize,
    /// Positive atlas height, subject to the shared RGBA budget.
    pub texture_height: usize,
    /// Opaque-white tile used by solid geometry.
    pub white_icon: PathBuf,
    /// Named PNGs inserted in recipe order after the white tile.
    pub icons: Vec<IconAsset>,
    /// Fonts inserted in recipe order, including the required body font.
    pub fonts: Vec<FontAsset>,
}

/// One named font under construction, before contiguous table offsets are assigned.
struct Font {
    /// Owned name written into the final shared name pool.
    name: String,
    /// Pixel distance between baselines.
    line_size: i32,
    /// Baseline position within a line.
    baseline: i32,
    /// Requested rasterization size.
    size: i32,
    /// Concrete metrics and destination rectangles in Unicode order.
    glyphs: Vec<AssetGlyph>,
}

/// One named atlas icon under construction.
struct Icon {
    /// Owned name written after font names in the final name pool.
    name: String,
    /// Positive rectangle in the builder's CPU image.
    rect: Rect,
}

/// A glyph reservation prepared before any rasterization changes live atlas state.
struct PlannedGlyph {
    /// Printable Unicode scalar passed to both fontdue operations.
    character: char,
    /// Allocation-free metrics that the later rasterizer must reproduce.
    metrics: Metrics,
    /// Destination rectangle reserved against a cloned packer.
    rect: Rect,
}

/// Concrete mutable atlas constructor; failed insertion leaves its previous state intact.
pub struct Builder {
    /// Bounded CPU image used by every packing operation.
    image: RgbaImage,
    /// Skyline reservations published only after an operation succeeds.
    packer: Packer,
    /// Named fonts accumulated before the final contiguous table is built.
    fonts: Vec<Font>,
    /// Named icons accumulated before final name offsets are built.
    icons: Vec<Icon>,
}

impl Builder {
    /// Reads one complete file recipe in its specified packing order.
    pub fn from_config(config: &Config) -> io::Result<Self> {
        let mut builder = Self::new(config.texture_width, config.texture_height)?;
        builder.add_icon_named("white", &config.white_icon)?;
        for icon in &config.icons {
            builder.add_icon_named(&icon.name, &icon.path)?;
        }
        for font in &config.fonts {
            builder.add_font_named(&font.name, &font.path, font.size)?;
        }
        Ok(builder)
    }

    /// Allocates one empty bounded atlas, without reading files or assigning runtime IDs.
    pub fn new(width: usize, height: usize) -> io::Result<Self> {
        let size = Size {
            width: i32::try_from(width).map_err(io::Error::other)?,
            height: i32::try_from(height).map_err(io::Error::other)?,
        };
        let bytes = RgbaImage::byte_count(size)?;
        Ok(Self {
            image: RgbaImage { size, pixels: vec![0; bytes] },
            packer: Packer::new(size.width, size.height),
            fonts: Vec::new(),
            icons: Vec::new(),
        })
    }

    /// Repacks validated base resources, optionally omitting names replaced by a theme recipe.
    pub fn from_data(base: &AssetData, width: usize, height: usize, excluded_fonts: &[&str], excluded_icons: &[&str]) -> io::Result<Self> {
        // Validation precedes all indexing and copying. Stable names and the COM catalog,
        // rather than producer-local numeric IDs, identify resources across revisions.
        base.validate().map_err(io::Error::other)?;
        let mut builder = Self::new(width, height)?;
        for icon in base.tables.icons.iter() {
            let name = base.name(icon.name).map_err(io::Error::other)?;
            if !excluded_icons.contains(&name) {
                let pixels = Self::extract(base, icon.rect);
                let rect = builder.add_tile(icon.rect.width as usize, icon.rect.height as usize, &pixels)?;
                builder.icons.push(Icon { name: name.into(), rect });
            }
        }
        for font in base.tables.fonts.iter() {
            let name = base.name(font.name).map_err(io::Error::other)?;
            if excluded_fonts.contains(&name) {
                continue;
            }
            let first = font.glyphs.first as usize;
            let end = first + font.glyphs.count as usize;
            let mut glyphs = base.tables.glyphs[first..end].to_vec();
            // Deterministic scalar order gives repacking the same result for any producer.
            glyphs.sort_unstable_by_key(|glyph| glyph.scalar);
            for glyph in &mut glyphs {
                let pixels = Self::extract(base, glyph.rect);
                glyph.rect = builder.add_tile(glyph.rect.width as usize, glyph.rect.height as usize, &pixels)?;
            }
            builder.fonts.push(Font {
                name: name.into(),
                line_size: font.line_size,
                baseline: font.baseline,
                size: font.font_size,
                glyphs,
            });
        }
        Ok(builder)
    }

    /// Copies only a validated source rectangle, excluding adjacent artwork and unused padding.
    fn extract(base: &AssetData, rect: Rect) -> Vec<u8> {
        let mut pixels = Vec::with_capacity(rect.width as usize * rect.height as usize * 4);
        for row in rect.y..rect.y + rect.height {
            let start = (row as usize * base.tables.size.width as usize + rect.x as usize) * 4;
            pixels.extend_from_slice(&base.pixels[start..start + rect.width as usize * 4]);
        }
        pixels
    }

    /// Decodes and inserts a named PNG after checking that the name is available.
    pub fn add_icon_named(&mut self, name: &str, path: &Path) -> io::Result<u32> {
        self.check_icon_name(name)?;
        let bytes = Self::read_asset_file(path)?;
        self.add_icon_bytes_named(name, path, &bytes)
    }

    /// Inserts caller-supplied PNG bytes while preserving the source path in diagnostics.
    pub fn add_icon_bytes_named(&mut self, name: &str, path: &Path, bytes: &[u8]) -> io::Result<u32> {
        self.check_icon_name(name)?;
        let image = RgbaImage::decode(bytes).map_err(|error| io::Error::other(format!("{}: {error}", path.display())))?;
        let rect = self.add_tile(image.size.width as usize, image.size.height as usize, &image.pixels)?;
        let index = self.icons.len() as u32;
        self.icons.push(Icon { name: name.into(), rect });
        Ok(index)
    }

    /// Rejects duplicate and unrepresentable icon names before file I/O or packing.
    fn check_icon_name(&self, name: &str) -> io::Result<()> {
        if name.is_empty()
            || name.len() > MAX_ASSET_NAMES as usize
            || self.icons.len() >= MAX_ASSET_ICONS as usize
            || self.icons.iter().any(|icon| icon.name == name)
        {
            return Err(io::Error::other(format!("invalid or duplicate icon name `{name}`")));
        }
        Ok(())
    }

    /// Rasterizes a named font file without retaining any source-file or decoder state.
    pub fn add_font_named(&mut self, name: &str, path: &Path, size: usize) -> io::Result<()> {
        self.check_font(name, size)?;
        let bytes = Self::read_asset_file(path)?;
        self.add_font_bytes_named(name, path, size, &bytes)
    }

    /// Reserves the complete printable range before rasterizing and publishing a font.
    pub fn add_font_bytes_named(&mut self, name: &str, path: &Path, size: usize, bytes: &[u8]) -> io::Result<()> {
        self.check_font(name, size)?;
        if bytes.len() > MAX_ASSET_PIXELS_BYTES {
            return Err(io::Error::other("font input exceeds 64 MiB"));
        }
        let font = RasterFont::from_bytes(bytes, FontSettings::default()).map_err(|error| io::Error::other(format!("{}: {error}", path.display())))?;
        let mut next_packer = self.packer.clone();
        let mut planned = Vec::with_capacity(95);
        let mut min_y = i64::MAX;
        let mut max_y = i64::MIN;
        for scalar in 32u8..127 {
            let character = char::from(scalar);
            let metrics = font.metrics(character, size as f32);
            let rect = self.reserve(&mut next_packer, metrics.width, metrics.height)?;
            let bottom = size as i64 - i64::from(metrics.ymin) - i64::from(rect.height);
            min_y = min_y.min(bottom);
            max_y = max_y.max(bottom);
            planned.push(PlannedGlyph { character, metrics, rect });
        }
        let (line_size, baseline) = match font.horizontal_line_metrics(size as f32) {
            Some(metrics) => (metrics.new_line_size.round() as i32, metrics.ascent.round() as i32),
            None => {
                let height = i32::try_from(max_y.saturating_sub(min_y)).map_err(io::Error::other)?;
                (height, height)
            }
        };
        if line_size <= 0 || baseline < 0 || baseline > line_size {
            return Err(io::Error::other(format!("font `{name}` has invalid baseline metrics")));
        }

        // Stage every bitmap before touching the live image. Fontdue's metrics and
        // rasterization operations must describe exactly the same glyph storage.
        let mut glyphs = Vec::with_capacity(planned.len());
        let mut bitmaps = Vec::with_capacity(planned.len());
        for glyph in &planned {
            let (metrics, bitmap) = font.rasterize(glyph.character, size as f32);
            if metrics != glyph.metrics || metrics.width.checked_mul(metrics.height) != Some(bitmap.len()) {
                return Err(io::Error::other("font metrics changed during rasterization"));
            }
            let mut rgba = Vec::with_capacity(bitmap.len() * 4);
            for alpha in bitmap {
                rgba.extend_from_slice(&[255, 255, 255, alpha]);
            }
            bitmaps.push(rgba);
            glyphs.push(AssetGlyph {
                scalar: u32::from(glyph.character),
                offset_x: metrics.xmin,
                offset_y: metrics.ymin,
                advance_x: metrics.advance_width as i32,
                advance_y: metrics.advance_height as i32,
                rect: glyph.rect,
            });
        }
        for (glyph, pixels) in planned.iter().zip(&bitmaps) {
            self.copy_tile(glyph.rect, pixels);
        }
        self.packer = next_packer;
        self.fonts.push(Font {
            name: name.into(),
            line_size,
            baseline,
            size: size as i32,
            glyphs,
        });
        Ok(())
    }

    /// Rejects unusable font names and sizes before opening files or reserving rectangles.
    fn check_font(&self, name: &str, size: usize) -> io::Result<()> {
        if name.is_empty()
            || name.len() > MAX_ASSET_NAMES as usize
            || self.fonts.len() >= MAX_ASSET_FONTS as usize
            || size == 0
            || size > i32::MAX as usize
            || self.fonts.iter().any(|font| font.name == name)
        {
            return Err(io::Error::other(format!("invalid or duplicate font `{name}` at size {size}")));
        }
        Ok(())
    }

    /// Reads a source file with a hard ceiling even if its contents grow during the read.
    pub fn read_asset_file(path: &Path) -> io::Result<Vec<u8>> {
        let file = File::open(path)?;
        if file.metadata()?.len() > MAX_ASSET_PIXELS_BYTES as u64 {
            return Err(io::Error::other(format!("{} exceeds 64 MiB", path.display())));
        }
        let mut bytes = Vec::new();
        file.take(MAX_ASSET_PIXELS_BYTES as u64 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > MAX_ASSET_PIXELS_BYTES {
            return Err(io::Error::other(format!("{} exceeds 64 MiB", path.display())));
        }
        Ok(bytes)
    }

    /// Reserves a tile against a candidate packer and checks its coordinates before use.
    fn reserve(&self, packer: &mut Packer, width: usize, height: usize) -> io::Result<Rect> {
        let width = i32::try_from(width).map_err(io::Error::other)?;
        let height = i32::try_from(height).map_err(io::Error::other)?;
        if width == 0 || height == 0 {
            return Ok(Rect::default());
        }
        let rect = packer
            .pack(width, height, false)
            .ok_or_else(|| io::Error::other("atlas has no room for the requested tile"))?;
        if rect.x < 0
            || rect.y < 0
            || rect.width != width
            || rect.height != height
            || i64::from(rect.x) + i64::from(width) > i64::from(self.image.size.width)
            || i64::from(rect.y) + i64::from(height) > i64::from(self.image.size.height)
        {
            return Err(io::Error::other("packer returned an invalid tile rectangle"));
        }
        Ok(rect)
    }

    /// Inserts one exact RGBA tile, publishing the packer only after all checks succeed.
    fn add_tile(&mut self, width: usize, height: usize, pixels: &[u8]) -> io::Result<Rect> {
        if width.checked_mul(height).and_then(|count| count.checked_mul(4)) != Some(pixels.len()) {
            return Err(io::Error::other("tile dimensions do not match RGBA storage"));
        }
        let mut packer = self.packer.clone();
        let rect = self.reserve(&mut packer, width, height)?;
        self.copy_tile(rect, pixels);
        self.packer = packer;
        Ok(rect)
    }

    /// Copies rows after reservation has proved the source and destination bounds.
    fn copy_tile(&mut self, rect: Rect, pixels: &[u8]) {
        let width = rect.width as usize * 4;
        for row in 0..rect.height as usize {
            let destination = ((rect.y as usize + row) * self.image.size.width as usize + rect.x as usize) * 4;
            self.image.pixels[destination..destination + width].copy_from_slice(&pixels[row * width..(row + 1) * width]);
        }
    }

    /// Consumes the constructor into complete validated tables with the default resolved style.
    pub fn build(self) -> io::Result<AssetData> {
        let mut names = Vec::new();
        let mut fonts = Vec::new();
        let mut glyphs = Vec::new();
        let mut icons = Vec::new();
        for font in self.fonts {
            let name = append_name(&mut names, &font.name)?;
            let range = AssetRange {
                first: glyphs.len() as u32,
                count: font.glyphs.len() as u32,
            };
            glyphs.extend(font.glyphs);
            fonts.push(AssetFont {
                name,
                glyphs: range,
                line_size: font.line_size,
                baseline: font.baseline,
                font_size: font.size,
            });
        }
        for icon in self.icons {
            icons.push(AssetIcon {
                name: append_name(&mut names, &icon.name)?,
                rect: icon.rect,
            });
        }
        let mut data = AssetData {
            pixels: self.image.pixels.into(),
            tables: AssetTables {
                size: self.image.size,
                fonts: fonts.into(),
                glyphs: glyphs.into(),
                icons: icons.into(),
                names: names.into(),
                metrics: AssetMetrics::default(),
                chrome: style::compile_chrome(WindowChromeSkin::default()),
                surfaces: Vec::new().into(),
                controls: Vec::new().into(),
                menus: Vec::new().into(),
                window_chrome: Vec::new().into(),
            },
        };
        style::apply_flat(&mut data, Insets::uniform(1), &FlatPalette::default());
        data.validate().map_err(io::Error::other)?;
        Ok(data)
    }
}

/// Appends one bounded UTF-8 name while deriving its exact contiguous range.
fn append_name(names: &mut Vec<u8>, name: &str) -> io::Result<AssetRange> {
    let end = names
        .len()
        .checked_add(name.len())
        .ok_or_else(|| io::Error::other("asset name pool overflow"))?;
    if end > MAX_ASSET_NAMES as usize {
        return Err(io::Error::other("asset name pool exceeds 4 MiB"));
    }
    let range = AssetRange {
        first: names.len() as u32,
        count: name.len() as u32,
    };
    names.extend_from_slice(name.as_bytes());
    Ok(range)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A partially planned font must not consume atlas space or alter any live pixels.
    #[test]
    fn failed_font_reservation_leaves_pixels_and_packing_unchanged() {
        let recipe = crate::test_recipe::atlas_config();
        let mut attempted = Builder::new(64, 64).unwrap();
        let mut original = Builder::new(64, 64).unwrap();
        attempted.add_icon_named("white", &recipe.white_icon).unwrap();
        original.add_icon_named("white", &recipe.white_icon).unwrap();
        assert!(attempted.add_font_named("too-large", &recipe.fonts[0].path, 32).is_err());
        assert!(attempted.fonts.is_empty());
        assert_eq!(attempted.image.pixels, original.image.pixels);

        // Identical later placements prove that a failed draft also retained its packer.
        attempted.add_icon_named("after-failure", &recipe.white_icon).unwrap();
        original.add_icon_named("after-failure", &recipe.white_icon).unwrap();
        assert_eq!(attempted.icons[1].rect, original.icons[1].rect);
        assert_eq!(attempted.image.pixels, original.image.pixels);
    }
}
