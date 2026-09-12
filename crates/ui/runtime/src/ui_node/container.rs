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

use crate::math::RectExt;
use crate::{Constraints, Dimensioni, Recti, Skin, UiInputEvent, Vec2i, Widget};
use super::ChildParticipation;
use crate::ui_node::layout_service::ScopedLayout;
use neocogi_ui_core_contracts::ContainerGeometry;
use neocogi_com::NcStatus;
#[cfg(not(test))]
use crate::ui_node::layout_service::ScopedLayout as ChildLayout;
#[cfg(test)]
use crate::test_support::layout::ChildLayout;

/// Private statically dispatched algorithms for built-in COM containers.
/// Child ownership and traversal remain behind the same scoped service used by external plugins.
pub trait ContainerWidget: Widget {
    /// Supplies the native fixture's weak topology link; deployed widgets use IChildrenEditor.
    #[cfg(test)]
    fn bind_children(&mut self, _children: crate::ChildrenHandle) {}
    /// Computes preferred content from indexed child measurements.
    fn measure(&self, ctx: &mut MeasureCtx<'_>, constraints: Constraints) -> Dimensioni;
    /// Authors child placements and one complete copied parent geometry result.
    fn place(&mut self, ctx: &mut ContainerLayoutCtx<'_>, rect: Recti);
    /// Filters an already selected event without changing router ownership.
    fn accepts_event(&self, _event: &UiInputEvent) -> bool {
        true
    }
}

/// Scoped measurement services borrowed by one concrete built-in algorithm.
pub struct MeasureCtx<'a> {
    /// Complete skin for this invocation's immutable resource revision.
    skin: &'a Skin,
    /// Atlas paired with that skin; no resource identity crosses an unchecked boundary.
    atlas: &'a crate::AtlasHandle,
    /// Expiring child geometry authority, never a borrowed production node collection.
    layout: ChildLayout<'a>,
}
impl<'a> MeasureCtx<'a> {
    /// Binds the validated service for exactly this synchronous COM call.
    pub(crate) fn scoped(skin: &'a Skin, atlas: &'a crate::AtlasHandle, layout: ScopedLayout<'a>) -> Self {
        #[cfg(test)]
        let layout = ChildLayout::Com(layout);
        Self { skin, atlas, layout }
    }
    /// Supports native algorithm fixtures without changing the deployed representation.
    #[cfg(test)]
    pub(crate) fn new(skin: &'a Skin, atlas: &'a crate::AtlasHandle, children: &'a mut crate::Children) -> Self {
        Self {
            skin,
            atlas,
            layout: ChildLayout::native(skin, atlas, children, None),
        }
    }
    /// Returns the first failed child operation before the caller publishes a preference.
    pub(crate) fn result(&self) -> Result<(), NcStatus> {
        self.layout.result()
    }
    /// Returns the skin installed for this call.
    pub fn skin(&self) -> &Skin {
        self.skin
    }
    /// Returns the exact atlas paired with the installed skin.
    pub fn atlas(&self) -> &crate::AtlasHandle {
        self.atlas
    }
    /// Returns cardinality validated before entering the algorithm.
    pub fn child_count(&self) -> usize {
        self.layout.child_count()
    }
    /// Measures one child under explicit finite or unbounded limits.
    pub fn measure_child(&mut self, index: usize, constraints: Constraints) -> Option<Dimensioni> {
        self.layout.measure(index, constraints)
    }
}

/// Scoped placement services and copied parent output for one built-in COM container.
pub struct ContainerLayoutCtx<'a> {
    /// Immutable appearance for this layout pass.
    skin: &'a Skin,
    /// Immutable resources used for intrinsic geometry.
    atlas: &'a crate::AtlasHandle,
    /// Assigned content rectangle limits the descendant viewport.
    content: Recti,
    /// No retained runtime node is borrowed by a production algorithm.
    layout: ChildLayout<'a>,
    /// Published atomically only when all child operations succeeded.
    geometry: ContainerGeometry,
}
impl<'a> ContainerLayoutCtx<'a> {
    /// Initializes complete output for one validated scoped layout call.
    pub(crate) fn scoped(skin: &'a Skin, atlas: &'a crate::AtlasHandle, content: Recti, layout: ScopedLayout<'a>) -> Self {
        #[cfg(test)]
        let layout = ChildLayout::Com(layout);
        Self::with_layout(skin, atlas, content, layout)
    }
    /// Shares output initialization between the real service and native algorithm fixtures.
    fn with_layout(skin: &'a Skin, atlas: &'a crate::AtlasHandle, content: Recti, layout: ChildLayout<'a>) -> Self {
        Self {
            skin,
            atlas,
            content,
            layout,
            geometry: ContainerGeometry::new(crate::boundary::copied_rect(content)),
        }
    }
    /// Native fixtures use the same copied parent output as actual COM containers.
    #[cfg(test)]
    pub(crate) fn new(
        runtime: &'a mut crate::ui_node::LayoutTraversal,
        skin: &'a Skin,
        atlas: &'a crate::AtlasHandle,
        content: Recti,
        children: &'a mut crate::Children,
    ) -> Self {
        Self::with_layout(skin, atlas, content, ChildLayout::native(skin, atlas, children, Some(runtime)))
    }
    /// Rejects partial output after any failed child service operation.
    pub(crate) fn result(&self) -> Result<ContainerGeometry, NcStatus> {
        self.layout.result()?;
        Ok(self.geometry)
    }
    /// Returns the skin used for measurement and placement in this invocation.
    pub fn skin(&self) -> &Skin {
        self.skin
    }
    /// Returns its paired immutable atlas.
    pub fn atlas(&self) -> &crate::AtlasHandle {
        self.atlas
    }
    /// Returns the direct child count validated before entering policy code.
    pub fn child_count(&self) -> usize {
        self.layout.child_count()
    }
    /// Measures without exposing retained geometry caches.
    pub fn measure_child(&mut self, index: usize, constraints: Constraints) -> Option<Dimensioni> {
        self.layout.measure(index, constraints)
    }
    /// Assigns one exact parent-owned child rectangle.
    pub fn layout_child(&mut self, index: usize, rect: Recti) -> Option<Dimensioni> {
        self.layout.place(index, rect)
    }
    /// Controls participation of the complete indexed child subtree.
    pub fn set_child_participation(&mut self, index: usize, participation: ChildParticipation) -> bool {
        self.layout.set_participation(index, participation)
    }
    /// Invalidates a shared placement dependency even when allocation is unchanged.
    pub fn child_placement_changed(&mut self, index: usize) -> bool {
        self.layout.invalidate_placement(index)
    }
    /// Replaces derived extent without modifying the descendant viewport.
    pub fn set_content_size(&mut self, size: Dimensioni) {
        self.geometry.content = neocogi_ui_core_contracts::Size { width: size.width, height: size.height };
    }
    /// Narrows descendant clipping to the assigned content and copies its translation.
    pub fn set_children_viewport(&mut self, viewport: Recti, offset: Vec2i) {
        let viewport = viewport
            .positive_intersection(self.content)
            .unwrap_or_else(|| Recti::new(self.content.x, self.content.y, 0, 0));
        self.geometry.viewport = crate::boundary::copied_rect(viewport);
        self.geometry.offset_x = offset.x;
        self.geometry.offset_y = offset.y;
    }
    /// Selects whether visible descendant overflow enlarges the parent's logical extent.
    pub fn set_child_overflow_propagation(&mut self, propagate: bool) {
        self.geometry.propagate_overflow = propagate;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};
    use crate::{Container, Children, WidgetOption};

