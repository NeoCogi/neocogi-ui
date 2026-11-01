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

//! Concrete retained surface ownership and cross-surface traversal policy.

use std::{
    collections::{HashMap, HashSet},
    fmt,
};

mod interaction;
mod lifecycle;
mod synchronization;
mod traversal;

use super::*;
use crate::menu::{MenuAction, MenuEntry, MenuSlot, MenuSurface};
use crate::math::RectExt;
use crate::{MouseButton, Node, UiInputEvent, Vec2i, rect};

use super::root_chrome::{
    RootCaptionButton, RootChromeGeometry, RootChromePart, RootChromeVisualState, RootFrameKind, RootInteraction, RootResizeAxis, record_root_background,
    record_root_overlay, root_chrome_geometry,
};

/// Failure reason reported by a checked window or popup operation.
///
/// Window chrome and popup definitions are owned directly by the window manager, so failures
/// describe only stale identity or explicit ownership and layer policy. Operations that borrow
/// their target return this value directly. Fallible creation wraps it in
/// [`SurfaceCreationError`] so the uniquely owned [`Window`] or [`Node`] is not lost.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SurfaceMutationError {
    /// Removal would destroy a currently executing widget or checked-out child collection.
    Busy,
    /// The supplied [`WindowHandle`] is stale or belongs to another Context.
    UnknownWindow,
    /// The supplied [`PopupHandle`] no longer identifies a window-owned popup definition.
    UnknownPopup,
    /// A dialog owner is hidden when shown or is not an ordinary window.
    InvalidDialogOwner,
    /// An authenticated child-window parent is modal or otherwise unable to own ordinary children.
    InvalidChildWindowParent,
    /// The requested fixed layer is outside [`MIN_LAYER`] through [`MAX_LAYER`], inclusive.
    InvalidLayer(u8),
    /// The requested window occupies a manager-controlled child or modal layer.
    ManagedLayer,
    /// A popup owner is hidden, blocked by a modal, or missing from the active parent path.
    InvalidPopupParent,
}

impl fmt::Display for SurfaceMutationError {
    /// Describes the rejected capability or surface policy without exposing private identities.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Keep every public variant independently meaningful so this reason can be displayed both
        // on its own and as the source of an owner-preserving creation error.
        match self {
            Self::Busy => f.write_str("a widget in the removed subtree is executing"),
            Self::UnknownWindow => f.write_str("the window handle is stale or belongs to another Context"),
            Self::UnknownPopup => f.write_str("the popup handle is stale or belongs to another Context"),
            Self::InvalidDialogOwner => f.write_str("the selected window cannot own or show this dialog"),
            Self::InvalidChildWindowParent => f.write_str("the selected window cannot own a structural child window"),
            Self::InvalidLayer(layer) => {
                write!(f, "window layer {layer} is outside the supported range {MIN_LAYER}..={MAX_LAYER}")
            }
            Self::ManagedLayer => f.write_str("this surface's layer is controlled by its structural owner"),
            Self::InvalidPopupParent => f.write_str("the popup owner is not in the active visible surface path"),
        }
    }
}

/// Failed surface creation together with the unchanged value offered for registration.
///
/// `T` is the exact declaration offered to the failed operation: [`Window`] for child-window and
/// dialog creation, [`Node`] for a widget popup, or [`Menu`](crate::Menu) for a popup menu. The
/// concrete type parameter prevents a caller from recovering the wrong kind of input and avoids
/// type erasure. Read [`Self::reason`] before consuming the error with [`Self::into_input`] to retry,
/// revise, or otherwise retain the original value.
pub struct SurfaceCreationError<T> {
    /// Concrete validation failure that prevented any forest mutation.
    reason: SurfaceMutationError,
    /// Unmodified unique declaration supplied by the caller.
    _input: T,
}

impl<T> SurfaceCreationError<T> {
    /// Pairs a failed pre-registration validation with the still-unconsumed input value.
    fn new(reason: SurfaceMutationError, input: T) -> Self {
        // Construction stays beside the manager's validation boundary so no error can claim to
        // preserve a value after registration has partially transferred it into the forest.
        Self { reason, _input: input }
    }
}

impl<T> fmt::Debug for SurfaceCreationError<T> {
    /// Reports the validation reason while retaining support for non-Debug unique owners.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Unique declaration types intentionally expose no structural Debug requirement. Mark the
        // omitted input as non-exhaustive instead of adding an erased/debug-only inspection path.
        f.debug_struct("SurfaceCreationError").field("reason", &self.reason).finish_non_exhaustive()
    }
}

impl<T> fmt::Display for SurfaceCreationError<T> {
    /// Describes why registration failed while leaving owner recovery to the typed API.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Prefix the reusable policy reason with the operation boundary represented by this type.
        write!(f, "surface creation failed: {}", self.reason)
    }
}

impl<T> std::error::Error for SurfaceCreationError<T> {
    /// Exposes the concrete policy reason for standard error-chain inspection.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // SurfaceMutationError has no further source; this one link preserves useful categorization.
        Some(&self.reason)
    }
}

/// Process-unique identifier for one popup definition inside its owning window.
///
/// The identifier is intentionally private and allocated independently from lifecycle events.
/// Application code proves popup identity by carrying a [`PopupHandle`], so popup definitions
/// cannot be passed to generic window APIs.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub(super) struct PopupId(
    /// Shared non-reused value hidden behind the popup-specific type boundary.
    crate::identity::ProcessUniqueId,
);

impl PopupId {
    /// Allocates one popup key that cannot collide across Contexts or later object lifetimes.
    fn allocate() -> Self {
        // Menu and application popups share this concrete wrapper because both occupy popup nodes;
        // only application popups expose a handle and dismissal output.
        Self(crate::identity::ProcessUniqueId::allocate())
    }
}

/// Stable lookup identity; the manager or declaration owns all presentation and delivery state.
#[derive(Clone)]
pub struct PopupHandle {
    /// Process-unique identity; never reused or derived from a receiver allocation.
    id: PopupId,
}
impl PopupHandle {
    /// Captures the identity allocated by the sole semantic owner.
    fn new(id: PopupId) -> Self {
        Self { id }
    }
    /// Selects the authoritative record without retaining its owner.
    pub(super) const fn id(&self) -> PopupId {
        self.id
    }
}
impl fmt::Debug for PopupHandle {
    /// Keeps the private process-local key out of public diagnostics.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PopupHandle").finish_non_exhaustive()
    }
}

/// Storage and traversal state shared by a window body or popup body.
///
/// This is a value nested directly in its owner; it is not a registry or a polymorphic root. The
/// small common record keeps measurement and painting identical without duplicating window and
/// popup layout code.
struct Surface {
    /// Diagnostic name and optional window title text.
    name: String,
    /// Frame, padding, and automatic-size policy.
    options: WindowOption,
    /// Authoritative outer rectangle in screen coordinates.
    rect: Recti,
    /// Chrome and application-body geometry from the latest layout commit.
    geometry: RootChromeGeometry,
    /// Effective screen-space clip inherited from the viewport and structural window ancestors.
    ///
    /// Keeping this snapshot beside geometry lets manager chrome, compact menus, retained content,
    /// and cross-window hit testing consume the same committed visibility boundary.
    clip: Recti,
    /// Concrete content body with no erased node or dynamic downcast.
    body: SurfaceBody,
}

