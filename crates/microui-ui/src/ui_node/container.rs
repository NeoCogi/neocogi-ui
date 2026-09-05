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

use std::{cell::RefCell, rc::Rc};

use crate::math::RectExt;
use crate::{Constraints, Dimensioni, Recti, SliceInsets, Skin, TypedWidgetHandle, UiInputEvent, Vec2i, Widget, WidgetOption};

use crate::com::layout::ScopedLayout;
use microui_contracts::{ContainerGeometry, LayoutParticipation};
use nc_com::NcStatus;

use super::{ChildParticipation, Children, ChildrenHandle, NodeLayout, NodeRuntime, LayoutTraversal, WidgetStorage};

/// Scoped services for measuring one retained container's children.
///
/// The context owns the mutable child access required by node-local measurement caches but exposes
/// only derived geometry. Neither a [`Node`](crate::Node) nor the child collection crosses the
/// public container-widget boundary.
pub struct MeasureCtx<'a> {
    /// Complete context skin used by the actively measured container.
    skin: &'a Skin,
    /// Immutable atlas paired with `skin` for concrete font and icon resolution.
    atlas: &'a crate::AtlasHandle,
    /// Authoritative direct children available only for this scoped measure call.
    children: MeasureChildren<'a>,
}

/// Private transition between the existing local baseline and owned COM child geometry.
enum MeasureChildren<'a> {
    /// Local nodes remain borrowed only inside the existing baseline implementation.
    Local(&'a mut Children),
    /// COM calls access runtime-owned storage through a validated scoped service.
    Scoped(ScopedLayout<'a>),
}

impl<'a> MeasureCtx<'a> {
    /// Creates one runtime-scoped measurement context.
    pub(crate) fn new(skin: &'a Skin, atlas: &'a crate::AtlasHandle, children: &'a mut Children) -> Self {
        // Store the complete resolved skin and matching atlas borrowed for this measurement scope.
        Self {
            skin,
            atlas,
            children: MeasureChildren::Local(children),
        }
    }

    /// Reuses concrete built-in measurement against the same public service used by independent containers.
    pub(crate) fn scoped(skin: &'a Skin, atlas: &'a crate::AtlasHandle, layout: ScopedLayout<'a>) -> Self {
        Self {
            skin,
            atlas,
            children: MeasureChildren::Scoped(layout),
        }
    }

    /// Returns any child-service error before the enclosing implementation publishes a size.
    pub(crate) fn result(&self) -> Result<(), NcStatus> {
        match &self.children {
            MeasureChildren::Local(_) => Ok(()),
            MeasureChildren::Scoped(layout) => layout.result(),
        }
    }

    /// Returns the active UI skin.
    pub fn skin(&self) -> &Skin {
        self.skin
    }

    /// Returns the active atlas.
    pub fn atlas(&self) -> &crate::AtlasHandle {
        self.atlas
    }

    /// Returns the number of direct retained children.
    pub fn child_count(&self) -> usize {
        match &self.children {
            MeasureChildren::Local(children) => children.len(),
            MeasureChildren::Scoped(layout) => layout.count,
        }
    }

    /// Measures one indexed child's desired size under explicit constraints.
    pub fn measure_child(&mut self, index: usize, constraints: Constraints) -> Option<Dimensioni> {
        match &mut self.children {
            MeasureChildren::Local(children) => Some(children.get_mut(index)?.measure(self.skin, self.atlas, constraints)),
            MeasureChildren::Scoped(layout) => layout.measure(index, constraints),
        }
    }
}

/// Complete behavior of one retained branch widget.
///
/// The concrete widget owns semantic, interaction, and layout behavior while [`Container`] owns the
/// heterogeneous child collection separately. Runtime calls never retain the widget borrow while
/// recursively visiting descendants.
pub trait ContainerWidget: Widget {
    /// Receives weak topology access once during construction, before the container is observable.
    ///
    /// Mutable custom containers store this in a private field initialized with `Default`.
    /// Fixed-topology widgets can ignore it. Do not edit during this hook: initial children and
    /// any matching layout metadata must already describe the same sequence. Topology operations
    /// invalidate measurement independently of the widget's [`crate::Change`] result.
    fn bind_children(&mut self, _children: ChildrenHandle) {}

    /// Measures preferred content from the authoritative child collection.
    fn measure(&self, ctx: &mut MeasureCtx<'_>, constraints: Constraints) -> Dimensioni;

    /// Places retained children and commits descendant viewport/content geometry.
    fn place(&mut self, ctx: &mut ContainerLayoutCtx<'_>, rect: Recti);

    /// Returns whether this container accepts an already-selected event.
    ///
    /// Returning `false` permits ancestor bubbling but never exposes a covered sibling.
    fn accepts_event(&self, _event: &UiInputEvent) -> bool {
        true
    }
}

/// Retained owner for one erased typed branch widget and an opaque ordered child collection.
///
/// This is the only strong owner of its direct [`Node`](crate::Node) values. The concrete widget is
/// independently allocated so runtime phases can scope widget and child access to one operation
/// without exposing either retained allocation through the public container API.
pub struct Container {
    /// One authoritative shared cell for mounted descendants.
    children: Rc<RefCell<Children>>,
    /// Sole persistent strong owner of the concrete typed container widget.
    widget: Rc<RefCell<WidgetStorage<dyn ContainerWidget>>>,
}

impl Container {
    /// Creates an unmounted typed branch widget and its weak concrete handle.
    pub fn new<W>(widget: W, children: impl IntoIterator<Item = super::Node>) -> (TypedWidgetHandle<W>, Self)
    where
        W: ContainerWidget + 'static,
    {
        Self::from_shared(Rc::new(RefCell::new(children.into_iter().collect())), widget)
    }

    /// Creates a typed branch around child storage prepared by a built-in constructor.
    pub(crate) fn from_shared<W>(children: Rc<RefCell<Children>>, widget: W) -> (TypedWidgetHandle<W>, Self)
    where
        W: ContainerWidget + 'static,
    {
        let widget = Rc::new(RefCell::new(WidgetStorage::new(widget)));
        Self::from_shared_owner(children, widget)
    }

    /// Adopts a concrete widget allocation prepared for internal weak composition links.
    pub(crate) fn from_shared_owner<W>(children: Rc<RefCell<Children>>, widget: Rc<RefCell<WidgetStorage<W>>>) -> (TypedWidgetHandle<W>, Self)
    where
        W: ContainerWidget + 'static,
    {
        // Built-in and downstream widgets receive exactly the same weak ownership capability.
        widget.borrow_mut().widget.bind_children(ChildrenHandle::new(&children));
        let handle = TypedWidgetHandle::new(&widget);
        let widget: Rc<RefCell<WidgetStorage<dyn ContainerWidget>>> = widget;
        (handle, Self { children, widget })
    }

    /// Returns whether the installed surface supports one dispatcher-selected event.
    pub(crate) fn accepts_event(&self, event: &UiInputEvent) -> bool {
        // A geometry-only container has no event-receiving surface. An installed surface owns only
        // its additional state-dependent filter; generic option policy remains in the dispatcher.
        self.widget
            .try_borrow()
            .unwrap_or_else(|_| typed_container_borrow_conflict())
            .widget
            .accepts_event(event)
    }

    /// Runs one immutable framework traversal without exposing child storage publicly.
    pub(crate) fn with_children<R>(&self, f: impl FnOnce(&Children) -> R) -> R {
        // A failed borrow is a phase-ordering bug, not a recoverable absence of children. Panicking
        // here reports the invariant violation at the ownership boundary instead of deeper in
        // recursive traversal.
        let children = self
            .children
            .try_borrow()
            .unwrap_or_else(|_| panic!("retained child invariant violated: collection is mutably borrowed during immutable traversal"));
        f(&children)
    }

    /// Runs one mutable framework traversal while preventing concurrent topology mutation.
    pub(crate) fn with_children_mut<R>(&self, f: impl FnOnce(&mut Children) -> R) -> R {
        // Hold the borrow for the complete callback so topology cannot change while a recursive
        // runtime pass is using node references from this collection.
        let mut children = self
            .children
            .try_borrow_mut()
            .unwrap_or_else(|_| panic!("retained child invariant violated: collection is already borrowed during mutable traversal"));
        f(&mut children)
    }

    /// Invokes the geometry implementation for one placement pass.
    pub(crate) fn place(&mut self, runtime: &mut LayoutTraversal, skin: &Skin, atlas: &crate::AtlasHandle, rect: Recti, current: &mut NodeRuntime) {
        // The widget receives the authoritative collection only for this call. A typed mutation
        // attempted against the same container while this borrow is active is rejected cleanly by
        // ChildrenHandle rather than invalidating indices during placement.
        let mut children = self
            .children
            .try_borrow_mut()
            .unwrap_or_else(|_| panic!("retained child invariant violated: collection is already borrowed during layout"));
        let mut ctx = ContainerLayoutCtx::new(runtime, skin, atlas, rect, current, &mut children);
        self.widget
            .try_borrow_mut()
            .unwrap_or_else(|_| typed_container_borrow_conflict())
            .widget
            .place(&mut ctx, rect);
    }

    /// Resolves frame insets and measures content under one typed-runtime borrow.
    pub(crate) fn measure_content_with_frame(&mut self, style: &Skin, atlas: &crate::AtlasHandle, constraints: Constraints) -> (SliceInsets, Dimensioni) {
        let mut children = self
            .children
            .try_borrow_mut()
            .unwrap_or_else(|_| panic!("retained child invariant violated: collection is already borrowed during measurement"));
        let widget = self.widget.try_borrow().unwrap_or_else(|_| typed_container_borrow_conflict());
        let frame_insets = if widget.widget.effective_widget_opt().intersects(WidgetOption::FRAME) {
            super::frame::normal_frame_visual(style, widget.widget.frame_appearance_role())
                .patch
                .insets
                .normalized()
        } else {
            SliceInsets::ZERO
        };
        let mut ctx = MeasureCtx::new(style, atlas, &mut children);
        let measured = ContainerWidget::measure(&widget.widget, &mut ctx, super::frame::content_constraints(constraints, frame_insets));
        (frame_insets, measured)
    }

    /// Returns the concrete widget's effective options.
    pub(crate) fn effective_widget_opt(&self) -> WidgetOption {
        self.widget
            .try_borrow()
            .unwrap_or_else(|_| typed_container_borrow_conflict())
            .widget
            .effective_widget_opt()
    }

    /// Runs one common widget query against the concrete container object.
    pub(crate) fn with_widget<R>(&self, f: impl FnOnce(&dyn Widget) -> R) -> R {
        let widget = self.widget.try_borrow().unwrap_or_else(|_| typed_container_borrow_conflict());
        f(&widget.widget)
    }

    /// Runs one common mutable widget phase against the concrete container object.
    pub(crate) fn with_widget_mut<R>(&mut self, f: impl FnOnce(&mut dyn Widget) -> R) -> R {
        let mut widget = self.widget.try_borrow_mut().unwrap_or_else(|_| typed_container_borrow_conflict());
        f(&mut widget.widget)
    }

    /// Combines semantic widget edits and independently issued topology edits.
    pub(crate) fn is_measurement_dirty(&self) -> bool {
        self.children.borrow().measurement_dirty
            || self
                .widget
                .try_borrow()
                .unwrap_or_else(|_| typed_container_borrow_conflict())
                .is_measurement_dirty()
    }

    /// Consumes both sources, even when the first already requires measurement.
    pub(crate) fn take_measurement_dirty(&mut self) -> bool {
        let topology_dirty = std::mem::take(&mut self.children.borrow_mut().measurement_dirty);
        topology_dirty
            | self
                .widget
                .try_borrow_mut()
                .unwrap_or_else(|_| typed_container_borrow_conflict())
                .take_measurement_dirty()
    }

    /// Invalidates geometry following a widget-reported change.
    pub(crate) fn mark_measurement_dirty(&mut self) {
        self.widget
            .try_borrow_mut()
            .unwrap_or_else(|_| typed_container_borrow_conflict())
            .mark_measurement_dirty();
    }
}

/// Reports application access that overlaps a typed container runtime phase.
#[cold]
#[inline(never)]
fn typed_container_borrow_conflict() -> ! {
    panic!("retained widget invariant violated: a typed access closure must finish before runtime traversal")
}

/// Framework-scoped geometry services available to one active [`ContainerWidget::place`] call.
///
/// The context measures or places indexed children and commits derived viewport/participation
/// results. It never lends a node or permits topology mutation.
pub struct ContainerLayoutCtx<'a> {
    /// Complete immutable skin for this invocation's resource revision.
    skin: &'a Skin,
    /// Atlas paired with the skin for concrete algorithms.
    atlas: &'a crate::AtlasHandle,
    /// Parent-owned content allocation limits descendant clipping.
    content: Recti,
    /// Runtime geometry is either locally borrowed or reached through the scoped COM service.
    target: PlacementTarget<'a>,
}

/// Private placement transition; neither variant crosses a COM boundary.
enum PlacementTarget<'a> {
    /// Local baseline retains its existing direct traversal until final migration cleanup.
    Local {
        /// Geometry-only recursion contains no input/runtime ownership.
        runtime: &'a mut LayoutTraversal,
        /// Current retained node receives derived output after each local algorithm operation.
        current: &'a mut NodeRuntime,
        /// Indexed authoritative children for the local baseline.
        children: &'a mut Children,
    },
    /// COM implementations call the public service and return one copied complete parent result.
    Scoped {
        /// Expiring child geometry authority with first-error propagation.
        layout: ScopedLayout<'a>,
        /// No runtime node is borrowed or stored; geometry is plain copied output.
        geometry: ContainerGeometry,
    },
}

impl<'a> ContainerLayoutCtx<'a> {
    /// Creates the existing local placement context without exposing the child collection to policy.
    pub(crate) fn new(
        runtime: &'a mut LayoutTraversal,
        skin: &'a Skin,
        atlas: &'a crate::AtlasHandle,
        content: Recti,
        current: &'a mut NodeRuntime,
        children: &'a mut Children,
    ) -> Self {
        Self {
            skin,
            atlas,
            content,
            target: PlacementTarget::Local { runtime, current, children },
        }
    }

    /// Creates a synchronous Rust adapter over the runtime-owned COM frame.
    pub(crate) fn scoped(skin: &'a Skin, atlas: &'a crate::AtlasHandle, content: Recti, layout: ScopedLayout<'a>) -> Self {
        Self {
            skin,
            atlas,
            content,
            target: PlacementTarget::Scoped {
                layout,
                geometry: ContainerGeometry::new(microui_contracts::Rect {
                    x: content.x,
                    y: content.y,
                    width: content.width,
                    height: content.height,
                }),
            },
        }
    }

    /// Returns complete output only after all child calls succeeded.
    pub(crate) fn result(&self) -> Result<ContainerGeometry, NcStatus> {
        match &self.target {
            PlacementTarget::Scoped { layout, geometry } => {
                layout.result()?;
                Ok(*geometry)
            }
            PlacementTarget::Local { .. } => unreachable!("local baseline publishes directly into retained node geometry"),
        }
    }

    /// Returns the active UI skin.
    pub fn skin(&self) -> &Skin {
        self.skin
    }

    /// Returns the immutable atlas paired with this skin.
    pub fn atlas(&self) -> &crate::AtlasHandle {
        self.atlas
    }

    /// Returns the direct child count validated before entering a scoped algorithm.
    pub fn child_count(&self) -> usize {
        match &self.target {
            PlacementTarget::Local { children, .. } => children.len(),
            PlacementTarget::Scoped { layout, .. } => layout.count,
        }
    }

    /// Measures one indexed child with exact finite/unbounded constraints.
    pub fn measure_child(&mut self, index: usize, constraints: Constraints) -> Option<Dimensioni> {
        match &mut self.target {
            PlacementTarget::Local { runtime, children, .. } => Some(runtime.measure_node(children.get_mut(index)?, self.skin, self.atlas, constraints)),
            PlacementTarget::Scoped { layout, .. } => layout.measure(index, constraints),
        }
    }

    /// Assigns one exact parent-owned rectangle; the runtime does not reinterpret sizing policy.
    pub fn layout_child(&mut self, index: usize, rect: Recti) -> Option<Dimensioni> {
        match &mut self.target {
            PlacementTarget::Local { runtime, children, .. } => Some(runtime.layout_node_ref(children.get_mut(index)?, self.skin, self.atlas, rect)),
            PlacementTarget::Scoped { layout, .. } => {
                let index = layout.index(index)?;
                let rect = microui_contracts::Rect {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                };
                layout
                    .capture(layout.service.place_child(layout.scope, index, rect))
                    .map(|size| Dimensioni::new(size.width, size.height))
            }
        }
    }

    /// Reads one child's copied logical extent without exposing any layout cache.
    pub fn child_content_size(&self, index: usize) -> Option<Dimensioni> {
        match &self.target {
            PlacementTarget::Local { children, .. } => children.get(index).map(|node| node.state.layout.content_size),
            PlacementTarget::Scoped { layout, .. } => {
                let index = layout.index(index)?;
                layout
                    .capture(layout.service.child_content_size(layout.scope, index))
                    .map(|size| Dimensioni::new(size.width, size.height))
            }
        }
    }

    /// Commits responsive parent-controlled participation for the complete indexed subtree.
    pub fn set_child_participation(&mut self, index: usize, participation: ChildParticipation) -> bool {
        match &mut self.target {
            PlacementTarget::Local { children, .. } => {
                let Some(node) = children.get_mut(index) else {
                    return false;
                };
                node.state.participation = participation;
                true
            }
            PlacementTarget::Scoped { layout, .. } => {
                let Some(index) = layout.index(index) else {
                    return false;
                };
                let participation = match participation {
                    ChildParticipation::Active => LayoutParticipation::ACTIVE,
                    ChildParticipation::Disabled => LayoutParticipation::DISABLED,
                    ChildParticipation::Hidden => LayoutParticipation::HIDDEN,
                };
                layout.capture(layout.service.set_participation(layout.scope, index, participation)).is_some()
            }
        }
    }

    /// Invalidates a shared placement dependency even when the assigned rectangle is unchanged.
    pub fn child_placement_changed(&mut self, index: usize) -> bool {
        match &mut self.target {
            PlacementTarget::Local { children, .. } => {
                let Some(node) = children.get_mut(index) else {
                    return false;
                };
                node.state.invalidate_layout();
                true
            }
            PlacementTarget::Scoped { layout, .. } => {
                let Some(index) = layout.index(index) else {
                    return false;
                };
                layout.capture(layout.service.child_placement_changed(layout.scope, index)).is_some()
            }
        }
    }

    /// Replaces this container's derived logical extent while preserving viewport policy.
    pub fn set_content_size(&mut self, size: Dimensioni) {
        match &mut self.target {
            PlacementTarget::Local { current, .. } => current.set_layout(current.layout.with_content_size(size)),
            PlacementTarget::Scoped { geometry, .. } => geometry.content = microui_contracts::Size { width: size.width, height: size.height },
        }
    }

    /// Narrows descendant clipping to the content allocation and installs a copied translation.
    pub fn set_children_viewport(&mut self, viewport: Recti, offset: Vec2i) {
        let viewport = viewport
            .positive_intersection(self.content)
            .unwrap_or_else(|| Recti::new(self.content.x, self.content.y, 0, 0));
        match &mut self.target {
            PlacementTarget::Local { current, .. } => {
                let mut layout = NodeLayout::from_parts(current.layout.allocation, viewport, current.layout.content_size);
                layout.children.offset = offset;
                current.set_layout(layout);
            }
            PlacementTarget::Scoped { geometry, .. } => {
                geometry.viewport = microui_contracts::Rect {
                    x: viewport.x,
                    y: viewport.y,
                    width: viewport.width,
                    height: viewport.height,
                };
                geometry.offset_x = offset.x;
                geometry.offset_y = offset.y;
            }
        }
    }

    /// Controls whether visible descendant overflow contributes to the parent-visible extent.
    pub fn set_child_overflow_propagation(&mut self, propagate: bool) {
        match &mut self.target {
            PlacementTarget::Local { current, .. } => current.set_layout(current.layout.with_child_overflow_propagation(propagate)),
            PlacementTarget::Scoped { geometry, .. } => geometry.propagate_overflow = propagate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal layout implementation proving that concrete ownership needs geometry only.
    struct GeometryOnly;

    impl Widget for GeometryOnly {
        fn widget_opt(&self) -> &WidgetOption {
            &WidgetOption::NO_INTERACT
        }

        fn paint(&mut self, _ctx: &mut crate::WidgetPaintCtx<'_>) {}
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
        let handle = crate::ui_node::children::ChildrenHandle::new(&children);
        let (_, owner) = Container::from_shared(children, GeometryOnly);

        assert_eq!(owner.with_children(Children::len), 1);
        owner.with_children_mut(|children| children.push(text_node("second")));
        assert_eq!(handle.len(), Some(2));
        drop(owner);
        assert_eq!(handle.len(), None);
    }
}
