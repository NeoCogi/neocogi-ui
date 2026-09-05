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

use crate::ControlRole;

use std::rc::Rc;

use crate::{
    AtlasHandle, ChildParticipation, Container, ContainerWidget, ControlState, Dimensioni, MeasureCtx, PointerState, Recti, Skin, TypedWidgetHandle,
    UiInputEvent, Widget, WidgetOption, WidgetPaintCtx, WidgetUpdateCtx,
};

use super::{ContainerLayoutCtx, LinearItem, Node};
use crate::{Linear, LinearParameters};

/// Immutable header presentation shared by disclosure layout and its actual interactive header.
#[derive(Copy, Clone, Eq, PartialEq)]
pub(crate) enum DisclosureVariant {
    /// Framed header row with a non-indented body.
    Header,
    /// Unframed tree row with an indented body.
    Tree,
}

/// Explicit expansion data shared by the actual header and its layout parent, without object backpointers.
pub(crate) struct DisclosureState {
    /// Current body participation policy.
    expanded: std::cell::Cell<bool>,
    /// Shared edits persist until the parent collects them, even while its body is hidden.
    dirty: std::cell::Cell<bool>,
}
impl DisclosureState {
    /// Creates one authoritative expansion value for the composed widget.
    pub(crate) fn new(expanded: bool) -> Rc<Self> {
        Rc::new(Self {
            expanded: std::cell::Cell::new(expanded),
            dirty: std::cell::Cell::new(false),
        })
    }
    /// Changes participation only when the requested value differs.
    fn set(&self, expanded: bool) {
        if self.expanded.replace(expanded) != expanded {
            self.dirty.set(true);
        }
    }
}

/// One-shot construction input for a stateful disclosure container.
///
/// Label, header/tree presentation, and base options are initialization-only. Initial expansion
/// and children move into [`Disclosure`] for mounted mutation.
pub struct DisclosureParameters {
    label: String,
    expanded: bool,
    children: Vec<LinearItem>,
    variant: DisclosureVariant,
    opt: WidgetOption,
}

impl DisclosureParameters {
    /// Creates the framed header presentation and its top-to-bottom body items.
    ///
    /// Plain [`Node`] values use content height. Pass [`LinearItem`] when an item has an explicit
    /// fixed or flexible relationship to the private vertical Linear.
    pub fn header<T>(label: impl Into<String>, expanded: bool, children: impl IntoIterator<Item = T>) -> Self
    where
        T: Into<LinearItem>,
    {
        Self {
            label: label.into(),
            expanded,
            children: children.into_iter().map(Into::into).collect(),
            variant: DisclosureVariant::Header,
            opt: WidgetOption::FRAME,
        }
    }

    /// Creates the unframed, indented tree presentation and its body items.
    pub fn tree<T>(label: impl Into<String>, expanded: bool, children: impl IntoIterator<Item = T>) -> Self
    where
        T: Into<LinearItem>,
    {
        Self {
            label: label.into(),
            expanded,
            children: children.into_iter().map(Into::into).collect(),
            variant: DisclosureVariant::Tree,
            opt: WidgetOption::NONE,
        }
    }

    /// Overrides the presentation's default widget options.
    pub fn with_options(mut self, opt: WidgetOption) -> Self {
        self.opt = opt;
        self
    }
}

/// Application-facing state for a disclosure container.
///
/// Expansion gates descendant traversal while retaining every owned runtime and its state. It does
/// not expose or mutate generic node visibility.
pub struct Disclosure {
    /// Temporary legacy editing link; COM composition owns its actual ILinear body projection directly.
    content: Option<TypedWidgetHandle<Linear>>,
    /// Same expansion scalar read by the actual header object and parent layout.
    model: Rc<DisclosureState>,
    variant: DisclosureVariant,
}

impl Disclosure {
    /// Returns whether descendants participate in traversal.
    pub fn is_expanded(&self) -> bool {
        self.model.expanded.get()
    }

    /// Returns whether descendants are currently gated.
    pub fn is_collapsed(&self) -> bool {
        !self.model.expanded.get()
    }

    /// Makes descendants participate in traversal.
    pub fn expand(&mut self) {
        self.model.set(true);
    }

    /// Gates descendants while retaining their ownership and state.
    pub fn collapse(&mut self) {
        self.model.set(false);
    }

    /// Reverses the current descendant gate.
    pub fn toggle(&mut self) {
        self.model.set(!self.model.expanded.get());
    }