/// Concrete content variants owned by a forest surface.
// Unit-only traversal counters enlarge the inline runtime; production storage needs no extra allocation.
#[cfg_attr(test, allow(clippy::large_enum_variant))]
enum SurfaceBody {
    /// Ordinary application-authored retained widget tree.
    Widgets {
        /// Sole application-authored root node owned by this surface.
        root: Node,
        /// Traversal state paired exclusively with `root` for its mounted lifetime.
        runtime: UiRuntime,
    },
    /// Compact manager-owned menu popup surface.
    Menu(MenuSurface),
}

impl Surface {
    /// Creates a surface around one uniquely owned application node.
    fn new(name: String, options: WindowOption, rect: Recti, content: Node) -> Self {
        // Geometry begins empty and is replaced before a visible surface can receive input or paint.
        Self {
            name,
            options,
            rect,
            geometry: RootChromeGeometry::default(),
            clip: Recti::default(),
            body: SurfaceBody::Widgets { root: content, runtime: UiRuntime::new() },
        }
    }

    /// Creates an auto-sized popup around one concrete menu surface.
    fn menu(name: String, surface: MenuSurface) -> Self {
        // MenuPopup owns its own semantic border and interior. Avoid wrapping it in WindowFrame:
        // classic popup menus use a narrow menu outline, not top-level window corner artwork.
        Self {
            name,
            options: WindowOption::NO_PADDING | WindowOption::NO_TITLE | WindowOption::NO_RESIZE | WindowOption::AUTO_SIZE,
            rect: Recti::default(),
            geometry: RootChromeGeometry::default(),
            clip: Recti::default(),
            body: SurfaceBody::Menu(surface),
        }
    }

    /// Measures automatic axes and lays out the application tree in the derived body.
    fn layout(
        &mut self,
        menu_bar: Option<&mut MenuSurface>,
        frame_kind: RootFrameKind,
        style: &Skin,
        atlas: &crate::AtlasHandle,
        clip: Recti,
        minimized_content_height: Option<i32>,
    ) -> Result<(), neocogi_com::NcStatus> {
        // Commit the inherited surface boundary before any concrete body sees it. Every later
        // manager-owned hit or paint path reads this same snapshot until the next complete layout.
        self.clip = clip;
        let auto_width = self.options.intersects(WindowOption::AUTO_WIDTH);
        let auto_height = self.options.intersects(WindowOption::AUTO_HEIGHT);
        let mut menu_bar = menu_bar;
        let menu_intrinsic = menu_bar.as_deref_mut().map(|bar| bar.measure(style, atlas));
        if self.options.intersects(WindowOption::AUTO_SIZE) && !matches!(frame_kind, RootFrameKind::Popup) {
            // Convert retained outer bounds to application-body bounds before asking the content
            // tree for intrinsic size. Fixed axes retain their programmed outer extent.
            let shell = root_chrome_geometry(
                self.rect,
                Dimensioni::default(),
                menu_intrinsic,
                &self.name,
                self.options,
                frame_kind,
                style,
                atlas,
            );
            let horizontal_chrome = self.rect.width.saturating_sub(shell.body.width);
            let vertical_chrome = self.rect.height.saturating_sub(shell.body.height);
            let constraints = crate::Constraints::new(
                if auto_width {
                    crate::AvailableSpace::Unbounded
                } else {
                    crate::AvailableSpace::bounded(self.rect.width).shrink(horizontal_chrome)
                },
                if auto_height {
                    crate::AvailableSpace::Unbounded
                } else {
                    crate::AvailableSpace::bounded(self.rect.height).shrink(vertical_chrome)
                },
            );
            let child = self.body.measure(style, atlas, constraints)?;
            let intrinsic = root_chrome_geometry(Recti::default(), child, menu_intrinsic, &self.name, self.options, frame_kind, style, atlas).intrinsic_outer;
            if auto_width {
                self.rect.width = intrinsic.width;
            }
            if auto_height {
                self.rect.height = intrinsic.height;
            }
        }

        // Store one geometry snapshot shared by application layout, chrome hit testing, and paint.
        self.geometry = root_chrome_geometry(
            self.rect,
            Dimensioni::default(),
            menu_intrinsic,
            &self.name,
            self.options,
            frame_kind,
            style,
            atlas,
        );
        if let Some(content_height) = minimized_content_height {
            // Derive the compact outer height from the ordinary committed body. Reusing the same
            // geometry function keeps frame, title, menu, padding, paint, and hit testing aligned.
            self.rect.height = self.rect.height.saturating_sub(self.geometry.body.height).saturating_add(content_height.max(0));
            self.geometry = root_chrome_geometry(
                self.rect,
                Dimensioni::default(),
                menu_intrinsic,
                &self.name,
                self.options,
                frame_kind,
                style,
                atlas,
            );
        }
        if let (Some(bar), Some(rect)) = (menu_bar, self.geometry.menu_bar) {
            // The menu consumes the exact chrome rectangle derived above. Popup anchors, overlay
            // paint, and hit testing subsequently read the MenuSurface's matching committed rect.
            bar.layout(rect, clip);
        }
        self.body.layout(style, atlas, self.geometry.body, clip)
    }

    /// Measures popup content at its permitted width, then fits and commits one outer allocation.
    /// Automatic sizing is resolved here and cannot re-expand the fitted height in layout.
    fn fit_popup(&mut self, request: PopupPlacement, clip: Recti, style: &Skin, atlas: &crate::AtlasHandle) -> Result<(), neocogi_com::NcStatus> {
        let requested = request.preferred_rect();
        // A zero-sized previous request must not erase frame/padding before
        // measuring wrapping content at the width it will actually receive.
        let reference = Recti::new(0, 0, i32::MAX, i32::MAX);
        let shell = root_chrome_geometry(
            reference,
            Dimensioni::default(),
            None,
            &self.name,
            self.options,
            RootFrameKind::Popup,
            style,
            atlas,
        );
        let chrome = Dimensioni::new(
            reference.width.saturating_sub(shell.body.width),
            reference.height.saturating_sub(shell.body.height),
        );
        let width = if self.options.intersects(WindowOption::AUTO_WIDTH) {
            clip.width
        } else {
            requested.width.min(clip.width)
        };
        let height = if self.options.intersects(WindowOption::AUTO_HEIGHT) {
            crate::AvailableSpace::Unbounded
        } else {
            crate::AvailableSpace::bounded(requested.height.min(clip.height)).shrink(chrome.height)
        };
        let content = self.body.measure(
            style,
            atlas,
            crate::Constraints::new(crate::AvailableSpace::bounded(width).shrink(chrome.width), height),
        )?;
        let intrinsic = root_chrome_geometry(Recti::default(), content, None, &self.name, self.options, RootFrameKind::Popup, style, atlas).intrinsic_outer;
        let mut desired = Dimensioni::new(
            if self.options.intersects(WindowOption::AUTO_WIDTH) {
                intrinsic.width
            } else {
                requested.width
            },
            if self.options.intersects(WindowOption::AUTO_HEIGHT) {
                intrinsic.height
            } else {
                requested.height
            },
        );
        let font = style.resolve_font(atlas, &crate::FontRef::Role(crate::FontRole::Body));
        let gap = i32::try_from(atlas.get_font_height(font)).unwrap_or(i32::MAX);
        let minimum = self.body.menu_mut().map_or(1, |menu| menu.minimum_height()).saturating_add(chrome.height);
        let (first, _) = interaction::fit_popup_rect(request, clip, desired, gap, minimum);
        if let Some(menu) = self.body.menu_mut() {
            let body_height = first.height.saturating_sub(chrome.height).max(0);
            desired.height = menu.fit_height(body_height).saturating_add(chrome.height).min(first.height);
            if self.options.intersects(WindowOption::AUTO_WIDTH) {
                desired.width = menu.width_for_height(desired.height.saturating_sub(chrome.height)).saturating_add(chrome.width);
            }
        }
        self.rect = interaction::fit_popup_rect(request, clip, desired, gap, minimum).0;
        Ok(())
    }

