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
#![deny(missing_docs)]
// `clippy::pedantic` is useful as an occasional review tool for this crate, but these categories
// are intentionally outside the local lint profile. The UI/rendering path performs many bounded
// pixel/UV casts, internal modules use crate preludes heavily, and the retained public API
// should not grow `#[must_use]` or pedantic doc-section noise mechanically.
#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::similar_names,
    clippy::struct_field_names,
    clippy::too_many_arguments,
    clippy::wildcard_imports
)]
//! Retained implementation used by the UI plugin and native rendering/asset assemblies.
//!
//! Applications use `microui-redux`, whose concrete COM clients link no UI implementation.
//! Each retained COM node owns the widget capabilities used for routing, layout and painting.
//! The engine returns its traversal state before the application's `EventSession` dispatches
//! concrete handlers, then commits their geometry edits before processing the next input event.
//!
//! Measurement caches are invalidated by truthful update results, changed setters, constraints,
//! topology and installed resource revisions. Rendering observes the last complete commit and
//! rejects stale geometry before acquiring a native frame. Author obligations, deployment and
//! independent extension examples are documented in the workspace's `docs/COM_AUTHORS.md`,
//! `docs/COM_NATIVE.md` and `plugins/extension-example`.

// Compile the Rust examples in standalone guides as part of `cargo test --doc` without publishing
// duplicate guide modules in the normal API reference.
#[cfg(doctest)]
mod guide_doctests {
    #[doc = include_str!("../docs/ARCHITECTURE.md")]
    mod architecture {}
    #[doc = include_str!("../docs/BACKENDS.md")]
    mod backends {}
    #[doc = include_str!("../docs/LAYOUT.md")]
    mod layout {}
    #[doc = include_str!("../docs/MENUS.md")]
    mod menus {}
    #[doc = include_str!("../docs/SKINNING.md")]
    mod skinning {}
    // This guide's API is supplied entirely by the optional JSON theme loader.
    #[cfg(feature = "theme-json")]
    #[doc = include_str!("../docs/THEMES.md")]
    mod themes {}
    #[doc = include_str!("../docs/TYPOGRAPHY.md")]
    mod typography {}
    #[doc = include_str!("../docs/WIDGETS.md")]
    mod widgets {}
}

pub mod atlas;
pub mod assets;
mod context;
mod event;
mod identity;
pub mod image;
mod input;
mod math;
mod menu;
pub mod render;
#[cfg(test)]
mod test_support;
pub mod theme;
mod ui_node;
pub use ui_node::layout;
pub use ui_node::widgets;
mod window_manager;

/// Retained UI authoring types.
///
/// This module groups the stable retained concepts used by application code without exposing
/// low-level renderer details or manual container drawing helpers through default imports.
pub mod retained {
    pub use crate::event::{EventSource, TypedWidget, WidgetEventPortHandle};
    pub use crate::menu::{Menu, MenuBar, MenuItem, MenuItemAccessError, MenuItemHandle, MenuItemMark, MenuItemParameters, MenuItemSubmitted};
    pub use crate::render::{CustomRenderArgs, CustomRenderHandle};
    pub use crate::ui_node::{
        AvailableSpace, Change, ChildParticipation, Children, ChildrenHandle, Constraints, Container, ContainerLayoutCtx, ContainerWidget, Disclosure, DisclosureParameters,
        Grid, GridItem, GridParameters, GridSpan, KeyboardAction, KeyboardBehavior, LeafWidget, Linear, LinearCrossSize, LinearDirection, LinearItem,
        LinearParameters, MeasureCtx, Node, ScrollArea, ScrollAreaOption, ScrollAreaParameters, Scrollbar, ScrollbarAxis, ScrollbarChanged,
        ScrollbarParameters, TrackSize, UiInputEvent, Widget, TextWrap, TypedWidgetHandle, WidgetFillOption, WidgetOption, WidgetPaintCtx, WidgetUpdateCtx,
    };
    pub use crate::context::{Context, ContextFrame, Ui};
    pub use crate::window_manager::{
        ChildWindowClip, DEFAULT_LAYER, LayerBinding, MAX_LAYER, MIN_LAYER, PopupEvent, PopupHandle, SurfaceCreationError, SurfaceMutationError, Window,
        WindowEvent, WindowHandle, WindowOption,
    };
}

