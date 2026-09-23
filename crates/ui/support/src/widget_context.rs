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
//! Phase-specific widget execution contexts.

use rs_math3d::{Recti, Vec2i};
use neocogi_ui_core_contracts::{IPainterAbi, PaintFont, PaintIcon, PaintScope, Point, WidgetVisualState};
use neocogi_com::{ComPtr, ComRef, NcCopyFromBufferOffset, NcStatus};

use crate::atlas::{AtlasHandle, FontId, IconId};
use crate::math::RectExt;
use crate::render::{DisplayList, Painter};
use crate::input::{Modifiers, MouseButton};
use crate::{KeyboardAction, KeyboardBehavior, WidgetOption};
use crate::theme::{Color, ControlRole, ControlState, MenuRole, MenuState, Skin, SurfaceRole, SurfaceState};
use crate::text_layout::control_text_position_with_font;

use crate::UiInputEvent;

/// Converts one screen point into a saturated coordinate relative to `origin`.
fn local_point(origin: Vec2i, point: Vec2i) -> Vec2i {
    // Event positions and root origins are independently application-controlled. Saturating each
    // subtraction keeps localization total when their mathematical distance exceeds i32.
    Vec2i::new(point.x.saturating_sub(origin.x), point.y.saturating_sub(origin.y))
}

/// Converts one screen-space clip into coordinates relative to a content rectangle.
fn localize_clip(content_rect: Recti, screen_clip: Recti) -> Recti {
    // RectExt owns the saturated subtraction policy shared by clipping and hit testing.
    screen_clip.relative_to(Vec2i::new(content_rect.x, content_rect.y))
}

/// Packs one native color into the renderer contract's fixed little-endian RGBA order.
fn packed_color(color: Color) -> u32 {
    u32::from_le_bytes([color.r, color.g, color.b, color.a])
}

/// Projects one exhaustive native control state into its typed paint-contract counterpart.
fn visual_state(state: ControlState) -> WidgetVisualState {
    match state {
        ControlState::Disabled => WidgetVisualState::Disabled,
        ControlState::Enabled(crate::PointerState::Normal) => WidgetVisualState::Normal,
        ControlState::Enabled(crate::PointerState::Hovered) => WidgetVisualState::Hovered,
        ControlState::Enabled(crate::PointerState::Pressed) => WidgetVisualState::Pressed,
        ControlState::Focused(crate::PointerState::Normal) => WidgetVisualState::Focused,
        ControlState::Focused(crate::PointerState::Hovered) => WidgetVisualState::FocusedHovered,
        ControlState::Focused(crate::PointerState::Pressed) => WidgetVisualState::FocusedPressed,
    }
}

/// Converts one routed event into coordinates relative to `origin`.
pub fn localize_event(origin: Vec2i, event: UiInputEvent) -> UiInputEvent {
    match event {
        UiInputEvent::MouseMove { pos, delta } => UiInputEvent::MouseMove { pos: local_point(origin, pos), delta },
        UiInputEvent::MouseDrag { pos, delta, buttons } => UiInputEvent::MouseDrag {
            pos: local_point(origin, pos),
            delta,
            buttons,
        },
        UiInputEvent::MouseDown { pos, button } => UiInputEvent::MouseDown { pos: local_point(origin, pos), button },
        UiInputEvent::MouseUp { pos, button } => UiInputEvent::MouseUp { pos: local_point(origin, pos), button },
        UiInputEvent::Scroll { pos, delta } => UiInputEvent::Scroll { pos: local_point(origin, pos), delta },
        event => event,
    }
}

