//! Statically linked state shared by concrete widget implementations.
//!
//! This helper lives inside the widget's COM allocation. Its Rust layout never
//! crosses an interface boundary and it owns no children, runtime or app sink.

use core::cell::Cell;
use nc_com::{ComPtr, ComRef, NcStatus, NC_E_INVALID_ARG, com_int_enum, com_struct};
use crate::{GeometryChange, IWidgetHostAbi, UI_E_CLOSED, UI_E_OWNER};

/// Common logical lifetime and sticky geometry state for one concrete widget.
///
/// Compose this value into each widget class. The host capability must contain
/// only authority state, without a strong reference back to the owning tree.
/// Setters call `writable` before mutation and `record_change` after committing
/// changed measurement or placement inputs. Hidden widgets retain that marker.
pub struct WidgetState {
    /// Runtime authority is shared across widgets without an ownership cycle.
    host: ComPtr<IWidgetHostAbi>,
    /// Mutable common presentation/routing options shared by all concrete controls.
    options: Cell<WidgetOptions>,
    /// Geometry changes persist until the runtime collects them before layout.
    dirty: Cell<bool>,
    /// Attachment is a one-way transition for the lifetime of this allocation.
    mounted: Cell<bool>,
    /// Logical closure invalidates retained handles before final COM release.
    closed: Cell<bool>,
}

impl WidgetState {
    /// Creates unmounted state; the first node measurement starts uncached.
    pub fn new(host: ComPtr<IWidgetHostAbi>, options: WidgetOptions) -> Self {
        Self {
            host,
            options: Cell::new(options),
            dirty: Cell::new(false),
            mounted: Cell::new(false),
            closed: Cell::new(false),
        }
    }

    /// Reads the current copied options without exposing a reference into state.
    pub fn options(&self) -> Result<WidgetOptions, NcStatus> {
        self.readable()?;
        Ok(self.options.get())
    }

    /// Validates an option replacement before mutation and retains its geometry marker.
    pub fn set_options(&self, options: WidgetOptions) -> Result<(), NcStatus> {
        self.writable()?;
        options.validate()?;
        if self.options.replace(options) != options {
            self.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }

    /// Borrows the immutable owner capability for attachment authentication.
    pub fn host(&self) -> ComRef<'_, IWidgetHostAbi> {
        self.host.as_com_ref()
    }

    /// Allows observations during layout/paint while rejecting logical closure.
    pub fn readable(&self) -> Result<(), NcStatus> {
        if self.closed.get() {
            return Err(UI_E_CLOSED);
        }
        self.host.check_open()
    }

    /// Authorizes an ordinary mutation before the implementation borrows state.
    pub fn writable(&self) -> Result<(), NcStatus> {
        if self.closed.get() {
            return Err(UI_E_CLOSED);
        }
        self.host.check_mutation()
    }

    /// Records an already authorized, committed geometry change without a call out.
    ///
    /// `NO_CHANGE` preserves an earlier setter's pending marker. Event-only or
    /// visual-only changes need no measurement invalidation. External state must
    /// be synchronized through an authorized setter or a truthful update result.
    pub fn record_change(&self, change: GeometryChange) {
        if change == GeometryChange::CHANGED {
            self.dirty.set(true);
        }
    }

    /// Drains the marker once, including when update was skipped for this widget.
    pub fn take_change(&self) -> Result<GeometryChange, NcStatus> {
        self.readable()?;
        Ok(if self.dirty.replace(false) {
            GeometryChange::CHANGED
        } else {
            GeometryChange::NO_CHANGE
        })
    }

    /// Commits attachment after all owner, capability and topology checks pass.
    pub fn mount(&self) -> Result<(), NcStatus> {
        self.writable()?;
        if self.mounted.replace(true) { Err(UI_E_OWNER) } else { Ok(()) }
    }

    /// Closes this widget once and reports whether ordinary event queues need clearing.
    ///
    /// Runtime teardown may already have closed the host. Otherwise observational
    /// phases still reject closure, just as they reject semantic setters. The
    /// implementation clears its typed queues after this method returns `true`.
    pub fn close(&self) -> Result<bool, NcStatus> {
        if self.closed.get() {
            return Ok(false);
        }
        match self.host.check_mutation() {
            Ok(()) | Err(UI_E_CLOSED) => {}
            Err(error) => return Err(error),
        }
        self.closed.set(true);
        self.dirty.set(false);
        Ok(true)
    }
}

#[com_struct]
#[derive(Debug, Default, Eq, PartialEq)]
/// Common widget option bits; parent-child sizing rules belong to container edges.
pub struct WidgetOptions {
    /// Union of the documented option constants; unknown bits are rejected.
    pub bits: u32,
}

