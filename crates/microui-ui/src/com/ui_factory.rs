//! Dynamically linked widget-construction shorthand over concrete COM contracts.
use super::*;

#[com_object(UiFactory)]
/// Factory object retaining only the host capability shared by its products.
pub struct UiFactoryObject {
    /// Non-owning runtime authority used for lifetime checks, resources, and child stores.
    host: ComPtr<IWidgetHostAbi>,
}

impl UiFactoryObject {
    /// Builds the factory projection used by a newly constructed UI runtime.
    pub(super) fn create(host: ComPtr<IWidgetHostAbi>) -> Result<ComPtr<UiFactoryAbi>, NcStatus> {
        let value = <Self as UiFactory>::new(host.as_com_ref())?;
        Ok(Self::from_value(value)
            .query()
            .expect("UiFactoryObject declares the UiFactory class contract"))
    }
}

impl UiFactory for UiFactoryObject {
    /// Validates the supplied host before retaining its independent capability.
    fn new(host: ComRef<'_, IWidgetHostAbi>) -> Result<Self, NcStatus> {
        host.check_open()?;
        Ok(Self { host: host.add_ref() })
    }

    /// Returns the exact host used by every product without retaining a UI tree.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        self.host.check_open()?;
        Ok(self.host.clone())
    }

    /// Constructs the complete semantic combo header with this factory's host.
    fn create_combo(&self) -> Result<ComPtr<ComboAbi>, NcStatus> {
        self.host.check_mutation()?;
        super::combo::ComboObject::create(self.host.clone())
    }

    /// Validates complete text and wrapping before constructing the fixed scroll/editor composition.
    fn create_text_area(
        &self,
        input: NcCopyFromBufferOffset<'_, u8>,
        wrapping: TextWrapping,
        scrolling: bool,
    ) -> Result<ComPtr<TextAreaAbi>, NcStatus> {
        super::text_area::TextAreaObject::create(self.host.clone(), input, wrapping, scrolling)
    }

    /// Validates text and presentation before constructing either fixed structural child.
    fn create_disclosure(
        &self,
        label: NcCopyFromBufferOffset<'_, u8>,
        style: DisclosureStyle,
        expanded: bool,
    ) -> Result<ComPtr<DisclosureAbi>, NcStatus> {
        super::disclosure::DisclosureObject::create(self.host.clone(), label, style, expanded)
    }

    /// Composes real bars and a transform container around validated application content.
    fn create_scroll_area(
        &self,
        content: ComRef<'_, IWidgetAbi>,
        enabled: bool,
    ) -> Result<ComPtr<ScrollAreaAbi>, NcStatus> {
        super::scroll::ScrollAreaObject::create(self.host.clone(), content, enabled)
    }

    /// Validates both policies before creating a container and its host-owned child store.
    fn create_linear(
        &self,
        direction: FlowDirection,
        cross: CrossSizeAbi,
    ) -> Result<ComPtr<LinearAbi>, NcStatus> {
        self.host.check_mutation()?;
        super::containers::LinearObject::create(self.host.clone(), direction, cross)
    }

    /// Validates complete typed track sequences before allocating Grid state.
    fn create_grid(
        &self,
        columns: NcCopyFromBufferOffset<'_, LayoutTrackAbi>,
        rows: NcCopyFromBufferOffset<'_, LayoutTrackAbi>,
    ) -> Result<ComPtr<GridAbi>, NcStatus> {
        self.host.check_mutation()?;
        super::containers::GridObject::create(self.host.clone(), columns, rows)
    }

    /// Validates complete checkbox initialization before allocating its COM identity.
    fn create_checkbox(
        &self,
        checked: bool,
        label: NcCopyFromBufferOffset<'_, u8>,
    ) -> Result<ComPtr<CheckboxAbi>, NcStatus> {
        self.host.check_mutation()?;
        super::Checkbox::create_com(self.host.clone(), checked, label)
    }

    /// Validates complete textbox initialization before allocating its COM identity.
    fn create_textbox(&self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<TextboxAbi>, NcStatus> {
        self.host.check_mutation()?;
        super::leaves::TextboxObject::create(self.host.clone(), input)
    }

    /// Validates text and wrapping before allocating the display object's identity.
    fn create_text_block(
        &self,
        input: NcCopyFromBufferOffset<'_, u8>,
        wrapping: TextWrapping,
    ) -> Result<ComPtr<TextBlockAbi>, NcStatus> {
        self.host.check_mutation()?;
        super::TextBlock::create_com(self.host.clone(), input, wrapping)
    }

    /// Validates every arithmetic field before allocating a slider identity.
    fn create_slider(&self, options: SliderOptions) -> Result<ComPtr<SliderAbi>, NcStatus> {
        self.host.check_mutation()?;
        super::leaves::SliderObject::create(self.host.clone(), options)
    }

    /// Validates every arithmetic field before allocating a number identity.
    fn create_number(&self, options: NumberOptions) -> Result<ComPtr<NumberAbi>, NcStatus> {
        self.host.check_mutation()?;
        super::leaves::NumberObject::create(self.host.clone(), options)
    }

    /// Validates the label before allocating a color swatch with copied RGBA state.
    fn create_color_swatch(
        &self,
        rgba: u32,
        label: NcCopyFromBufferOffset<'_, u8>,
    ) -> Result<ComPtr<ColorSwatchAbi>, NcStatus> {
        self.host.check_mutation()?;
        super::ColorSwatch::create_com(self.host.clone(), rgba, label)
    }

    /// Validates the label before allocating a standalone list-row identity.
    fn create_list_item(&self, label: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<ListItemAbi>, NcStatus> {
        self.host.check_mutation()?;
        super::ListItem::create_com(self.host.clone(), label)
    }

    /// Validates the label before allocating a button-like list entry.
    fn create_list_box(&self, label: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<ListBoxAbi>, NcStatus> {
        self.host.check_mutation()?;
        super::ListBox::create_com(self.host.clone(), label)
    }

    /// Validates axis and range arithmetic before allocating the scrollbar.
    fn create_scrollbar(&self, range: ScrollRange) -> Result<ComPtr<ScrollbarAbi>, NcStatus> {
        self.host.check_mutation()?;
        super::Scrollbar::create_com(self.host.clone(), range)
    }

    /// Constructs a complete Button in one call through its concrete class implementation.
    fn create_button(
        &self,
        label: NcCopyFromBufferOffset<'_, u8>,
        options: WidgetOptions,
    ) -> Result<ComPtr<ButtonAbi>, NcStatus> {
        self.host.check_mutation()?;
        super::Button::create_com(self.host.clone(), label, options)
    }
}
