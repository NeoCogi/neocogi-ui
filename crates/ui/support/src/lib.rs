//! Binary-local resource and presentation helpers for independent UI providers.
//!
//! This non-deployable crate owns validated asset imports, text/frame geometry, and phase-local
//! recording helpers. It owns no retained nodes, root storage, runtime, factory, concrete control,
//! or plugin descriptor. Providers communicate through generated core COM contracts; sharing
//! these Rust helpers neither gives nodes access to the engine nor makes core depend on nodes.

#![deny(missing_docs)]

pub mod assets;
pub mod atlas;
pub mod boundary;
pub mod frame;
pub mod identity;
pub mod image;
pub mod input;
pub mod input_contract;
pub mod interaction;
pub mod layout;
pub mod math;
pub mod render;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub mod text_layout;
pub mod theme;
pub mod widget_context;
mod widget_input;

/// Resource, geometry and backend imports for implementation crates.
/// Applications and extension widgets use the neocogi-ui facade and canonical COM contracts.
pub mod prelude {
    pub use crate::atlas::{AtlasHandle, FontId, IconId};
    pub use crate::image::{ImageError, ImageSource, ImageStorageError, MAX_DECODED_RGBA_BYTES, load_image_bytes};
    pub use crate::input::{Key, KeyEvent, KeyState, Modifiers, MouseButton};
    pub use crate::render::{
        FrameError, FrameInfo, FrameInfoError, NinePatch, NinePatchCell, NinePatchCells, NinePatchContent, NinePatchImage, RendererBackend, RendererFrame,
        SliceInsets, TextureError,
    };
    pub use crate::math::{expand_rect, rect, vec2};
    pub use crate::theme::{
        AssetChrome, AssetMetrics, CaptionSide, ChromeRole, ChromeState, Color, ControlRole, ControlState, FlatPalette, FontRef, FontRole, FrameRole, IconRef,
        IconRole, MenuRole, MenuState, PointerState, ResourceCatalog, Skin, SkinBundle, SurfaceRole, SurfaceState, TitleAlignment, Visual, color,
    };
    pub use rs_math3d::{
        Box3f, Color4b, CrossProduct, Dimension, Dimensioni, FloatVector, Mat4f, Quat, Quatf, Rect, Recti, Vec2f, Vec2i, Vec3f, Vec4f, Vector, Vector3,
        color4b, ortho4,
    };
}

pub use atlas::{AtlasHandle, FontId, IconId};
pub use image::{ImageError, ImageSource, ImageStorageError, MAX_DECODED_RGBA_BYTES, load_image_bytes};
pub use input::{Key, KeyEvent, KeyState, Modifiers, MouseButton};
pub use layout::{AvailableSpace, Constraints, TrackSize};
pub use math::{expand_rect, rect, vec2};
pub use render::{NinePatch, NinePatchCell, NinePatchCells, NinePatchContent, NinePatchImage, SliceInsets, TextureError};
pub use text_layout::TextWrap;
pub use interaction::{Change, KeyboardAction, KeyboardBehavior, WidgetOption};
pub use widget_context::{WidgetPaintCtx, WidgetUpdateCtx};
pub use widget_input::UiInputEvent;
pub use theme::{
    AssetChrome, AssetMetrics, CaptionSide, ChromeRole, ChromeState, Color, ControlRole, ControlState, FlatPalette, FontRef, FontRole, FrameRole, IconRef,
    IconRole, MenuRole, MenuState, PointerState, ResourceCatalog, Skin, SkinBundle, SurfaceRole, SurfaceState, TitleAlignment, Visual, color,
};
#[allow(unused_imports)]
pub(crate) use rs_math3d::{Color4b, Dimensioni, Recti, Vec2f, Vec2i, Vec4f};
pub(crate) use rs_math3d::{color4b, Dimension};
pub(crate) use std::{cmp::max, rc::Rc};

/// Dispatch-owner origin scopes; widget posting handles expose no scope mutation.
pub use neocogi_ui::events::EventOriginOwner;

// This unit-test executable owns one runtime; normal library builds never embed it.
#[cfg(test)]
extern crate neocogi_com_host as _;