    /// Returns the number of owned child nodes.
    pub fn len(&self) -> Option<usize> {
        self.content.as_ref()?.try_read(Linear::len).flatten()
    }

    /// Returns whether the disclosure owns no children.
    pub fn is_empty(&self) -> Option<bool> {
        self.content.as_ref()?.try_read(Linear::is_empty).flatten()
    }

    /// Appends one still-unmounted body item and its vertical Linear-owned height track.
    #[allow(clippy::result_large_err)] // Failure returns the exact unique node and its edge metadata.
    pub fn push(&mut self, item: impl Into<LinearItem>) -> Result<(), LinearItem> {
        match &self.content {
            Some(content) => content.try_update_with(item.into(), Linear::push)?,
            None => Err(item.into()),
        }
    }

    /// Inserts an item, returning it unchanged when `index > len`.
    #[allow(clippy::result_large_err)] // The exact unboxed owner is the failure value by contract.
    pub fn insert(&mut self, index: usize, item: LinearItem) -> Result<(), LinearItem> {
        match &self.content {
            Some(content) => content.try_update_with(item, |content, item| content.insert(index, item))?,
            None => Err(item),
        }
    }

    /// Drops one indexed child owner and reports whether it existed.
    pub fn remove_drop(&mut self, index: usize) -> Option<bool> {
        self.content.as_ref()?.try_update(|content| content.remove_drop(index)).flatten()
    }

    /// Drops every child owner.
    pub fn clear(&mut self) -> Option<()> {
        self.content.as_ref()?.try_update(Linear::clear).flatten()
    }

    /// Replaces all body items in iterator order.
    pub fn replace<T, I>(&mut self, items: I) -> Result<(), I>
    where
        T: Into<LinearItem>,
        I: IntoIterator<Item = T>,
    {
        match &self.content {
            Some(content) => content.try_update_with(items, Linear::replace)?,
            None => Err(items),
        }
    }

    // Structural child order is also retained keyboard traversal order. Keeping the visible
    // header first makes Tab reach it before any expanded descendants without a parallel order.
    const HEADER: usize = 0;
    const BODY: usize = 1;

    fn indent(&self, style: &Skin) -> i32 {
        if matches!(self.variant, DisclosureVariant::Tree) {
            style.metrics.indent.max(0)
        } else {
            0
        }
    }
}

/// Dispatcher-addressable disclosure header.
///
/// The header owns no descendants and shares only expansion data with its layout parent.
pub(crate) struct DisclosureHeader {
    /// Shared scalar state owns no parent widget or tree.
    model: Rc<DisclosureState>,
    label: String,
    variant: DisclosureVariant,
    opt: WidgetOption,
}

impl DisclosureHeader {
    /// Measures the complete custom-painted header, including its optional internal frame.
    fn preferred(&self, style: &Skin, atlas: &AtlasHandle) -> Dimensioni {
        // Resolve icon and text metrics independently, then build the one-row content preference.
        let padding = style.metrics.padding.max(0);
        let vertical_pad = (padding / 2).max(1);
        let body_font = style.resolve_font_role(atlas, crate::FontRole::Body);
        let font_height = atlas.get_font_height(body_font) as i32;
        // Expansion changes only presentation state, not the header's retained allocation. Reserve
        // the component-wise maximum of both possible icons so toggling cannot clip a larger
        // collapse image or require a state-dependent measurement invalidation.
        let expand_icon = atlas.get_icon_size(style.resolve_icon_role(atlas, crate::IconRole::Expand));
        let collapse_icon = atlas.get_icon_size(style.resolve_icon_role(atlas, crate::IconRole::Collapse));
        let icon = Dimensioni::new(expand_icon.width.max(collapse_icon.width), expand_icon.height.max(collapse_icon.height));
        let text_width = if self.label.is_empty() {
            0
        } else {
            atlas.get_text_size(body_font, &self.label).width
        };
        let content_height = font_height.max(icon.height).max(0).saturating_add(vertical_pad.saturating_mul(2));
        let icon_width = content_height.saturating_sub(padding).max(icon.width).max(0);
        let content_width = padding.saturating_mul(2).saturating_add(icon_width).saturating_add(text_width.max(0));
        let content = Dimensioni::new(content_width, content_height);
        // A header frame is internal to this child, so preferred size must include its inset here.
        let frame = if self.opt.intersects(WidgetOption::FRAME) {
            style
                .control(ControlRole::Button, ControlState::Enabled(PointerState::Normal))
                .patch
                .insets
                .normalized()
        } else {
            crate::SliceInsets::ZERO
        };
        crate::ui_node::frame::outer_preferred(content, frame)
    }
}