/// Read-only state shared by update and paint without sharing their capabilities.
///
/// Keeping this type private is intentional: public widgets receive one phase-specific wrapper,
/// while the runtime has one authoritative place for derived content geometry and interaction
/// snapshots.
struct WidgetContextData<'a> {
    /// Derived content allocation; the parent-owned outer border box is never stored here.
    content_rect: Recti,
    /// Effective screen-space clip derived by retained traversal.
    screen_clip: Recti,
    /// Skin used by built-in widget layout and paint helpers.
    style: &'a Skin,
    /// Atlas used for text and icon metrics.
    atlas: &'a AtlasHandle,
    /// Whether the routed pointer is currently over this widget.
    hovered: bool,
    /// Whether this widget currently owns focus.
    focused: bool,
    /// Whether this widget received the current click transition.
    clicked: bool,
    /// Whether this widget is in an active pointer interaction.
    active: bool,
}

impl<'a> WidgetContextData<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        content_rect: Recti,
        screen_clip: Recti,
        style: &'a Skin,
        atlas: &'a AtlasHandle,
        hovered: bool,
        focused: bool,
        clicked: bool,
        active: bool,
    ) -> Self {
        Self {
            content_rect,
            screen_clip,
            style,
            atlas,
            hovered,
            focused,
            clicked,
            active,
        }
    }

    fn local_rect(&self) -> Recti {
        Recti::new(0, 0, self.content_rect.width, self.content_rect.height)
    }

    fn local_clip(&self) -> Recti {
        // Reuse the crate-wide saturated rectangle translation so update hit tests and paint clips
        // agree even when screen and content origins span the complete coordinate domain.
        localize_clip(self.content_rect, self.screen_clip)
    }
}

/// Interaction and state-mutation services decoded from the copied core `IWidget::update` input.
///
/// This context deliberately has no `DisplayList`, [`Painter`], or drawing helpers. Consequently
/// an update implementation cannot record visual work, and retained traversal does not need to
/// carry rendering state through the update pass. Interaction and focus are dispatcher-produced
/// snapshots; widgets can inspect them but cannot cooperatively assign or clear focus.
///
/// These private algorithm services mirror the external IWidget phase separation: update receives
/// copied input, while paint alone receives a scoped recorder.
pub struct WidgetUpdateCtx<'a> {
    /// Common read-only data, intentionally separated from phase capabilities.
    common: WidgetContextData<'a>,
    /// Whether this widget is inside the current hover root.
    in_hover_root: bool,
    /// Mouse buttons held after applying the current raw input event.
    mouse_buttons: MouseButton,
    /// Modifier state committed by the latest logical keyboard transition.
    modifiers: Modifiers,
    /// First synchronous receiver failure, returned by the enclosing COM update.
    event_error: Option<NcStatus>,
}

impl<'a> WidgetUpdateCtx<'a> {
    /// Creates an update context from router-owned interaction state.
    #[cfg(any(test, feature = "test-support"))]
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_interaction(
        rect: Recti,
        screen_clip: Recti,
        style: &'a Skin,
        atlas: &'a AtlasHandle,
        in_hover_root: bool,
        hovered: bool,
        focused: bool,
        clicked: bool,
        active: bool,
        mouse_buttons: MouseButton,
        modifiers: Modifiers,
    ) -> Self {
        Self::new_with_content_geometry(
            rect,
            screen_clip,
            style,
            atlas,
            in_hover_root,
            hovered,
            focused,
            clicked,
            active,
            mouse_buttons,
            modifiers,
        )
    }

