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
//! Applications use `neocogi-ui`, whose concrete COM clients link no UI implementation.
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
    #[doc = include_str!("../docs/TYPOGRAPHY.md")]
    mod typography {}
    #[doc = include_str!("../docs/WIDGETS.md")]
    mod widgets {}
}

pub mod assets;
pub mod atlas;
mod context;
mod boundary;
mod event;
mod identity;
pub mod image;
mod input;
mod math;
mod menu;
pub mod render;
mod runtime;
#[cfg(test)]
mod test_support;
pub mod theme;
mod ui_node;
pub(crate) use ui_node::widgets;
mod ui_factory;
mod window_manager;

/// Resource, geometry and backend imports for implementation crates.
/// Applications and extension widgets use the neocogi-ui facade and canonical COM contracts.
pub mod prelude {
    pub use crate::atlas::{AtlasHandle, CharEntry, FontId, IconId};
    #[cfg(test)]
    pub use crate::atlas::{AtlasError, AtlasSource, FontEntry};
    pub use crate::image::{ImageError, ImageSource, ImageStorageError, MAX_DECODED_RGBA_BYTES, load_image_bytes};
    pub use crate::input::{Key, KeyEvent, KeyState, Modifiers, MouseButton};
    pub use crate::render::{
        AtlasUploadError, FrameError, FrameInfo, FrameInfoError, NinePatch, NinePatchCell, NinePatchCells, NinePatchContent, NinePatchImage, RendererBackend,
        RendererFrame, SliceInsets, TextureError, TextureId,
    };
    #[cfg(test)]
    pub use crate::test_support::custom_render::{CustomRenderHandle};
    pub use crate::math::{expand_rect, rect, vec2};
    pub use crate::theme::{
        CaptionButtonSide, CaptionButtonsSkin, ChromeRole, ChromeState, Color, ControlRole, ControlState, FlatPalette, FontRef, FontRole, FrameRole, IconRef,
        IconRole, MenuRole, MenuState, PointerState, ResourceCatalog, Skin, SkinBundle, SkinMetrics, SurfaceRole, SurfaceState, TitleBackdropSkin, Visual,
        WindowChromeSkin, WindowTitleAlignment, color,
    };
    pub use rs_math3d::{
        Box3f, Color4b, CrossProduct, Dimension, Dimensioni, FloatVector, Mat4f, Quat, Quatf, Rect, Recti, Vec2f, Vec2i, Vec3f, Vec4f, Vector, Vector3,
        color4b, ortho4,
    };
}