    /// Returns whether the outer surface contains one screen-space point.
    fn contains(&self, point: Vec2i) -> bool {
        // A structural child can overlap its parent chrome geometrically while remaining clipped
        // out of that area. Require both committed boundaries so input matches recorded pixels.
        self.rect.contains_point(point) && self.clip.contains_point(point)
    }
}

impl SurfaceBody {
    /// Measures either concrete body without type erasure.
    fn measure(&mut self, style: &Skin, atlas: &crate::AtlasHandle, constraints: crate::Constraints) -> Result<Dimensioni, neocogi_com::NcStatus> {
        match self {
            Self::Widgets { root, runtime } => runtime.measure_tree_root(root, style, atlas, constraints),
            Self::Menu(menu) => Ok(menu.measure(style, atlas)),
        }
    }

    /// Commits one body allocation through the concrete variant.
    fn layout(&mut self, style: &Skin, atlas: &crate::AtlasHandle, rect: Recti, viewport: Recti) -> Result<(), neocogi_com::NcStatus> {
        match self {
            Self::Widgets { root, runtime } => runtime.layout_tree_root(root, style, atlas.clone(), rect, viewport),
            Self::Menu(menu) => {
                menu.layout(rect, viewport);
                Ok(())
            }
        }
    }

    /// Reports an active capture or a removed target's still-unconsumed gesture tail.
    fn owns_pointer_gesture(&self) -> bool {
        match self {
            Self::Widgets { runtime, .. } => runtime.owns_pointer_gesture(),
            Self::Menu(menu) => menu.has_capture(),
        }
    }

    /// Clears pointer state while preserving application keyboard focus where applicable.
    fn clear_pointer_targets(&mut self) {
        match self {
            Self::Widgets { runtime, .. } => {
                // Manager-owned overlays may preempt an application gesture while preserving the
                // independent keyboard-focus identity retained by the application runtime.
                runtime.clear_pointer_targets();
            }
            Self::Menu(menu) => menu.clear_pointer_targets(),
        }
    }

    /// Clears every transient input identity through the concrete body variant.
    fn clear_transient_targets(&mut self) {
        match self {
            Self::Widgets { runtime, .. } => runtime.clear_transient_targets(),
            Self::Menu(menu) => menu.clear_pointer_targets(),
        }
    }

    /// Returns the concrete menu body when this is a private menu popup.
    fn menu(&self) -> Option<&MenuSurface> {
        match self {
            Self::Menu(menu) => Some(menu),
            Self::Widgets { .. } => None,
        }
    }

    /// Returns the mutable concrete menu body when this is a private menu popup.
    fn menu_mut(&mut self) -> Option<&mut MenuSurface> {
        match self {
            Self::Menu(menu) => Some(menu),
            Self::Widgets { .. } => None,
        }
    }

    /// Starts an update cycle for widget bodies; concrete menus have no traversal runtime.
    fn begin_update(&mut self) {
        if let Self::Widgets { runtime, .. } = self {
            runtime.begin_update();
        }
    }

    /// Stages event-local routing only for a retained widget tree.
    fn begin_input_event(&mut self, pointer_input_enabled: bool, event: &UiInputEvent) {
        if let Self::Widgets { runtime, .. } = self {
            runtime.begin_input_event(pointer_input_enabled, event);
        }
    }

    /// Routes drag continuation or release to the captured application node, if this is a widget body.
    fn route_captured_pointer(&mut self, style: &Skin, mouse_buttons: MouseButton, event: &UiInputEvent) -> Option<bool> {
        let Self::Widgets { root, runtime } = self else {
            return None;
        };
        runtime.route_captured_pointer_input_event(std::slice::from_mut(root), style, mouse_buttons, event)
    }

    /// Returns whether a widget runtime accepts an ordinary pointer hit for the staged event.
    fn accepts_pointer_input(&self) -> bool {
        matches!(self, Self::Widgets { runtime, .. } if runtime.accepts_pointer_input())
    }

    /// Routes one ordinary widget hit and records any resulting capture transition.
    fn route_pointer(&mut self, style: &Skin, event: &UiInputEvent, mouse_buttons: MouseButton) -> Option<crate::ui_node::RuntimeNodeId> {
        let Self::Widgets { root, runtime } = self else {
            return None;
        };
        // Capture changes only after the selected target and its ancestors classify the event.
        let (owner, result) = runtime.route_input_event_to_node_ref(root, style, event)?;
        runtime.update_pointer_capture(owner, result, event, mouse_buttons);
        Some(owner)
    }

    /// Routes keyboard/text input only to application widget bodies.
    fn route_focus(&mut self, style: &Skin, event: &UiInputEvent) {
        if let Self::Widgets { root, runtime } = self {
            runtime.route_focus_input_event(std::slice::from_mut(root), style, event);
        }
    }

    /// Advances focus inside a retained widget body and reports whether it contains a Tab stop.
    fn advance_focus(&mut self, reverse: bool) -> bool {
        let Self::Widgets { root, runtime } = self else {
            return false;
        };
        runtime.advance_focus(std::slice::from_mut(root), reverse)
    }

    /// Updates widget bodies; menu state is committed synchronously by direct pointer routing.
    fn update(&mut self, style: &Skin, atlas: crate::AtlasHandle, input: crate::input::InputSnapshot) -> Result<(), neocogi_com::NcStatus> {
        if let Self::Widgets { root, runtime } = self {
            runtime.update_tree_root(root, style, atlas, input)?;
        }
        Ok(())
    }

    /// Paints one concrete body into the shared manager display list.
    fn paint(
        &mut self,
        display_list: &mut crate::render::DisplayList,
        style: &Skin,
        atlas: &crate::AtlasHandle,
        focus_visible: bool,
        window_active: bool,
        enabled: bool,
    ) -> Result<(), neocogi_com::NcStatus> {
        // Widget and compact-menu bodies share explicit activation and enabled facts so neither
        // infers disabled presentation from focus, hover, or the concrete body variant.
        match self {
            Self::Widgets { root, runtime } => runtime.paint_tree_root(root, display_list, style, atlas.clone(), focus_visible, window_active, enabled),
            Self::Menu(menu) => menu.paint(display_list, style, atlas, window_active, enabled),
        }
    }
}

