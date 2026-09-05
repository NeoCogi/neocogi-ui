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

//! Private frame coordinator used by the UI COM runtime.
//!
//! Context owns the concrete renderer and delegates retained roots, input, layout,
//! modality and style to WindowManager. Application event dispatch belongs to the
//! facade EventSession; the native authoring convenience lives only in test fixtures.

use crate::window_manager::{WindowManager};
#[cfg(test)]
use crate::{Ui, PopupHandle};
#[cfg(test)]
use crate::window_manager::RootId;
#[cfg(test)]
use crate::render::RenderItemArgs;
use crate::render::{FrameInfo, RenderError, Renderer, RendererBackend};
#[cfg(test)]
use crate::test_support::custom_render::{CustomRenderHandle, CustomRenderRegistryError};
use crate::{KeyEvent, MouseButton, Recti, ResourceCatalog, SkinBundle};
#[cfg(test)]
use crate::{AtlasHandle, Dimensioni, Skin, TextureError, TextureId};
#[cfg(all(test, feature = "theme-json"))]
use crate::{LoadedTheme, ThemeLoadError};
#[cfg(all(test, feature = "theme-json"))]
use std::path::Path;

/// Private owner of the retained window manager and concrete rendering executor.
///
/// The COM runtime supplies ordered input and owns application event dispatch. This coordinator
/// lends the manager only for a bounded transaction, then validates and submits the completed
/// display list. WindowManager owns focus, capture, modal routing and child-window clipping;
/// Context holds no second tree or callback dispatcher.
///
/// A ContextFrame exclusively borrows this owner until submission or cancellation. The enclosing
/// COM runtime adds phase/reentry authority for callers retaining independent widget interfaces.
pub struct Context<B: RendererBackend> {
    /// High-level renderer that replays root display lists.
    renderer: Renderer<B>,
    /// Immutable application fonts and icons from which every later theme is derived.
    resource_catalog: ResourceCatalog,
    /// Backend- and application-state-independent retained window manager.
    pub(crate) window_manager: WindowManager,
    /// Drawable size used by retained behavior tests that drive complete frames tersely.
    #[cfg(test)]
    test_dimensions: Dimensioni,
}

/// One private paint/submission borrow of the retained frame coordinator.
///
/// Submission consumes this frame; dropping it cancels without acquiring GPU resources. The
/// enclosing COM runtime checks phase authority and the last geometry commit before painting.
#[must_use = "call render_ui() to submit this UI frame; dropping it cancels"]
pub struct ContextFrame<'a, B: RendererBackend> {
    context: &'a mut Context<B>,
    info: FrameInfo,
    completed: bool,
}

// Construction and test fixtures.

impl<B: RendererBackend> Context<B> {
    /// Creates a new UI context with unique ownership of the provided backend.
    ///
    /// The default style binds conventional semantic font and icon names from the backend atlas.
    ///
    /// # Panics
    ///
    /// Panics when the backend atlas lacks the `body` font or any semantic icon required by
    /// [`crate::IconRole::ALL`]. Every constructible atlas already has a validated white
    /// rendering tile.
    #[cfg(test)]
    pub fn new(backend: B) -> Self {
        let catalog = ResourceCatalog::new(backend.get_atlas());
        Self::from_bundle(backend, catalog.default_skin_bundle())
    }

    /// Constructs retained traversal from the exact same immutable revision as COM widget services.
    /// The caller initializes its backend from this bundle's atlas before publishing either owner.
    pub(crate) fn from_bundle(backend: B, bundle: SkinBundle) -> Self {
        let renderer = Renderer::new(backend);
        debug_assert!(renderer.atlas().ptr_eq(bundle.atlas()), "backend and retained bundle must share one atlas");
        let resource_catalog = ResourceCatalog::new(bundle.atlas().clone());
        Self {
            renderer,
            resource_catalog,
            window_manager: WindowManager::new(bundle),
            #[cfg(test)]
            test_dimensions: Dimensioni::new(1, 1),
        }
    }

