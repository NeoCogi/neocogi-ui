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
//! Backend-independent retained root-window coordination.
//!
//! [`WindowManager`] owns retained roots, ordered input, modal policy, layout state, and display-list
//! recording. The production retained context owns it beside the GFX compositor. WindowManager chooses the eligible root, then the [`UiRuntime`]-owned
//! input router chooses a node; neither invokes application widget-event subscribers.
use bitflags::bitflags;

use crate::runtime::input::Input;
use crate::runtime::menu::MenuBar;
use crate::runtime::render::DisplayList;
use crate::runtime::{Dimensioni, Node, Recti, Skin, SkinBundle, UiRuntime};
use roots::{SurfaceForest, SurfaceKey};
mod root_chrome;
mod roots;

pub use root_chrome::WindowHandle;
// Popup identity/events and checked surface failures describe forest policy rather than chrome.
// Re-export both borrowed-mutation and owner-preserving creation errors with the public window API.
pub use roots::{PopupHandle, SurfaceMutationError};

bitflags! {
    #[derive(Copy, Clone)]
    /// Presentation options shared by windows, dialogs, and window-owned popup surfaces.
    pub struct WindowOption : u32 {
        /// Shows a minimize caption button that toggles the skin-sized compact window height.
        const MINIMIZE_BUTTON = 1;
        /// Shows a maximize caption button that toggles the inherited viewport and saved rectangle.
        const MAXIMIZE_BUTTON = 2;
        /// Keeps the surface visible while disabling its chrome, menus, and complete widget tree.
        ///
        /// Disabled roots continue ordinary update traversal so programmatic state stays current,
        /// but they cannot own pointer, keyboard, popup, move, resize, or caption interactions.
        const DISABLED = 4;
        /// Gives the surface a Skin-owned outer border and inset content area.
        const FRAME = 1024;
        /// Adapts the surface width to its content while retaining its programmed height.
        const AUTO_WIDTH = 256;
        /// Adapts the surface height to its content while retaining its programmed width.
        const AUTO_HEIGHT = 512;
        /// Adapts both surface axes to intrinsic content size.
        const AUTO_SIZE = Self::AUTO_WIDTH.bits() | Self::AUTO_HEIGHT.bits();
        /// Hides the title bar.
        const NO_TITLE = 128;
        /// Hides the close button.
        const NO_CLOSE = 64;
        /// Prevents the user from resizing the window or dialog.
        const NO_RESIZE = 16;
        /// Removes the Skin-owned inset around surface content.
        ///
        /// This is useful for edge-to-edge application surfaces whose content, such as a menu bar,
        /// already owns its internal spacing. It is independent of [`Self::FRAME`]: removing the
        /// frame does not otherwise remove the ordinary window-content inset.
        const NO_PADDING = 8;
        /// No special options.
        const NONE = 0;
    }
}

/// Complete retained definition consumed when a window or dialog is created.
///
/// The optional [`MenuBar`] and descendant clip policy belong to this value rather than to an
/// application-side coordinator. Creation transfers the body, bar, and recursive menus to the
/// window manager as one owner. Child windows remain separate `Window` values registered later
/// through [`crate::runtime::Ui::create_child_window`].
pub struct Window {
    /// Diagnostic name and visible title text.
    name: String,
    /// Initial outer rectangle in screen coordinates.
    rect: Recti,
    /// Uniquely owned application body displayed below an optional menu bar.
    content: Node,
    /// Declarative bar and recursive menu hierarchy installed with this window.
    menu_bar: Option<MenuBar>,
    /// Policy applied to the complete surfaces of structural child windows.
    child_window_clip: ChildWindowClip,
}

impl Window {
    /// Creates a window definition without a menu bar.
    pub fn new(name: impl Into<String>, rect: Recti, content: Node) -> Self {
        // Keep all construction values together so creation cannot install a detached bar later.
        Self {
            name: name.into(),
            rect,
            content,
            menu_bar: None,
            child_window_clip: ChildWindowClip::None,
        }
    }

    /// Transfers all construction values to the private manager implementation.
    pub(crate) fn into_parts(self) -> (String, Recti, Node, Option<MenuBar>, ChildWindowClip) {
        // Destructuring makes the single ownership transfer explicit at the registration boundary.
        (self.name, self.rect, self.content, self.menu_bar, self.child_window_clip)
    }
}

/// Process-unique identity for a window or dialog retained by [`crate::runtime::Context`].
///
/// Application code carries a [`WindowHandle`] whose private copy selects this concrete key. A
/// process-wide source prevents a handle created by one Context from colliding with a surface in
/// another, while the distinct wrapper prevents popup keys from entering root-only operations.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub(crate) struct RootId(
    /// Shared non-reused value hidden behind the root-specific type boundary.
    crate::runtime::identity::ProcessUniqueId,
);