impl Widget for DisclosureHeader {
    fn widget_opt(&self) -> &WidgetOption {
        // Static options describe only the header child, never the complete disclosure allocation.
        &self.opt
    }

    fn update(&mut self, ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) -> crate::Change {
        // Pointer/Enter/Space toggle while directional keys expose the conventional tree behavior.
        match ctx.action(input, self.keyboard_behavior()) {
            Some(crate::KeyboardAction::Activate) => {
                self.model.set(!self.model.expanded.get());
            }
            Some(crate::KeyboardAction::Expand) => {
                self.model.set(true);
            }
            Some(crate::KeyboardAction::Collapse) => {
                self.model.set(false);
            }
            Some(crate::KeyboardAction::Decrease | crate::KeyboardAction::Increase) | None => {}
        }

        // The header size is fixed; the parent independently collects the shared expansion marker.
        crate::Change::NoChange
    }

    fn paint(&mut self, ctx: &mut WidgetPaintCtx<'_>) {
        // Expansion determines the icon, while presentation variant determines background/frame.
        let expanded = self.model.expanded.get();
        let mut row = ctx.local_rect();
        match self.variant {
            DisclosureVariant::Header => {
                if self.opt.intersects(WidgetOption::FRAME) {
                    // The header owns this internal frame, so it resolves the complete button patch
                    // rather than asking retained traversal to inset the disclosure container.
                    row = ctx.draw_control(ControlRole::Button, row).unwrap_or_default();
                } else {
                    ctx.draw_control_center(ControlRole::Item, row);
                }
            }
            DisclosureVariant::Tree => ctx.draw_control_center(ControlRole::Item, row),
        }

        // Reserve a square icon cell from row height, then paint text in the remaining rectangle.
        let content_role = if self.variant == DisclosureVariant::Header && self.opt.intersects(WidgetOption::FRAME) {
            ControlRole::Button
        } else {
            ControlRole::Item
        };
        let text_color = ctx.control_content_color(content_role);
        let icon = if expanded {
            ctx.skin().resolve_icon_role(ctx.atlas(), crate::IconRole::Collapse)
        } else {
            ctx.skin().resolve_icon_role(ctx.atlas(), crate::IconRole::Expand)
        };
        ctx.draw_icon(icon, Recti::new(row.x, row.y, row.height, row.height), text_color);
        // text_offset = row_height - padding.
        let offset = row.height.saturating_sub(ctx.skin().metrics.padding);
        // text_x = row_x + text_offset; text_width = row_width - text_offset.
        let text_rect = Recti::new(row.x.saturating_add(offset), row.y, row.width.saturating_sub(offset), row.height);
        let body_font = ctx.skin().resolve_font_role(ctx.atlas(), crate::FontRole::Body);
        ctx.draw_control_text_with_font(body_font, &self.label, text_rect, content_role, self.opt);
    }

    // Framing belongs to the header sub-rectangle, not the complete descendant allocation.
    fn effective_widget_opt(&self) -> WidgetOption {
        crate::com::widget::WidgetAlgorithm::effective_options(self, self.opt)
    }

    fn keyboard_behavior(&self) -> crate::KeyboardBehavior {
        // The addressable header, rather than the structural disclosure container, participates in
        // sequential focus and later shared activation.
        crate::KeyboardBehavior::TAB_STOP
            | crate::KeyboardBehavior::ACTIVATE_ENTER
            | crate::KeyboardBehavior::ACTIVATE_SPACE
            | crate::KeyboardBehavior::EXPAND_COLLAPSE
    }
}

impl crate::LeafWidget for DisclosureHeader {
    fn measure(&self, style: &Skin, atlas: &AtlasHandle, _constraints: crate::Constraints) -> Dimensioni {
        // Header content is intrinsically one line and does not stretch to the offered bound.
        self.preferred(style, atlas)
    }
}