    /// Creates update services for one traversal-derived content rectangle.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_content_geometry(
        content_rect: Recti,
        screen_clip: Recti,
        style: &'a Skin,
        atlas: &'a AtlasHandle,
        in_hover_root: bool,
        hovered: bool,
        focused: bool,
        clicked: bool,
        active: bool,
        mouse_buttons: MouseButton,
        modifiers: Modifiers,
    ) -> Self {
        Self {
            common: WidgetContextData::new(content_rect, screen_clip, style, atlas, hovered, focused, clicked, active),
            in_hover_root,
            mouse_buttons,
            modifiers,
            event_error: None,
        }
    }

    /// Records a port-posting failure while the native algorithm completes its state transition.
    pub fn posted(&mut self, result: Result<(), NcStatus>) {
        if self.event_error.is_none() {
            self.event_error = result.err();
        }
    }
    /// Ends the copied input transaction and reports its first receiver failure.
    pub fn finish(self) -> Result<(), NcStatus> {
        self.event_error.map_or(Ok(()), Err)
    }

    /// Returns the derived content rectangle in widget-local coordinates.
    ///
    /// Routed pointer positions use this same `(0, 0)`-origin coordinate space. The runtime-owned
    /// outer border and hit rectangle is not exposed.
    pub fn local_rect(&self) -> Recti {
        self.common.local_rect()
    }

    /// Returns the widget's derived content allocation in screen coordinates.
    pub fn screen_content_rect(&self) -> Recti {
        self.common.content_rect
    }

    /// Returns the effective content-local clip committed for this update.
    pub fn local_clip(&self) -> Recti {
        self.common.local_clip()
    }

    /// Whether local pointer tests belong to the current hover root.
    pub fn in_hover_root(&self) -> bool {
        self.in_hover_root
    }

    /// Returns whether the pointer is currently over this widget.
    pub fn hovered(&self) -> bool {
        self.common.hovered
    }

    /// Returns whether this widget currently owns focus.
    pub fn focused(&self) -> bool {
        self.common.focused
    }

    /// Returns whether this widget received the current click transition.
    pub fn clicked(&self) -> bool {
        self.common.clicked
    }

    /// Resolves this update's pointer click or declared key press to one shared widget action.
    pub fn action(&self, input: Option<&UiInputEvent>, keyboard: KeyboardBehavior) -> Option<KeyboardAction> {
        if self.clicked() {
            // The router records only an eligible primary-button press as a click transition, so
            // pointer and keyboard activation can share the same semantic branch in each widget.
            Some(KeyboardAction::Activate)
        } else {
            keyboard.action(input)
        }
    }

    /// Returns whether this widget owns pointer capture while the left button is held.
    ///
    /// Widgets that retain a local drag mode must reconcile it from this value on every update.
    /// A `false` value is the complete capture-loss signal; no separate lifecycle callback runs.
    pub fn active(&self) -> bool {
        // UiRuntime commits this immutable capture snapshot before calling Widget::update.
        self.common.active
    }

    /// Returns mouse buttons held after applying the current input event.
    pub fn mouse_buttons(&self) -> MouseButton {
        self.mouse_buttons
    }

    /// Returns the modifier snapshot committed by the latest keyboard transition.
    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    /// Returns this widget's resolved skin for update logic.
    pub fn skin(&self) -> &Skin {
        self.common.style
    }

    /// Returns the active atlas for built-in metric queries.
    pub fn atlas(&self) -> &AtlasHandle {
        self.common.atlas
    }

    /// Hit-tests a widget-local rectangle against a routed local pointer and the active clip.
    pub fn mouse_over(&self, local_rect: Recti, mouse_pos: Vec2i) -> bool {
        if !self.in_hover_root {
            return false;
        }
        local_rect.contains_point(mouse_pos) && self.common.local_clip().contains_point(mouse_pos)
    }
}

/// Geometry and recording services backed by the scoped core `IWidget::paint` capability.
///
/// Interaction values are immutable snapshots produced by update. Painting may inspect them to
/// choose visuals, but it cannot mutate focus or consume input.
pub struct WidgetPaintCtx<'a> {
    /// Common read-only data, intentionally separated from phase capabilities.
    common: WidgetContextData<'a>,
    /// Whether this widget and every visible ancestor accept interaction.
    enabled: bool,
    /// Whether the containing top-level window currently owns activation.
    window_active: bool,
    /// Either the native frame list or the scoped COM facade to that same runtime-owned list.
    output: PaintOutput<'a>,
    /// Current local clip; nested helpers save and restore this one context-owned value.
    draw_clip: Recti,
    /// First scoped paint failure, returned after the built-in semantic algorithm completes.
    error: Option<NcStatus>,
    /// Copied caller-authored options from the widget's authoritative common state.
    options: WidgetOption,
}