impl RootId {
    /// Allocates a new root identity independently from every event endpoint allocation.
    pub(crate) fn allocate() -> Self {
        // The concrete wrapper is retained in both the forest key and the private application
        // handle; neither exposes the shared numeric value.
        Self(crate::runtime::identity::ProcessUniqueId::allocate())
    }
}

/// Lowest application-selectable root layer.
pub const MIN_LAYER: u8 = 0;
/// Highest application-selectable root layer.
pub const MAX_LAYER: u8 = 15;
/// Layer assigned to newly created independent windows.
pub const DEFAULT_LAYER: u8 = MAX_LAYER;

/// Controls whether one window narrows the visible region of its structural child windows.
///
/// This policy belongs to the parent window rather than to any individual child, so every direct
/// child receives the same structural boundary without storing a second clip setting. The policy
/// affects each complete descendant surface, including its background, application body, chrome,
/// menu bar, and popup surfaces.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum ChildWindowClip {
    /// Lets child windows use the complete clip inherited from earlier ancestors and the viewport.
    #[default]
    None,
    /// Intersects every direct child's inherited clip with this window's application body.
    ///
    /// The body excludes the parent frame, title, padding, and intrinsic menu bar. Descendants are
    /// therefore unable to paint or receive uncaptured pointer input over those parent-owned areas.
    Content,
}

/// Describes the structural stacking layer of one retained window.
///
/// Independent windows use a caller-selectable fixed layer and structural children report the
/// fixed layer inherited from their top-level family root. Dialogs occupy a dedicated modal layer
/// above every fixed value. Popups do not expose a binding because they derive their transient band
/// directly from their owner window.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LayerBinding {
    /// A selected or structurally inherited application layer in the inclusive range
    /// [`MIN_LAYER`]..=[`MAX_LAYER`].
    Fixed(u8),
    /// A dialog in the dedicated layer above all application-selectable layers.
    Modal,
}

/// Backend- and application-state-independent retained window manager.
pub(crate) struct WindowManager {
    /// First receiver failure from infallible native routing, surfaced at the COM phase boundary.
    event_error: Option<neocogi_com::NcStatus>,
    /// Reusable operation storage for window-manager frame and chrome drawing.
    display_list: DisplayList,
    /// Atomic resolved skin and atlas shared by every retained surface and chrome part.
    bundle: SkinBundle,

    /// Concrete ownership forest for independent windows, child windows, dialogs, and popups.
    ///
    /// Every retained surface lives in one collection and has at most one parent edge. The forest
    /// uses that collection's root-node order as global activation chronology and separately caches
    /// the parent-first visible traversal used by layout and retained updates. Paint and pointer
    /// selection recursively project the same parent edges in opposite directions around overlays.
    surfaces: SurfaceForest,
    /// Whether drag/release events from a capture revoked with a dismissed popup must be swallowed.
    ///
    /// Popup-local routers cannot consume this tail after their surface leaves the active path, so
    /// the cross-surface manager retains the gesture boundary until release.
    discard_pointer_capture_tail: bool,
    /// Sole retained surface selected for keyboard and text input.
    ///
    /// A concrete root or popup identity is stored directly instead of projecting every transient
    /// back to its owning window. Activation remains independent of stacking, so a user can focus
    /// a surface in a low layer without moving it over windows in a higher layer.
    active_surface: Option<SurfaceKey>,
    /// Whether an unchorded Alt press is waiting for its matching release.
    ///
    /// A non-Alt key cancels this candidate so Alt+Down can reach a focused combo without first
    /// entering the window menu.
    pending_menu_alt: bool,
    /// Whether a manager-owned Ctrl+F6 chord is waiting for its physical F6 release.
    ///
    /// Retaining this one transition bit prevents the release from reaching the newly activated
    /// window even when the user releases Control before F6 and the platform modifier snapshot no
    /// longer identifies the original chord.
    window_cycle_key_down: bool,
    /// Ordered input state owned and consumed directly by this window manager.
    input: Input,
    /// Dimensions of the most recent complete update/layout commit.
    ui_commit: Option<Dimensioni>,
    /// Conservative surface mutation marker consumed by transient private dispatch selection.
    dispatch_order_changed: bool,
}

impl WindowManager {
    /// Completes the native transition before exposing a failed synchronous port post.
    fn posted(&mut self, result: Result<(), neocogi_com::NcStatus>) {
        if self.event_error.is_none() {
            self.event_error = result.err();
        }
    }
    /// Returns and clears the first event transport failure for this runtime operation.
    pub(crate) fn event_result(&mut self) -> Result<(), neocogi_com::NcStatus> {
        self.event_error.take().map_or(Ok(()), Err)
    }