impl ContainerWidget for Disclosure {
    fn measure(&self, ctx: &mut MeasureCtx<'_>, constraints: crate::Constraints) -> Dimensioni {
        // The header always contributes. Body measurement is conditional so collapsed content does
        // not influence root auto-size while its state and nodes remain retained.
        let header = ctx.measure_child(Self::HEADER, constraints).unwrap_or_default();
        if !self.model.expanded.get() {
            return header;
        }
        let indent = self.indent(ctx.skin());
        let spacing = ctx.skin().metrics.spacing.max(0);
        let body_constraints = crate::Constraints::new(
            constraints.width.shrink(indent),
            constraints.height.shrink(header.height.saturating_add(spacing)),
        );
        let body = ctx.measure_child(Self::BODY, body_constraints).unwrap_or_default();
        Dimensioni::new(
            header.width.max(body.width.saturating_add(indent)),
            header.height.saturating_add(spacing).saturating_add(body.height),
        )
    }

    fn place(&mut self, ctx: &mut ContainerLayoutCtx<'_>, rect: Recti) {
        // Header is a fixed structural role and always occupies the first visible row.
        let preferred = ctx
            .measure_child(Self::HEADER, crate::Constraints::bounded(Dimensioni::new(rect.width, rect.height)))
            .unwrap_or_default();
        let header_height = preferred.height.min(rect.height.max(0));
        let _ = ctx.set_child_participation(Self::HEADER, ChildParticipation::Active);
        let _ = ctx.layout_child(Self::HEADER, Recti::new(rect.x, rect.y, rect.width, header_height));

        // Participation is the single gate consumed by update, paint, hit testing, and target
        // sanitation. No generic visibility bit or topology rewrite is needed for collapse.
        let participation = if self.model.expanded.get() {
            ChildParticipation::Active
        } else {
            ChildParticipation::Hidden
        };
        let _ = ctx.set_child_participation(Self::BODY, participation);
        if self.model.expanded.get() {
            let indent = self.indent(ctx.skin());
            let spacing = ctx.skin().metrics.spacing.max(0);
            // body_origin = container_origin + (indent, header_height + spacing).
            let body_x = rect.x.saturating_add(indent);
            let body_y = rect.y.saturating_add(header_height).saturating_add(spacing);
            // body_extent = container_extent - (indent, header_height + spacing).
            let body_width = rect.width.saturating_sub(indent);
            let body_height = rect.height.saturating_sub(header_height).saturating_sub(spacing);
            let body = Recti::new(body_x, body_y, body_width, body_height);
            let _ = ctx.layout_child(Self::BODY, body);
        }
    }
}

impl Widget for Disclosure {
    /// Expansion changes made by the header invalidate this parent even when ordinary updates skip descendants.
    fn take_shared_change(&self) -> bool {
        self.model.dirty.replace(false)
    }
    fn widget_opt(&self) -> &WidgetOption {
        &WidgetOption::NO_INTERACT
    }