    /// Borrows the context-owned retained UI domain for root and popup operations.
    ///
    /// The returned concrete façade is the same capability supplied to context-aware event
    /// handlers. Its borrow ends independently of renderer and event-dispatcher APIs, allowing
    /// application code to keep those concerns separate without duplicating manager methods on
    /// `Context` itself.
    #[cfg(test)]
    pub fn ui(&mut self) -> Ui<'_> {
        // Restrict the borrow to WindowManager so the façade remains backend- and application-independent.
        Ui::new(&mut self.window_manager)
    }

    /// Borrows the immutable application font and icon catalog captured at construction.
    ///
    /// Selecting another skin bundle never changes this value. Applications can therefore create
    /// checked exact [`crate::FontRef`] and [`crate::IconRef`] values here and keep their typed IDs
    /// in retained widget state across themes derived from this catalog.
    #[cfg(test)]
    pub fn resource_catalog(&self) -> &ResourceCatalog {
        // Expose one-time named lookup without exposing the catalog's source atlas itself.
        &self.resource_catalog
    }

    /// Creates a Context whose test-only frame helper uses `dimensions`.
    #[cfg(test)]
    pub(crate) fn new_test(backend: B, dimensions: Dimensioni) -> Self {
        let mut context = Self::new(backend);
        context.test_dimensions = dimensions;
        context
    }
}

impl<B: RendererBackend> Context<B> {
    /// Updates and renders once for retained behavior tests.
    #[cfg(test)]
    pub(crate) fn update_and_render_ui(&mut self) {
        let info = FrameInfo::try_new(self.test_dimensions, crate::color(0, 0, 0, 0)).expect("test Context dimensions must be positive");
        self.update_ui(self.test_dimensions);
        self.frame(info).render_ui().expect("test backend frame should render");
    }
}

// Retained update and application-event dispatch.

impl<B: RendererBackend> Context<B> {
    /// Runs eventless widget work, drains input, and commits layout for a polling-only context.
    #[track_caller]
    #[cfg(test)]
    pub fn update_ui(&mut self, dimensions: Dimensioni) {
        assert!(dimensions.width > 0 && dimensions.height > 0, "update_ui dimensions must be positive");
        self.window_manager.update(dimensions);
    }
}

// Ordered input forwarding.

impl<B: RendererBackend> Context<B> {
    /// Queues one mouse-pointer position transition without coalescing.
    pub fn mousemove(&mut self, x: i32, y: i32) {
        self.window_manager.mousemove(x, y);
    }

    /// Queues one mouse-button press at the supplied pointer position.
    pub fn mousedown(&mut self, x: i32, y: i32, btn: MouseButton) {
        self.window_manager.mousedown(x, y, btn);
    }

    /// Queues one mouse-button release at the supplied pointer position.
    pub fn mouseup(&mut self, x: i32, y: i32, btn: MouseButton) {
        self.window_manager.mouseup(x, y, btn);
    }

    /// Queues one scroll-wheel or trackpad delta without accumulating adjacent calls.
    pub fn scroll(&mut self, x: i32, y: i32) {
        self.window_manager.scroll(x, y);
    }

    /// Queues one backend-normalized logical keyboard transition.
    ///
    /// Character keys identify shortcut candidates; composed UTF-8 input must still be forwarded
    /// separately through [`Self::text`]. The supplied modifier snapshot is authoritative for this
    /// transition and for later pointer updates until another key transition replaces it.
    pub fn key(&mut self, event: KeyEvent) {
        self.window_manager.key(event);
    }

    /// Queues one UTF-8 text transition, including an empty string.
    ///
    /// Each editor applies its documented storage policy: single-line controls remove line endings,
    /// while multiline controls normalize CR and CRLF to LF. Rendering remains limited to glyphs
    /// in the selected atlas font and substitutes underscore metrics for missing characters.
    pub fn text(&mut self, text: &str) {
        self.window_manager.text(text);
    }

    /// Queues loss of host focus or pointer capture in input order.
    ///
    /// Hosts must call this on blur, pointer cancellation, or unexpected capture loss. The next
    /// update clears held buttons/modifiers and widget focus/capture without generating a click.
    /// Text editors apply their normal blur policy; a cancellation does not roll back their value.
    pub fn cancel_input(&mut self) {
        self.window_manager.cancel_input();
    }

    /// Returns the focused text editor's visible area in committed framebuffer coordinates.
    ///
    /// Hosts can place their native text input/IME anchor here. This is the editor's content area,
    /// not an exact caret rectangle. Returns `None` before a UI commit or outside editable focus.
    /// Custom editors opt in with [`crate::KeyboardBehavior::TEXT_INPUT`].
    pub fn text_input_area(&self) -> Option<Recti> {
        self.window_manager.text_input_area()
    }
}

// Skin, backend callbacks, and renderer resources.

