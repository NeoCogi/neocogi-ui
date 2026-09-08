//! Fixed multiline composition forwards editing to one actual content leaf, sharing scalar scrolling only.
use super::*;
use super::widget::WidgetCore;
use nc_com::{INcStringAbi, NcString};

leaf_class!(TextEditor, crate::TextArea, IMultilineEditor, IMultilineEditorAbi, IFontStyle, ITextEvents);
font_style_impl!(TextEditor);
text_subscription_impl!(TextEditor, crate::TextAreaChanged, crate::TextAreaSubmitted);

impl ITextContent for TextEditor {
    /// Copies normalized text from the only editor buffer into an immutable snapshot.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.widget.state.readable()?;
        Ok(NcString::from_utf8_str(self.widget.take()?.text()))
    }
    /// A replacement also requests caret reveal, so even equal normalized text can require new scroll placement.
    fn set_text(&self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        self.widget.take()?.set_text(text(input)?);
        self.widget.state.record_change(GeometryChange::CHANGED);
        Ok(())
    }
}
impl ITextEditor for TextEditor {
    /// Reads the scalar-aligned byte offset from the same buffer used by routed input.
    fn cursor(&self) -> Result<u64, NcStatus> {
        self.widget.state.readable()?;
        Ok(self.widget.take()?.cursor() as u64)
    }
    /// Clamps the byte offset and retains the pending caret reveal until an ordinary update can resolve its geometry.
    fn set_cursor(&self, cursor: u64) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        self.widget.take()?.set_cursor(usize::try_from(cursor).unwrap_or(usize::MAX));
        self.widget.state.record_change(GeometryChange::CHANGED);
        Ok(())
    }
}
impl IMultilineEditor for TextEditor {
    /// Encodes the exact wrapping policy consumed by the existing multiline layout algorithm.
    fn wrapping(&self) -> Result<TextWrapping, NcStatus> {
        self.widget.state.readable()?;
        Ok(match self.widget.take()?.com_wrap() {
            crate::TextWrap::None => TextWrapping::NONE,
            crate::TextWrap::Word => TextWrapping::WORD,
        })
    }
    /// Validates before changing wrapping and invalidating both text geometry and caret navigation.
    fn set_wrapping(&self, wrapping: TextWrapping) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        let wrap = super::leaves::local_wrap(wrapping)?;
        let mut core = self.widget.take()?;
        if core.com_wrap() != wrap {
            core.com_set_wrap(wrap);
            self.widget.state.record_change(GeometryChange::CHANGED);
        }
        Ok(())
    }
}

#[com_object(microui_contracts::TextArea)]
/// One composed root owns scrolling while inherited text calls edit its fixed actual content child.
pub struct TextAreaObject {
    /// Shared concrete scroll algorithm; no additional wrapper node surrounds the existing scroll tree.
    widget: WidgetCore<crate::ScrollArea>,
    /// Authoritative fixed surface and bars, identical to ordinary ScrollArea construction.
    editor: ComPtr<IChildrenEditorAbi>,
    /// Actual multiline content capability; buffer, cursor, input and text painting all live there.
    text: ComPtr<IMultilineEditorAbi>,
    /// Cached typography capability on that same editor, with no duplicate font selection.
    font: ComPtr<IFontStyleAbi>,
    /// Cached event capability of that same fixed editor, never a root-owned duplicate queue.
    events: ComPtr<ITextEventsAbi>,
}
fixed_phases!(TextAreaObject);
scrolling_impl!(TextAreaObject);

impl TextAreaObject {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(super) fn create(
        host: ComPtr<IWidgetHostAbi>,
        text: NcCopyFromBufferOffset<'_, u8>,
        wrapping: TextWrapping,
        scrolling: bool,
    ) -> Result<ComPtr<TextAreaAbi>, NcStatus> {
        let value = <Self as TextArea>::new(host.as_com_ref(), text, wrapping, scrolling)?;
        Ok(Self::from_value(value)
            .query()
            .expect("TextAreaObject declares the TextArea class contract"))
    }
}

impl TextArea for TextAreaObject {
    /// Prepares the fixed scroll structure and attaches one real editor before publishing the root.
    fn new(
        host: ComRef<'_, IWidgetHostAbi>,
        input: NcCopyFromBufferOffset<'_, u8>,
        wrapping: TextWrapping,
        scrolling: bool,
    ) -> Result<Self, NcStatus> {
        let text_value = text(input)?;
        let wrap = super::leaves::local_wrap(wrapping)?;
        let parts = super::scroll::ScrollParts::prepare(host.add_ref(), scrolling)?;
        let text = TextEditor::create(host.add_ref(), crate::WidgetOption::NONE, crate::TextArea::com_new(text_value, wrap, parts.model))?;
        let font = text.query()?;
        let events = text.query()?;
        parts.content.insert(0, text.query::<IWidgetAbi>()?.as_com_ref())?.finish()?;
        // The transform surface owns its private editor. This root intentionally
        // retains no content-replacement authority and cannot detach its text leaf.
        Ok(Self {
            widget: parts.widget,
            editor: parts.editor,
            text,
            font,
            events,
        })
    }
    /// Forwards snapshots after checking the outer object's logical lifetime.
    fn text(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.widget.state.readable()?;
        self.text.text()
    }
    /// Forwards normalized replacement; the actual child's marker invalidates the owning subtree.
    fn set_text(&self, text: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        self.text.set_text(text)
    }
    /// Reads the actual content editor's cursor without retaining a local copy.
    fn cursor(&self) -> Result<u64, NcStatus> {
        self.widget.state.readable()?;
        self.text.cursor()
    }
    /// Requests scalar-aligned cursor positioning and caret reveal on the actual child.
    fn set_cursor(&self, cursor: u64) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        self.text.set_cursor(cursor)
    }
    /// Returns the actual editor's wrapping policy.
    fn wrapping(&self) -> Result<TextWrapping, NcStatus> {
        self.widget.state.readable()?;
        self.text.wrapping()
    }
    /// Changes only the one policy used by text measurement and input.
    fn set_wrapping(&self, wrapping: TextWrapping) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        self.text.set_wrapping(wrapping)
    }
    /// Resolves a semantic font on the actual editor and invalidates its existing layout cache.
    fn set_font_role(&self, role: TextFontRole) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        self.font.set_font_role(role)
    }
    /// Resolves one stable named font through the actual editor's shared resource owner.
    fn set_named_font(&self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.widget.state.writable()?;
        self.font.set_named_font(name)
    }
    /// Forwards changed subscriptions to the actual fixed content editor.
    fn subscribe_changed(&self, sink: ComRef<'_, ITextChangedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.widget.state.writable()?;
        self.events.subscribe_changed(sink)
    }
    /// Forwards the independent submitted port without adding root queue ownership.
    fn subscribe_submitted(&self, sink: ComRef<'_, ITextSubmittedSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.widget.state.writable()?;
        self.events.subscribe_submitted(sink)
    }
}
