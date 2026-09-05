//! Example-specific editing capabilities. These are separate from the toolkit's standard widgets.
use microui_contracts::*;
use nc_com::{ComPtr, ComRef, NcGuid, NcStatus, com_interface, com_struct};

/// Optional test/example assembly; ordinary demo deployments do not include it.
pub const EXTENSION_PROVIDER_CLASS: NcGuid = nc_com::guid_from_qualified_name("microui.example.ExtensionProvider");

#[com_struct]
#[derive(Debug, Default, Eq, PartialEq)]
/// Observable phase attempts on one example widget, without exposing its native state.
pub struct WidgetCalls {
    /// Preferred-size queries that reached the widget's algorithm.
    pub measures: u64,
    /// Placement queries that reached the container's algorithm.
    pub placements: u64,
    /// Routed or eventless update attempts.
    pub updates: u64,
    /// Paint attempts, including a failed outward painter call.
    pub paints: u64,
    /// Committed logical closures; final COM release is a separate lifetime event.
    pub closes: u64,
}

#[com_interface(auto_guid = "microui.example.WidgetProbe:v1")]
/// Example diagnostics remain readable after logical closure for lifetime assertions.
pub trait IWidgetProbe {
    /// Returns counts from the actual allocation, with no callback or copied semantic model.
    fn calls(&self) -> Result<WidgetCalls, NcStatus>;
}

#[com_interface(auto_guid = "microui.example.Counter:v1")]
/// A concrete custom control inherits all common widget calls on the same COM object.
pub trait ICounter: IWidget {
    /// Reads the actual current activation count.
    fn count(&self) -> Result<u32, NcStatus>;
    /// Silently replaces the displayed count and invalidates its preferred text size when changed.
    fn set_count(&self, value: u32) -> Result<(), NcStatus>;
}

#[com_interface(auto_guid = "microui.example.Inset:v1")]
/// A one-child container demonstrates a domain-specific layout policy over the shared child editor.
pub trait IInset: IContainer {
    /// Installs or replaces its one child; removed-child teardown uses the ordinary committed outcome.
    fn set_child(&self, child: ComRef<'_, IWidgetAbi>) -> Result<ChildEditResult, NcStatus>;
    /// Sets a nonnegative inset, bounded to avoid arithmetic overflow in doubled extents.
    fn set_padding(&self, padding: i32) -> Result<(), NcStatus>;
}

#[com_interface(auto_guid = "microui.example.ExtensionProvider:v1")]
/// Stateless factory receives the runtime authority explicitly; it never links the UI implementation.
pub trait IExtensionProvider {
    /// Creates an unmounted concrete counter with a standard activation-event capability.
    fn counter(&self, host: ComRef<'_, IWidgetHostAbi>) -> Result<ComPtr<ICounterAbi>, NcStatus>;
    /// Creates an empty custom container; child attachment follows the same rules as built-ins.
    fn inset(&self, host: ComRef<'_, IWidgetHostAbi>) -> Result<ComPtr<IInsetAbi>, NcStatus>;
}