impl<B: RendererBackend> Context<B> {
    /// Registers one backend-specific callback for retained custom-render nodes.
    ///
    /// The callback is observational with respect to retained application state, topology,
    /// interaction, and layout. It may mutate callback-private rendering caches, but using a
    /// captured [`TypedWidgetHandle`](crate::TypedWidgetHandle) to mutate retained UI during frame
    /// execution is a contract violation rather than a deferred-next-frame update.
    ///
    /// A callback written for another backend frame type cannot be registered, and the active frame
    /// borrow cannot escape one callback invocation. These negative guarantees are checked by the
    /// diagnostic-matched fixtures under `tests/ui/`; ordinary doctests are unsuitable because any
    /// unrelated compiler error would also make a `compile_fail` block appear successful.
    #[cfg(test)]
    pub fn register_custom_renderer<F>(&mut self, callback: F) -> Result<CustomRenderHandle<B>, CustomRenderRegistryError>
    where
        F: for<'frame> FnMut(&mut B::Frame<'frame>, RenderItemArgs) + 'static,
    {
        self.renderer.register_custom_renderer(callback)
    }

    /// Replaces the current UI skin.
    ///
    /// Image-backed visuals must originate from this Context's current bundle atlas. Start from
    /// `context.skin().clone()` when changing scalar values.
    ///
    /// # Panics
    ///
    /// Panics when the skin contains any resource identity absent from the current atlas.
    #[cfg(test)]
    pub fn set_skin(&mut self, skin: Skin) {
        // Re-pair the edited skin with the existing immutable atlas at the one validated boundary.
        // The renderer needs no upload because this operation cannot replace atlas pixels.
        let bundle = SkinBundle::new(self.window_manager.skin_bundle().atlas().clone(), skin);
        self.window_manager.set_skin_bundle(bundle);
    }

    /// Uploads and installs one complete skin bundle as a single context transaction.
    ///
    /// The backend receives the atlas before the retained manager publishes either projection. If
    /// upload fails, the previous bundle remains active and renderable.
    ///
    /// # Errors
    ///
    /// Returns [`crate::SkinInstallError`] if any original catalog identity is missing, or if the
    /// backend rejects the upload. Matching names alone do not establish resource compatibility.
    pub fn set_skin_bundle(&mut self, bundle: &SkinBundle) -> Result<(), crate::SkinInstallError> {
        // A valid standalone bundle can still omit resources used by this Context's widgets.
        // Reject that case before invoking the backend, then publish only after a successful upload.
        self.resource_catalog.validate_bundle(bundle)?;
        self.renderer.replace_atlas(bundle.atlas().clone()).map_err(crate::SkinInstallError::Upload)?;
        debug_assert!(self.renderer.atlas().ptr_eq(bundle.atlas()));
        self.window_manager.set_skin_bundle(bundle.clone());
        Ok(())
    }

    /// Installs the loaded theme's complete skin bundle as one transaction.
    ///
    /// The backend uploads the theme atlas before this Context publishes the new bundle. If upload
    /// fails, the previous bundle remains active. Theme artwork is baked into the bundle atlas, so
    /// no secondary texture transaction is required.
    ///
    /// # Errors
    ///
    /// Returns [`crate::SkinInstallError`] for incompatible catalog resources or a backend upload
    /// failure. The current theme remains unchanged.
    ///
    #[cfg(feature = "theme-json")]
    #[cfg(test)]
    pub fn set_theme(&mut self, theme: &LoadedTheme) -> Result<(), crate::SkinInstallError> {
        // LoadedTheme contributes only a selection name around the same atomic runtime value.
        self.set_skin_bundle(theme.bundle())
    }

    /// Borrows the complete resolved skin and matching immutable atlas.
    #[cfg(test)]
    pub fn skin_bundle(&self) -> &SkinBundle {
        // WindowManager owns the authoritative pair used by every retained phase.
        self.window_manager.skin_bundle()
    }

    /// Returns the resolved UI skin currently used by this context.
    #[cfg(test)]
    pub fn skin(&self) -> &Skin {
        // Preserve the concise read API as a projection, never as an independently owned field.
        self.skin_bundle().skin()
    }

    /// Returns the immutable atlas capability supplied by this context's rendering backend.
    ///
    /// Applications use this handle to resolve named fonts/icons and to construct a compatible
    /// [`Skin`]. Frame execution and resource bookkeeping remain context-owned implementation
    /// details; custom GPU work continues through [`Context::register_custom_renderer`].
    #[cfg(test)]
    pub fn atlas(&self) -> AtlasHandle {
        // Return the atlas from the same bundle as `skin()` rather than consulting a parallel
        // renderer cache. Successful installation keeps the renderer on this exact allocation.
        self.skin_bundle().atlas().clone()
    }

    /// Attempts to load an RGBA image into this Context and returns its [`TextureId`].
    ///
    /// Dimensions and byte length are validated before an id is allocated; no failed operation
    /// records texture state in the Context.
    ///
    /// # Errors
    ///
    /// Returns a concrete [`TextureError`] that distinguishes invalid image data, identifier
    /// exhaustion, and backend creation or upload failure.
    #[cfg(test)]
    pub fn try_load_image_rgba(&mut self, width: i32, height: i32, pixels: &[u8]) -> Result<TextureId, TextureError> {
        self.renderer.try_load_texture_rgba(width, height, pixels)
    }

    /// Loads a versioned JSON theme and bakes every assigned image patch into its immutable atlas.
    ///
    /// Relative image paths are resolved against the JSON file's directory. Missing patch entries
    /// remain concrete flat-color patches derived from the document's palette. Each unique image
    /// path becomes one atlas region, allowing callers to retain several
    /// [`LoadedTheme`] values and switch their complete skin bundles safely.
    ///
    /// # Errors
    ///
    /// Returns [`ThemeLoadError`] for definition I/O, strict JSON schema errors, invalid role or
    /// slice data, font/artwork atlas construction, or image decoding. Loading never mutates the live
    /// backend; only a later [`Context::set_theme`] uploads the completed atlas transactionally.
    #[cfg(feature = "theme-json")]
    #[cfg(test)]
    pub fn load_theme_file(&mut self, path: impl AsRef<Path>) -> Result<LoadedTheme, ThemeLoadError> {
        // The loader owns every construction stage and returns only the complete immutable bundle.
        // Applications can preload several values and choose one later without repeated file I/O.
        crate::theme::loader::load(path.as_ref(), self.resource_catalog.atlas())
    }
}