    fn paint(&mut self, _ctx: &mut WidgetPaintCtx<'_>) {}
}

/// Builds the fixed body/header structure before wrapping it in the public owning node.
fn create_container(parameters: DisclosureParameters) -> (TypedWidgetHandle<Disclosure>, Container) {
    let (content, body) = Linear::create(LinearParameters::vertical(parameters.children));
    let model = DisclosureState::new(parameters.expanded);
    let widget = Disclosure {
        content: Some(content),
        model: model.clone(),
        variant: parameters.variant,
    };
    let header = DisclosureHeader {
        model,
        label: parameters.label,
        variant: parameters.variant,
        opt: parameters.opt,
    };
    // Visual and keyboard order are the same authoritative fixed header/body order.
    Container::new(widget, [Node::widget_internal(header), body])
}

impl Disclosure {
    /// Creates a child-owning disclosure and returns its weak typed widget handle plus completed node.
    ///
    /// Construction is complete before the node is returned: callers never observe or manipulate
    /// the two structural children separately.
    pub fn create(parameters: DisclosureParameters) -> (TypedWidgetHandle<Disclosure>, Node) {
        // Add the common Node runtime state around the concrete Container ownership boundary.
        let (state, container) = create_container(parameters);
        (state, Node::container(container))
    }
}



impl Disclosure {
    /// Creates COM semantic state while its concrete class retains the actual typed body capability.
    pub(crate) fn com_new(model: Rc<DisclosureState>, variant: DisclosureVariant) -> Self {
        Self { content: None, model, variant }
    }
}
impl DisclosureHeader {
    /// Creates the real header leaf with its shared expansion value and initialized label.
    pub(crate) fn com_new(model: Rc<DisclosureState>, label: String, variant: DisclosureVariant) -> Self {
        let opt = if variant == DisclosureVariant::Header {
            WidgetOption::FRAME
        } else {
            WidgetOption::NONE
        };
        Self { model, label, variant, opt }
    }
    /// Reads the label used by both preferred-size measurement and complete header painting.
    pub(crate) fn com_label(&self) -> &str {
        &self.label
    }
    /// Replaces the actual label without changing expansion or body ownership.
    pub(crate) fn com_set_label(&mut self, label: &str) {
        self.label.clear();
        self.label.push_str(label);
    }
}
impl crate::com::widget::WidgetAlgorithm for Disclosure {
    /// Parent has no surface-specific option storage; common COM options remain authoritative.
    fn set_options(&mut self, _options: WidgetOption) {}
}
impl crate::com::widget::WidgetAlgorithm for DisclosureHeader {
    /// Header-owned internal framing and text alignment use these concrete options.
    fn set_options(&mut self, options: WidgetOption) {
        self.opt = options;
    }
    /// Header painting already owns its internal frame; generic traversal must not add a second inset.
    fn effective_options(&self, options: WidgetOption) -> WidgetOption {
        options & !WidgetOption::FRAME
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Constructs one valid theme atlas whose expanded-state icon is larger than the collapsed one.
    fn asymmetric_disclosure_atlas() -> AtlasHandle {
        // Every texel is opaque white so overlapping semantic rectangles remain valid; only the
        // declared dimensions matter to this measurement regression.
        let pixels = vec![0xFF; 10 * 5 * 4];
        let icons = [
            ("white", Recti::new(0, 0, 1, 1)),
            ("close", Recti::new(0, 0, 1, 1)),
            ("expand", Recti::new(0, 0, 1, 1)),
            ("collapse", Recti::new(0, 0, 10, 5)),
            ("check", Recti::new(0, 0, 1, 1)),
            ("expand_down", Recti::new(0, 0, 1, 1)),
            ("open_folder", Recti::new(0, 0, 1, 1)),
            ("closed_folder", Recti::new(0, 0, 1, 1)),
            ("file", Recti::new(0, 0, 1, 1)),
        ];
        let glyphs = [(
            '_',
            crate::CharEntry {
                offset: crate::Vec2i::new(0, 0),
                advance: crate::Vec2i::new(1, 0),
                rect: Recti::new(0, 0, 1, 1),
            },
        )];
        let fonts = [(
            "body",
            crate::FontEntry {
                line_size: 1,
                baseline: 1,
                font_size: 1,
                entries: &glyphs,
            },
        )];
        let source = crate::AtlasSource {
            width: 10,
            height: 5,
            pixels: &pixels,
            icons: &icons,
            fonts: &fonts,
            format: crate::SourceFormat::Raw,
        };

        AtlasHandle::try_from(&source).expect("asymmetric disclosure fixture must satisfy the atlas contract")
    }

    #[test]
    fn header_and_tree_preserve_state_and_fixed_structural_children() {
        let (header_state, header) = create_container(DisclosureParameters::header("Header", false, std::iter::empty::<LinearItem>()));
        assert_eq!(header_state.try_read(Disclosure::is_collapsed), Some(true));
        header_state.try_update(Disclosure::toggle).unwrap();
        assert_eq!(header_state.try_read(Disclosure::is_expanded), Some(true));
        assert_eq!(Node::container(header).debug_node_count(), 3);

        let custom_opt = WidgetOption::ALIGN_RIGHT | WidgetOption::NO_INTERACT;
        let (tree_state, tree) = create_container(DisclosureParameters::tree("Tree", true, std::iter::empty::<LinearItem>()).with_options(custom_opt));
        assert_eq!(tree_state.try_read(Disclosure::is_expanded), Some(true));
        assert_eq!(Node::container(tree).debug_node_count(), 3);
    }

    /// Verifies either expansion state fits the one state-independent header allocation.
    #[test]
    fn header_measurement_reserves_the_larger_expand_or_collapse_icon() {
        let atlas = asymmetric_disclosure_atlas();
        let style = Skin::from_atlas(&atlas).with_metrics(|metrics| metrics.padding = 0);
        let (_, container) = create_container(DisclosureParameters::tree("", false, std::iter::empty::<LinearItem>()));
        let mut root = Node::container(container);
        let mut runtime = crate::ui_node::UiRuntime::new();

        let preferred = runtime.measure_tree_root(&mut root, &style, &atlas, crate::Constraints::unbounded());

        // The 10x5 collapse icon plus one vertical pixel on each side determines 10x7. Measuring
        // only the 1x1 expand icon would incorrectly return 3x3 and clip after expansion.
        assert_eq!((preferred.width, preferred.height), (10, 7));
    }
}
