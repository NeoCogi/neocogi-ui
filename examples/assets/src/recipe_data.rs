//! Shared asset names and package-relative paths for native loading and Cargo compilation.

/// Atlas dimensions in pixels; every provider uses the same packing recipe.
pub const SIZE: (u32, u32) = (512, 256);
/// Opaque white tile used for solid geometry.
pub const WHITE: &str = "assets/WHITE.png";
/// Semantic icon name and relative PNG path, in catalog order.
pub const ICONS: &[(&str, &str)] = &[
    ("close", "assets/CLOSE.png"),
    ("expand", "assets/PLUS.png"),
    ("collapse", "assets/MINUS.png"),
    ("check", "assets/CHECK.png"),
    ("expand_down", "assets/EXPAND_DOWN.png"),
    ("open_folder", "assets/OPEN_FOLDER_16.png"),
    ("closed_folder", "assets/CLOSED_FOLDER_16.png"),
    ("file", "assets/FILE_16.png"),
];
/// Semantic font name, relative TrueType path and pixel size, in catalog order.
pub const FONTS: &[(&str, &str, u32)] = &[
    ("body", "assets/NORMAL.ttf", 12),
    ("small", "assets/NORMAL.ttf", 10),
    ("title", "assets/BOLD.ttf", 12),
    ("heading", "assets/NORMAL.ttf", 18),
    ("mono", "assets/CONSOLE.ttf", 14),
    ("calculator-display", "assets/CONSOLE.ttf", 28),
];
