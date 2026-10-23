//! Independently deployable built-in node implementations and standard UI compositions.
//!
//! The core UI plugin owns retained traversal, windows, command menus, and choice-menu popup
//! policy. This plugin owns `UiFactory`, every concrete built-in node class, and the standard file
//! picker composition. Binding crosses only generated COM contracts; it never receives core
//! runtime storage, root identifiers, or implementation callbacks.

#![deny(missing_docs)]
// `clippy::pedantic` is useful as an occasional review tool for this crate, but these categories
// are intentionally outside the local lint profile. The UI/rendering path performs many bounded
// pixel/UV casts, internal modules use crate preludes heavily, and the retained public API
// should not grow `#[must_use]` or pedantic doc-section noise mechanically.
#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::similar_names,
    clippy::struct_field_names,
    clippy::too_many_arguments,
    clippy::wildcard_imports
)]

// Compile the nodes-owned authoring guide without publishing a second API module.
#[cfg(doctest)]
mod guide_doctests {
    #[doc = include_str!("../docs/WIDGETS.md")]
    mod widgets {}
}

mod container;
/// Concrete COM containers and their own semantic layout policies.
pub mod containers;
mod dialogs;
mod factory;
mod layout_client;
#[cfg(test)]
mod test_support;
mod widget;
mod widget_object;
/// Concrete COM leaf controls and their semantic algorithms.
pub mod widgets;

pub use factory::UiFactory;
pub use containers::{Disclosure, Form, Grid, Linear, ScrollArea, SplitView};
pub use widgets::{Button, Checkbox, ColorSwatch, Combo, ListBox, ListItem, Number, Scrollbar, Slider, TextArea, TextBlock, Textbox};
#[allow(unused_imports)]
pub(crate) use neocogi_ui_support::prelude::*;
pub(crate) use neocogi_ui_support::{assets, boundary, frame, input_contract, layout, math, render, text_layout};
pub(crate) use neocogi_ui_support::{
    AvailableSpace, Change, Constraints, KeyboardAction, KeyboardBehavior, TextWrap, TrackSize, UiInputEvent, WidgetOption, WidgetPaintCtx, WidgetUpdateCtx,
};
pub(crate) use neocogi_ui_core_contracts::LayoutParticipation as ChildParticipation;
pub(crate) use container::{ContainerLayoutCtx, ContainerWidget, MeasureCtx};
pub(crate) use containers::{ScrollComposition, ScrollModel, ScrollParts};
pub(crate) use frame::frame_contract;
pub(crate) use widgets::{
    ButtonParameters, CheckboxParameters, ColorSwatchParameters, ComboParameters, DecimalPrecision, ListBoxParameters, ListItemParameters, NumberParameters,
    SliderParameters, Real, ScrollbarAxis, ScrollbarParameters, TextBlockParameters, TextboxParameters,
};
#[cfg(test)]
pub(crate) use neocogi_ui_support::widget_context;

/// Binds this package's concrete built-in nodes to a core runtime for static tests and monoliths.
///
/// Dynamic deployments acquire the factory of `neocogi.ui.nodes.runtime` and call its generated `NodesPluginFactory`
/// instead. This helper performs the same typed binding without a loader and does not make the
/// core runtime object expose or construct `UiFactory`.
pub fn bind_builtin_nodes(runtime: neocogi_com::ComPtr<neocogi_ui_core_contracts::UiRuntimeAbi>) -> Result<neocogi_ui::Ui, neocogi_com::NcStatus> {
    let factory = UiFactory::bind(runtime.as_com_ref())?;
    neocogi_ui::Ui::from_interfaces(runtime, factory)
}

use neocogi_com::{ComPtr, ComRef, NcGuid, NcStatus, guid_from_qualified_name};
use neocogi_ui_core_contracts::UiRuntimeAbi;
use neocogi_ui_nodes_contracts::{NodesPluginFactory, UiFactoryAbi};

/// Hard provider dependency retained by the assembly planner before node-provider activation.
const UI_RUNTIME_PLUGIN_ID: NcGuid = guid_from_qualified_name("neocogi.ui.runtime");

#[neocogi_com::com_object(neocogi_ui_nodes_contracts::NodesPluginFactory)]
/// Stateless activation root that binds concrete nodes to one independently created core runtime.
struct NodesPluginFactoryObject;

impl NodesPluginFactory for NodesPluginFactoryObject {
    /// Captures only public host, choice-menu, resource, and window capabilities from the runtime.
    fn bind(&self, runtime: ComRef<'_, UiRuntimeAbi>) -> Result<ComPtr<UiFactoryAbi>, NcStatus> {
        UiFactory::bind(runtime)
    }
}

/// Acquires one owned root; object construction remains on its typed methods.
#[neocogi_com::plugin(name = "neocogi.ui.nodes.runtime",
    dependencies = [UI_RUNTIME_PLUGIN_ID],
    )]
fn acquire_root(_runtime: ::neocogi_com::Runtime) -> Result<neocogi_com::ComPtr<neocogi_ui_nodes_contracts::NodesPluginFactoryAbi>, neocogi_com::NcStatus> {
    NodesPluginFactoryObject::from_value(NodesPluginFactoryObject).query()
}

// Static consumers call `plugin_descriptor` directly. Only a final dynamic artifact
// exposes the loader entrypoint, avoiding duplicate exports in monolithic applications.
pub(crate) use widget::{Widget, LeafWidget, WidgetFillOption};

// This unit-test executable owns one runtime; normal library builds never embed it.
#[cfg(test)]
extern crate nc_com as _;