/// Concrete destination for one widget paint call; both variants reach the same runtime frame.
enum PaintOutput<'a> {
    /// Native traversal has direct exclusive access to the frame display list.
    Frame(&'a mut DisplayList),
    /// A built-in COM object appends through the runtime's checked invocation capability.
    Scoped {
        /// Invocation identity remains copied and can only be narrowed by `draw_clip`.
        scope: PaintScope,
        /// Borrowed interface cannot outlive the widget's synchronous `paint` call.
        painter: ComRef<'a, IPainterAbi>,
    },
}

impl<'a> WidgetPaintCtx<'a> {
    /// Creates paint services for one traversal-derived content rectangle.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_content_geometry(
        content_rect: Recti,
        display_list: &'a mut DisplayList,
        screen_clip: Recti,
        style: &'a Skin,
        atlas: &'a AtlasHandle,
        enabled: bool,
        hovered: bool,
        focused: bool,
        clicked: bool,
        active: bool,
        window_active: bool,
        options: WidgetOption,
    ) -> Self {
        let common = WidgetContextData::new(content_rect, screen_clip, style, atlas, hovered, focused, clicked, active);
        let draw_clip = common.local_clip();
        Self {
            common,
            enabled,
            window_active,
            output: PaintOutput::Frame(display_list),
            draw_clip,
            error: None,
            options,
        }
    }

    /// Creates a built-in context whose semantic operations target the runtime's active COM scope.
    #[allow(clippy::too_many_arguments)]
    pub fn new_scoped(
        content_rect: Recti,
        screen_clip: Recti,
        style: &'a Skin,
        atlas: &'a AtlasHandle,
        enabled: bool,
        hovered: bool,
        focused: bool,
        clicked: bool,
        active: bool,
        window_active: bool,
        options: WidgetOption,
        scope: PaintScope,
        painter: ComRef<'a, IPainterAbi>,
    ) -> Self {
        let common = WidgetContextData::new(content_rect, screen_clip, style, atlas, hovered, focused, clicked, active);
        let draw_clip = common.local_clip();
        Self {
            common,
            enabled,
            window_active,
            output: PaintOutput::Scoped { scope, painter },
            draw_clip,
            error: None,
            options,
        }
    }

    /// Returns the first scoped recording failure after a built-in semantic paint completes.
    pub fn finish(self) -> Result<(), NcStatus> {
        self.error.map_or(Ok(()), Err)
    }

    /// Reads one coherent option snapshot without retaining or borrowing widget state.
    pub fn options(&self) -> WidgetOption {
        self.options
    }

    /// Returns the derived content rectangle in widget-local coordinates.
    ///
    /// Its top-left corner is always `(0, 0)`, and its size is the bounds used by [`Self::painter`].
    pub fn local_rect(&self) -> Recti {
        self.common.local_rect()
    }

    /// Moves the runtime's single frame display list into a synchronous COM paint invocation.
    ///
    /// The empty replacement contains no allocation. [`Self::restore_display_list`] returns the
    /// same owned buffers after the foreign call, so no mutable Rust reference crosses the ABI.
    pub fn take_display_list(&mut self) -> DisplayList {
        match &mut self.output {
            PaintOutput::Frame(display_list) => std::mem::take(*display_list),
            PaintOutput::Scoped { .. } => unreachable!("only native traversal owns the frame display list"),
        }
    }

    /// Restores the runtime's frame display list after a synchronous COM paint invocation.
    pub fn restore_display_list(&mut self, display_list: DisplayList) {
        // Only PaintService can take the list, and it restores it before traversal can continue.
        match &mut self.output {
            PaintOutput::Frame(target) => **target = display_list,
            PaintOutput::Scoped { .. } => unreachable!("only native traversal owns the frame display list"),
        }
    }

    /// Returns the traversal-derived content allocation in screen coordinates to the paint driver.
    pub fn paint_content_rect(&self) -> Recti {
        self.common.content_rect
    }

    /// Returns the authoritative screen clip which a caller-authored scope may only narrow.
    pub fn paint_screen_clip(&self) -> Recti {
        self.common.screen_clip
    }

    /// Returns the traversal-derived effective clip in widget-local coordinates.
    pub fn local_clip(&self) -> Recti {
        // The initial value is traversal-derived. A nested paint helper may only replace it with a
        // positive intersection and restores the parent value before returning.
        self.draw_clip
    }

    /// Returns whether the pointer is currently over this widget.
    pub fn hovered(&self) -> bool {
        self.common.hovered
    }

    /// Returns whether this widget currently owns focus.
    pub fn focused(&self) -> bool {
        self.common.focused
    }

    /// Returns whether this widget received the current click transition.
    pub fn clicked(&self) -> bool {
        self.common.clicked
    }

    /// Returns the pointer-capture activity snapshot committed during the latest update.
    pub fn active(&self) -> bool {
        // Paint observes the retained update result and cannot alter capture ownership.
        self.common.active
    }

    /// Returns whether this widget and its visible ancestor chain accept interaction.
    pub fn enabled(&self) -> bool {
        // Participation is inherited during paint so a disabled container gives every descendant
        // the disabled theme state without rewriting each concrete widget's options.
        self.enabled
    }

    /// Returns whether this widget belongs to the manager's active top-level window.
    pub fn window_active(&self) -> bool {
        // Activation is a paint-time presentation fact and does not rewrite focus or pointer state.
        self.window_active
    }

    /// Resolves the exact control state for this paint snapshot.
    pub fn control_state(&self) -> ControlState {
        // Capture counts as a visible press only while the pointer remains over the widget. A drag
        // outside retains capture for correct release delivery but returns to the non-pressed art.
        ControlState::from_interaction(self.enabled, self.hovered(), self.focused(), self.active() && self.hovered())
    }

    /// Executes drawing with a local clip narrowed inside this runtime-owned paint context.
    pub fn with_clip(&mut self, rect: Recti, paint: impl FnOnce(&mut Self)) {
        let previous = self.draw_clip;
        self.draw_clip = previous
            .positive_intersection(rect)
            .unwrap_or_else(|| Recti::new(previous.x.max(rect.x), previous.y.max(rect.y), 0, 0));
        paint(self);
        self.draw_clip = previous;
    }

    /// Preserves the first COM recording error while allowing infallible semantic widget code.
    fn record(&mut self, result: Result<(), NcStatus>) {
        if self.error.is_none()
            && let Err(error) = result
        {
            self.error = Some(error);
        }
    }

    /// Copies the current invocation with its context-owned effective local clip.
    fn scoped_paint(&self) -> Option<(PaintScope, ComRef<'a, IPainterAbi>)> {
        match &self.output {
            PaintOutput::Frame(_) => None,
            PaintOutput::Scoped { scope, painter } => Some((
                PaintScope {
                    invocation: scope.invocation,
                    clip: crate::boundary::copied_rect(self.draw_clip),
                },
                *painter,
            )),
        }
    }

    /// Resolves the effective screen clip for the native display-list path.
    fn screen_draw_clip(&self) -> Recti {
        self.draw_clip.translated(Vec2i::new(self.common.content_rect.x, self.common.content_rect.y))
    }

    /// Returns a widget-local painter that records directly into the current frame display list.
    ///
    /// The painter receives the widget origin, local bounds, and traversal-derived effective clip.
    /// Custom widgets can narrow that clip with [`Painter::with_clip`] without mutating traversal
    /// state.
    pub fn painter(&mut self) -> Painter<'_> {
        let screen_content_bounds = self.common.content_rect;
        let screen_clip = self.screen_draw_clip();
        match &mut self.output {
            PaintOutput::Frame(display_list) => Painter::for_widget(display_list, screen_content_bounds, screen_clip),
            PaintOutput::Scoped { .. } => unreachable!("built-in COM painting uses typed WidgetPaintCtx operations"),
        }
    }

    /// Returns this widget's resolved skin.
    pub fn skin(&self) -> &Skin {
        self.common.style
    }

    /// Returns the active atlas.
    pub fn atlas(&self) -> &AtlasHandle {
        self.common.atlas
    }

    /// Draws a filled rectangle through a widget-local painter.
    pub fn draw_rect(&mut self, rect: Recti, color: Color) {
        if let Some((scope, mut painter)) = self.scoped_paint() {
            let result = painter.fill(scope, crate::boundary::copied_rect(rect), packed_color(color));
            self.record(result);
        } else {
            let result = self.painter().fill_rect(rect, color);
            self.record(result);
        }
    }

    /// Draws an atlas icon through a widget-local painter.
    pub fn draw_icon(&mut self, id: IconId, rect: Recti, color: Color) {
        // Callers resolve a semantic content color before reaching this primitive, so explicit image
        // tints and role-specific state colors are preserved exactly.
        if let Some((scope, mut painter)) = self.scoped_paint() {
            let result = painter.icon(
                scope,
                PaintIcon { index: self.common.atlas.icon_index(id) },
                crate::boundary::copied_rect(rect),
                packed_color(color),
            );
            self.record(result);
        } else {
            let result = self.painter().icon(id, rect, color);
            self.record(result);
        }
    }

    /// Draws a renderer-owned image without creating a widget-local display list or geometry pass.
    pub fn draw_texture(&mut self, rect: Recti, color: Color, texture: ComPtr<neocogi_ui_core_contracts::ITextureAbi>) {
        if let Some((scope, mut painter)) = self.scoped_paint() {
            let result = painter.texture(
                scope,
                crate::boundary::copied_rect(rect),
                neocogi_ui_core_contracts::TextureRegion::FULL,
                packed_color(color),
                texture.as_com_ref(),
            );
            self.record(result);
        } else {
            let result = self.painter().com_texture(rect, color, texture);
            self.record(result);
        }
    }

    /// Draws one exact-font text run through the active native or scoped frame target.
    pub fn draw_text(&mut self, font: FontId, text: &str, position: Vec2i, color: Color) {
        if let Some((scope, mut painter)) = self.scoped_paint() {
            let result = painter.text_run(
                scope,
                PaintFont {
                    index: self.common.atlas.font_index(font),
                },
                Point { x: position.x, y: position.y },
                packed_color(color),
                NcCopyFromBufferOffset::new(0, text.as_bytes()),
            );
            self.record(result);
        } else {
            let result = self.painter().text(font, text, position, color);
            self.record(result);
        }
    }

    /// Draws one control using this widget's resolved interaction state.
    pub fn draw_control(&mut self, role: ControlRole, rect: Recti) -> Option<Recti> {
        // Resolve state before borrowing the display list through Painter so immutable and mutable
        // context borrows do not overlap.
        let state = self.control_state();
        self.draw_control_state(role, state, rect)
    }

    /// Draws one control using an explicit nested control state.
    pub fn draw_control_state(&mut self, role: ControlRole, state: ControlState, rect: Recti) -> Option<Recti> {
        // Composite controls can select a child part's state without converting it into a universal
        // state accepted by unrelated surface, menu, or chrome families.
        if let Some((scope, mut painter)) = self.scoped_paint() {
            let result = painter.frame(
                scope,
                crate::boundary::copied_rect(rect),
                crate::frame::frame_contract(crate::FrameRole::Control(role)),
                visual_state(state),
                false,
            );
            return match result {
                Ok(content) if content.width > 0 && content.height > 0 => Some(crate::boundary::local_rect(content)),
                Ok(_) => None,
                Err(error) => {
                    self.record(Err(error));
                    None
                }
            };
        }
        let patch = self.common.style.control(role, state).patch;
        let mut painter = self.painter();
        match crate::frame::paint_internal_frame(&mut painter, rect, patch) {
            Ok(content) => content,
            Err(error) => {
                self.record(Err(error));
                None
            }
        }
    }

    /// Draws only the stretchable center of one control over an unframed rectangle.
    pub fn draw_control_center(&mut self, role: ControlRole, rect: Recti) {
        // Resolve the complete nested state before recording the center cell.
        self.draw_control_center_state(role, self.control_state(), rect);
    }

    /// Draws only the stretchable center of one control using an explicit state.
    pub fn draw_control_center_state(&mut self, role: ControlRole, state: ControlState, rect: Recti) {
        // Zero destination insets collapse all eight outer cells. Flat patches retain their center
        // cell and image patches sample only their center source, preserving the old unframed fill
        // contract without introducing a second theme asset vocabulary.
        if let Some((scope, mut painter)) = self.scoped_paint() {
            let result = painter
                .frame(
                    scope,
                    crate::boundary::copied_rect(rect),
                    crate::frame::frame_contract(crate::FrameRole::Control(role)),
                    visual_state(state),
                    true,
                )
                .map(|_| ());
            self.record(result);
        } else {
            let patch = self.common.style.control(role, state).patch.with_insets(crate::SliceInsets::ZERO);
            let mut painter = self.painter();
            let result = crate::frame::paint_internal_frame(&mut painter, rect, patch);
            self.record(result.map(|_| ()));
        }
    }

    /// Draws only the stretchable center of one structural surface.
    pub fn draw_surface_center_state(&mut self, role: SurfaceRole, state: SurfaceState, rect: Recti) {
        // Center-only structural fills share the same nine-patch behavior as unframed controls.
        if let Some((scope, mut painter)) = self.scoped_paint() {
            let state = match state {
                SurfaceState::Normal => WidgetVisualState::Normal,
                SurfaceState::Disabled => WidgetVisualState::Disabled,
            };
            let result = painter
                .frame(
                    scope,
                    crate::boundary::copied_rect(rect),
                    crate::frame::frame_contract(crate::FrameRole::Surface(role)),
                    state,
                    true,
                )
                .map(|_| ());
            self.record(result);
        } else {
            let patch = self.common.style.surface(role, state).patch.with_insets(crate::SliceInsets::ZERO);
            let mut painter = self.painter();
            let result = crate::frame::paint_internal_frame(&mut painter, rect, patch);
            self.record(result.map(|_| ()));
        }
    }

    /// Draws one menu part using its menu-specific state.
    pub fn draw_menu_state(&mut self, role: MenuRole, state: MenuState, rect: Recti) -> Option<Recti> {
        // The Open state remains confined to menu rendering.
        let patch = self.common.style.menu(role, state).patch;
        let mut painter = self.painter();
        match crate::frame::paint_internal_frame(&mut painter, rect, patch) {
            Ok(content) => content,
            Err(error) => {
                self.record(Err(error));
                None
            }
        }
    }

    /// Resolves a control content color using this widget's complete interaction state.
    pub fn control_content_color(&self, role: ControlRole) -> Color {
        // ControlState gives disabling precedence and nests pointer state beneath focus ownership.
        self.common.style.control(role, self.control_state()).content_color
    }

    /// Resolves a menu content color using an explicit menu selection state.
    pub fn menu_content_color(&self, role: MenuRole, state: MenuState) -> Color {
        // Composite menu panels own their exact open, pointer, and keyboard selection state.
        self.common.style.menu(role, state).content_color
    }

    /// Draws aligned control text using the content color for one concrete control role.
    pub fn draw_control_text_with_font(&mut self, font: FontId, text: &str, rect: Recti, role: ControlRole, opt: WidgetOption) {
        // Resolve the same state as the adjacent appearance before mutable display-list recording.
        let color = self.control_content_color(role);
        self.draw_control_text_color_with_font(font, text, rect, color, opt);
    }

    /// Draws aligned control text with an explicit font and resolved color.
    ///
    /// Most built-in controls select one complete semantic appearance role and should continue to use
    /// [`Self::draw_control_text_with_font`]. Composite controls such as menu panels need to derive
    /// a disabled-text color from that role while retaining the same padding, alignment, clipping,
    /// and glyph-placement rules. Keeping that variation here prevents each composite from
    /// duplicating the authoritative control-text geometry.
    pub fn draw_control_text_color_with_font(&mut self, font: FontId, text: &str, rect: Recti, color: Color, opt: WidgetOption) {
        // Composite callers have already resolved an exact role/state content color. This primitive
        // owns only authoritative text geometry, clipping, and recording.
        let pos = control_text_position_with_font(self.common.style, self.common.atlas, font, text, rect, opt);
        self.with_clip(rect, |ctx| ctx.draw_text(font, text, pos, color));
    }
}