/// Private identity for one node in the concrete surface forest.
///
/// Public code carries stable [`WindowHandle`] and [`PopupHandle`] capabilities. Internally, this
/// compact process-unique typed key is sufficient for common layout, input, and paint traversal
/// without an erased payload, trait object, or `(owner, popup)` adapter pair.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub(super) enum SurfaceKey {
    /// An independent window, structural child window, or modal dialog.
    Root(RootId),
    /// A window- or popup-owned transient surface.
    Popup(PopupId),
}

/// Layer policy stored only on root nodes.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum RootMode {
    /// Independent application window in one caller-selected fixed layer.
    Normal {
        /// Numeric stacking layer in the public fixed range.
        layer: u8,
    },
    /// Ordinary window structurally nested below another independent or child window.
    ///
    /// A child has no independent global layer. Its family root supplies the fixed band, while its
    /// position in forest chronology supplies order among siblings with the same direct parent.
    Child,
    /// Dialog in the manager-owned modal layer.
    ///
    /// Its ordinary owner is represented by [`SurfaceNode::parent`], not duplicated here.
    Modal,
}

/// Root-specific policy attached to a concrete surface node.
struct RootState {
    /// Independent fixed-layer, structurally inherited child, or manager-owned modal policy.
    mode: RootMode,
    /// Local visibility intent for this root.
    ///
    /// A child or dialog participates only when this bit and every ancestor's bit are true. Keeping
    /// the local value lets children recover after a temporarily hidden parent becomes visible.
    visible: bool,
    /// Manager-owned title movement or resize gesture.
    interaction: RootInteraction,
    /// Chrome region under the pointer during the latest routed pointer event.
    hovered_chrome: Option<RootChromePart>,
    /// Exact normal rectangle retained while this window is maximized.
    ///
    /// `None` means normal placement; `Some` simultaneously records restoration geometry and the
    /// maximized mode without a second boolean that could disagree with it.
    restore_rect: Option<Recti>,
    /// Expanded outer height retained while the minimize button shows a compact window.
    unminimized_height: Option<i32>,
    /// Strong owner of the unified window geometry/lifecycle event stream.
    events: neocogi_ui::abi::core::EventPort,
    /// Optional uniquely owned bar; windows without a bar do not reserve its viewport state.
    /// One allocation per installed bar keeps that optional state out of every forest node.
    menu_bar: Option<Box<MenuSurface>>,
    /// Descendant viewport policy applied when layout enters direct structural child windows.
    child_window_clip: ChildWindowClip,
}

impl RootState {
    /// Copies the interaction facts needed by chrome paint without lending manager policy storage.
    fn chrome_visual_state(&self) -> RootChromeVisualState {
        // `restore_rect` is the authoritative maximized-mode representation, so paint derives its
        // maximize-versus-restore role from the same value used by layout and caption actions.
        RootChromeVisualState {
            hovered: self.hovered_chrome,
            interaction: self.interaction,
            maximized: self.restore_rect.is_some(),
        }
    }
}

/// Original application placement, retained separately from the fitted screen rectangle.
#[derive(Clone, Copy)]
enum PopupPlacement {
    /// Explicit origin and preferred allocation; fitting never overwrites this intent.
    At(Recti),
    /// Snapshot trigger box and preferred direction, with the popup's requested allocation.
    Anchored {
        /// Complete trigger border box in screen coordinates.
        anchor: Recti,
        /// Validated preferred side; the opposite may be chosen to show more content.
        side: neocogi_ui::abi::core::PopupSide,
        /// Requested fixed-axis size, distinct from trigger dimensions and last fitted size.
        preferred: Dimensioni,
    },
}

impl PopupPlacement {
    /// Requested allocation before automatic sizing; resizing never drifts from fitted bounds.
    fn preferred_rect(self) -> Recti {
        match self {
            Self::At(rect) => rect,
            Self::Anchored { anchor, preferred, .. } => Recti::new(anchor.x, anchor.y, preferred.width, preferred.height),
        }
    }
}

/// Popup-specific policy attached to a concrete surface node.
enum PopupState {
    /// Application-authored popup with exact placement and an observable dismissal event.
    Application {
        /// Original placement request supplied by the public popup API.
        placement: PopupPlacement,
        /// Strong owner of policy-driven popup lifecycle events.
        events: neocogi_ui::abi::core::EventPort,
        /// Whether the next committed layout must select this popup's first keyboard target.
        focus_on_layout: bool,
    },
    /// Private menu popup positioned from one slot in its direct forest parent.
    Menu {
        /// Heading or parent-row index that opens and anchors this popup.
        trigger_slot: usize,
    },
}

/// Concrete role data for a retained surface node.
enum SurfaceKind {
    /// Window or dialog policy.
    Root(RootState),
    /// Transient popup policy.
    Popup(PopupState),
}

/// One retained surface and its sole structural parent edge.
///
/// Independent roots have no parent, child windows and dialogs point to their ordinary owner,
/// top-level popups point to a root, and submenu popups point to their parent popup. This is the
/// only ownership relation stored by the manager; the owning root of any descendant is derived by
/// following it.
struct SurfaceNode {
    /// Stable concrete identity of this node.
    key: SurfaceKey,
    /// Sole owner edge, always directed toward an already retained ancestor.
    parent: Option<SurfaceKey>,
    /// Geometry and uniquely owned application tree shared by both concrete roles.
    surface: Surface,
    /// Role-specific policy with no type erasure.
    kind: SurfaceKind,
}

impl SurfaceNode {
    /// Returns root policy when this node is a window or dialog.
    fn root(&self) -> Option<&RootState> {
        match &self.kind {
            SurfaceKind::Root(root) => Some(root),
            SurfaceKind::Popup(_) => None,
        }
    }

    /// Returns mutable root policy when this node is a window or dialog.
    fn root_mut(&mut self) -> Option<&mut RootState> {
        match &mut self.kind {
            SurfaceKind::Root(root) => Some(root),
            SurfaceKind::Popup(_) => None,
        }
    }

    /// Returns popup policy when this node is transient.
    fn popup(&self) -> Option<&PopupState> {
        match &self.kind {
            SurfaceKind::Popup(popup) => Some(popup),
            SurfaceKind::Root(_) => None,
        }
    }

    /// Returns mutable popup policy when this node is transient.
    fn popup_mut(&mut self) -> Option<&mut PopupState> {
        match &mut self.kind {
            SurfaceKind::Popup(popup) => Some(popup),
            SurfaceKind::Root(_) => None,
        }
    }

    /// Reports whether chrome, menus or application content still own the pointer gesture.
    fn owns_pointer_gesture(&self) -> bool {
        self.root().is_some_and(|root| root.interaction != RootInteraction::None)
            || self.root().and_then(|root| root.menu_bar.as_deref()).is_some_and(MenuSurface::has_capture)
            || self.surface.body.owns_pointer_gesture()
    }