// Test-only retained-state inspection.

#[cfg(test)]
impl<B: RendererBackend> Context<B> {
    /// Returns visible window and popup names in base-surface paint order for internal tests.
    pub(crate) fn debug_rendered_root_names(&self) -> Vec<String> {
        // Names expose ordering policy without leaking private stacking records.
        self.window_manager.debug_rendered_root_names()
    }

    /// Returns the manager-owned root name for internal behavioral tests.
    pub(crate) fn debug_root_name(&self, root: RootId) -> Option<String> {
        // Forward through the test-only façade without exposing production chrome access.
        self.window_manager.debug_root_name(root)
    }

    /// Returns the authoritative root rectangle for internal behavioral tests.
    pub(crate) fn debug_root_rect(&self, root: RootId) -> Option<Recti> {
        // Copy geometry out so tests cannot retain references into manager storage.
        self.window_manager.debug_root_rect(root)
    }

    /// Returns the committed effective surface clip for internal hierarchy tests.
    pub(crate) fn debug_root_clip(&self, root: RootId) -> Option<Recti> {
        // Copy the private snapshot used jointly by retained paint and cross-window hit testing.
        self.window_manager.debug_root_clip(root)
    }

    /// Returns whether a registered root is visible for internal behavioral tests.
    pub(crate) fn debug_root_visible(&self, root: RootId) -> Option<bool> {
        // Preserve `None` for an unknown or destroyed root.
        self.window_manager.debug_root_visible(root)
    }

    /// Returns a window-owned popup's authoritative rectangle for internal tests.
    pub(crate) fn debug_popup_rect(&self, popup: &PopupHandle) -> Option<Recti> {
        // Keep popup inspection typed just like the production mutation façade.
        self.window_manager.debug_popup_rect(popup)
    }

    /// Returns whether a popup belongs to the manager's active path for internal tests.
    pub(crate) fn debug_popup_visible(&self, popup: &PopupHandle) -> Option<bool> {
        // Popup definitions store no visibility mirror, so the manager derives this from its path.
        self.window_manager.debug_popup_visible(popup)
    }

    /// Returns the laid-out content size of a window-owned popup for internal tests.
    pub(crate) fn debug_popup_content_size(&self, popup: &PopupHandle) -> Option<Dimensioni> {
        // Keep layout inspection behind the typed handle so popup storage remains private.
        self.window_manager.debug_popup_content_size(popup)
    }

    /// Returns one retained node rectangle inside a window-owned popup for internal tests.
    pub(crate) fn debug_popup_node_rect(&self, popup: &PopupHandle, node: crate::ui_node::RuntimeNodeId) -> Option<Recti> {
        // The manager validates both the popup handle and retained node before returning geometry.
        self.window_manager.debug_popup_node_rect(popup, node)
    }

    /// Returns the active popup branch in parent-to-child order for internal tests.
    pub(crate) fn debug_active_popup_names(&self) -> Vec<String> {
        // Names make path assertions readable without exposing private popup identifiers.
        self.window_manager.debug_active_popup_names()
    }