/// Common imports for retained UI applications.
///
/// The prelude intentionally favors retained authoring, widget state, skin/input/image types, and
/// renderer integration. Low-level backend contracts live under [`render`].
pub mod prelude {
    pub use crate::event::{EventSource, TypedWidget, WidgetEventPortHandle};
    pub use crate::atlas::{AtlasError, AtlasHandle, AtlasSource, CharEntry, FontEntry, FontId, IconId, SourceFormat};
    pub use crate::menu::{Menu, MenuBar, MenuItem, MenuItemAccessError, MenuItemHandle, MenuItemMark, MenuItemParameters, MenuItemSubmitted};
    pub use crate::image::{ImageError, ImageSource, ImageStorageError, MAX_DECODED_RGBA_BYTES, load_image_bytes};
    pub use crate::input::{Key, KeyEvent, KeyState, Modifiers, MouseButton};
    pub use crate::render::{
        AtlasUploadError, FrameError, FrameInfo, FrameInfoError, NinePatch, NinePatchCell, NinePatchCells, NinePatchContent, NinePatchImage, RendererBackend,
        RendererFrame, SliceInsets, TextureError, TextureId,
    };
    pub use crate::retained::{
        Change, ChildParticipation, ChildWindowClip, Children, ChildrenHandle, Container, ContainerLayoutCtx, ContainerWidget, Context, ContextFrame, Ui, CustomRenderArgs,
        AvailableSpace, Constraints, CustomRenderHandle, Disclosure, DisclosureParameters, Grid, GridItem, GridParameters, GridSpan, KeyboardAction,
        KeyboardBehavior, Linear, LinearCrossSize, LinearDirection, LinearItem, LinearParameters, Node, PopupEvent, PopupHandle, MeasureCtx,
        SurfaceCreationError, SurfaceMutationError, WindowEvent, WindowHandle, LayerBinding, DEFAULT_LAYER, MAX_LAYER, MIN_LAYER, ScrollArea, ScrollAreaOption,
        ScrollAreaParameters, Scrollbar, ScrollbarAxis, ScrollbarChanged, ScrollbarParameters, TrackSize, LeafWidget, TextWrap, UiInputEvent,
        TypedWidgetHandle, Widget, WidgetFillOption, WidgetOption, WidgetPaintCtx, WidgetUpdateCtx, Window, WindowOption,
    };
    pub use crate::math::{expand_rect, rect, vec2};
    pub use crate::theme::{
        CaptionButtonSide, CaptionButtonsSkin, ChromeRole, ChromeState, Color, ControlRole, ControlState, FlatPalette, FontRef, FontRole, FrameRole, IconRef,
        IconRole, MenuRole, MenuState, PointerState, ResourceCatalog, Skin, SkinBundle, SkinInstallError, SkinMetrics, SurfaceRole, SurfaceState,
        TitleBackdropSkin, Visual, WindowChromeSkin, WindowTitleAlignment, color,
    };
    #[cfg(feature = "theme-json")]
    pub use crate::theme::{LoadedTheme, THEME_SCHEMA_VERSION, ThemeLoadError};
    pub use crate::widgets::{
        Button, ButtonContent, ButtonParameters, ButtonSubmitted, Checkbox, CheckboxChanged, CheckboxParameters, ColorSwatch, ColorSwatchParameters, Combo,
        ComboChanged, ComboParameters, ComboSubmitted, Custom, CustomParameters, DecimalPrecision, DecimalPrecisionError, ListBox, ListBoxParameters,
        ListBoxSubmitted, ListItem, ListItemParameters, ListItemSubmitted, Number, NumberChanged, NumberParameters, NumericParameterError, Slider,
        SliderChanged, SliderParameters, TextArea, TextAreaChanged, TextAreaParameters, TextAreaSubmitted, TextBlock, TextBlockParameters, Textbox,
        TextboxChanged, TextboxParameters, TextboxSubmitted, Real,
    };
    pub use rs_math3d::{
        Box3f, Color4b, CrossProduct, Dimension, Dimensioni, FloatVector, Mat4f, Quat, Quatf, Rect, Recti, Vec2f, Vec2i, Vec3f, Vec4f, Vector, Vector3,
        color4b, ortho4,
    };
}