    /// Clears every transient input identity retained by this surface.
    fn clear_transient_targets(&mut self) {
        // Only roots have manager chrome, while both roles own a concrete widget runtime.
        if let Some(root) = self.root_mut() {
            root.interaction = RootInteraction::None;
            root.hovered_chrome = None;
        }
        if let Some(root) = self.root_mut()
            && let Some(menu) = root.menu_bar.as_deref_mut()
        {
            menu.clear_pointer_targets();
        }
        self.surface.body.clear_transient_targets();
    }

    /// Clears chrome and body pointer targets without erasing retained keyboard focus.
    fn clear_pointer_targets(&mut self) {
        if let Some(root) = self.root_mut() {
            root.interaction = RootInteraction::None;
            root.hovered_chrome = None;
        }
        if let Some(root) = self.root_mut()
            && let Some(menu) = root.menu_bar.as_deref_mut()
        {
            menu.clear_pointer_targets();
        }
        self.surface.body.clear_pointer_targets();
    }

    /// Shows or hides a root without dropping its application state.
    fn set_root_visible(&mut self, visible: bool) -> Result<(), neocogi_com::NcStatus> {
        let root = self.root_mut().expect("visibility is defined only for root nodes");
        let hidden = root.visible && !visible;
        root.visible = visible;
        if !visible {
            self.clear_transient_targets();
        }
        // Commit visibility before even the mapper can observe the captured transition.
        match hidden {
            true => self.emit_window_event(neocogi_ui::abi::core::events::window_hidden()?),
            false => Ok(()),
        }
    }

    /// Replaces root chrome options and revokes a gesture disabled by the new policy.
    fn set_root_options(&mut self, options: WindowOption) {
        self.surface.options = options;
        let (interaction, restore_rect, unminimized_height, remains_maximized, remains_minimized) = {
            let root = self.root_mut().expect("chrome options are defined only for roots");
            let restore_rect = if options.intersects(WindowOption::MAXIMIZE_BUTTON) {
                None
            } else {
                // Removing the only restore affordance must leave the root in normal mode.
                root.restore_rect.take()
            };
            let unminimized_height = if options.intersects(WindowOption::MINIMIZE_BUTTON) {
                None
            } else {
                // Removing the minimize affordance must leave the root at its expanded height.
                root.unminimized_height.take()
            };
            (
                root.interaction,
                restore_rect,
                unminimized_height,
                root.restore_rect.is_some(),
                root.unminimized_height.is_some(),
            )
        };
        if let Some(rect) = restore_rect {
            // Removing the positive maximize affordance also exits its mode, preserving the exact
            // normal rectangle rather than leaving an un-restorable maximized window.
            self.surface.rect = rect;
            if remains_minimized {
                self.root_mut().expect("chrome options are defined only for roots").unminimized_height = Some(rect.height);
            }
        }
        if let Some(height) = unminimized_height
            && restore_rect.is_none()
        {
            // A maximized window expands to its live viewport; an ordinary one restores the height
            // captured by minimize. If both modes were removed, `restore_rect` already won above.
            self.surface.rect.height = if remains_maximized { self.surface.clip.height } else { height };
        }
        let disabled = options.intersects(WindowOption::DISABLED);
        let revokes_interaction = (options.intersects(WindowOption::NO_TITLE) && interaction == RootInteraction::Moving)
            || (options.intersects(WindowOption::NO_RESIZE | WindowOption::AUTO_SIZE) && matches!(interaction, RootInteraction::Resizing(_)))
            || (matches!(interaction, RootInteraction::Caption(RootCaptionButton::Minimize)) && !options.intersects(WindowOption::MINIMIZE_BUTTON))
            || (matches!(interaction, RootInteraction::Caption(RootCaptionButton::Maximize)) && !options.intersects(WindowOption::MAXIMIZE_BUTTON))
            || (matches!(interaction, RootInteraction::Caption(RootCaptionButton::Close)) && options.intersects(WindowOption::NO_CLOSE));
        if disabled {
            // Whole-window disabling suspends focus for later reactivation while revoking every
            // pointer-derived gesture immediately, matching modal exclusion semantics.
            self.clear_pointer_targets();
        } else if revokes_interaction {
            self.clear_transient_targets();
        }
    }

    /// Returns the committed root-chrome region under one screen-space point.
    fn chrome_part_at(&self, point: Vec2i) -> Option<RootChromePart> {
        self.root()?;
        // Chrome outside an ancestor-provided child clip is neither visible nor interactive.
        self.surface.clip.contains_point(point).then(|| self.surface.geometry.hit_test(point)).flatten()
    }

    /// Returns whether a parent-owned overlay precedes structural children at `point`.
    fn overlay_contains(&self, point: Vec2i) -> bool {
        // The frame border is visual chrome even though it has no action of its own. Selecting the
        // parent there acts as a hit shield, matching the border that is repainted after children.
        let frame = self.root().is_some()
            && self.surface.options.intersects(WindowOption::FRAME)
            && self.surface.clip.contains_point(point)
            && self.surface.rect.contains_point(point)
            && !self.surface.geometry.client.contains_point(point);

        // Actionable title/close/resize geometry and the intrinsic menu bar use their committed
        // rectangles. Together with `frame`, these are precisely the parent-owned regions recorded
        // in the overlay pass rather than the earlier application-content pass.
        frame || self.chrome_part_at(point).is_some() || self.root().and_then(|root| root.menu_bar.as_deref()).is_some_and(|menu| menu.contains(point))
    }

    /// Emits one concrete window event at the next safe application dispatch boundary.
    fn emit_window_event(&mut self, event: neocogi_com::ComBox<neocogi_ui::abi::core::IEventAbi>) -> Result<(), neocogi_com::NcStatus> {
        // The record owns its receiver; posting owns no queue and invokes no app behavior.
        let events = &mut self.root_mut().expect("window events are emitted only by roots").events;
        events.post(event)
    }

    /// Dismisses an active popup and emits exactly one policy event.
    fn dismiss_popup(&mut self) -> Result<(), neocogi_com::NcStatus> {
        // The caller walks the active leaf upward, so descendants are always notified first.
        self.surface.body.clear_transient_targets();
        if let Some(PopupState::Application { events, .. }) = self.popup_mut() {
            // Menu popups have no unobservable lifecycle port; their visibility is wholly internal.
            events.post(neocogi_ui::abi::core::events::popup_dismissed()?)?;
        }
        Ok(())
    }

    /// Consumes the application-popup request to focus its first target after layout.
    fn take_popup_focus_request(&mut self) -> bool {
        let Some(PopupState::Application { focus_on_layout, .. }) = self.popup_mut() else {
            return false;
        };
        // A request belongs to one visibility transition and must not reset focus every frame.
        std::mem::take(focus_on_layout)
    }

