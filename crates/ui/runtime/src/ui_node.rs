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

//! Retained UI contracts, built-in components, owning node model, and traversal runtime.
//!
//! One node representation owns either a widget or a container; no parallel legacy tree or
//! generated public identity path remains.
//!
//! Topology is assembled from unique owning nodes and concrete child-owning containers. Update and
//! paint visit a node before its eligible children in forward sibling order; pointer input visits
//! eligible children first in reverse sibling order so the deepest, topmost node wins. Measurement
//! and layout recurse only through a container's scoped child APIs.
//!
//! Each node retains an allocation in its parent's child coordinates plus a node-local child
//! offset and clip. Recursive passes carry one stack-only [`Transform`]. Resolved outer rectangles
//! and outer clips remain runtime stack locals; phase contexts expose node-local content geometry.
//! Each production node owns one actual IWidget plus its cached leaf or container
//! layout capability. A branch retains one authoritative COM ChildStore. Scoped
//! layout services take owned child transactions; no widget retains borrowed tree state.
//! Headless tests mount these same COM capabilities through the public factory.
//!
//! # Source layout
//!
//! Singular modules such as `widget` and `container` own extension contracts; plural modules such
//! as `widgets` and `containers` are built-in catalogs. Named module files are used at every depth,
//! and each public built-in component family has one correspondingly named file. The `layout`
//! module owns reusable constraints and algorithms rather than pretending those helpers are
//! concrete containers; runtime recursion is named `layout_traversal` at its narrower ownership
//! boundary. Tests follow their owning subject without forcing production code into a `mod.rs`
//! layout. A built-in's `com_object` declaration, callable-class implementation, shared widget
//! projections, semantic state, and native algorithm all live in that same family file; the
//! shared object projections and retained services live beside the node model that owns them.
pub(crate) mod child_store;
pub(crate) mod frame;
mod input;
pub(crate) mod input_contract;
pub(crate) mod layout_service;
pub(crate) mod mounted_node;
pub(crate) mod node_factory;
pub(crate) mod paint_service;
pub(crate) mod widget_host;
pub(crate) mod widget_object;
pub use input::UiInputEvent;
pub mod layout;
pub use layout::{AvailableSpace, Constraints, TrackSize};
pub(crate) mod text_layout;
pub use text_layout::TextWrap;
mod widget;
mod widget_context;
pub use widget::{Change, KeyboardAction, KeyboardBehavior, LeafWidget, Widget, WidgetFillOption, WidgetOption, WidgetPaintCtx, WidgetUpdateCtx};
pub mod widgets;
pub use widgets::{ScrollbarAxis, ScrollbarParameters};

mod children;
pub use children::Children;
mod node_layout;
pub use node_layout::ChildParticipation;
pub(crate) use node_layout::{NodeLayout, RuntimeNodeId, Transform};
mod node;
pub use node::Node;
pub(crate) use node::NodeRuntime;
pub(crate) use mounted_node::{ComNode, frame_contract};
pub(crate) use widget_host::{HostState, Observation, WidgetHost};
mod runtime;
pub(crate) use runtime::{UiRuntime, LayoutTraversal};
mod container;
pub use container::{ContainerLayoutCtx, ContainerWidget, MeasureCtx};
mod containers;
pub(crate) use containers::{ScrollComposition, ScrollModel, ScrollParts};
pub use containers::{Disclosure, Grid, Linear, ScrollArea};