pub use atlas::{AtlasError, AtlasHandle, AtlasSource, CharEntry, FontEntry, FontId, IconId, SourceFormat};
pub use context::{Context, ContextFrame, Ui};
pub use window_manager::{
    ChildWindowClip, DEFAULT_LAYER, LayerBinding, MAX_LAYER, MIN_LAYER, PopupEvent, PopupHandle, SurfaceCreationError, SurfaceMutationError, Window,
    WindowEvent, WindowHandle, WindowOption,
};
pub use event::{EventSource, TypedWidget, WidgetEventPortHandle};
pub use menu::{Menu, MenuBar, MenuItem, MenuItemAccessError, MenuItemHandle, MenuItemMark, MenuItemParameters, MenuItemSubmitted};
pub use image::{ImageError, ImageSource, ImageStorageError, MAX_DECODED_RGBA_BYTES, load_image_bytes};
pub use input::{Key, KeyEvent, KeyState, Modifiers, MouseButton};
pub use math::{expand_rect, rect, vec2};
pub use render::{AtlasUploadError, NinePatch, NinePatchCell, NinePatchCells, NinePatchContent, NinePatchImage, SliceInsets, TextureError, TextureId};
pub use theme::{
    CaptionButtonSide, CaptionButtonsSkin, ChromeRole, ChromeState, Color, ControlRole, ControlState, FlatPalette, FontRef, FontRole, FrameRole, IconRef,
    IconRole, MenuRole, MenuState, PointerState, ResourceCatalog, Skin, SkinBundle, SkinInstallError, SkinMetrics, SurfaceRole, SurfaceState,
    TitleBackdropSkin, Visual, WindowChromeSkin, WindowTitleAlignment, color,
};
#[cfg(feature = "theme-json")]
pub use theme::{LoadedTheme, THEME_SCHEMA_VERSION, ThemeLoadError};
pub use ui_node::{
    AvailableSpace, Change, ChildParticipation, Children, ChildrenHandle, Constraints, Container, ContainerLayoutCtx, ContainerWidget, Disclosure, DisclosureParameters, Grid,
    GridItem, GridParameters, GridSpan, KeyboardAction, KeyboardBehavior, LeafWidget, Linear, LinearCrossSize, LinearDirection, LinearItem, LinearParameters,
    MeasureCtx, Node, ScrollArea, ScrollAreaOption, ScrollAreaParameters, Scrollbar, ScrollbarAxis, ScrollbarChanged, ScrollbarParameters, TextWrap, TrackSize,
    UiInputEvent, Widget, TypedWidgetHandle, WidgetFillOption, WidgetOption, WidgetPaintCtx, WidgetUpdateCtx,
};
pub use widgets::{
    Button, ButtonContent, ButtonParameters, ButtonSubmitted, Checkbox, CheckboxChanged, CheckboxParameters, ColorSwatch, ColorSwatchParameters, Combo,
    ComboChanged, ComboParameters, ComboSubmitted, Custom, CustomParameters, DecimalPrecision, DecimalPrecisionError, ListBox, ListBoxParameters,
    ListBoxSubmitted, ListItem, ListItemParameters, ListItemSubmitted, Number, NumberChanged, NumberParameters, NumericParameterError, Slider, SliderChanged,
    SliderParameters, TextArea, TextAreaChanged, TextAreaParameters, TextAreaSubmitted, Real, TextBlock, TextBlockParameters, Textbox, TextboxChanged,
    TextboxParameters, TextboxSubmitted,
};

#[allow(unused_imports)]
pub(crate) use rs_math3d::{
    Box3f, Color4b, CrossProduct, Dimension, Dimensioni, FloatVector, Mat4f, Quat, Quatf, Rect, Recti, Vec2f, Vec2i, Vec3f, Vec4f, Vector, Vector3, color4b,
    ortho4,
};
pub(crate) use std::{cmp::max, hash::Hash, rc::Rc};
pub(crate) use render::geometry::UNCLIPPED_RECT;
pub(crate) use ui_node::UiRuntime;

/// COM implementation entrypoints used by the UI plugin assembly.
pub mod com;

#[cfg(test)]
pub use event::SubscribeError;