    /// Lays out this node and its optional root-owned menu bar through concrete bodies.
    fn layout(&mut self, style: &Skin, atlas: &crate::AtlasHandle, viewport: Recti) -> Result<(), neocogi_com::NcStatus> {
        let (maximized, unminimized_height) = self
            .root()
            .map(|root| (root.restore_rect.is_some(), root.unminimized_height))
            .unwrap_or((false, None));
        if maximized {
            // Follow the effective viewport on every layout so native drawable or ancestor-clip
            // changes resize a maximized top-level or child window without losing restoration data.
            self.surface.rect = viewport;
        } else if let Some(height) = unminimized_height {
            // Begin from the saved expanded height on every layout. The shared geometry pass below
            // can then respond consistently to theme, menu, and content-inset changes.
            self.surface.rect.height = height;
        }
        let frame_kind = match &self.kind {
            // Modal roots retain dialog artwork, ordinary roots retain window artwork, and every
            // transient definition derives its structural inset from popup-panel appearance.
            SurfaceKind::Root(root) if root.mode == RootMode::Modal => RootFrameKind::Dialog,
            SurfaceKind::Root(_) => RootFrameKind::Window,
            SurfaceKind::Popup(_) => RootFrameKind::Popup,
        };
        let menu_bar = match &mut self.kind {
            SurfaceKind::Root(root) => root.menu_bar.as_deref_mut(),
            SurfaceKind::Popup(_) => None,
        };
        let minimized_content_height = unminimized_height.map(|_| style.window_chrome().minimized_content_height);
        self.surface.layout(menu_bar, frame_kind, style, atlas, viewport, minimized_content_height)?;
        if maximized || unminimized_height.is_some() {
            // Managed size modes keep caption actions but expose no conflicting resize regions.
            self.surface.geometry = self.surface.geometry.without_resize();
        }
        Ok(())
    }
}

/// Inherited root policy computed once per forest change in parent-first order.
#[derive(Copy, Clone)]
struct SurfacePolicy {
    /// Nearest root: a window owns itself, while popup edges inherit their window owner.
    owner: RootId,
    /// Fixed or modal band; structural children inherit their parent's resolved band.
    mode: RootMode,
    /// Local root visibility combined with all structural owners' visibility.
    visible: bool,
    /// Structural children inherit disablement; modal roots start their own enabled scope.
    enabled: bool,
}

/// Disposable lookup and traversal data derived exclusively from the authoritative node vector.
#[derive(Default)]
struct SurfaceIndex {
    /// Current vector position; root activation can move it without changing stable identity.
    position: usize,
    /// Direct child keys in vector chronology, used forward for paint and backward for hit testing.
    children: Vec<SurfaceKey>,
    /// Complete inherited policy, absent only while a private rebuild is in progress.
    policy: Option<SurfacePolicy>,
    /// Constant-time membership in the committed visible traversal, including active popups.
    displayed: bool,
}

/// Concrete ownership forest plus incrementally maintained traversal order.
///
/// `nodes` owns every application tree exactly once, and the relative order of its root nodes is the
/// global back-to-front activation chronology. Popup nodes may be interleaved without participating
/// in that chronology because their sole active branch is positioned from parent edges.
/// `active_popup` stores only the deepest visible popup; its visible ancestors are derived from the
/// parent edges. `visible_order` is the reusable layered, parent-first traversal consumed by layout,
/// retained updates, and diagnostics. Paint and hit testing recurse over the same child edges to
/// place each parent's overlays on the far side of its descendants.
pub(super) struct SurfaceForest {
    /// Retained surface storage and authoritative chronological order for root nodes.
    ///
    /// Indices are deliberately not exposed as identity because fronting a root moves its complete
    /// node to this vector's tail. Stable keys and parent edges remain valid across that move.
    nodes: Vec<SurfaceNode>,
    /// Derived key lookup, adjacency, and inherited policy. This never owns a surface.
    index: HashMap<SurfaceKey, SurfaceIndex>,
    /// Deepest popup in the sole active branch, or no visible popup branch.
    active_popup: Option<PopupId>,
    /// Materialized parent-first base traversal, reused without per-frame allocation.
    ///
    /// Fixed and modal bands remain back-to-front globally. Within a structural family, however,
    /// this order describes layout/update ownership; paint and pointer selection additionally
    /// account for the parent overlay that follows all of its child surfaces.
    visible_order: Vec<SurfaceKey>,
    /// Reusable child-to-parent workspace used while materializing the active popup path.
    popup_path_scratch: Vec<SurfaceKey>,
}

impl SurfaceForest {
    /// Walks one authoritative subtree in postorder without callbacks or new adjacency storage.
    fn collect_dispatch_tree(
        &self,
        key: SurfaceKey,
        drivers: &mut Vec<neocogi_com::WeakComBox<neocogi_ui::abi::core::IWidgetDispatcherAbi>>,
    ) -> Result<(), neocogi_com::NcStatus> {
        let entry = &self.index[&key];
        for child in entry.children.iter().rev() {
            self.collect_dispatch_tree(*child, drivers)?;
        }
        if let SurfaceBody::Widgets { root, .. } = &self.nodes[entry.position].surface.body {
            root.collect_dispatch_widgets(drivers)?;
        }
        Ok(())
    }

    /// Preflights exactly the requested surface family using the existing index.
    /// The UI is thread-local, so no other thread can enter between validation
    /// and removal; no foreign callbacks occur during this recursive inspection.
    fn ensure_subtree_idle(&self, key: SurfaceKey) -> Result<(), SurfaceMutationError> {
        let entry = &self.index[&key];
        if let SurfaceBody::Widgets { root, .. } = &self.nodes[entry.position].surface.body {
            root.data.ensure_idle().map_err(|_| SurfaceMutationError::Busy)?;
        }
        for &child in &entry.children {
            self.ensure_subtree_idle(child)?;
        }
        Ok(())
    }

    /// Creates an empty concrete forest with reusable visible-order workspaces.
    pub(super) fn new() -> Self {
        Self {
            nodes: Vec::new(),
            index: HashMap::new(),
            active_popup: None,
            visible_order: Vec::new(),
            popup_path_scratch: Vec::new(),
        }
    }

    /// Resolves a stable surface key to its current storage index.
    fn node_index(&self, key: SurfaceKey) -> Option<usize> {
        self.index.get(&key).map(|entry| entry.position)
    }

    /// Adds one freshly appended node to the lookup and its parent's chronological child sequence.
    fn index_node(&mut self, position: usize) {
        let node = &self.nodes[position];
        let old = self.index.insert(node.key, SurfaceIndex { position, ..SurfaceIndex::default() });
        debug_assert!(old.is_none(), "surface identities are unique");
        if let Some(parent) = node.parent {
            self.index
                .get_mut(&parent)
                .expect("a new surface's parent is already retained")
                .children
                .push(node.key);
        }
    }

    /// Rebuilds lookup and adjacency after vector positions or membership change.
    fn rebuild_index(&mut self) {
        self.index.clear();
        // Fronting can place a parent after its child, so populate every key before resolving edges.
        for (position, node) in self.nodes.iter().enumerate() {
            self.index.insert(node.key, SurfaceIndex { position, ..SurfaceIndex::default() });
        }
        for node in &self.nodes {
            if let Some(parent) = node.parent {
                self.index.get_mut(&parent).expect("retained parent edges must resolve").children.push(node.key);
            }
        }
    }

