//! Clipboard receipts cross COM as copied facts; only the owning editor applies text mutations.
use super::{
    editing::Editing,
    text_edit::{apply_text_input, ReturnBehavior},
};
use crate::{Key, KeyEvent, Modifiers};
use neocogi_com::{ComPtr, ComRef, NcCopyFromBufferOffset, NcStatus, com_object};
use neocogi_ui_core_contracts::{IClipboardCompletion, IClipboardCompletionAbi, IWidgetHostAbi, MAX_CLIPBOARD_BYTES};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};

/// Meaning of one in-flight system clipboard request.
#[derive(Clone, Copy)]
enum Operation {
    /// Copy has no document mutation after its receipt.
    Copy,
    /// Remove the still-current selection only after successful publication.
    Cut,
    /// Insert returned text only while the requesting caret/selection remains current.
    Paste,
}

/// One bounded receipt channel owned only while the requesting text/selection remains current.
pub(super) struct Pending {
    /// Interpretation of a successful completion.
    operation: Operation,
    /// At most one copied platform receipt, owned exclusively by the editor.
    receive: Receiver<Result<String, NcStatus>>,
}

#[com_object(interfaces(neocogi_ui_core_contracts::IClipboardCompletion))]
/// Platform-retainable endpoint with no widget, runtime, or renderer ownership.
struct Completion {
    /// Bounded queue rejects duplicate delivery without blocking a browser callback.
    send: SyncSender<Result<String, NcStatus>>,
}
impl IClipboardCompletion for Completion {
    /// Validates the complete copied result before it becomes an editor-owned fact.
    fn complete(&self, status: i32, text: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        if text.offset() != 0 || text.as_slice().len() > MAX_CLIPBOARD_BYTES || (status != 0 && !text.as_slice().is_empty()) || status > 0 {
            return Err(neocogi_com::NC_E_INVALID_ARG);
        }
        let result = match status {
            0 => Ok(std::str::from_utf8(text.as_slice()).map_err(|_| neocogi_com::NC_E_INVALID_ARG)?.to_owned()),
            _ => Err(NcStatus::from_code(status)),
        };
        self.send.try_send(result).map_err(|_| neocogi_ui_core_contracts::UI_E_CLOSED)
    }
}

/// Keeps a committed receipt visible when a later shortcut request fails in the same update.
fn preserve_change(changed: bool, error: NcStatus) -> Result<bool, NcStatus> {
    if changed { Ok(true) } else { Err(error) }
}

/// Applies ready receipts and starts one shortcut request without borrowing widget state across COM.
pub(super) fn advance(
    editing: &mut Editing,
    host: ComRef<'_, IWidgetHostAbi>,
    text: &mut String,
    cursor: &mut usize,
    key: Option<KeyEvent>,
    behavior: ReturnBehavior,
    focused: bool,
) -> Result<bool, NcStatus> {
    if !focused {
        editing.pending = None;
        return Ok(false);
    }
    let mut changed = receive(editing, text, cursor, behavior)?;
    let operation = key
        .filter(|event| event.is_pressed() && event.modifiers.intersects(Modifiers::CTRL | Modifiers::SUPER) && !event.modifiers.intersects(Modifiers::ALT))
        .and_then(|event| match event.key {
            Key::Character('c' | 'C') => Some(Operation::Copy),
            Key::Character('x' | 'X') => Some(Operation::Cut),
            Key::Character('v' | 'V') => Some(Operation::Paste),
            _ => None,
        });
    let Some(operation) = operation else { return Ok(changed) };
    if editing.pending.is_some() {
        // Keep at most one platform operation per editor; repeated shortcuts cannot grow a queue.
        return Ok(changed);
    }
    let clipboard = match host.clipboard() {
        Ok(clipboard) => clipboard,
        Err(neocogi_com::NC_E_NOT_FOUND) => return Ok(changed), // Headless or explicitly clipboard-free hosts.
        Err(error) => return preserve_change(changed, error),
    };
    let selected = match operation {
        Operation::Paste => "",
        Operation::Copy | Operation::Cut => match editing.selection(text, *cursor) {
            Some(range) => &text[range],
            None => return Ok(changed),
        },
    };
    if selected.len() > MAX_CLIPBOARD_BYTES {
        return preserve_change(changed, neocogi_com::NC_E_OUT_OF_RANGE);
    }
    let (send, receipts) = mpsc::sync_channel(1);
    let completion: ComPtr<IClipboardCompletionAbi> = Completion::from_value(Completion { send })
        .query()
        .expect("clipboard completion declares its contract");
    let request = match operation {
        Operation::Paste => clipboard.read(completion.as_com_ref()),
        Operation::Copy | Operation::Cut => clipboard.write(NcCopyFromBufferOffset::new(0, selected.as_bytes()), completion.as_com_ref()),
    };
    if let Err(error) = request {
        return preserve_change(changed, error);
    }
    editing.pending = Some(Pending { operation, receive: receipts });
    // Native providers may already have replied; browser providers complete on a later update.
    changed |= match receive(editing, text, cursor, behavior) {
        Ok(changed) => changed,
        Err(error) => return preserve_change(changed, error),
    };
    Ok(changed)
}