impl WidgetOptions {
    /// No optional frame or routing override.
    pub const NONE: Self = Self { bits: 0 };
    /// Centers content within its allocation.
    pub const ALIGN_CENTER: Self = Self { bits: 1 };
    /// Aligns content with the right edge.
    pub const ALIGN_RIGHT: Self = Self { bits: 2 };
    /// Makes only this surface inert; enabled descendants may still receive input.
    pub const NO_INTERACT: Self = Self { bits: 4 };
    /// Consumes hovered wheel input.
    pub const GRAB_SCROLL: Self = Self { bits: 32 };
    /// Requests the theme-owned outer frame for the widget's semantic role.
    pub const FRAME: Self = Self { bits: 512 };

    /// Rejects undefined flags before callers commit a replacement.
    pub fn validate(self) -> Result<(), NcStatus> {
        if self.bits & !(1 | 2 | 4 | 32 | 512) == 0 {
            Ok(())
        } else {
            Err(NC_E_INVALID_ARG)
        }
    }
}

impl core::ops::BitOr for WidgetOptions {
    type Output = Self;
    /// Combines independent flags without hiding a stored closure or policy object.
    fn bitor(self, other: Self) -> Self {
        Self { bits: self.bits | other.bits }
    }
}

com_int_enum! {
    /// Semantic outer-frame role; concrete theme resources stay in the runtime.
    pub enum WidgetFrame: u8 {
        /// Generic appearance in the runtime theme.
        GENERIC = 0,
        /// Button appearance in the runtime theme.
        BUTTON = 1,
        /// Checkbox appearance in the runtime theme.
        CHECKBOX = 2,
        /// Text input appearance in the runtime theme.
        TEXT_INPUT = 3,
        /// Item appearance in the runtime theme.
        ITEM = 4,
        /// Combo appearance in the runtime theme.
        COMBO = 5,
        /// Slider track appearance in the runtime theme.
        SLIDER_TRACK = 6,
        /// Slider thumb appearance in the runtime theme.
        SLIDER_THUMB = 7,
        /// Scrollbar track appearance in the runtime theme.
        SCROLLBAR_TRACK = 8,
        /// Scrollbar thumb appearance in the runtime theme.
        SCROLLBAR_THUMB = 9,
        /// Close button appearance in the runtime theme.
        CLOSE_BUTTON = 10,
        /// Minimize button appearance in the runtime theme.
        MINIMIZE_BUTTON = 11,
        /// Maximize button appearance in the runtime theme.
        MAXIMIZE_BUTTON = 12,
        /// Restore button appearance in the runtime theme.
        RESTORE_BUTTON = 13,
        /// Resize grip appearance in the runtime theme.
        RESIZE_GRIP = 14,
        /// Window appearance in the runtime theme.
        WINDOW = 15,
        /// Panel appearance in the runtime theme.
        PANEL = 16,
    }
}

#[com_struct]
#[derive(Debug, Eq, PartialEq)]
/// Current common widget policy, cached only within a synchronized runtime transaction.
pub struct WidgetProperties {
    /// Current frame and pointer-routing options.
    pub options: WidgetOptions,
    /// Keyboard capability bits matching the runtime's shared mapping: focus=1,
    /// Tab=2, Enter=4, Space=8, horizontal=16, vertical=32, hierarchy=64,
    /// popup=128, committed text=256.
    pub keyboard: u16,
    /// Semantic role used when FRAME is enabled.
    pub frame: WidgetFrame,
}

impl WidgetProperties {
    /// Rejects unsupported routing/appearance values at the component boundary.
    pub fn validate(self) -> Result<(), NcStatus> {
        self.options.validate()?;
        if self.keyboard & !511 != 0 || self.frame.raw() > 16 {
            return Err(NC_E_INVALID_ARG);
        }
        Ok(())
    }
}

#[com_struct]
#[derive(Debug, Default)]
/// Copied, observational paint state; contains no borrowed runtime storage.
pub struct WidgetPaint {
    /// Content allocation in local coordinates, starting at zero.
    pub rect: crate::Rect,
    /// Effective content-local clip.
    pub clip: crate::Rect,
    /// Whether the containing surface permits interaction.
    pub enabled: bool,
    /// Pointer hover state for theme selection.
    pub hovered: bool,
    /// Keyboard focus state for theme selection.
    pub focused: bool,
    /// Current click transition for theme selection.
    pub clicked: bool,
    /// Active pointer interaction state.
    pub active: bool,
    /// Whether the logical window owns activation.
    pub window_active: bool,
}

impl WidgetPaint {
    /// Rejects malformed local geometry before a component records any operations.
    pub fn validate(self) -> Result<(), NcStatus> {
        if self.rect.x != 0 || self.rect.y != 0 || self.rect.width < 0 || self.rect.height < 0 || self.clip.width < 0 || self.clip.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        Ok(())
    }
}