    /// Reads already committed inherited policy without rescanning an ancestor chain.
    fn policy(&self, key: SurfaceKey) -> Option<SurfacePolicy> {
        self.index.get(&key)?.policy
    }

    /// Resolves one node from its parent's policy, then visits each direct child exactly once.
    fn derive_policy_tree(&mut self, key: SurfaceKey, parent: Option<SurfacePolicy>) {
        let node = self.node(key).expect("indexed surface must remain retained");
        let policy = match (key, node.root()) {
            (SurfaceKey::Root(owner), Some(state)) => SurfacePolicy {
                owner,
                mode: if state.mode == RootMode::Child {
                    parent.expect("child has a root parent").mode
                } else {
                    state.mode
                },
                visible: state.visible && (matches!(state.mode, RootMode::Normal { .. }) || parent.is_some_and(|parent| parent.visible)),
                enabled: !node.surface.options.intersects(WindowOption::DISABLED)
                    && (state.mode != RootMode::Child || parent.is_some_and(|parent| parent.enabled)),
            },
            (SurfaceKey::Popup(_), None) => parent.expect("popup has a retained owner"),
            _ => unreachable!("surface identity and body role must agree"),
        };
        self.index.get_mut(&key).expect("derived surface must be indexed").policy = Some(policy);
        // Copy one key at a time so recursion does not clone adjacency or retain a mutable borrow.
        let count = self.index[&key].children.len();
        for child in 0..count {
            self.derive_policy_tree(self.index[&key].children[child], Some(policy));
        }
    }

    /// Tests committed traversal membership without a linear visible-order scan.
    fn is_displayed(&self, key: SurfaceKey) -> bool {
        self.index.get(&key).is_some_and(|entry| entry.displayed)
    }

    /// Borrows one retained surface node by stable identity.
    fn node(&self, key: SurfaceKey) -> Option<&SurfaceNode> {
        self.node_index(key).map(|index| &self.nodes[index])
    }

    /// Mutably borrows one retained surface node by stable identity.
    fn node_mut(&mut self, key: SurfaceKey) -> Option<&mut SurfaceNode> {
        let index = self.node_index(key)?;
        Some(&mut self.nodes[index])
    }

    /// Borrows one retained root node.
    fn root_node(&self, root: RootId) -> Option<&SurfaceNode> {
        self.node(SurfaceKey::Root(root))
    }

    /// Mutably borrows one retained root node.
    fn root_node_mut(&mut self, root: RootId) -> Option<&mut SurfaceNode> {
        self.node_mut(SurfaceKey::Root(root))
    }

    /// Borrows one retained popup node.
    fn popup_node(&self, popup: PopupId) -> Option<&SurfaceNode> {
        self.node(SurfaceKey::Popup(popup))
    }

    /// Mutably borrows one retained popup node.
    fn popup_node_mut(&mut self, popup: PopupId) -> Option<&mut SurfaceNode> {
        self.node_mut(SurfaceKey::Popup(popup))
    }

    /// Borrows common surface storage for one traversal key.
    fn surface(&self, key: SurfaceKey) -> Option<&Surface> {
        self.node(key).map(|node| &node.surface)
    }

    /// Mutably borrows common surface storage for one traversal key.
    fn surface_mut(&mut self, key: SurfaceKey) -> Option<&mut Surface> {
        self.node_mut(key).map(|node| &mut node.surface)
    }

    /// Reports uncommitted widget measurement state in the surfaces painted by this commit.
    pub(super) fn has_visible_measurement_dirty(&self) -> bool {
        self.visible_order.iter().copied().any(|key| {
            let surface = self.surface(key).expect("visible surface must remain retained");
            match &surface.body {
                SurfaceBody::Widgets { root, .. } => root.has_measurement_dirty(),
                SurfaceBody::Menu(_) => false,
            }
        })
    }

    /// Inserts a root at the newest point in global activation chronology.
    fn insert_root(&mut self, node: SurfaceNode) {
        debug_assert!(matches!(node.key, SurfaceKey::Root(_)) && node.root().is_some());
        // Registration is chronologically newest. Hidden dialogs do not participate until shown,
        // and showing them moves the same node to the tail again.
        self.nodes.push(node);
        self.index_node(self.nodes.len() - 1);
        self.rebuild_visible_order();
    }

    /// Inserts a retained popup whose visibility remains derived from `active_popup`.
    fn insert_popup(&mut self, node: SurfaceNode) {
        debug_assert!(matches!(node.key, SurfaceKey::Popup(_)) && node.popup().is_some());
        let parent = self.policy(node.parent.expect("new popup has a retained parent"));
        let key = node.key;
        self.nodes.push(node);
        self.index_node(self.nodes.len() - 1);
        // A new hidden leaf cannot alter the active path or any existing policy. Derive only it.
        self.derive_policy_tree(key, parent);
    }

    /// Moves one complete root node to the newest point in global activation chronology.
    fn move_root_to_front(&mut self, root: RootId) {
        let index = self
            .node_index(SurfaceKey::Root(root))
            .filter(|index| self.nodes[*index].root().is_some())
            .expect("fronted root must remain retained");
        // Vec::remove performs only shallow Rust moves of the trailing records; widget allocations,
        // event ports, stable keys, and parent edges retain their identities.
        let node = self.nodes.remove(index);
        self.nodes.push(node);
        self.rebuild_index();
        self.rebuild_visible_order();
    }

    /// Changes one normal root's fixed layer without changing its activation chronology.
    fn set_root_layer(&mut self, root: RootId, layer: u8) {
        let state = self
            .root_node_mut(root)
            .and_then(SurfaceNode::root_mut)
            .expect("layer mutation requires a retained root");
        // The node stays at its existing chronological position; layered traversal filters that
        // single order when rebuilding the visible cache.
        state.mode = RootMode::Normal { layer };
        self.rebuild_visible_order();
    }

    /// Reads the fixed or modal band committed from root ancestry.
    fn stacking_mode(&self, root: RootId) -> Option<RootMode> {
        self.policy(SurfaceKey::Root(root)).map(|policy| policy.mode)
    }

    /// Reads local visibility combined with structural owner visibility.
    fn root_is_effectively_visible(&self, root: RootId) -> bool {
        self.policy(SurfaceKey::Root(root)).is_some_and(|policy| policy.visible)
    }

    /// Reads structural disablement, with modal dialogs retaining their independent enabled scope.
    fn root_is_effectively_enabled(&self, root: RootId) -> bool {
        self.policy(SurfaceKey::Root(root)).is_some_and(|policy| policy.enabled)
    }

