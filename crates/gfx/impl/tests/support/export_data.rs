// Independent nondefault input and semantic comparison for generated-source tests.
use neocogi_gfx_support::{AssetData, AssetTables, abi::*};

/// Appends one exact UTF-8 range; fonts precede icons as required by bundle admission.
fn name(bytes: &mut Vec<u8>, text: &str) -> AssetRange {
    let result = AssetRange {
        first: bytes.len() as u32,
        count: text.len() as u32,
    };
    bytes.extend_from_slice(text.as_bytes());
    result
}

/// Distinct nested states catch lost/swapped fields, including both patch variants.
fn visual(sequence: &mut u32) -> AssetVisual {
    *sequence += 1;
    let n = *sequence;
    AssetVisual {
        insets: Insets {
            left: (n % 5) as i32,
            top: 2,
            right: 3,
            bottom: 4,
        },
        center_visible: (n % 2) as u8,
        content_color: 0xfedc0000 | n,
        patch: if n.is_multiple_of(2) {
            AssetPatch::Image {
                icon: 1 + n % 8,
                source: Insets { left: 1, top: 2, right: 3, bottom: 4 },
                tint: 0x98760000 | n,
            }
        } else {
            AssetPatch::Flat {
                top_left: n,
                top: n + 1,
                top_right: n + 2,
                left: n + 3,
                center: n + 4,
                right: n + 5,
                bottom_left: n + 6,
                bottom: n + 7,
                bottom_right: n + 8,
            }
        },
    }
}

/// Every pointer branch gets independent data rather than a repeated default.
fn pointer(sequence: &mut u32) -> AssetPointerVisuals {
    AssetPointerVisuals {
        normal: visual(sequence),
        hovered: visual(sequence),
        pressed: visual(sequence),
    }
}

/// Minimal valid atlas with signed glyphs, UTF-8 names and asymmetric appearance data.
pub fn data() -> AssetData {
    let mut names = Vec::new();
    let fonts = vec![
        AssetFont {
            name: name(&mut names, "body"),
            glyphs: AssetRange { first: 0, count: 2 },
            line_size: 13,
            baseline: 9,
            font_size: 11,
        },
        AssetFont {
            name: name(&mut names, "méta"),
            glyphs: AssetRange { first: 2, count: 1 },
            line_size: 17,
            baseline: 12,
            font_size: 15,
        },
    ];
    let glyphs = ['_', 'é', '_']
        .into_iter()
        .enumerate()
        .map(|(index, scalar)| AssetGlyph {
            scalar: scalar as u32,
            offset_x: -3 + index as i32,
            offset_y: -7,
            advance_x: 5 + index as i32,
            advance_y: -2,
            rect: Rect {
                x: index as i32,
                y: 0,
                width: 1,
                height: 1,
            },
        })
        .collect::<Vec<_>>();
    let icons = core::iter::once("white")
        .chain(IconRole::ALL.into_iter().map(|role| role.canonical_name().unwrap()))
        .enumerate()
        .map(|(index, text)| AssetIcon {
            name: name(&mut names, text),
            rect: if index == 0 {
                Rect { x: 0, y: 0, width: 1, height: 1 }
            } else {
                Rect { x: 1, y: 1, width: 2, height: 2 }
            },
        })
        .collect::<Vec<_>>();
    let mut pixels = (0..64).map(|index| (index * 3) as u8).collect::<Vec<_>>();
    pixels[..4].fill(255);
    let mut sequence = 0;
    let surfaces = SurfaceRole::ALL
        .map(|_| AssetSurfaceVisuals {
            normal: visual(&mut sequence),
            disabled: visual(&mut sequence),
        })
        .to_vec();
    let controls = ControlRole::ALL
        .map(|_| AssetControlVisuals {
            disabled: visual(&mut sequence),
            enabled: pointer(&mut sequence),
            focused: pointer(&mut sequence),
        })
        .to_vec();
    let menus = MenuRole::ALL
        .map(|_| AssetMenuVisuals {
            normal: visual(&mut sequence),
            hovered: visual(&mut sequence),
            pressed: visual(&mut sequence),
            focused: visual(&mut sequence),
            open: visual(&mut sequence),
            disabled: visual(&mut sequence),
        })
        .to_vec();
    let window_chrome = ChromeRole::ALL
        .map(|_| AssetChromeVisuals {
            base: visual(&mut sequence),
            active: visual(&mut sequence),
            disabled: visual(&mut sequence),
        })
        .to_vec();
    AssetData {
        pixels: pixels.into(),
        tables: AssetTables {
            size: Size { width: 4, height: 4 },
            names: names.into(),
            fonts: fonts.into(),
            glyphs: glyphs.into(),
            icons: icons.into(),
            metrics: AssetMetrics {
                default_cell_width: 31,
                padding: 2,
                window_content_insets: Insets { left: 1, top: 2, right: 3, bottom: 4 },
                spacing: 3,
                indent: 7,
                title_height: 23,
                window_border: Insets { left: 3, top: 1, right: 4, bottom: 2 },
                scrollbar_size: 9,
                thumb_size: 6,
            },
            chrome: AssetChrome {
                title_alignment: TitleAlignment::Centered,
                close_side: CaptionSide::Trailing,
                minimize_side: CaptionSide::Leading,
                maximize_side: CaptionSide::Trailing,
                vertical_spacing: 3,
                minimum_extent: 11,
                inner_spacing: 4,
                show_without_activation: 1,
                minimized_content_height: 5,
                has_backdrop: 1,
                backdrop_rgba: 0x3322117f,
                backdrop_padding: 2,
            },
            surfaces: surfaces.into(),
            controls: controls.into(),
            menus: menus.into(),
            window_chrome: window_chrome.into(),
        },
    }
}

