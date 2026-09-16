//! Independently deployable built-in node implementations and standard UI compositions.
//!
//! The core UI plugin owns retained traversal, windows, command menus, and choice-menu popup
//! policy. This plugin owns `UiFactory`, every concrete built-in node class, and the standard file
//! picker composition. Binding crosses only generated COM contracts; it never receives core
//! runtime storage, root identifiers, or implementation callbacks.

#![deny(missing_docs)]

use neocogi_com::{ComPtr, ComRef, NcGuid, NcStatus, guid_from_qualified_name};
use neocogi_ui_core_contracts::UiRuntimeAbi;
use neocogi_ui_nodes_contracts::{NodesPluginFactory, UiFactoryAbi};

/// Hard provider dependency retained by the assembly planner before node-provider activation.
const UI_RUNTIME_PLUGIN_ID: NcGuid = guid_from_qualified_name("neocogi.ui.runtime");

#[neocogi_com::com_factory(neocogi_ui_nodes_contracts::NodesPluginFactory)]
/// Stateless activation root that binds concrete nodes to one independently created core runtime.
struct NodesPluginFactoryObject;

impl NodesPluginFactory for NodesPluginFactoryObject {
    /// Captures only public host, choice-menu, resource, and window capabilities from the runtime.
    fn bind(&self, runtime: ComRef<'_, UiRuntimeAbi>) -> Result<ComPtr<UiFactoryAbi>, NcStatus> {
        neocogi_ui_implementation::UiFactory::bind(runtime)
    }
}

neocogi_com::factory_plugin! {
    name = "neocogi.ui.nodes.runtime",
    version = "0.9.0-alpha.0",
    dependencies = [UI_RUNTIME_PLUGIN_ID],
    contracts = [neocogi_ui_nodes_contracts::MODULE],
    factory = NodesPluginFactoryObject,
}

// Static consumers call `factory_plugin_description` directly. Only a final dynamic artifact
// exposes the loader entrypoint, avoiding duplicate exports in monolithic applications.
#[cfg(feature = "dynamic-plugin")]
neocogi_com::export_factory_plugin!();
