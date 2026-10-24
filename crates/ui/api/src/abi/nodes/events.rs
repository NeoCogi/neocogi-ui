//! Immutable COM event construction. Every result is the sole owner of its captured snapshot.
use crate::abi::nodes::*;
use neocogi_com::{ComBox, NcStatus, NC_E_INVALID_ARG};
use neocogi_ui::abi::core::{IEvent, IEventAbi};

#[neocogi_com::com_object(interfaces(crate::abi::nodes::ICheckboxChangedEvent), ownership = exclusive)]
/// Immutable captured fact; this allocation retains no source widget or service.
struct CheckboxChanged {
    /// Checked state captured independently of subsequent producer edits.
    checked: bool,
}
impl IEvent for CheckboxChanged {}
impl ICheckboxChangedEvent for CheckboxChanged {
    /// Reads the captured checked state through the public event ABI.
    fn checked(&self) -> Result<bool, NcStatus> {
        Ok(self.checked)
    }
}
/// Captures the checked state in an exclusive snapshot with no source ownership.
pub fn checkbox_changed(checked: bool) -> Result<ComBox<IEventAbi>, NcStatus> {
    CheckboxChanged::into_box(CheckboxChanged { checked })
        .query()
        .map_err(|(status, _unpublished)| status)
}

#[neocogi_com::com_object(interfaces(crate::abi::nodes::INumericChangedEvent), ownership = exclusive)]
/// Immutable captured fact; this allocation retains no source widget or service.
struct NumericChanged {
    /// Captured value, independent of subsequent producer edits.
    value: f32,
}
impl IEvent for NumericChanged {}
impl INumericChangedEvent for NumericChanged {
    /// Reads the immutable value snapshot through the public event ABI.
    fn value(&self) -> Result<f32, NcStatus> {
        Ok(self.value)
    }
}
/// Captures a finite numeric value; NaN and infinity return InvalidArg.
pub fn numeric_changed(value: f32) -> Result<ComBox<IEventAbi>, NcStatus> {
    if !value.is_finite() {
        return Err(NC_E_INVALID_ARG);
    }
    NumericChanged::into_box(NumericChanged { value })
        .query()
        .map_err(|(status, _unpublished)| status)
}

#[neocogi_com::com_object(interfaces(crate::abi::nodes::IScrollbarChangedEvent), ownership = exclusive)]
/// Immutable captured fact; this allocation retains no source widget or service.
struct ScrollbarChanged {
    /// Captured offset, independent of subsequent producer edits.
    offset: i32,
}
impl IEvent for ScrollbarChanged {}
impl IScrollbarChangedEvent for ScrollbarChanged {
    /// Reads the immutable offset snapshot through the public event ABI.
    fn offset(&self) -> Result<i32, NcStatus> {
        Ok(self.offset)
    }
}
/// Captures a nonnegative scrollbar offset; a negative value returns InvalidArg.
pub fn scrollbar_changed(offset: i32) -> Result<ComBox<IEventAbi>, NcStatus> {
    if offset < 0 {
        return Err(NC_E_INVALID_ARG);
    }
    ScrollbarChanged::into_box(ScrollbarChanged { offset })
        .query()
        .map_err(|(status, _unpublished)| status)
}

#[neocogi_com::com_object(interfaces(crate::abi::nodes::ISplitChangedEvent), ownership = exclusive)]
/// Immutable captured fact; this allocation retains no source widget or service.
struct SplitChanged {
    /// Captured position, independent of subsequent producer edits.
    position: i32,
}
impl IEvent for SplitChanged {}
impl ISplitChangedEvent for SplitChanged {
    /// Reads the immutable position snapshot through the public event ABI.
    fn position(&self) -> Result<i32, NcStatus> {
        Ok(self.position)
    }
}
/// Captures a nonnegative divider position; a negative value returns InvalidArg.
pub fn split_changed(position: i32) -> Result<ComBox<IEventAbi>, NcStatus> {
    if position < 0 {
        return Err(NC_E_INVALID_ARG);
    }
    SplitChanged::into_box(SplitChanged { position })
        .query()
        .map_err(|(status, _unpublished)| status)
}
