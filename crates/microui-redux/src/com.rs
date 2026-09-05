//! Typed static clients for dynamically or statically linked COM implementations.
//!
//! Clients keep no shadow widget state or topology. Every edit crosses the
//! checked interface and every returned snapshot owns its storage. This module
//! will become the crate's primary facade as the retained implementation moves
//! into the UI plugin; its dependencies already consist only of the contracts
//! and COM object support.

pub use microui_contracts::*;
use nc_com::{ComInterface, ComPtr, ComRef, INcStringComPtrExt, NcCopyFromBufferOffset, NcStatus};

/// Generic static proxy for one concrete widget editing capability.
pub struct Widget<I: ComInterface> {
    /// One checked, owned reference to the actual plugin object.
    interface: ComPtr<I>,
}

impl<I: ComInterface> Clone for Widget<I> {
    /// Clones the COM reference without copying widget state or topology.
    fn clone(&self) -> Self { Self { interface: self.interface.clone() } }
}

impl<I: ComInterface> Widget<I> {
    /// Adopts a reference obtained through checked activation or QueryInterface.
    pub fn from_interface(interface: ComPtr<I>) -> Self { Self { interface } }

    /// Borrows the exact typed editing interface without changing ownership.
    pub fn as_interface(&self) -> ComRef<'_, I> { self.interface.as_com_ref() }

    /// Obtains the common node capability for attachment; callers cache this at mount.
    pub fn node_interface(&self) -> Result<ComPtr<IWidgetAbi>, NcStatus> {
        // Querying preserves object identity and validates the full projected ABI.
        self.interface.query()
    }
}

/// Button proxy whose concrete methods remain statically discoverable.
pub type Button = Widget<IButtonAbi>;

impl Widget<IButtonAbi> {
    /// Replaces the label with a synchronous UTF-8 copy owned by the plugin.
    pub fn set_label(&self, label: &str) -> Result<(), NcStatus> {
        self.interface.set_label(NcCopyFromBufferOffset::new(0, label.as_bytes()))
    }

    /// Copies an immutable plugin snapshot into an ordinary application String.
    pub fn label(&self) -> Result<String, NcStatus> { self.interface.label()?.to_string() }

    /// Connects a concrete typed sink and returns the caller-owned subscription.
    /// Retain this lease in the application event session; the runtime never owns
    /// an application handler. A second live subscriber is rejected by the source.
    pub fn subscribe(&self, sink: ComRef<'_, IButtonSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.interface.query::<IButtonEventsAbi>()?.subscribe(sink)
    }
}

/// Runtime-bound factory client; generic application state is never stored here.
pub struct WidgetFactory {
    /// The plugin's factory on the same identity as its runtime/window manager.
    interface: ComPtr<IUiFactoryAbi>,
}

impl WidgetFactory {
    /// Adopts a typed factory obtained from UI runtime activation.
    pub fn from_interface(interface: ComPtr<IUiFactoryAbi>) -> Self { Self { interface } }

    /// Creates and initializes an unmounted button in one concrete API operation.
    pub fn button(&self, label: &str) -> Result<Button, NcStatus> {
        let button = Button::from_interface(self.interface.create_button()?);
        button.set_label(label)?;
        Ok(button)
    }
}

/// Borrowed painting convenience with the invocation token hidden from each call.
/// The underlying owned service validates expiry even if a direct COM caller
/// retains its interface and token beyond this Rust wrapper's lifetime.
pub struct Painter<'a> {
    /// Borrowed recording capability, never an adapter over stack-owned drawing state.
    interface: ComRef<'a, IPainterAbi>,
    /// Unique invocation authority issued and expired by the UI runtime.
    scope: PaintScope,
}

impl<'a> Painter<'a> {
    /// Wraps the capability and token received by a widget's COM paint method.
    pub fn from_call(interface: ComRef<'a, IPainterAbi>, scope: PaintScope) -> Self {
        Self { interface, scope }
    }

    /// Records a local rectangle within this invocation.
    pub fn fill(&self, rect: Rect, rgba: u32) -> Result<(), NcStatus> {
        self.interface.fill(self.scope, rect, rgba)
    }
}