    /// Returns active popup rectangles in parent-to-child order for internal anchor tests.
    pub(crate) fn debug_active_popup_rects(&self) -> Vec<Recti> {
        // Geometry order matches the authoritative popup path used by layout and paint.
        self.window_manager.debug_active_popup_rects()
    }

    /// Returns relational menu trigger rectangles in their compiled declaration order for tests.
    pub(crate) fn debug_menu_anchor_rects(&self, root: RootId) -> Option<Vec<Option<Recti>>> {
        // The manager resolves private trigger identities; Context exposes only copied geometry.
        self.window_manager.debug_menu_anchor_rects(root)
    }

    /// Returns the committed full menu-bar allocation for root layout tests.
    pub(crate) fn debug_menu_bar_rect(&self, root: RootId) -> Option<Recti> {
        self.window_manager.debug_menu_bar_rect(root)
    }

    /// Returns active compact menu rows in parent-to-child popup order for tests.
    pub(crate) fn debug_active_menu_row_rects(&self) -> Vec<Vec<Recti>> {
        // Logical declaration slots replace the removed per-row RuntimeNodeId identities.
        self.window_manager.debug_active_menu_row_rects()
    }

    /// Returns whether any manager-owned chrome gesture is active for tests.
    pub(crate) fn debug_root_active(&self, root: RootId) -> Option<bool> {
        // This intentionally excludes application widget capture details.
        self.window_manager.debug_root_active(root)
    }

    /// Returns whether a title move gesture is active for internal tests.
    pub(crate) fn debug_root_moving(&self, root: RootId) -> Option<bool> {
        // Delegate exact private interaction inspection to WindowManager.
        self.window_manager.debug_root_moving(root)
    }

    /// Returns whether a resize gesture is active for internal tests.
    pub(crate) fn debug_root_resizing(&self, root: RootId) -> Option<bool> {
        // Delegate exact private interaction inspection to WindowManager.
        self.window_manager.debug_root_resizing(root)
    }

    /// Returns the currently active ordinary window for internal routing tests.
    pub(crate) fn debug_active_root(&self) -> Option<RootId> {
        // Activation is intentionally distinct from z-order and modal selection.
        self.window_manager.debug_active_root()
    }

    /// Returns the frontmost visible dialog for internal modal-policy tests.
    pub(crate) fn debug_modal_root(&self) -> Option<RootId> {
        // The manager derives modal activation from flat window visibility and z-order.
        self.window_manager.debug_modal_root()
    }

    /// Returns the committed application-body rectangle for internal chrome tests.
    pub(crate) fn debug_root_body(&self, root: RootId) -> Option<Recti> {
        // The manager retains the exact body snapshot consumed by application layout.
        self.window_manager.debug_root_body(root)
    }

    /// Returns the committed framed client rectangle for internal chrome tests.
    pub(crate) fn debug_root_client(&self, root: RootId) -> Option<Recti> {
        // Client geometry lets tests distinguish full-width chrome from padded application content.
        self.window_manager.debug_root_client(root)
    }

    /// Returns retained traversal counters for one window or dialog in internal tests.
    pub(crate) fn debug_root_runtime_metrics(&self, root: RootId) -> Option<crate::ui_node::RuntimeMetrics> {
        // Metrics stay test-only so production traversal exposes no diagnostic state.
        self.window_manager.debug_root_runtime_metrics(root)
    }

    /// Returns whether window chrome or application content owns pointer capture in tests.
    pub(crate) fn debug_root_has_pointer_capture(&self, root: RootId) -> Option<bool> {
        // Combine both capture domains exactly as cross-window routing does.
        self.window_manager.debug_root_has_pointer_capture(root)
    }

    /// Counts application-authored retained nodes inside one window for internal tests.
    pub(crate) fn debug_root_node_count(&self, root: RootId) -> Option<usize> {
        // Manager chrome is intentionally absent from this application topology count.
        self.window_manager.debug_root_node_count(root)
    }

    /// Returns one retained application node rectangle inside a window for internal tests.
    pub(crate) fn debug_root_node_rect(&self, root: RootId, node: crate::ui_node::RuntimeNodeId) -> Option<Recti> {
        // Resolve the node through the window-local runtime without exposing that runtime.
        self.window_manager.debug_root_node_rect(root, node)
    }

    /// Returns title, close, and resize rectangles for one window in internal tests.
    pub(crate) fn debug_root_chrome(&self, root: RootId) -> Option<(Option<Recti>, Option<Recti>, Option<Recti>)> {
        // Use the renderer atlas so test geometry matches production title measurement.
        self.window_manager.debug_root_chrome(root)
    }

