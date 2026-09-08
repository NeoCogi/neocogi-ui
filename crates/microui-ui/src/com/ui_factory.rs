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
    ) -> Result<ComPtr<ITextAreaAbi>, NcStatus> {
        super::text_area::TextArea::create(
            self.host.clone(),
            text(input)?,
            super::leaves::local_wrap(wrapping)?,
            scrolling,
        )
    }

    /// Validates text and presentation before constructing either fixed structural child.
    fn create_disclosure(
        &self,
        label: NcCopyFromBufferOffset<'_, u8>,
        style: DisclosureStyle,
        expanded: bool,
    ) -> Result<ComPtr<IDisclosureAbi>, NcStatus> {
        super::disclosure::Disclosure::create(self.host.clone(), text(label)?, style, expanded)
    }

    /// Composes real bars and a transform container around validated application content.
    fn create_scroll_area(
        &self,
        content: ComRef<'_, IWidgetAbi>,
        enabled: bool,
    ) -> Result<ComPtr<IScrollAreaAbi>, NcStatus> {
        super::scroll::ScrollArea::create(self.host.clone(), content, enabled)
    }

    /// Validates both policies before creating a container and its host-owned child store.
    fn create_linear(
        &self,
        direction: FlowDirection,
        cross: CrossSizeAbi,
    ) -> Result<ComPtr<ILinearAbi>, NcStatus> {
        self.host.check_mutation()?;
        let direction = super::containers::local_direction(direction)?;
        let cross = super::containers::local_cross(cross)?;
        super::containers::Linear::create(self.host.clone(), crate::Linear::com_new(direction, cross))
    }

    /// Validates complete typed track sequences before allocating Grid state.
    fn create_grid(
        &self,
        columns: NcCopyFromBufferOffset<'_, LayoutTrackAbi>,
        rows: NcCopyFromBufferOffset<'_, LayoutTrackAbi>,
    ) -> Result<ComPtr<IGridAbi>, NcStatus> {
        self.host.check_mutation()?;
        let columns = super::containers::local_tracks(columns)?;
        let rows = super::containers::local_tracks(rows)?;
        super::containers::Grid::create(self.host.clone(), crate::Grid::com_new(columns, rows))
    }

    /// Validates complete checkbox initialization before allocating its COM identity.
    fn create_checkbox(
        &self,
        checked: bool,
        label: NcCopyFromBufferOffset<'_, u8>,
    ) -> Result<ComPtr<CheckboxAbi>, NcStatus> {
        self.host.check_mutation()?;
        super::leaves::CheckboxObject::create(self.host.clone(), checked, label)
    }

    /// Validates complete textbox initialization before allocating its COM identity.
    fn create_textbox(&self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<ITextboxAbi>, NcStatus> {
        self.host.check_mutation()?;
        let parameters = crate::TextboxParameters::new(text(input)?);
        super::leaves::Textbox::create(self.host.clone(), parameters.opt, crate::Textbox::from(parameters))
    }

    /// Validates text and wrapping before allocating the display object's identity.
    fn create_text_block(
        &self,
        input: NcCopyFromBufferOffset<'_, u8>,
        wrapping: TextWrapping,
    ) -> Result<ComPtr<ITextBlockAbi>, NcStatus> {
        self.host.check_mutation()?;
        let parameters = crate::TextBlockParameters::with_wrap(text(input)?, super::leaves::local_wrap(wrapping)?);
        super::leaves::TextBlock::create(self.host.clone(), parameters.opt, crate::TextBlock::from(parameters))
    }

    /// Validates every arithmetic field before allocating a slider identity.
    fn create_slider(&self, options: SliderOptions) -> Result<ComPtr<ISliderAbi>, NcStatus> {
        self.host.check_mutation()?;
        let parameters = crate::SliderParameters::with_opt(
            options.value,
            options.low,
            options.high,
            options.step,
            crate::DecimalPrecision::try_from(usize::from(options.precision)).map_err(|_| NC_E_INVALID_ARG)?,
            crate::WidgetOption::FRAME,
        )
        .map_err(|_| NC_E_INVALID_ARG)?;
        super::leaves::Slider::create(self.host.clone(), parameters.opt, crate::Slider::from(parameters))
    }

    /// Validates every arithmetic field before allocating a number identity.
    fn create_number(&self, options: NumberOptions) -> Result<ComPtr<INumberAbi>, NcStatus> {
        self.host.check_mutation()?;
        let parameters = crate::NumberParameters::new(
            options.value,
            options.step,
            crate::DecimalPrecision::try_from(usize::from(options.precision)).map_err(|_| NC_E_INVALID_ARG)?,
        )
        .map_err(|_| NC_E_INVALID_ARG)?;
        super::leaves::Number::create(self.host.clone(), parameters.opt, crate::Number::from(parameters))
    }

    /// Validates the label before allocating a color swatch with copied RGBA state.
    fn create_color_swatch(
        &self,
        rgba: u32,
        label: NcCopyFromBufferOffset<'_, u8>,
    ) -> Result<ComPtr<IColorSwatchAbi>, NcStatus> {
        self.host.check_mutation()?;
        let parameters = crate::ColorSwatchParameters::new(super::render::color(rgba)).label(text(label)?);
        super::leaves::ColorSwatch::create(self.host.clone(), parameters.opt, crate::ColorSwatch::from(parameters))
    }

    /// Validates the label before allocating a standalone list-row identity.
    fn create_list_item(&self, label: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<IListItemAbi>, NcStatus> {
        self.host.check_mutation()?;
        let parameters = crate::ListItemParameters::new(text(label)?);
        super::leaves::ListItem::create(self.host.clone(), parameters.opt, crate::ListItem::from(parameters))
    }

    /// Validates the label before allocating a button-like list entry.
    fn create_list_box(&self, label: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<IListBoxAbi>, NcStatus> {
        self.host.check_mutation()?;
        let parameters = crate::ListBoxParameters::new(text(label)?, None);
        super::leaves::ListBox::create(self.host.clone(), parameters.opt, crate::ListBox::from(parameters))
    }

    /// Validates axis and range arithmetic before allocating the scrollbar.
    fn create_scrollbar(&self, range: ScrollRange) -> Result<ComPtr<ScrollbarAbi>, NcStatus> {
        self.host.check_mutation()?;
        super::leaves::ScrollbarObject::create(self.host.clone(), range)
    }

    /// Constructs a complete Button in one call through its concrete class implementation.
    fn create_button(
        &self,
        label: NcCopyFromBufferOffset<'_, u8>,
        options: WidgetOptions,
    ) -> Result<ComPtr<ButtonAbi>, NcStatus> {
        self.host.check_mutation()?;
        super::button::ButtonObject::create(self.host.clone(), label, options)
    }
}