    /// Queues an ordered host cancellation and requires a new UI commit before paint.
    pub(crate) fn cancel_input(&mut self) {
        self.input.cancel();
        self.invalidate_ui_commit();
    }

    /// Creates an empty manager with a resolved skin and no committed UI frame.
    pub(crate) fn new(bundle: SkinBundle) -> Self {
        // Retained identities come from the process-wide allocator, so manager construction needs
        // no local namespace or counter that could collide with another Context.
        Self {
            event_error: None,
            display_list: DisplayList::new(),
            bundle,
            surfaces: SurfaceForest::new(),
            discard_pointer_capture_tail: false,
            active_surface: None,
            pending_menu_alt: false,
            window_cycle_key_down: false,
            input: Input::default(),
            ui_commit: None,
            dispatch_order_changed: true,
        }
    }

    /// Marks the current retained layout as unavailable for rendering.
    pub(crate) fn invalidate_ui_commit(&mut self) {
        // Every semantic or input mutation must be followed by an update before paint.
        self.ui_commit = None;
        self.dispatch_order_changed = true;
    }

    /// Consumes surface changes only after selection can be rebuilt; node edits have their own marker.
    pub(crate) fn take_dispatch_order_changed(&mut self) -> bool {
        std::mem::take(&mut self.dispatch_order_changed)
    }

    /// Borrows the atomic resolved skin and atlas installed in this manager.
    pub(crate) fn skin_bundle(&self) -> &SkinBundle {
        // Context uses this same pair for public projections and renderer synchronization checks.
        &self.bundle
    }

    /// Replaces the complete skin bundle and invalidates geometry measured with its old pair.
    pub(crate) fn set_skin_bundle(&mut self, bundle: SkinBundle) {
        // Every SkinBundle owns a fresh complete revision. Retained measurement entries compare
        // that token lazily, so hidden trees need no eager downward invalidation traversal.
        self.bundle = bundle;
        self.invalidate_ui_commit();
    }

    /// Queues one pointer-movement transition in input order.
    pub(crate) fn mousemove(&mut self, x: i32, y: i32) {
        // Pending input makes the previous update/layout commit unrenderable.
        self.input.mousemove(x, y);
        self.invalidate_ui_commit();
    }

    /// Queues one pointer-button press in input order.
    pub(crate) fn mousedown(&mut self, x: i32, y: i32, button: crate::runtime::MouseButton) {
        // Preserve the exact call order so popup dismissal and revealed-target routing are atomic.
        self.input.mousedown(x, y, button);
        self.invalidate_ui_commit();
    }

    /// Queues one pointer-button release in input order.
    pub(crate) fn mouseup(&mut self, x: i32, y: i32, button: crate::runtime::MouseButton) {
        // Captured chrome or content consumes the release during the next retained update.
        self.input.mouseup(x, y, button);
        self.invalidate_ui_commit();
    }

    /// Queues one scroll delta in input order.
    pub(crate) fn scroll(&mut self, x: i32, y: i32) {
        // Scroll targeting depends on the layout and pointer position at this ordered transition.
        self.input.scroll(x, y);
        self.invalidate_ui_commit();
    }

    /// Queues one logical keyboard transition in input order.
    pub(crate) fn key(&mut self, event: crate::runtime::KeyEvent) {
        // Focus resolution remains deferred until the next complete retained update. The single
        // transition retains its own modifier snapshot, avoiding parallel key queues that could
        // disagree about ordering.
        self.input.key(event);
        self.invalidate_ui_commit();
    }

    /// Queues UTF-8 text input for the current retained focus owner.
    pub(crate) fn text(&mut self, text: &str) {
        // The input buffer copies the text, so this borrowed slice need not outlive the call.
        self.input.text(text);
        self.invalidate_ui_commit();
    }

    /// Returns whether dimensions, input, and visible widget state match the last complete update.
    pub(crate) fn can_render(&self, dimensions: Dimensioni) -> bool {
        // Rendering is observational and therefore requires exact dimensions, an empty FIFO, and
        // no context-free measurement mutation in a tree that the committed frame would paint.
        self.ui_commit
            .is_some_and(|committed| (committed.width, committed.height) == (dimensions.width, dimensions.height))
            && !self.input.has_pending()
            && !self.surfaces.has_visible_measurement_dirty()
    }

    /// Borrows the reusable display list after retained paint has recorded it.
    pub(crate) fn display_list_mut(&mut self) -> &mut DisplayList {
        // The production compositor replay drains this same manager-owned allocation.
        &mut self.display_list
    }
}