    /// Returns optional caption and per-axis resize rectangles for internal window tests.
    pub(crate) fn debug_root_chrome_controls(&self, root: RootId) -> Option<crate::window_manager::DebugRootChromeControls> {
        // Use the renderer atlas for the same title-height calculation as committed layout.
        self.window_manager.debug_root_chrome_controls(root)
    }
}

// Logical frame ownership and submission.

impl<B: RendererBackend> Context<B> {
    /// Starts one paint/submission frame for state previously committed by
    /// [`Context::update_ui`].
    pub fn frame(&mut self, info: FrameInfo) -> ContextFrame<'_, B> {
        ContextFrame { context: self, info, completed: false }
    }
}

impl<B: RendererBackend> ContextFrame<'_, B> {
    /// Consumes this logical frame, paints the last committed UI once, and submits it once.
    ///
    /// Returns [`RenderError::UiUpdateRequired`] before paint or backend acquisition when no commit
    /// exists for these dimensions, raw input is pending, or an invalidating typed mutation dirtied
    /// a visible widget tree. Specialized measurement-preserving setters may leave the current
    /// commit renderable while derived state waits for the next update. This operation is
    /// paint-only: it does not route input, update
    /// semantic state, run layout, synthesize timers, or produce a generic
    /// frame-result/resource-state object. Widget paint is observational with respect to
    /// application-authored semantic state, topology, interaction, and committed layout. Widgets
    /// may update private rendering caches; custom-render callbacks may update callback-private
    /// rendering caches only. Neither kind of cache can alter the current commit or publish
    /// application-coordination events.
    pub fn render_ui(mut self) -> Result<(), RenderError> {
        let dimensions = self.info.dimensions();
        if !self.context.window_manager.can_render(dimensions) {
            self.completed = true;
            return Err(RenderError::UiUpdateRequired);
        }
        self.context.window_manager.paint(self.info.dimensions());
        let Context { renderer, window_manager, .. } = &mut *self.context;
        let result = renderer.render(self.info, window_manager.display_list_mut());
        self.completed = true;
        result
    }
}

impl<B: RendererBackend> Drop for ContextFrame<'_, B> {
    fn drop(&mut self) {
        if !self.completed {
            self.context.window_manager.cancel_frame();
        }
    }
}

#[cfg(all(test, feature = "theme-json", feature = "save-to-rust"))]
mod theme_tests {
    //! Context-level theme upload transaction tests.

    use super::*;
    use crate::render::{AtlasUploadError, FrameError, RendererFrame, Vertex};
    #[cfg(test)]
    use crate::render::{TextureError};
    use crate::test_support::{recording_backend, test_atlas};
    use std::fs;

    /// Minimal backend that can deterministically accept or reject atlas publication.
    struct ThemeAtlasBackend {
        /// Atlas returned to a newly constructed Context and replaced after successful uploads.
        atlas: AtlasHandle,
        /// Whether the next replacement should fail without changing `atlas`.
        reject_replacement: bool,
    }

    /// Empty frame used because these tests exercise resource mutation between frames only.
    #[must_use]
    struct ThemeAtlasFrame;

    impl RendererFrame for ThemeAtlasFrame {
        /// Completes this backend fixture; it owns no fallible native submission.
        fn finish(self) -> Result<(), crate::render::FrameError> {
            Ok(())
        }

        /// Ignores atlas-backed quads because no test frame is rendered.
        fn push_quad(&mut self, _vertices: [Vertex; 4]) {}

        /// Ignores atlas-backed triangles because no test frame is rendered.
        fn push_triangle(&mut self, _vertices: [Vertex; 3]) {}

        /// Has no buffered work to flush.
        fn flush(&mut self) {}

        /// Ignores external textures because no test frame is rendered.
        fn draw_texture(&mut self, _id: TextureId, _vertices: [Vertex; 4]) {}
    }

    impl RendererBackend for ThemeAtlasBackend {
        type Frame<'a> = ThemeAtlasFrame;

        /// Returns the currently published atlas capability.
        fn get_atlas(&self) -> AtlasHandle {
            self.atlas.clone()
        }

        /// Models the required all-or-nothing backend replacement contract.
        fn replace_atlas(&mut self, atlas: AtlasHandle) -> Result<(), AtlasUploadError> {
            if self.reject_replacement {
                return Err(AtlasUploadError::new("fixture rejected atlas"));
            }
            self.atlas = atlas;
            Ok(())
        }

        /// Provides an inert frame for completeness of the backend contract.
        fn frame(&mut self, _info: FrameInfo) -> Result<Self::Frame<'_>, FrameError> {
            Ok(ThemeAtlasFrame)
        }

