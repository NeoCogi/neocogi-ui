//! Immutable COM event construction. Every result is the sole owner of its captured snapshot.
use crate::*;
use neocogi_com::{ComBox, ComPtr, NcStatus, NcString, INcStringAbi, NC_E_INVALID_ARG};
use crate::{IEvent, IEventAbi};

#[neocogi_com::com_object(interfaces(crate::IActivatedEvent), ownership = exclusive)]
/// Activation occurrence; the envelope independently retains weak source identity.
struct Activated;
impl IEvent for Activated {}
impl IActivatedEvent for Activated {}
/// Captures activation without copying presentation text or owning its source.
pub fn activated() -> Result<ComBox<IEventAbi>, NcStatus> {
    Activated::into_box(Activated).query().map_err(|(status, _unpublished)| status)
}

#[neocogi_com::com_object(interfaces(crate::ITextChangedEvent), ownership = exclusive)]
/// Immutable captured fact; this allocation retains no source widget or service.
struct TextChanged {
    /// Captured text, independent of subsequent producer edits.
    text: ComPtr<INcStringAbi>,
    /// Captured cursor, independent of subsequent producer edits.
    cursor: u64,
}
impl IEvent for TextChanged {}
impl ITextChangedEvent for TextChanged {
    /// Reads the immutable text snapshot through the public event ABI.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        Ok(self.text.clone())
    }
    /// Reads the immutable cursor snapshot through the public event ABI.
    fn cursor(&self) -> Result<u64, NcStatus> {
        Ok(self.cursor)
    }
}
/// Copies edited text and its byte cursor into an exclusive snapshot.
/// Rejects cursors outside the text or inside a UTF-8 scalar with InvalidArg.
pub fn text_changed(text: &str, cursor: u64) -> Result<ComBox<IEventAbi>, NcStatus> {
    if !text.is_char_boundary(usize::try_from(cursor).map_err(|_| NC_E_INVALID_ARG)?) {
        return Err(NC_E_INVALID_ARG);
    }
    TextChanged::into_box(TextChanged {
        text: NcString::from_utf8_str(text),
        cursor,
    })
    .query()
    .map_err(|(status, _unpublished)| status)
}

#[neocogi_com::com_object(interfaces(crate::ITextSubmittedEvent), ownership = exclusive)]
/// Immutable captured fact; this allocation retains no source widget or service.
struct TextSubmitted {
    /// Captured text, independent of subsequent producer edits.
    text: ComPtr<INcStringAbi>,
}
impl IEvent for TextSubmitted {}
impl ITextSubmittedEvent for TextSubmitted {
    /// Reads the immutable text snapshot through the public event ABI.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        Ok(self.text.clone())
    }
}
/// Copies submitted text into an exclusive snapshot with no source ownership.
pub fn text_submitted(text: &str) -> Result<ComBox<IEventAbi>, NcStatus> {
    TextSubmitted::into_box(TextSubmitted { text: NcString::from_utf8_str(text) })
        .query()
        .map_err(|(status, _unpublished)| status)
}

#[neocogi_com::com_object(interfaces(crate::IWindowGeometryChangedEvent), ownership = exclusive)]
/// Immutable captured fact; this allocation retains no source widget or service.
struct WindowGeometryChanged {
    /// Captured rect, independent of subsequent producer edits.
    rect: Rect,
}
impl IEvent for WindowGeometryChanged {}
impl IWindowEvent for WindowGeometryChanged {}
impl IWindowGeometryChangedEvent for WindowGeometryChanged {
    /// Reads the immutable rect snapshot through the public event ABI.
    fn rect(&self) -> Result<Rect, NcStatus> {
        Ok(self.rect)
    }
}
/// Captures a window geometry changed fact independently of the live window.
/// Negative width or height returns InvalidArg; coordinates may be negative.
pub fn window_geometry_changed(rect: Rect) -> Result<ComBox<IEventAbi>, NcStatus> {
    if rect.width < 0 || rect.height < 0 {
        return Err(NC_E_INVALID_ARG);
    }
    WindowGeometryChanged::into_box(WindowGeometryChanged { rect })
        .query()
        .map_err(|(status, _unpublished)| status)
}