pub use atlas::{AtlasHandle, CharEntry, FontId, IconId};
#[cfg(test)]
pub use atlas::{AtlasError, AtlasSource, FontEntry};
pub(crate) use context::{Context};
#[cfg(test)]
pub(crate) use test_support::ui::Ui;
pub(crate) use window_manager::{PopupHandle, Window, WindowHandle, WindowOption};
#[cfg(test)]
pub(crate) use window_manager::{ChildWindowClip, DEFAULT_LAYER, LayerBinding, MIN_LAYER, SurfaceMutationError};
pub(crate) use event::{EventSource, TypedWidget, WidgetEventPortHandle};
pub(crate) use menu::{Menu, MenuBar, MenuItem, MenuItemAccessError, MenuItemHandle, MenuItemMark, MenuItemParameters};
#[cfg(test)]
pub(crate) use menu::MenuItemSubmitted;
pub use image::{ImageError, ImageSource, ImageStorageError, MAX_DECODED_RGBA_BYTES, load_image_bytes};
pub use input::{Key, KeyEvent, KeyState, Modifiers, MouseButton};
pub use math::{expand_rect, rect, vec2};
pub use render::{AtlasUploadError, NinePatch, NinePatchCell, NinePatchCells, NinePatchContent, NinePatchImage, SliceInsets, TextureError, TextureId};
pub use theme::{
    CaptionButtonSide, CaptionButtonsSkin, ChromeRole, ChromeState, Color, ControlRole, ControlState, FlatPalette, FontRef, FontRole, FrameRole, IconRef,
    IconRole, MenuRole, MenuState, PointerState, ResourceCatalog, Skin, SkinBundle, SkinMetrics, SurfaceRole, SurfaceState, TitleBackdropSkin, Visual,
    WindowChromeSkin, WindowTitleAlignment, color,
};
pub(crate) use theme::SkinInstallError;
pub(crate) use ui_node::{
    AvailableSpace, Change, ChildParticipation, Children, Constraints, ContainerLayoutCtx, ContainerWidget, KeyboardAction, KeyboardBehavior,
    LeafWidget, MeasureCtx, Node, ScrollbarAxis, ScrollbarChanged,
    ScrollbarParameters, TextWrap, TrackSize, UiInputEvent, Widget, WidgetFillOption, WidgetOption, WidgetPaintCtx, WidgetUpdateCtx,
};
pub use ui_node::{Disclosure, Grid, Linear, ScrollArea};
pub use ui_node::widgets::{Button, Checkbox, ColorSwatch, Combo, ListBox, ListItem, Number, Scrollbar, Slider, TextArea, TextBlock, Textbox};
#[cfg(test)]
pub(crate) use ui_node::{ScrollAreaOption};
#[cfg(test)]
pub(crate) use ui_node::{
    ChildrenHandle, Container, DisclosureParameters, GridParameters, LinearCrossSize, LinearItem, LinearParameters, ScrollAreaParameters,
    TypedWidgetHandle,
};
pub(crate) use widgets::{
    ButtonParameters, ButtonSubmitted, CheckboxChanged, CheckboxParameters, ColorSwatchParameters, ComboChanged, ComboParameters, ComboSubmitted,
    DecimalPrecision, ListBoxParameters, ListBoxSubmitted, ListItemParameters, ListItemSubmitted, NumberChanged, NumberParameters, SliderChanged,
    SliderParameters, TextAreaChanged, TextAreaSubmitted, Real, TextBlockParameters, TextboxChanged, TextboxParameters, TextboxSubmitted,
};
#[cfg(test)]
pub(crate) use widgets::TextEditor;
#[cfg(test)]
pub(crate) use widgets::{TextAreaParameters};
#[cfg(test)]
pub(crate) use widgets::{Custom, CustomParameters};

#[allow(unused_imports)]
pub(crate) use rs_math3d::{
    Box3f, Color4b, CrossProduct, Dimension, Dimensioni, FloatVector, Mat4f, Quat, Quatf, Rect, Recti, Vec2f, Vec2i, Vec3f, Vec4f, Vector, Vector3, color4b,
    ortho4,
};
pub(crate) use std::{cmp::max, hash::Hash, rc::Rc};
pub(crate) use render::geometry::UNCLIPPED_RECT;
pub(crate) use ui_node::UiRuntime;
pub use runtime::{Runtime, create_runtime_from_assets};
pub use ui_factory::UiFactory;

#[cfg(test)]
pub(crate) use event::SubscribeError;

// The runtime is the deployable UI component. Keeping its descriptor beside
// the implementations removes the former wrapper crate without introducing a
// second ABI boundary between traversal, built-in nodes, surfaces, and window
// management.
neocogi_com::plugin! {
    plugin_id(auto_guid = "neocogi.ui.runtime"),
    name = "neocogi.ui.runtime",
    version = "0.9.0-alpha.0",
    classes = [
        crate::Runtime,
        crate::UiFactory,
        crate::Button,
        crate::Checkbox,
        crate::Scrollbar,
        crate::Combo,
        crate::Textbox,
        crate::TextBlock,
        crate::TextArea,
        crate::Slider,
        crate::Number,
        crate::ColorSwatch,
        crate::ListItem,
        crate::ListBox,
        crate::Linear,
        crate::Grid,
        crate::ScrollArea,
        crate::Disclosure,
    ],
}

// Static consumers call `plugin_descriptor` directly. Only a final dynamic
// artifact exposes the stable loader entrypoint, avoiding duplicate exports in
// applications that link the runtime as an ordinary Rust library.
#[cfg(feature = "dynamic-plugin")]
neocogi_com::export_plugin!();
