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

//! Core-owned retained topology, scoped COM services, routing, and phase traversal.
//!
//! This module has no built-in widget/container catalog. Each mounted node retains public core
//! widget/layout capabilities, and the independently deployed nodes provider implements them.
//! Resource and phase-local presentation helpers come from provider-neutral support; root/node
//! identities, authoritative children, host validation, and recursive traversal remain here.
pub(crate) mod child_store;
pub(crate) use neocogi_ui_support::{frame, layout, text_layout, widget_context};
pub use neocogi_ui_support::UiInputEvent;
pub(crate) mod input_contract;
pub(crate) mod layout_service;
pub(crate) mod mounted_node;
pub(crate) mod node_factory;
pub(crate) mod paint_service;
pub(crate) mod widget_host;
pub use layout::{AvailableSpace, Constraints};
pub use neocogi_ui_support::{Change, KeyboardBehavior, WidgetOption, WidgetPaintCtx, WidgetUpdateCtx};

mod children;
pub use children::Children;
mod node_layout;
pub use node_layout::ChildParticipation;
pub(crate) use node_layout::{NodeLayout, RuntimeNodeId, Transform};
mod node;
pub use node::Node;
pub(crate) use node::NodeRuntime;
pub(crate) use mounted_node::ComNode;
pub(crate) use widget_host::{HostState, Observation, WidgetHost};
mod runtime;
pub(crate) use runtime::{UiRuntime, LayoutTraversal};
/// The engine's one replaceable revision, shared by independently allocated core services.
/// One receiver cannot exclusively own their shared slot, which never exposes runtime storage.
/// Each revision is immutable and locally owned through Rc, so a service can
/// release the cache borrow before invoking a provider getter, AddRef or renderer.
/// The outer Rc shares the replaceable slot; the inner Rc retains a selected value.
pub(crate) type SharedAssets = std::rc::Rc<std::cell::RefCell<std::rc::Rc<crate::assets::ImportedAssets>>>;