/// Restores pending ownership on an empty queue; edits revoke this receiver before stale delivery.
fn receive(editing: &mut Editing, text: &mut String, cursor: &mut usize, behavior: ReturnBehavior) -> Result<bool, NcStatus> {
    let Some(pending) = editing.pending.take() else { return Ok(false) };
    let result = match pending.receive.try_recv() {
        Ok(result) => result?,
        Err(TryRecvError::Empty) => {
            editing.pending = Some(pending);
            return Ok(false);
        }
        Err(TryRecvError::Disconnected) => return Err(neocogi_ui_core_contracts::UI_E_CLOSED),
    };
    match pending.operation {
        Operation::Copy => Ok(false),
        Operation::Cut => {
            let previous = editing.snapshot(text, *cursor);
            let changed = editing.delete_selection(text, cursor);
            if changed {
                editing.record(previous);
            }
            Ok(changed)
        }
        Operation::Paste => {
            let outcome = apply_text_input(text, *cursor, editing, &result, None, behavior);
            *cursor = outcome.cursor;
            Ok(outcome.changed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neocogi_ui_core_contracts::{ITextClipboard, ITextClipboardAbi, UiRuntimeAbi};

    /// Deferred platform operations are observable without borrowing or mutating a fake widget.
    enum Request {
        /// A requested clipboard read retains only its completion.
        Read(ComPtr<IClipboardCompletionAbi>),
        /// A requested write owns the exact selected text and its completion.
        Write(String, ComPtr<IClipboardCompletionAbi>),
    }
    #[com_object(interfaces(neocogi_ui_core_contracts::ITextClipboard))]
    /// Asynchronous provider lets each test decide when and whether a platform request succeeds.
    struct DeferredClipboard {
        /// Ordinary channel separates fake platform observations from the production editor.
        requests: mpsc::Sender<Request>,
    }
    impl ITextClipboard for DeferredClipboard {
        /// Queues a read without fabricating a synchronous result.
        fn read(&self, completion: ComRef<'_, IClipboardCompletionAbi>) -> Result<(), NcStatus> {
            self.requests
                .send(Request::Read(completion.add_ref()))
                .map_err(|_| neocogi_ui_core_contracts::UI_E_CLOSED)
        }
        /// Captures text before returning and deliberately delays its acknowledgement.
        fn write(&self, text: NcCopyFromBufferOffset<'_, u8>, completion: ComRef<'_, IClipboardCompletionAbi>) -> Result<(), NcStatus> {
            let text = crate::boundary::text(text)?.to_owned();
            self.requests
                .send(Request::Write(text, completion.add_ref()))
                .map_err(|_| neocogi_ui_core_contracts::UI_E_CLOSED)
        }
    }
    #[com_object(interfaces(neocogi_ui_core_contracts::ITextClipboard))]
    /// A provider that rejects a new request after an older receipt has completed.
    struct RejectClipboard;
    impl ITextClipboard for RejectClipboard {
        /// Simulates a denied or unavailable platform read.
        fn read(&self, _completion: ComRef<'_, IClipboardCompletionAbi>) -> Result<(), NcStatus> {
            Err(neocogi_com::NC_E_INVALID_ARG)
        }
        /// Simulates a denied or unavailable platform write.
        fn write(&self, _text: NcCopyFromBufferOffset<'_, u8>, _completion: ComRef<'_, IClipboardCompletionAbi>) -> Result<(), NcStatus> {
            Err(neocogi_com::NC_E_INVALID_ARG)
        }
    }

    /// Real core host authority paired with a controllable external clipboard provider.
    struct Harness {
        /// Keeps the actual production construction host open for each operation.
        _runtime: ComPtr<UiRuntimeAbi>,
        /// Normal widget host, including mutation checks and service installation.
        host: ComPtr<IWidgetHostAbi>,
        /// Platform requests awaiting explicit test completion.
        requests: Receiver<Request>,
        /// Same exclusive model used by concrete textboxes and text areas.
        editing: Editing,
        /// Current user text whose ownership never crosses the completion boundary.
        text: String,
        /// Current active byte endpoint.
        cursor: usize,
    }
    impl Harness {
        /// Installs a fake asynchronous service through the real runtime's public method.
        fn new(text: &str) -> Self {
            let mut runtime = crate::test_support::create_runtime(crate::test_support::test_atlas());
            let (send, requests) = mpsc::channel();
            let clipboard: ComPtr<ITextClipboardAbi> = DeferredClipboard::from_value(DeferredClipboard { requests: send }).query().unwrap();
            runtime.set_clipboard(clipboard.as_com_ref()).unwrap();
            let host = runtime.construction_host().unwrap().host().unwrap();
            let mut editing = Editing::default();
            let mut cursor = 0;
            editing.select_all(text, &mut cursor);
            Self {
                _runtime: runtime,
                host,
                requests,
                editing,
                text: text.into(),
                cursor,
            }
        }
        /// Starts a shortcut or polls an eventless update through the production transfer path.
        fn step(&mut self, key: Option<char>) -> Result<bool, NcStatus> {
            advance(
                &mut self.editing,
                self.host.as_com_ref(),
                &mut self.text,
                &mut self.cursor,
                key.map(|key| KeyEvent::pressed(Key::Character(key), Modifiers::CTRL)),
                ReturnBehavior::Submit,
                true,
            )
        }
    }

    /// A failed write cannot remove cut text; a later acknowledged cut is locally undoable.
    #[test]
    fn cut_waits_for_success_and_preserves_text_on_failure() {
        let mut editor = Harness::new("keep this");
        assert!(!editor.step(Some('x')).unwrap());
        assert_eq!(editor.text, "keep this");
        let Request::Write(text, completion) = editor.requests.recv().unwrap() else {
            panic!("expected write")
        };
        assert_eq!(text, "keep this");
        completion
            .complete(neocogi_com::NC_E_INVALID_ARG.code(), NcCopyFromBufferOffset::new(0, &[]))
            .unwrap();
        assert_eq!(editor.step(None), Err(neocogi_com::NC_E_INVALID_ARG));
        assert_eq!(editor.text, "keep this");
        editor.step(Some('x')).unwrap();
        let Request::Write(_, completion) = editor.requests.recv().unwrap() else {
            panic!("expected write")
        };
        completion.complete(0, NcCopyFromBufferOffset::new(0, &[])).unwrap();
        assert!(editor.step(None).unwrap());
        assert_eq!(editor.text, "");
        assert!(editor.editing.restore(&mut editor.text, &mut editor.cursor, false));
        assert_eq!(editor.text, "keep this");
    }

    /// Navigation and typing revoke delayed paste; a current paste replaces selection once.
    #[test]
    fn stale_paste_is_cancelled_and_current_paste_is_normalized() {
        let mut editor = Harness::new("original");
        editor.step(Some('v')).unwrap();
        let Request::Read(completion) = editor.requests.recv().unwrap() else {
            panic!("expected read")
        };
        editor.editing.move_to(&editor.text, &mut editor.cursor, 0, false);
        assert!(completion.complete(0, NcCopyFromBufferOffset::new(0, b"stale")).is_err());
        assert!(!editor.step(None).unwrap());
        assert_eq!(editor.text, "original");
        editor.editing.select_all(&editor.text, &mut editor.cursor);
        editor.step(Some('v')).unwrap();
        let Request::Read(completion) = editor.requests.recv().unwrap() else {
            panic!("expected read")
        };
        completion.complete(0, NcCopyFromBufferOffset::new(0, b"a\r\nb")).unwrap();
        assert!(editor.step(None).unwrap());
        assert_eq!(editor.text, "ab");
        assert!(!editor.step(None).unwrap());
        assert!(editor.editing.restore(&mut editor.text, &mut editor.cursor, false));
        assert_eq!(editor.text, "original");
    }

    /// Repeated keys cannot queue unbounded requests and malformed receipts never enter the model.
    #[test]
    fn pending_transfer_and_receipt_sizes_are_bounded() {
        let mut editor = Harness::new("text");
        editor.step(Some('v')).unwrap();
        let Request::Read(completion) = editor.requests.recv().unwrap() else {
            panic!("expected read")
        };
        editor.step(Some('v')).unwrap();
        assert!(editor.requests.try_recv().is_err());
        let oversized = vec![b'a'; MAX_CLIPBOARD_BYTES + 1];
        assert!(completion.complete(0, NcCopyFromBufferOffset::new(0, &oversized)).is_err());
        completion.complete(0, NcCopyFromBufferOffset::new(0, b"first")).unwrap();
        assert!(completion.complete(0, NcCopyFromBufferOffset::new(0, b"second")).is_err());
        assert!(editor.step(None).unwrap());
        assert_eq!(editor.text, "first");
    }
    /// A completed paste remains observable if a second shortcut request fails in that update.
    #[test]
    fn ready_paste_reports_change_even_when_next_read_is_denied() {
        let mut editor = Harness::new("original");
        assert!(!editor.step(Some('v')).unwrap());
        let Request::Read(completion) = editor.requests.recv().unwrap() else {
            panic!("expected read")
        };
        completion.complete(0, NcCopyFromBufferOffset::new(0, b"pasted")).unwrap();
        let rejecting: ComPtr<ITextClipboardAbi> = RejectClipboard::from_value(RejectClipboard).query().unwrap();
        editor._runtime.set_clipboard(rejecting.as_com_ref()).unwrap();
        assert_eq!(editor.step(Some('v')), Ok(true));
        assert_eq!(editor.text, "pasted");
        assert!(editor.requests.try_recv().is_err());
    }

    /// Expected browser permission denial must not turn an ordinary widget update into a frame failure.
    #[test]
    fn textbox_clipboard_denial_keeps_the_frame_and_text_valid() {
        let harness = Harness::new("unused");
        let runtime = crate::widget_object::WidgetRuntime::new(harness.host.clone(), crate::WidgetOption::FRAME).unwrap();
        let mut textbox = crate::Textbox::from_runtime(runtime, crate::TextboxParameters::new("retained"));
        let atlas = crate::test_support::test_atlas();
        let skin = crate::test_support::test_skin(&atlas);
        let bounds = crate::rect(0, 0, 120, 24);
        let update = |textbox: &mut crate::Textbox, key: Option<char>| {
            let mut context = crate::WidgetUpdateCtx::new_with_interaction(
                bounds,
                bounds,
                &skin,
                &atlas,
                true,
                true,
                true,
                false,
                false,
                crate::MouseButton::NONE,
                Modifiers::CTRL,
            );
            let input = key.map(|key| crate::UiInputEvent::Key {
                event: KeyEvent::pressed(Key::Character(key), Modifiers::CTRL),
            });
            crate::Widget::update(textbox, &mut context, input.as_ref());
            context.finish().unwrap();
        };
        update(&mut textbox, Some('a'));
        update(&mut textbox, Some('x'));
        let Request::Write(text, completion) = harness.requests.recv().unwrap() else {
            panic!("expected write")
        };
        assert_eq!(text, "retained");
        completion
            .complete(neocogi_com::NC_E_INVALID_ARG.code(), NcCopyFromBufferOffset::new(0, &[]))
            .unwrap();
        update(&mut textbox, None);
        assert_eq!(textbox.text(), "retained");
        update(&mut textbox, Some('v'));
        let Request::Read(completion) = harness.requests.recv().unwrap() else {
            panic!("expected read")
        };
        drop(textbox);
        assert!(completion.complete(0, NcCopyFromBufferOffset::new(0, b"late")).is_err());
    }
}