#[neocogi_com::com_object(interfaces(crate::IWindowCloseRequestedEvent), ownership = exclusive)]
/// Immutable captured fact; this allocation retains no source widget or service.
struct WindowCloseRequested {}
impl IEvent for WindowCloseRequested {}
impl IWindowEvent for WindowCloseRequested {}
impl IWindowCloseRequestedEvent for WindowCloseRequested {}
/// Captures window close requested as one exclusive event, without retaining its source.
pub fn window_close_requested() -> Result<ComBox<IEventAbi>, NcStatus> {
    WindowCloseRequested::into_box(WindowCloseRequested {})
        .query()
        .map_err(|(status, _unpublished)| status)
}

#[neocogi_com::com_object(interfaces(crate::IWindowHiddenEvent), ownership = exclusive)]
/// Immutable captured fact; this allocation retains no source widget or service.
struct WindowHidden {}
impl IEvent for WindowHidden {}
impl IWindowEvent for WindowHidden {}
impl IWindowHiddenEvent for WindowHidden {}
/// Captures window hidden as one exclusive event, without retaining its source.
pub fn window_hidden() -> Result<ComBox<IEventAbi>, NcStatus> {
    WindowHidden::into_box(WindowHidden {}).query().map_err(|(status, _unpublished)| status)
}

#[neocogi_com::com_object(interfaces(crate::IWindowDestroyedEvent), ownership = exclusive)]
/// Immutable captured fact; this allocation retains no source widget or service.
struct WindowDestroyed {}
impl IEvent for WindowDestroyed {}
impl IWindowEvent for WindowDestroyed {}
impl IWindowDestroyedEvent for WindowDestroyed {}
/// Captures window destroyed as one exclusive event, without retaining its source.
pub fn window_destroyed() -> Result<ComBox<IEventAbi>, NcStatus> {
    WindowDestroyed::into_box(WindowDestroyed {}).query().map_err(|(status, _unpublished)| status)
}

#[neocogi_com::com_object(interfaces(crate::IWindowMinimizedEvent), ownership = exclusive)]
/// Immutable captured fact; this allocation retains no source widget or service.
struct WindowMinimized {
    /// Captured rect, independent of subsequent producer edits.
    rect: Rect,
}
impl IEvent for WindowMinimized {}
impl IWindowEvent for WindowMinimized {}
impl IWindowMinimizedEvent for WindowMinimized {
    /// Reads the immutable rect snapshot through the public event ABI.
    fn rect(&self) -> Result<Rect, NcStatus> {
        Ok(self.rect)
    }
}
/// Captures a window minimized fact independently of the live window.
/// Negative width or height returns InvalidArg; coordinates may be negative.
pub fn window_minimized(rect: Rect) -> Result<ComBox<IEventAbi>, NcStatus> {
    if rect.width < 0 || rect.height < 0 {
        return Err(NC_E_INVALID_ARG);
    }
    WindowMinimized::into_box(WindowMinimized { rect })
        .query()
        .map_err(|(status, _unpublished)| status)
}

#[neocogi_com::com_object(interfaces(crate::IWindowMaximizedEvent), ownership = exclusive)]
/// Immutable captured fact; this allocation retains no source widget or service.
struct WindowMaximized {
    /// Captured rect, independent of subsequent producer edits.
    rect: Rect,
}
impl IEvent for WindowMaximized {}
impl IWindowEvent for WindowMaximized {}
impl IWindowMaximizedEvent for WindowMaximized {
    /// Reads the immutable rect snapshot through the public event ABI.
    fn rect(&self) -> Result<Rect, NcStatus> {
        Ok(self.rect)
    }
}
/// Captures a window maximized fact independently of the live window.
/// Negative width or height returns InvalidArg; coordinates may be negative.
pub fn window_maximized(rect: Rect) -> Result<ComBox<IEventAbi>, NcStatus> {
    if rect.width < 0 || rect.height < 0 {
        return Err(NC_E_INVALID_ARG);
    }
    WindowMaximized::into_box(WindowMaximized { rect })
        .query()
        .map_err(|(status, _unpublished)| status)
}