#[cfg(test)]
mod tests {
    //! Boundary tests for traversal-to-widget coordinate conversion.

    use super::*;

    /// Verifies pointer localization saturates both directions instead of overflowing Vec2i
    /// subtraction before a widget receives the routed event.
    #[test]
    fn event_localization_handles_extreme_screen_and_widget_origins() {
        let event = localize_event(
            Vec2i::new(i32::MAX, i32::MIN),
            UiInputEvent::MouseDown {
                pos: Vec2i::new(i32::MIN, i32::MAX),
                button: MouseButton::LEFT,
            },
        );
        let UiInputEvent::MouseDown { pos, .. } = event else {
            panic!("localization must preserve the concrete event variant");
        };
        assert_eq!((pos.x, pos.y), (i32::MIN, i32::MAX));

        let clip = localize_clip(Recti::new(i32::MAX, i32::MIN, 1, 1), Recti::new(i32::MIN, i32::MAX, 1, 1));
        assert_eq!((clip.x, clip.y, clip.width, clip.height), (i32::MIN, i32::MAX, 1, 1));
    }

    /// Verifies paint state distinguishes visible presses, capture outside, and disabled ancestry.
    #[test]
    fn paint_control_state_uses_enabled_hover_focus_and_visible_capture() {
        let atlas = crate::test_support::test_atlas();
        let style = crate::test_support::test_skin(&atlas);
        let bounds = Recti::new(0, 0, 20, 10);
        let mut list = DisplayList::new();

        {
            let ctx = WidgetPaintCtx::new_with_content_geometry(
                bounds,
                &mut list,
                bounds,
                &style,
                &atlas,
                true,
                true,
                true,
                false,
                true,
                true,
                WidgetOption::NONE,
            );
            assert_eq!(ctx.control_state(), ControlState::Focused(crate::PointerState::Pressed));
        }
        {
            // Capture remains active outside for release routing, but theme art is no longer pressed.
            let ctx = WidgetPaintCtx::new_with_content_geometry(
                bounds,
                &mut list,
                bounds,
                &style,
                &atlas,
                true,
                false,
                true,
                false,
                true,
                true,
                WidgetOption::NONE,
            );
            assert_eq!(ctx.control_state(), ControlState::Focused(crate::PointerState::Normal));
        }
        {
            // Disabled ancestry wins over stale retained interaction snapshots.
            let ctx = WidgetPaintCtx::new_with_content_geometry(
                bounds,
                &mut list,
                bounds,
                &style,
                &atlas,
                false,
                true,
                true,
                true,
                true,
                true,
                WidgetOption::NONE,
            );
            assert_eq!(ctx.control_state(), ControlState::Disabled);
        }
        {
            // Window activation does not turn an otherwise enabled widget into a disabled one.
            let ctx = WidgetPaintCtx::new_with_content_geometry(
                bounds,
                &mut list,
                bounds,
                &style,
                &atlas,
                true,
                true,
                true,
                true,
                true,
                false,
                WidgetOption::NONE,
            );
            assert_eq!(ctx.control_state(), ControlState::Focused(crate::PointerState::Pressed));
        }
    }
}
