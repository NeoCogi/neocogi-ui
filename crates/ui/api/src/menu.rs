//! Static compact menu construction transfers one declaration and retains only actual row capabilities.
use crate::*;
use neocogi_com::{ComPtr, ComRef, INcStringComPtrExt, NcCopyFromBufferOffset, NcStatus};

/// Typed runtime menu service; it owns no menu topology or row state in the application.
pub struct Menus {
    /// Projection on the same runtime identity as the factory and window manager.
    interface: ComPtr<IMenusAbi>,
}
impl Menus {
    /// Creates an append-only declaration root whose label names a standalone popup.
    pub fn create(&self, label: &str) -> Result<MenuBuilder, NcStatus> {
        Ok(MenuBuilder {
            interface: self.interface.create(NcCopyFromBufferOffset::new(0, label.as_bytes()))?,
        })
    }
    /// Installs a bar once; direct root entries must all be submenus, and failed validation leaves the builder usable.
    pub fn attach_bar(&self, window: ComRef<'_, IWindowAbi>, menu: &MenuBuilder) -> Result<(), NcStatus> {
        self.interface.attach_bar(window, menu.interface.as_com_ref())
    }
    /// Creates an ordinary hidden popup and consumes the builder's sole declaration tree.
    pub fn popup(&self, window: ComRef<'_, IWindowAbi>, menu: &MenuBuilder) -> Result<ComPtr<IPopupAbi>, NcStatus> {
        self.popup_with_sink(window, menu, None)
    }
    /// Constructs a menu popup with its final dismissal receiver before consuming the declaration.
    pub fn popup_with_sink(
        &self,
        window: ComRef<'_, IWindowAbi>,
        menu: &MenuBuilder,
        sink: Option<ComRef<'_, neocogi_ui_core_contracts::IPopupSinkAbi>>,
    ) -> Result<ComPtr<IPopupAbi>, NcStatus> {
        let events = neocogi_ui_core_contracts::IPopupEventBinding::from_sink(sink);
        self.interface
            .create_popup(window, menu.interface.as_com_ref(), neocogi_com::NcValueRef::new(&events)?)
    }
    /// Wraps an already checked service capability without caching manager data.
    pub fn from_interface(interface: ComPtr<IMenusAbi>) -> Self {
        Self { interface }
    }
}

/// Root or child append-only declaration client; keep the root alive until its tree is mounted.
#[derive(Clone)]
pub struct MenuBuilder {
    /// Actual UI-owned declaration capability; after transfer all further appends report CLOSED.
    interface: ComPtr<IMenuBuilderAbi>,
}
impl MenuBuilder {
    /// Appends an enabled unmarked item and returns its direct live editing/event capability.
    pub fn item(&self, label: &str) -> Result<MenuItem, NcStatus> {
        self.item_with_sink(label, None)
    }
    /// Constructs an item with its sole receiver before exposing its handle.
    pub fn item_with_sink(&self, label: &str, sink: Option<ComRef<'_, neocogi_ui_core_contracts::IMenuItemSinkAbi>>) -> Result<MenuItem, NcStatus> {
        Ok(MenuItem {
            interface: self.interface.item(
                NcCopyFromBufferOffset::new(0, label.as_bytes()),
                neocogi_com::NcValueRef::new(&neocogi_ui_core_contracts::IMenuItemEventBinding::from_sink(sink))?,
            )?,
        })
    }
    /// Captures activations into the application queue without later installation.
    pub fn item_observed<M: 'static>(
        &self,
        label: &str,
        sender: std::sync::mpsc::Sender<M>,
        map: impl FnMut(neocogi_ui_core_contracts::Activation) -> M + 'static,
    ) -> Result<MenuItem, NcStatus> {
        let sink = crate::events::activation::sink(sender, map)?;
        self.item_with_sink(label, Some(sink.as_com_ref()))
    }
    /// Appends a noninteractive separator in exact declaration order.
    pub fn separator(&self) -> Result<(), NcStatus> {
        self.interface.separator()
    }
    /// Appends a child menu; scoped child builders cannot be attached independently.
    pub fn submenu(&self, label: &str) -> Result<Self, NcStatus> {
        Ok(Self {
            interface: self.interface.submenu(NcCopyFromBufferOffset::new(0, label.as_bytes()))?,
        })
    }
    /// Borrows the concrete construction interface for custom static assembly helpers.
    pub fn as_interface(&self) -> ComRef<'_, IMenuBuilderAbi> {
        self.interface.as_com_ref()
    }
}

/// Direct compact-row client; its COM object resolves the same record before and after mounting.
#[derive(Clone)]
pub struct MenuItem {
    /// Actual item capability; retaining it never keeps its menu surface open.
    interface: ComPtr<IMenuItemAbi>,
}
impl MenuItem {
    /// Borrows the checked row interface without retaining a native record in the application.
    pub fn as_interface(&self) -> ComRef<'_, IMenuItemAbi> {
        self.interface.as_com_ref()
    }
    /// Copies the exact label currently used for measure and paint.
    pub fn label(&self) -> Result<String, NcStatus> {
        self.interface.label()?.to_string()
    }
    /// Changes the current label silently.
    pub fn set_label(&self, label: &str) -> Result<(), NcStatus> {
        self.interface.set_label(NcCopyFromBufferOffset::new(0, label.as_bytes()))
    }
    /// Reads current pointer and keyboard eligibility.
    pub fn enabled(&self) -> Result<bool, NcStatus> {
        self.interface.enabled()
    }
    /// Disables interaction without removing the row from presentation.
    pub fn set_enabled(&self, enabled: bool) -> Result<(), NcStatus> {
        self.interface.set_enabled(enabled)
    }
    /// Reads the complete check/radio role rather than guessing it from a boolean.
    pub fn mark(&self) -> Result<MenuMark, NcStatus> {
        self.interface.mark()?.try_into()
    }
    /// Updates presentation only; application code owns any check/radio-group policy.
    pub fn set_mark(&self, mark: MenuMark) -> Result<(), NcStatus> {
        self.interface.set_mark(mark.into())
    }
    /// Copies the display-only accelerator, with empty text representing absence.
    pub fn shortcut_hint(&self) -> Result<String, NcStatus> {
        self.interface.shortcut_hint()?.to_string()
    }
    /// Changes accelerator display without registering a keyboard command.
    pub fn set_shortcut_hint(&self, hint: &str) -> Result<(), NcStatus> {
        self.interface.set_shortcut_hint(NcCopyFromBufferOffset::new(0, hint.as_bytes()))
    }
    /// Selects a semantic font through the same concrete optional capability as ordinary text widgets.
    pub fn set_font_role(&self, role: FontRole) -> Result<(), NcStatus> {
        self.interface.query::<IFontStyleAbi>()?.set_font_role(role)
    }
    /// Resolves a named font against the owning runtime's resource bundle.
    pub fn set_named_font(&self, name: &str) -> Result<(), NcStatus> {
        self.interface
            .query::<IFontStyleAbi>()?
            .set_named_font(NcCopyFromBufferOffset::new(0, name.as_bytes()))
    }
}