    /// Selects the adjacent visible ordinary root in activation chronology, with wrapping.
    fn window_cycle_target(&self, current: Option<RootId>, reverse: bool) -> Option<RootId> {
        // A keyboard window switch is rare, so materializing only the compact root identities keeps
        // the wrap and direction policy obvious. Forest chronology remains authoritative even for
        // structural children whose visible traversal is nested beneath a parent.
        let roots: Vec<RootId> = self
            .nodes
            .iter()
            .filter_map(|node| match (node.key, node.root()) {
                (SurfaceKey::Root(root), Some(state))
                    if matches!(state.mode, RootMode::Normal { .. } | RootMode::Child)
                        && self.root_is_effectively_visible(root)
                        && self.root_is_effectively_enabled(root) =>
                {
                    Some(root)
                }
                _ => None,
            })
            .collect();
        let count = roots.len();
        if count == 0 {
            return None;
        }

        // A manager without an explicit active root starts at the appropriate chronological edge.
        // Normally `current` is the front keyboard surface, so this fallback matters only while no
        // eligible surface currently owns keyboard routing.
        let Some(index) = current.and_then(|current| roots.iter().position(|root| *root == current)) else {
            return Some(if reverse { roots[count - 1] } else { roots[0] });
        };
        let target = if reverse {
            index.checked_sub(1).unwrap_or(count - 1)
        } else {
            (index + 1) % count
        };
        Some(roots[target])
    }

    /// Appends one visible child-window family in parent-first base traversal order.
    fn append_visible_root_tree(&self, root: RootId, output: &mut Vec<SurfaceKey>) {
        // The caller admits only an effectively visible root. Descending through direct Child
        // edges preserves forest chronology among siblings while keeping unrelated descendants
        // out of this family's contiguous layout/update traversal.
        output.push(SurfaceKey::Root(root));
        for &key in &self.index[&SurfaceKey::Root(root)].children {
            let node = self.node(key).expect("indexed child must remain retained");
            if let (SurfaceKey::Root(child), Some(state)) = (key, node.root())
                && state.mode == RootMode::Child
                && state.visible
            {
                self.append_visible_root_tree(child, output);
            }
        }
    }

    /// Returns the newest effectively visible modal root from global activation chronology.
    fn active_modal_root(&self) -> Option<RootId> {
        // Reverse chronology selects exactly the dialog painted last in the dedicated modal band.
        // Effective visibility prevents a dialog below a hidden structural owner from resurfacing.
        self.nodes.iter().rev().find_map(|node| match (node.key, node.root()) {
            (SurfaceKey::Root(root), Some(state)) if state.mode == RootMode::Modal && self.root_is_effectively_visible(root) => Some(root),
            _ => None,
        })
    }

    /// Reads the nearest root resolved from sole parent edges during the last forest change.
    fn owning_root(&self, key: SurfaceKey) -> Option<RootId> {
        self.policy(key).map(|policy| policy.owner)
    }

    /// Returns a popup's zero-based depth below its owning root.
    fn popup_depth(&self, popup: PopupId) -> Option<usize> {
        let mut depth = 0usize;
        let mut key = SurfaceKey::Popup(popup);
        loop {
            match self.node(key)?.parent? {
                SurfaceKey::Root(_) => return Some(depth),
                parent @ SurfaceKey::Popup(_) => {
                    depth = depth.saturating_add(1);
                    key = parent;
                }
            }
        }
    }

    /// Returns whether one popup is an ancestor-or-self of the active leaf.
    fn popup_is_active(&self, popup: PopupId) -> bool {
        self.is_displayed(SurfaceKey::Popup(popup))
    }

    /// Returns the first popup below the root in the active branch.
    fn active_top_popup(&self) -> Option<PopupId> {
        let mut current = self.active_popup?;
        while let Some(SurfaceKey::Popup(parent)) = self.popup_node(current).and_then(|node| node.parent) {
            current = parent;
        }
        Some(current)
    }

    /// Replaces the deepest authoritative popup and refreshes derived visibility.
    fn set_active_popup(&mut self, popup: Option<PopupId>) {
        self.active_popup = popup;
        self.rebuild_visible_order();
    }

    /// Materializes the active branch parent-first into caller-owned reusable storage.
    fn fill_active_popup_path(&self, output: &mut Vec<SurfaceKey>) {
        output.clear();
        let mut current = self.active_popup;
        while let Some(popup) = current {
            output.push(SurfaceKey::Popup(popup));
            current = match self.popup_node(popup).and_then(|node| node.parent) {
                Some(SurfaceKey::Popup(parent)) => Some(parent),
                _ => None,
            };
        }
        output.reverse();
    }

    /// Rebuilds inherited policy and layered visibility only after a forest policy/order change.
    fn rebuild_visible_order(&mut self) {
        // Parent-first derivation visits every edge once, even after chronology moves a parent
        // behind its children. Geometry-only layout and paint reuse this committed projection.
        for entry in self.index.values_mut() {
            entry.displayed = false;
        }
        for position in 0..self.nodes.len() {
            let node = &self.nodes[position];
            if node.parent.is_none() {
                self.derive_policy_tree(node.key, None);
            }
        }
        let active_owner = self.active_popup.and_then(|popup| self.owning_root(SurfaceKey::Popup(popup)));
        let active_mode = active_owner.and_then(|root| self.stacking_mode(root));

        let mut popup_path = std::mem::take(&mut self.popup_path_scratch);
        self.fill_active_popup_path(&mut popup_path);
        let mut visible = std::mem::take(&mut self.visible_order);
        visible.clear();

        // Sixteen fixed scans admit only parentless family roots. Recursive insertion then keeps
        // each child family contiguous while preserving chronology independently among top-level
        // peers and among children that share one direct parent.
        for layer in MIN_LAYER..=MAX_LAYER {
            for node in &self.nodes {
                if let (SurfaceKey::Root(root), Some(state)) = (node.key, node.root())
                    && state.mode == (RootMode::Normal { layer })
                    && self.root_is_effectively_visible(root)
                {
                    self.append_visible_root_tree(root, &mut visible);
                }
            }
            if active_mode == Some(RootMode::Normal { layer }) {
                visible.extend_from_slice(&popup_path);
            }
        }
        // Modal roots remain a manager-owned tier above every fixed family. They are not structural
        // children for composition even though their sole parent edge still controls ownership.
        for node in &self.nodes {
            if let (SurfaceKey::Root(root), Some(state)) = (node.key, node.root())
                && state.mode == RootMode::Modal
                && self.root_is_effectively_visible(root)
            {
                visible.push(SurfaceKey::Root(root));
            }
        }
        if active_mode == Some(RootMode::Modal) {
            visible.extend_from_slice(&popup_path);
        }

        for key in &visible {
            self.index.get_mut(key).expect("visible surface must be indexed").displayed = true;
        }
        self.visible_order = visible;
        self.popup_path_scratch = popup_path;
    }

    /// Removes roots and all popup descendants whose derived owner is in `roots`.
    fn remove_roots(&mut self, roots: &[RootId]) {
        let roots: HashSet<_> = roots.iter().copied().collect();
        let removed: HashSet<_> = self
            .nodes
            .iter()
            .filter_map(|node| self.owning_root(node.key).filter(|owner| roots.contains(owner)).map(|_| node.key))
            .collect();
        self.nodes.retain(|node| !removed.contains(&node.key));
        if self.active_popup.is_some_and(|popup| removed.contains(&SurfaceKey::Popup(popup))) {
            self.active_popup = None;
        }
        self.rebuild_index();
        self.rebuild_visible_order();
    }
}

impl std::error::Error for SurfaceMutationError {}
