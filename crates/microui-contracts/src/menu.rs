//! Compact menu declarations and live item capabilities; rows are values, not synthetic widget trees.
use nc_com::{ComPtr, ComRef, INcStringAbi, NcCopyFromBufferOffset, NcStatus, com_enum, com_interface};

#[com_enum(tag = u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Persistent menu marker role remains present even when its selected boolean is false.
pub enum MenuMark {
    /// No marker role or required marker gutter.
    None,
    /// Checkable item whose boolean determines check-glyph visibility.
    Checked {
        /// Current presentation state; submission does not toggle it automatically.
        checked: bool,
    },
    /// Radio item whose boolean determines selection-marker visibility.
    Radio {
        /// Current presentation state; application code owns radio-group policy.
        selected: bool,
    },
}
#[com_interface(auto_guid = "microui.MenuItem:v1")]
/// Live compact row capability, editable before mounting and while its owning surface exists.
pub trait IMenuItem {
    /// Returns the same label used by measurement, paint and captured submissions.
    fn label(&self) -> Result<ComPtr<INcStringAbi>, NcStatus>;
    /// Replaces complete UTF-8 display text without publishing user activation.
    fn set_label(&self, label: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Reports pointer and keyboard eligibility, independently of marker presentation.
    fn enabled(&self) -> Result<bool, NcStatus>;
    /// Silently changes interaction eligibility; disabled rows remain visible.
    fn set_enabled(&self, enabled: bool) -> Result<(), NcStatus>;
    /// Returns the concrete check/radio role and its current presentation state.
    fn mark(&self) -> Result<MenuMarkAbi, NcStatus>;
    /// Silently changes the complete marker role; radio grouping remains the caller's responsibility.
    fn set_mark(&self, mark: MenuMarkAbi) -> Result<(), NcStatus>;
    /// Returns presentation-only accelerator text, with empty text representing absence.
    fn shortcut_hint(&self) -> Result<ComPtr<INcStringAbi>, NcStatus>;
    /// Changes presentation only; it does not install a keyboard shortcut handler.
    fn set_shortcut_hint(&self, hint: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus>;
}
#[com_interface(auto_guid = "microui.MenuBuilder:v1")]
/// Unique declaration tree consumed once by the owning manager; retained item capabilities remain live afterward.
pub trait IMenuBuilder {
    /// Appends an enabled unmarked row and returns its editing/event capability.
    fn item(&self, label: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<IMenuItemAbi>, NcStatus>;
    /// Appends one noninteractive separator in exact declaration order.
    fn separator(&self) -> Result<(), NcStatus>;
    /// Appends a submenu and returns a builder scoped to that child; only the root is mountable.
    fn submenu(&self, label: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<IMenuBuilderAbi>, NcStatus>;
}
#[com_interface(auto_guid = "microui.Menus:v1")]
/// Menu construction and attachment capability on the same runtime identity as the window manager.
pub trait IMenus {
    /// Creates one root declaration; for a bar its direct entries must all be submenus.
    fn create(&self, label: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<IMenuBuilderAbi>, NcStatus>;
    /// Installs a bar once on a live window; validates owner, declaration shape and prior attachment before transfer.
    fn attach_bar(&self, window: ComRef<'_, crate::IWindowAbi>, menu: ComRef<'_, IMenuBuilderAbi>) -> Result<(), NcStatus>;
    /// Creates an ordinary hidden popup from one root declaration, including all its nested menus.
    fn create_popup(&self, window: ComRef<'_, crate::IWindowAbi>, menu: ComRef<'_, IMenuBuilderAbi>) -> Result<ComPtr<crate::IPopupAbi>, NcStatus>;
}