        /// Accepts validated theme PNGs without retaining native resources.
        fn create_texture(&mut self, _id: TextureId, _pixels: &[u8]) -> Result<(), TextureError> {
            Ok(())
        }

        /// Has no native texture allocation to release.
        fn destroy_texture(&mut self, _id: TextureId) {}
    }

    /// Encodes one small opaque RGBA PNG for a temporary theme fixture.
    fn fixture_png(width: u32, height: u32) -> Vec<u8> {
        // Use the same png crate version as the production decoder while keeping the fixture fully
        // generated and independent of repository assets.
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("fixture PNG header must encode");
            let pixels = vec![0xFF; width as usize * height as usize * 4];
            writer.write_image_data(pixels.as_slice()).expect("fixture PNG pixels must encode");
        }
        bytes
    }

    /// Verifies a failed theme load never mutates backend texture state.
    #[test]
    fn failed_theme_load_leaves_backend_textures_untouched() {
        let directory = tempfile::tempdir().expect("temporary theme directory must be available");
        fs::write(directory.path().join("button.png"), fixture_png(3, 3)).expect("fixture PNG must be writable");
        fs::write(
            directory.path().join("theme.json"),
            r#"{
                "schema_version": 1,
                "name": "Rollback",
                "appearances": {
                    "control": {
                        "button": {
                            "insets": { "left": 1, "top": 1, "right": 1, "bottom": 1 },
                            "enabled": { "normal": { "patch": { "type": "image", "path": "button.png" } } }
                        },
                        "checkbox": {
                            "enabled": { "normal": { "patch": { "type": "image", "path": "absent.png" } } }
                        }
                    }
                }
            }"#,
        )
        .expect("fixture JSON must be writable");
        // Leave enough atlas capacity that the fixture reaches asset I/O before packing can fail;
        // this isolates the intended missing-file error while retaining CPU-local rollback.
        let source = crate::atlas::builder::Builder::from_atlas_with_size(&test_atlas(), 64, 64)
            .expect("expanded fixture resources must fit")
            .build()
            .expect("expanded fixture atlas must validate");
        let (backend, log) = recording_backend(source);
        let mut context = Context::<_>::new(backend);
        log.clear();

        let error = match context.load_theme_file(directory.path().join("theme.json")) {
            Ok(_) => panic!("missing second image must fail atlas construction"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            ThemeLoadError::AtlasBuild {
                source: crate::atlas::builder::BuilderError::Asset { source },
            } if source.kind() == std::io::ErrorKind::NotFound && source.to_string().contains("absent.png")
        ));
        assert!(
            log.snapshot().is_empty(),
            "theme decoding and atlas construction must remain CPU-local until selection"
        );
    }

    /// Verifies a successful selection publishes one matching skin bundle to the Context.
    #[test]
    fn set_theme_replaces_atlas_and_skin_together() {
        let initial = test_atlas();
        let replacement = crate::atlas::builder::Builder::from_atlas_with_size(&initial, 64, 64).unwrap().build().unwrap();
        let replacement_style = Skin::from_atlas(&replacement);
        let replacement_font = replacement_style.resolve_font_role(&replacement, crate::FontRole::Body);
        let theme = LoadedTheme::new("Replacement", SkinBundle::new(replacement.clone(), replacement_style));
        let mut context = Context::<ThemeAtlasBackend>::new(ThemeAtlasBackend {
            atlas: initial.clone(),
            reject_replacement: false,
        });

        context.set_theme(&theme).expect("fixture backend must accept replacement atlas");

        assert!(context.atlas().ptr_eq(&replacement));
        assert!(!context.atlas().ptr_eq(&initial));
        assert_eq!(context.skin().resolve_font_role(&context.atlas(), crate::FontRole::Body), replacement_font);
    }

    /// Verifies an atlas upload error leaves both sides of the active theme pair unchanged.
    #[test]
    fn set_theme_failure_preserves_previous_atlas_and_skin() {
        let initial = test_atlas();
        let initial_font = Skin::from_atlas(&initial).resolve_font_role(&initial, crate::FontRole::Body);
        let replacement = crate::atlas::builder::Builder::from_atlas_with_size(&initial, 64, 64).unwrap().build().unwrap();
        let theme = LoadedTheme::new("Rejected", SkinBundle::new(replacement.clone(), Skin::from_atlas(&replacement)));
        let mut context = Context::<ThemeAtlasBackend>::new(ThemeAtlasBackend {
            atlas: initial.clone(),
            reject_replacement: true,
        });

        let error = context.set_theme(&theme).expect_err("fixture backend must reject replacement atlas");

        assert_eq!(error, crate::SkinInstallError::Upload(AtlasUploadError::new("fixture rejected atlas")));
        assert!(context.atlas().ptr_eq(&initial));
        assert_eq!(context.skin().resolve_font_role(&context.atlas(), crate::FontRole::Body), initial_font);
    }

    /// Optional application resources are protected even when the candidate's own skin is valid.
    #[test]
    fn missing_catalog_resources_are_rejected_before_backend_upload() {
        let directory = tempfile::tempdir().unwrap();
        let icon_path = directory.path().join("application.png");
        fs::write(&icon_path, fixture_png(1, 1)).unwrap();
        let base = crate::test_support::test_atlas_with_font_sizes(&[("body", 10), ("application-font", 12)]);
        let mut builder = crate::atlas::builder::Builder::from_atlas_with_size(&base, 64, 64).unwrap();
        builder.add_icon_named("application-icon", &icon_path).unwrap();
        let source = builder.build().unwrap();

        for omit_font in [true, false] {
            // Excluding an optional resource still permits a fully valid standalone SkinBundle.
            let fonts = if omit_font { &["application-font"][..] } else { &[] };
            let icons = if omit_font { &[][..] } else { &["application-icon"] };
            let candidate = crate::atlas::builder::Builder::from_atlas_with_size_excluding(&source, 64, 64, fonts, icons)
                .unwrap()
                .build()
                .unwrap();
            let bundle = SkinBundle::from_atlas(candidate);
            let (backend, log) = recording_backend(source.clone());
            let mut context = Context::<_>::new(backend);
            log.clear();
            let error = context.set_skin_bundle(&bundle).unwrap_err();
            if omit_font {
                assert!(matches!(error, crate::SkinInstallError::MissingFont { name, id } if name == "application-font" && Some(id) == source.font_id(&name)));
            } else {
                assert!(matches!(error, crate::SkinInstallError::MissingIcon { name, id } if name == "application-icon" && Some(id) == source.icon_id(&name)));
            }
            assert!(log.snapshot().is_empty(), "incompatible bundles must not reach the backend");
            assert!(context.atlas().ptr_eq(&source));
        }
    }

    /// Verifies consecutive theme loads always derive from the pristine application catalog.
    #[test]
    fn consecutive_theme_loads_do_not_copy_private_assets_from_the_active_theme() {
        let directory = tempfile::tempdir().expect("temporary theme directory must be available");
        fs::write(directory.path().join("first.png"), fixture_png(2, 2)).expect("first fixture PNG must be writable");
        fs::write(directory.path().join("second.png"), fixture_png(2, 2)).expect("second fixture PNG must be writable");
        fs::write(
            directory.path().join("first.json"),
            r#"{
                "schema_version": 1,
                "name": "First",
                "appearances": {
                    "control": { "button": { "enabled": { "normal": { "patch": { "type": "image", "path": "first.png" } } } } }
                }
            }"#,
        )
        .expect("first fixture JSON must be writable");
        fs::write(
            directory.path().join("second.json"),
            r#"{
                "schema_version": 1,
                "name": "Second",
                "appearances": {
                    "control": { "checkbox": { "enabled": { "normal": { "patch": { "type": "image", "path": "second.png" } } } } }
                }
            }"#,
        )
        .expect("second fixture JSON must be writable");

        // Expand the compact shared source so each derived theme has room for one artwork tile.
        // The catalog must retain this allocation even after the backend publishes the first one.
        let source = crate::atlas::builder::Builder::from_atlas_with_size(&test_atlas(), 64, 64)
            .expect("expanded application resources must fit")
            .build()
            .expect("expanded application resources must validate");
        let mut context = Context::<ThemeAtlasBackend>::new(ThemeAtlasBackend {
            atlas: source.clone(),
            reject_replacement: false,
        });
        let first = context
            .load_theme_file(directory.path().join("first.json"))
            .expect("first theme must load from the catalog");
        context.set_theme(&first).expect("first theme atlas must publish");
        let second = context
            .load_theme_file(directory.path().join("second.json"))
            .expect("second theme must ignore the active theme's private artwork");

        assert!(context.resource_catalog.atlas().ptr_eq(&source));
        assert!(!context.atlas().ptr_eq(&source));
        assert_eq!(
            second
                .bundle()
                .atlas()
                .clone_icon_table()
                .iter()
                .filter(|(name, _)| name.starts_with("@microui-theme-image/"))
                .count(),
            1,
            "the second atlas must contain only its own private theme image"
        );
    }
}
