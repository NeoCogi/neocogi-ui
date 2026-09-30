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
    AtlasHandle, ChildParticipation, ContainerWidget, Dimensioni, MeasureCtx, Recti, Skin, UiInputEvent, Widget, WidgetOption, WidgetPaintCtx, WidgetUpdateCtx,
};

use super::{ContainerLayoutCtx};

/// Immutable header presentation shared by disclosure layout and its actual interactive header.
#[derive(Copy, Clone, Eq, PartialEq)]
enum DisclosureVariant {
    /// Framed header row with a non-indented body.
    Header,
    /// Unframed tree row with an indented body.
    Tree,
}

impl DisclosureVariant {
    /// Supplies construction defaults without retaining a second option value.
    fn header_options(self) -> WidgetOption {
        match self {
            Self::Header => WidgetOption::FRAME,
            Self::Tree => WidgetOption::NONE,
        }
    }
}

/// Explicit expansion data shared by the actual header and its layout parent, without object backpointers.
struct DisclosureState {
    /// Current body participation policy.
    expanded: std::cell::Cell<bool>,
    /// Shared edits persist until the parent collects them, even while its body is hidden.
    dirty: std::cell::Cell<bool>,
}
impl DisclosureState {
    /// Creates one authoritative expansion value for the composed widget.
    fn new(expanded: bool) -> Rc<Self> {
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

/// Application-facing state for a disclosure container.
///
/// Expansion gates descendant traversal while retaining every owned runtime and its state. It does
/// not expose or mutate generic node visibility.
#[neocogi_com::com_object(neocogi_ui_nodes_contracts::Disclosure)]
pub struct Disclosure {
    /// Non-semantic COM lifecycle and host-backed phase resources.
    runtime: crate::widget_object::WidgetRuntime,
    /// Authoritative fixed header/body ownership used by COM traversal.
    editor: neocogi_com::ComBox<neocogi_ui_core_contracts::IChildrenEditorAbi>,
    /// Text capability of the actual header child.
    header: neocogi_com::WeakComBox<neocogi_ui_core_contracts::ITextContentAbi>,
    /// Typed editing capability of the actual vertical body child.
    body: neocogi_com::WeakComBox<neocogi_ui_nodes_contracts::LinearAbi>,
    /// Same expansion scalar read by the actual header object and parent layout.
    model: Rc<DisclosureState>,
    variant: DisclosureVariant,
}

impl Disclosure {
    /// Combines one fixed child composition with the semantic expansion and layout policy.
    fn from_runtime(
        runtime: crate::widget_object::WidgetRuntime,
        editor: neocogi_com::ComBox<neocogi_ui_core_contracts::IChildrenEditorAbi>,
        header: neocogi_com::WeakComBox<neocogi_ui_core_contracts::ITextContentAbi>,
        body: neocogi_com::WeakComBox<neocogi_ui_nodes_contracts::LinearAbi>,
        model: Rc<DisclosureState>,
        variant: DisclosureVariant,
    ) -> Self {
        Self {
            runtime,
            editor,
            header,
            body,
            model,
            variant,
        }
    }

    /// Returns whether descendants participate in traversal.
    pub fn is_expanded(&self) -> bool {
        self.model.expanded.get()
    }

    /// Makes descendants participate in traversal.
    pub fn expand(&mut self) {
        self.model.set(true);
    }

    /// Gates descendants while retaining their ownership and state.
    pub fn collapse(&mut self) {
        self.model.set(false);
    }

    // Structural child order is also retained keyboard traversal order. Keeping the visible
    // header first makes Tab reach it before any expanded descendants without a parallel order.
    const HEADER: usize = 0;
    const BODY: usize = 1;

    fn indent(&self, style: &Skin) -> i32 {
        if matches!(self.variant, DisclosureVariant::Tree) {
            style.metrics().indent.max(0)
        } else {
            0
        }
    }
}

/// Dispatcher-addressable disclosure header.
///
/// The header owns no descendants and shares only expansion data with its layout parent.
#[neocogi_com::com_object(interfaces(neocogi_ui_core_contracts::ITextContent, neocogi_ui_core_contracts::ILeafLayout))]
struct DisclosureHeader {
    /// Non-semantic COM lifecycle and host-backed phase resources.
    runtime: crate::widget_object::WidgetRuntime,
    /// Shared scalar state owns no parent widget or tree.
    model: Rc<DisclosureState>,
    /// Owned text shared by measurement and painting.
    label: String,
    /// Chooses indentation and the common frame appearance.
    variant: DisclosureVariant,
}

impl DisclosureHeader {
    /// Measures intrinsic header content; retained traversal adds the optional common frame.
    fn preferred(&self, style: &Skin, atlas: &AtlasHandle) -> Dimensioni {
        // Resolve icon and text metrics independently, then build the one-row content preference.
        let padding = style.metrics().padding.max(0);
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
        Dimensioni::new(content_width, content_height)
    }
}

impl Widget for DisclosureHeader {
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

    fn paint(&self, ctx: &mut WidgetPaintCtx<'_>) {
        // Expansion determines the icon, while presentation variant determines background/frame.
        let expanded = self.model.expanded.get();
        let row = ctx.local_rect();
        // Common traversal already paints and insets a requested frame. An unframed
        // header supplies only the item center for hover/selection feedback.
        if !ctx.options().intersects(WidgetOption::FRAME) {
            ctx.draw_control_center(ControlRole::Item, row);
        }

        // Reserve a square icon cell from row height, then paint text in the remaining rectangle.
        let content_role = if self.variant == DisclosureVariant::Header && ctx.options().intersects(WidgetOption::FRAME) {
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
        let offset = row.height.saturating_sub(ctx.skin().metrics().padding);
        // text_x = row_x + text_offset; text_width = row_width - text_offset.
        let text_rect = Recti::new(row.x.saturating_add(offset), row.y, row.width.saturating_sub(offset), row.height);
        let body_font = ctx.skin().resolve_font_role(ctx.atlas(), crate::FontRole::Body);
        ctx.draw_control_text_with_font(body_font, &self.label, text_rect, content_role, ctx.options());
    }

    /// Selects one shared frame for measurement, hit geometry and painting.
    fn frame_appearance_role(&self) -> crate::FrameRole {
        crate::FrameRole::Control(match self.variant {
            DisclosureVariant::Header => ControlRole::Button,
            DisclosureVariant::Tree => ControlRole::Item,
        })
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
    /// Measures child preferences, stopping immediately on a failed service call.
    fn measure(&self, ctx: &mut MeasureCtx<'_>, constraints: crate::Constraints) -> Result<Dimensioni, NcStatus> {
        // The header always contributes. Body measurement is conditional so collapsed content does
        // not influence root auto-size while its state and nodes remain retained.
        let header = ctx.measure_child(Self::HEADER, constraints)?;
        if !self.model.expanded.get() {
            return Ok(header);
        }
        let indent = self.indent(ctx.skin());
        let spacing = ctx.skin().metrics().spacing.max(0);
        let body_constraints = crate::Constraints::new(
            constraints.width.shrink(indent),
            constraints.height.shrink(header.height.saturating_add(spacing)),
        );
        let body = ctx.measure_child(Self::BODY, body_constraints)?;
        Ok(Dimensioni::new(
            header.width.max(body.width.saturating_add(indent)),
            header.height.saturating_add(spacing).saturating_add(body.height),
        ))
    }

    /// Places children in policy order and returns the first failed operation.
    fn place(&mut self, ctx: &mut ContainerLayoutCtx<'_>, rect: Recti) -> Result<(), NcStatus> {
        // Header is a fixed structural role and always occupies the first visible row.
        let preferred = ctx.measure_child(Self::HEADER, crate::Constraints::bounded(Dimensioni::new(rect.width, rect.height)))?;
        let header_height = preferred.height.min(rect.height.max(0));
        ctx.set_child_participation(Self::HEADER, ChildParticipation::Active)?;
        ctx.layout_child(Self::HEADER, Recti::new(rect.x, rect.y, rect.width, header_height))?;

        // Participation is the single gate consumed by update, paint, hit testing, and target
        // sanitation. No generic visibility bit or topology rewrite is needed for collapse.
        let participation = if self.model.expanded.get() {
            ChildParticipation::Active
        } else {
            ChildParticipation::Hidden
        };
        ctx.set_child_participation(Self::BODY, participation)?;
        if self.model.expanded.get() {
            let indent = self.indent(ctx.skin());
            let spacing = ctx.skin().metrics().spacing.max(0);
            // body_origin = container_origin + (indent, header_height + spacing).
            let body_x = rect.x.saturating_add(indent);
            let body_y = rect.y.saturating_add(header_height).saturating_add(spacing);
            // body_extent = container_extent - (indent, header_height + spacing).
            let body_width = rect.width.saturating_sub(indent);
            let body_height = rect.height.saturating_sub(header_height).saturating_sub(spacing);
            let body = Recti::new(body_x, body_y, body_width, body_height);
            ctx.layout_child(Self::BODY, body)?;
        }
        Ok(())
    }
}

impl Widget for Disclosure {
    /// Expansion changes made by the header invalidate this parent even when ordinary updates skip descendants.
    fn take_shared_change(&self) -> bool {
        self.model.dirty.replace(false)
    }

    fn paint(&self, _ctx: &mut WidgetPaintCtx<'_>) {}
}

impl Disclosure {}

impl DisclosureHeader {
    /// Combines the shared expansion value and label with one concrete header runtime.
    pub(crate) fn from_runtime(runtime: crate::widget_object::WidgetRuntime, model: Rc<DisclosureState>, label: String, variant: DisclosureVariant) -> Self {
        Self { runtime, model, label, variant }
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
impl crate::widget_object::WidgetAlgorithm for Disclosure {}

impl crate::widget_object::HostedWidget for Disclosure {
    /// Borrows common phase state stored directly beside Disclosure's semantic fields.
    fn runtime(&self) -> &crate::widget_object::WidgetRuntime {
        &self.runtime
    }

    /// Exclusively borrows common phase state for mutation or resource refresh.
    fn runtime_mut(&mut self) -> &mut crate::widget_object::WidgetRuntime {
        &mut self.runtime
    }

}

impl crate::widget_object::WidgetAlgorithm for DisclosureHeader {}

impl crate::widget_object::HostedWidget for DisclosureHeader {
    /// Borrows common phase state stored directly beside the header's semantic fields.
    fn runtime(&self) -> &crate::widget_object::WidgetRuntime {
        &self.runtime
    }

    /// Exclusively borrows common phase state for mutation or resource refresh.
    fn runtime_mut(&mut self) -> &mut crate::widget_object::WidgetRuntime {
        &mut self.runtime
    }
}

use crate::widget_object::{HostedWidget, WidgetRuntime, direct_container_layout_impl, direct_leaf_layout_impl, direct_widget_phase_impl};
use neocogi_ui_core_contracts::{IChildrenEditorAbi, ITextContent, ITextContentAbi, IWidgetAbi, IWidgetHostAbi};
use neocogi_ui_nodes_contracts::{CrossSize, DisclosureAbi, DisclosureStyle, FlowDirection, LinearAbi};
use neocogi_com::{ComPtr, INcStringAbi, NcCopyFromBufferOffset, NcStatus, NcString};

direct_widget_phase_impl!(DisclosureHeader);
direct_leaf_layout_impl!(DisclosureHeader);

impl DisclosureHeader {
    /// Creates the internal header capability directly on its semantic allocation.
    fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        options: crate::WidgetOption,
        model: Rc<DisclosureState>,
        label: String,
        variant: DisclosureVariant,
        resources: Option<&crate::widget_object::SharedAssets>,
    ) -> Result<neocogi_com::ComBox<ITextContentAbi>, NcStatus> {
        let runtime = WidgetRuntime::for_resources(host, options, resources)?;
        Ok(Self::into_box(Self::from_runtime(runtime, model, label, variant))
            .query()
            .expect("DisclosureHeader declares its text and leaf capabilities"))
    }
}

impl ITextContent for DisclosureHeader {
    /// Copies the label used by this same header's measurement and painting.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.runtime().state.readable()?;
        Ok(NcString::from_utf8_str(self.com_label()))
    }

    /// Replaces the concrete header label and records intrinsic geometry invalidation.
    fn set_text(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        let label = crate::boundary::text(input)?;
        if self.com_label() != label {
            self.com_set_label(label);
            self.runtime_mut().state.record_change(neocogi_ui_core_contracts::GeometryChange::Changed);
        }
        Ok(())
    }
}

direct_widget_phase_impl!(Disclosure);
direct_container_layout_impl!(Disclosure);

impl Disclosure {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(crate) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: crate::widget_object::SharedAssets,
        label: NcCopyFromBufferOffset<'_, u8>,
        style: DisclosureStyle,
        expanded: bool,
    ) -> Result<neocogi_com::ComBox<DisclosureAbi>, NcStatus> {
        let value = Self::build(host, label, style, expanded, Some(&resources))?;
        Ok(Self::into_box(value).query().expect("Disclosure declares its callable class contract"))
    }

    /// Builds the fixed header/body tree with either direct or runtime-shared resources.
    fn build(
        host: ComPtr<IWidgetHostAbi>,
        label: NcCopyFromBufferOffset<'_, u8>,
        style: DisclosureStyle,
        expanded: bool,
        resources: Option<&crate::widget_object::SharedAssets>,
    ) -> Result<Self, NcStatus> {
        let label = crate::boundary::text(label)?;
        host.check_mutation()?;
        let variant = match style {
            DisclosureStyle::Header => DisclosureVariant::Header,
            DisclosureStyle::Tree => DisclosureVariant::Tree,
            _ => return Err(neocogi_com::NC_E_INVALID_ARG),
        };
        let model = DisclosureState::new(expanded);
        let runtime = WidgetRuntime::for_resources(host.clone(), crate::WidgetOption::NO_INTERACT, resources)?;
        let children = prepare_children(host, model.clone(), label.to_owned(), variant, resources)?;
        Ok(Self::from_runtime(runtime, children.editor, children.header, children.body, model, variant))
    }
}

impl neocogi_ui_nodes_contracts::Disclosure for Disclosure {
    /// Observes the same expansion bit used by header input and container layout.
    fn expanded(&self) -> Result<bool, NcStatus> {
        self.runtime().state.readable()?;
        Ok(self.is_expanded())
    }

    /// Changes the shared bit through the concrete algorithm's ordinary mutation path.
    fn set_expanded(&mut self, expanded: bool) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        if expanded {
            self.expand();
        } else {
            self.collapse();
        }
        Ok(())
    }

    /// Returns the actual header whose widget options control presentation and input.
    fn header(&self) -> Result<neocogi_com::WeakComBox<ITextContentAbi>, NcStatus> {
        self.runtime().state.readable()?;
        Ok(self.header.clone())
    }

    /// Returns the actual Linear body whose typed edits preserve relationship cardinality.
    fn body(&self) -> Result<neocogi_com::WeakComBox<LinearAbi>, NcStatus> {
        self.runtime().state.readable()?;
        Ok(self.body.clone())
    }
}

/// Completed fixed disclosure topology consumed by either construction path.
struct DisclosureChildren {
    /// Writer owning the header and body in permanent traversal order.
    editor: neocogi_com::ComBox<IChildrenEditorAbi>,
    /// Text capability of the actual header returned by the public class.
    header: neocogi_com::WeakComBox<ITextContentAbi>,
    /// Concrete Linear capability of the actual body returned by the public class.
    body: neocogi_com::WeakComBox<LinearAbi>,
}

/// Builds both structural children before publishing their authoritative sequence.
fn prepare_children(
    host: ComPtr<IWidgetHostAbi>,
    model: Rc<DisclosureState>,
    label: String,
    variant: DisclosureVariant,
    resources: Option<&crate::widget_object::SharedAssets>,
) -> Result<DisclosureChildren, NcStatus> {
    // Construct both children before publishing the sequence, so callers cannot observe a partial
    // disclosure. The shared scalar model contains no parent or child object backpointer.
    let header = DisclosureHeader::create_com(host.clone(), variant.header_options(), model, label, variant, resources)?;
    let body = if let Some(resources) = resources {
        super::Linear::create_com(host.clone(), resources.clone(), FlowDirection::TopToBottom, CrossSize::Stretch)?
    } else {
        let value = super::Linear::build(host.clone(), FlowDirection::TopToBottom, CrossSize::Stretch, None)?;
        super::Linear::into_box(value).query().expect("Linear declares its callable class contract")
    };
    let editor = host.create_children()?;
    let header_view = header.downgrade();
    let body_view = body.downgrade();
    editor.insert(0, header.query::<IWidgetAbi>()?)?;
    editor.insert(1, body.query::<IWidgetAbi>()?)?;
    let header = header_view;
    let body = body_view;
    Ok(DisclosureChildren { editor, header, body })
}

#[cfg(test)]
mod tests {}
