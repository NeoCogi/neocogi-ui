//! Exclusive selection and bounded local history shared by concrete text editors.
use super::text_edit::clamp_cursor_boundary;
use std::{collections::VecDeque, ops::Range};

/// Maximum retained text across both history directions for one editor.
const HISTORY_BYTES: usize = 1024 * 1024;
/// Maximum snapshots across undo and redo, independent of text size.
const HISTORY_STEPS: usize = 64;

/// A complete user-visible edit state before one text mutation.
#[derive(Clone, PartialEq)]
pub(crate) struct Snapshot {
    /// Owned source text, charged against the editor's history budget.
    text: String,
    /// Active end of the selection or collapsed caret.
    cursor: usize,
    /// Fixed end of an extended selection, if any.
    anchor: Option<usize>,
}

/// Selection and editing history belong to the widget and use ordinary exclusive mutation.
#[derive(Default)]
pub(crate) struct Editing {
    /// Pending platform receipt owns no borrow into this editor.
    pub(super) pending: Option<super::clipboard::Pending>,
    /// Fixed selection endpoint; absent means the caret is collapsed.
    anchor: Option<usize>,
    /// Prior snapshots in oldest-to-newest order.
    undo: VecDeque<Snapshot>,
    /// Undone snapshots, with the next replay at the end.
    redo: Vec<Snapshot>,
}
impl Editing {
    /// Returns a normalized nonempty selection, never an invalid source byte range.
    pub fn selection(&self, text: &str, cursor: usize) -> Option<Range<usize>> {
        let anchor = clamp_cursor_boundary(text, self.anchor?);
        let cursor = clamp_cursor_boundary(text, cursor);
        (anchor != cursor).then_some(anchor.min(cursor)..anchor.max(cursor))
    }
    /// Moves the active endpoint, either preserving/starting an anchor or collapsing selection.
    pub fn move_to(&mut self, text: &str, cursor: &mut usize, target: usize, extend: bool) {
        self.cancel_transfer();
        self.anchor = match extend {
            true => Some(self.anchor.unwrap_or_else(|| clamp_cursor_boundary(text, *cursor))),
            false => None,
        };
        *cursor = clamp_cursor_boundary(text, target);
    }
    /// Selects the complete current buffer without changing text or history.
    pub fn select_all(&mut self, text: &str, cursor: &mut usize) {
        self.cancel_transfer();
        self.anchor = Some(0);
        *cursor = text.len();
    }
    /// Removes the selected bytes and collapses the caret at their former start.
    pub fn delete_selection(&mut self, text: &mut String, cursor: &mut usize) -> bool {
        match self.selection(text, *cursor) {
            Some(range) => {
                *cursor = range.start;
                text.replace_range(range, "");
                self.anchor = None;
                true
            }
            None => false,
        }
    }
    /// Copies a prospective undo state only when it fits the complete history budget.
    pub fn snapshot(&self, text: &str, cursor: usize) -> Option<Snapshot> {
        (text.len() <= HISTORY_BYTES).then(|| Snapshot {
            text: text.into(),
            cursor,
            anchor: self.anchor,
        })
    }
    /// Accepts one actual mutation and invalidates redo. Oversized edits close the old timeline.
    pub fn record(&mut self, previous: Option<Snapshot>) {
        self.cancel_transfer();
        self.redo.clear();
        match previous {
            Some(previous) => self.undo.push_back(previous),
            None => self.undo.clear(),
        }
        self.trim();
    }
    /// Restores one snapshot and records its inverse without emitting application events here.
    pub fn restore(&mut self, text: &mut String, cursor: &mut usize, redo: bool) -> bool {
        let next = match redo {
            true => self.redo.pop(),
            false => self.undo.pop_back(),
        };
        let Some(next) = next else { return false };
        self.cancel_transfer();
        match self.snapshot(text, *cursor) {
            Some(current) => match redo {
                true => self.undo.push_back(current),
                false => self.redo.push(current),
            },
            None => {
                // Do not retain an unbounded inverse or jump across an unrecorded large edit.
                self.undo.clear();
                self.redo.clear();
            }
        }
        *text = next.text;
        *cursor = next.cursor;
        self.anchor = next.anchor;
        self.trim();
        true
    }
    /// Revokes in-flight clipboard authority by dropping its sole receipt consumer.
    fn cancel_transfer(&mut self) {
        self.pending = None;
    }
    /// Drops the oldest reachable states until both count and byte limits hold.
    fn trim(&mut self) {
        while self.undo.len() + self.redo.len() > HISTORY_STEPS
            || self.undo.iter().chain(&self.redo).map(|state| state.text.len()).sum::<usize>() > HISTORY_BYTES
        {
            match self.undo.pop_front() {
                Some(_) => {}
                None => {
                    self.redo.remove(0);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Key, KeyEvent, Modifiers};
    use super::super::text_edit::{apply_text_input, ReturnBehavior};

    /// Runs one semantic transition and updates the caller's active cursor.
    fn input(editing: &mut Editing, text: &mut String, cursor: &mut usize, key: Option<KeyEvent>, inserted: &str) {
        *cursor = apply_text_input(text, *cursor, editing, inserted, key, ReturnBehavior::Submit).cursor;
    }

    /// Replacement, undo, and redo preserve selection and retire an abandoned redo branch.
    #[test]
    fn selected_replacement_and_history_restore_exact_state() {
        let mut editing = Editing::default();
        let mut text = String::from("hello");
        let mut cursor = text.len();
        input(
            &mut editing,
            &mut text,
            &mut cursor,
            Some(KeyEvent::pressed(Key::ArrowLeft, Modifiers::SHIFT)),
            "",
        );
        input(
            &mut editing,
            &mut text,
            &mut cursor,
            Some(KeyEvent::pressed(Key::ArrowLeft, Modifiers::SHIFT)),
            "",
        );
        assert_eq!(editing.selection(&text, cursor), Some(3..5));
        input(&mut editing, &mut text, &mut cursor, None, "p");
        assert_eq!(text, "help");
        assert_eq!(editing.selection(&text, cursor), None);
        input(
            &mut editing,
            &mut text,
            &mut cursor,
            Some(KeyEvent::pressed(Key::Character('z'), Modifiers::CTRL)),
            "",
        );
        assert_eq!(text, "hello");
        assert_eq!(editing.selection(&text, cursor), Some(3..5));
        input(
            &mut editing,
            &mut text,
            &mut cursor,
            Some(KeyEvent::pressed(Key::Character('z'), Modifiers::CTRL | Modifiers::SHIFT)),
            "",
        );
        assert_eq!(text, "help");
        input(
            &mut editing,
            &mut text,
            &mut cursor,
            Some(KeyEvent::pressed(Key::Character('z'), Modifiers::CTRL)),
            "",
        );
        input(&mut editing, &mut text, &mut cursor, None, "d");
        assert_eq!(text, "held");
        assert!(!editing.restore(&mut text, &mut cursor, true));
    }

    /// Plain arrows collapse selections at the requested edge; deletion removes the whole range.
    #[test]
    fn select_all_collapse_and_delete_have_distinct_keyboard_semantics() {
        let mut editing = Editing::default();
        let mut text = String::from("abc");
        let mut cursor = 1;
        input(
            &mut editing,
            &mut text,
            &mut cursor,
            Some(KeyEvent::pressed(Key::Character('a'), Modifiers::CTRL)),
            "",
        );
        assert_eq!(editing.selection(&text, cursor), Some(0..3));
        input(
            &mut editing,
            &mut text,
            &mut cursor,
            Some(KeyEvent::pressed(Key::ArrowLeft, Modifiers::NONE)),
            "",
        );
        assert_eq!(cursor, 0);
        assert_eq!(editing.selection(&text, cursor), None);
        input(
            &mut editing,
            &mut text,
            &mut cursor,
            Some(KeyEvent::pressed(Key::Character('a'), Modifiers::CTRL)),
            "",
        );
        input(
            &mut editing,
            &mut text,
            &mut cursor,
            Some(KeyEvent::pressed(Key::Backspace, Modifiers::NONE)),
            "",
        );
        assert_eq!(text, "");
        assert!(editing.restore(&mut text, &mut cursor, false));
        assert_eq!(text, "abc");
    }

    /// Local history limits account for both directions and never retain an oversized snapshot.
    #[test]
    fn history_limits_survive_undo_redo_and_oversized_edits() {
        let mut editing = Editing::default();
        let mut text = String::new();
        let mut cursor = 0;
        for _ in 0..100 {
            input(&mut editing, &mut text, &mut cursor, None, "a");
        }
        let mut undos = 0;
        while editing.restore(&mut text, &mut cursor, false) {
            undos += 1;
        }
        assert_eq!(undos, HISTORY_STEPS);
        assert_eq!(editing.undo.len() + editing.redo.len(), HISTORY_STEPS);
        text = "x".repeat(HISTORY_BYTES + 1);
        cursor = text.len();
        input(&mut editing, &mut text, &mut cursor, None, "y");
        assert!(editing.undo.is_empty() && editing.redo.is_empty());
    }
}