    /// Minimal layout implementation proving that concrete ownership needs geometry only.
    struct GeometryOnly;

    impl Widget for GeometryOnly {
        fn widget_opt(&self) -> &WidgetOption {
            &WidgetOption::NO_INTERACT
        }

        fn paint(&self, _ctx: &mut crate::WidgetPaintCtx<'_>) {}
    }

    impl ContainerWidget for GeometryOnly {
        fn measure(&self, ctx: &mut MeasureCtx<'_>, _constraints: Constraints) -> Dimensioni {
            Dimensioni::new(ctx.child_count() as i32, 1)
        }

        fn place(&mut self, _ctx: &mut ContainerLayoutCtx<'_>, _rect: Recti) {}
    }

    /// Creates one real unique node for ownership checks.
    fn text_node(label: &str) -> super::super::Node {
        crate::TextBlock::create(crate::TextBlockParameters::new(label)).1
    }

    #[test]
    fn concrete_container_owns_one_collection_beside_its_widget() {
        let children = Rc::new(RefCell::new([text_node("first")].into_iter().collect()));
        let handle = crate::ChildrenHandle::new(&children);
        let (_, owner) = Container::from_shared(children, GeometryOnly);

        assert_eq!(owner.with_children(Children::len), 1);
        owner.with_children_mut(|children| children.push(text_node("second")));
        assert_eq!(handle.len(), Some(2));
        drop(owner);
        assert_eq!(handle.len(), None);
    }
}