/// Comparison uses every semantic field and byte, never Debug, padding or pointer identity.
/// This is only a test observation format; generated production tables remain ordinary Rust.
pub fn snapshot(data: &AssetData) -> Vec<u8> {
    let mut out = Vec::new();
    let tables = &data.tables;
    put(&mut out, &[tables.size.width as i64, tables.size.height as i64]);
    put(&mut out, &[data.pixels.len() as i64]);
    out.extend_from_slice(&data.pixels);
    put(&mut out, &[tables.names.len() as i64]);
    out.extend_from_slice(&tables.names);
    put(&mut out, &[tables.fonts.len() as i64]);
    for font in tables.fonts.iter() {
        range(&mut out, font.name);
        range(&mut out, font.glyphs);
        put(&mut out, &[font.line_size as i64, font.baseline as i64, font.font_size as i64]);
    }
    put(&mut out, &[tables.glyphs.len() as i64]);
    for glyph in tables.glyphs.iter() {
        put(
            &mut out,
            &[
                glyph.scalar as i64,
                glyph.offset_x as i64,
                glyph.offset_y as i64,
                glyph.advance_x as i64,
                glyph.advance_y as i64,
            ],
        );
        rect(&mut out, glyph.rect);
    }
    put(&mut out, &[tables.icons.len() as i64]);
    for icon in tables.icons.iter() {
        range(&mut out, icon.name);
        rect(&mut out, icon.rect);
    }
    let m = tables.metrics;
    put(&mut out, &[m.default_cell_width as i64, m.padding as i64]);
    insets(&mut out, m.window_content_insets);
    put(&mut out, &[m.spacing as i64, m.indent as i64, m.title_height as i64]);
    insets(&mut out, m.window_border);
    put(&mut out, &[m.scrollbar_size as i64, m.thumb_size as i64]);
    let c = tables.chrome;
    put(
        &mut out,
        &[if c.title_alignment == TitleAlignment::Leading {
            0
        } else {
            assert_eq!(c.title_alignment, TitleAlignment::Centered);
            1
        }],
    );
    for side in [c.close_side, c.minimize_side, c.maximize_side] {
        put(
            &mut out,
            &[if side == CaptionSide::Leading {
                0
            } else {
                assert_eq!(side, CaptionSide::Trailing);
                1
            }],
        );
    }
    put(
        &mut out,
        &[
            c.vertical_spacing as i64,
            c.minimum_extent as i64,
            c.inner_spacing as i64,
            c.show_without_activation as i64,
            c.minimized_content_height as i64,
            c.has_backdrop as i64,
            c.backdrop_rgba as i64,
            c.backdrop_padding as i64,
        ],
    );
    put(&mut out, &[tables.surfaces.len() as i64]);
    for value in tables.surfaces.iter() {
        for item in [value.normal, value.disabled] {
            appearance(&mut out, item);
        }
    }
    put(&mut out, &[tables.controls.len() as i64]);
    for value in tables.controls.iter() {
        appearance(&mut out, value.disabled);
        for item in [value.enabled, value.focused] {
            for item in [item.normal, item.hovered, item.pressed] {
                appearance(&mut out, item);
            }
        }
    }
    put(&mut out, &[tables.menus.len() as i64]);
    for value in tables.menus.iter() {
        for item in [value.normal, value.hovered, value.pressed, value.focused, value.open, value.disabled] {
            appearance(&mut out, item);
        }
    }
    put(&mut out, &[tables.window_chrome.len() as i64]);
    for value in tables.window_chrome.iter() {
        for item in [value.base, value.active, value.disabled] {
            appearance(&mut out, item);
        }
    }
    out
}

/// Fixed-width primitive observation preserves signed values and ordering.
fn put(out: &mut Vec<u8>, values: &[i64]) {
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
}
/// Every range's starting offset and exact count matter independently.
fn range(out: &mut Vec<u8>, value: AssetRange) {
    put(out, &[value.first as i64, value.count as i64]);
}
/// Pixel coordinates and all four rectangle fields are observed.
fn rect(out: &mut Vec<u8>, value: Rect) {
    put(out, &[value.x as i64, value.y as i64, value.width as i64, value.height as i64]);
}
/// Asymmetric source/destination/metric insets catch swapped edges.
fn insets(out: &mut Vec<u8>, value: Insets) {
    put(out, &[value.left as i64, value.top as i64, value.right as i64, value.bottom as i64]);
}
/// All patch variants and every nested appearance value have distinct observations.
fn appearance(out: &mut Vec<u8>, value: AssetVisual) {
    insets(out, value.insets);
    put(out, &[value.center_visible as i64, value.content_color as i64]);
    match value.patch {
        AssetPatch::Flat {
            top_left,
            top,
            top_right,
            left,
            center,
            right,
            bottom_left,
            bottom,
            bottom_right,
        } => {
            put(
                out,
                &[
                    0,
                    top_left as i64,
                    top as i64,
                    top_right as i64,
                    left as i64,
                    center as i64,
                    right as i64,
                    bottom_left as i64,
                    bottom as i64,
                    bottom_right as i64,
                ],
            );
        }
        AssetPatch::Image { icon, source, tint } => {
            put(out, &[1, icon as i64]);
            insets(out, source);
            put(out, &[tint as i64]);
        }
    }
}