#[neocogi_com::com_object(interfaces(crate::IWindowRestoredEvent), ownership = exclusive)]
/// Immutable captured fact; this allocation retains no source widget or service.
struct WindowRestored {
    /// Captured rect, independent of subsequent producer edits.
    rect: Rect,
}
impl IEvent for WindowRestored {}
impl IWindowEvent for WindowRestored {}
impl IWindowRestoredEvent for WindowRestored {
    /// Reads the immutable rect snapshot through the public event ABI.
    fn rect(&self) -> Result<Rect, NcStatus> {
        Ok(self.rect)
    }
}
/// Captures a window restored fact independently of the live window.
/// Negative width or height returns InvalidArg; coordinates may be negative.
pub fn window_restored(rect: Rect) -> Result<ComBox<IEventAbi>, NcStatus> {
    if rect.width < 0 || rect.height < 0 {
        return Err(NC_E_INVALID_ARG);
    }
    WindowRestored::into_box(WindowRestored { rect })
        .query()
        .map_err(|(status, _unpublished)| status)
}

#[neocogi_com::com_object(interfaces(crate::IPopupDismissedEvent), ownership = exclusive)]
/// Immutable captured fact; this allocation retains no source widget or service.
struct PopupDismissed {}
impl IEvent for PopupDismissed {}
impl IPopupDismissedEvent for PopupDismissed {}
/// Captures popup dismissed as one exclusive event, without retaining its source.
pub fn popup_dismissed() -> Result<ComBox<IEventAbi>, NcStatus> {
    PopupDismissed::into_box(PopupDismissed {}).query().map_err(|(status, _unpublished)| status)
}

#[neocogi_com::com_object(interfaces(crate::IChoiceChangedEvent), ownership = exclusive)]
/// Immutable captured fact; this allocation retains no source widget or service.
struct ChoiceChanged {
    /// Captured selection, independent of subsequent producer edits.
    selection: ChoiceSelection,
}
impl IEvent for ChoiceChanged {}
impl IChoiceChangedEvent for ChoiceChanged {
    /// Reads the immutable selection snapshot through the public event ABI.
    fn selection(&self) -> Result<ChoiceSelection, NcStatus> {
        Ok(self.selection)
    }
}
/// Captures choice changed as one exclusive event, without retaining its source.
pub fn choice_changed(selection: ChoiceSelection) -> Result<ComBox<IEventAbi>, NcStatus> {
    ChoiceChanged::into_box(ChoiceChanged { selection })
        .query()
        .map_err(|(status, _unpublished)| status)
}

#[neocogi_com::com_object(interfaces(crate::IFileAcceptedEvent), ownership = exclusive)]
/// Immutable captured fact; this allocation retains no source widget or service.
struct FileAccepted {
    /// Captured path, independent of subsequent producer edits.
    path: ComPtr<INativePathAbi>,
}
impl IEvent for FileAccepted {}
impl IFileAcceptedEvent for FileAccepted {
    /// Reads the immutable path snapshot through the public event ABI.
    fn path(&self) -> Result<ComPtr<INativePathAbi>, NcStatus> {
        Ok(self.path.clone())
    }
}
/// Captures file accepted as one exclusive event, without retaining its source.
pub fn file_accepted(path: ComPtr<INativePathAbi>) -> Result<ComBox<IEventAbi>, NcStatus> {
    FileAccepted::into_box(FileAccepted { path }).query().map_err(|(status, _unpublished)| status)
}

#[neocogi_com::com_object(interfaces(crate::IFileCancelledEvent), ownership = exclusive)]
/// Immutable captured fact; this allocation retains no source widget or service.
struct FileCancelled {}
impl IEvent for FileCancelled {}
impl IFileCancelledEvent for FileCancelled {}
/// Captures file cancelled as one exclusive event, without retaining its source.
pub fn file_cancelled() -> Result<ComBox<IEventAbi>, NcStatus> {
    FileCancelled::into_box(FileCancelled {}).query().map_err(|(status, _unpublished)| status)
}
