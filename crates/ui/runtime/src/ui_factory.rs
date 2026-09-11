//! Dynamically linked widget-construction shorthand over concrete COM contracts.

use neocogi_ui_contracts::*;
use neocogi_com::{ComPtr, ComRef, NcCopyFromBufferOffset, NcStatus, com_object};
use std::{cell::RefCell, rc::Rc};

#[com_object(constructible(neocogi_ui_contracts::UiFactory))]
/// Dynamically linked construction shorthand retaining its products' host and one asset value.
pub struct UiFactory {
    /// Non-owning runtime authority used for lifetime checks, resources, and child stores.
    host: ComPtr<IWidgetHostAbi>,
    /// One imported value shared by every widget produced through this factory.
    resources: crate::ui_node::widget_object::SharedAssets,
}

impl UiFactory {
    /// Builds the factory projection used by a newly constructed UI runtime.
    pub(crate) fn create(host: ComPtr<IWidgetHostAbi>, resources: crate::ui_node::widget_object::SharedAssets) -> Result<ComPtr<UiFactoryAbi>, NcStatus> {
        host.check_open()?;
        let value = Self { host, resources };
        Ok(Self::from_value(value).query().expect("UiFactory declares its callable class contract"))
    }
}

impl neocogi_ui_contracts::UiFactory for UiFactory {
    /// Validates the supplied host before retaining its independent capability.
    fn new(host: ComRef<'_, IWidgetHostAbi>) -> Result<Self, NcStatus> {
        host.check_open()?;
        let source = host.assets()?;
        let resources = Rc::new(RefCell::new(crate::assets::ImportedAssets::read(source.as_com_ref(), None)?));
        Ok(Self { host: host.add_ref(), resources })
    }

    /// Returns the exact host used by every product without retaining a UI tree.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        self.host.check_open()?;
        Ok(self.host.clone())
    }

    /// Constructs the complete semantic combo header with this factory's host.
    fn create_combo(&self) -> Result<ComPtr<ComboAbi>, NcStatus> {
        self.host.check_mutation()?;
        crate::Combo::create_com(self.host.clone(), self.resources.clone())
    }

    /// Validates complete text and wrapping before constructing the fixed scroll/editor composition.
    fn create_text_area(&self, input: NcCopyFromBufferOffset<'_, u8>, wrapping: TextWrapping, scrolling: bool) -> Result<ComPtr<TextAreaAbi>, NcStatus> {
        crate::TextArea::create_com(self.host.clone(), self.resources.clone(), input, wrapping, scrolling)
    }

    /// Validates text and presentation before constructing either fixed structural child.
    fn create_disclosure(&self, label: NcCopyFromBufferOffset<'_, u8>, style: DisclosureStyle, expanded: bool) -> Result<ComPtr<DisclosureAbi>, NcStatus> {
        crate::Disclosure::create_com(self.host.clone(), self.resources.clone(), label, style, expanded)
    }

    /// Composes real bars and a transform container around validated application content.
    fn create_scroll_area(&self, content: ComRef<'_, IWidgetAbi>, enabled: bool) -> Result<ComPtr<ScrollAreaAbi>, NcStatus> {
        crate::ScrollArea::create_com(self.host.clone(), self.resources.clone(), content, enabled)
    }

    /// Validates both policies before creating a container and its host-owned child store.
    fn create_linear(&self, direction: FlowDirection, cross: CrossSizeAbi) -> Result<ComPtr<LinearAbi>, NcStatus> {
        self.host.check_mutation()?;
        crate::Linear::create_com(self.host.clone(), self.resources.clone(), direction, cross)
    }

    /// Validates complete typed track sequences before allocating Grid state.
    fn create_grid(
        &self,
        columns: NcCopyFromBufferOffset<'_, LayoutTrackAbi>,
        rows: NcCopyFromBufferOffset<'_, LayoutTrackAbi>,
    ) -> Result<ComPtr<GridAbi>, NcStatus> {
        self.host.check_mutation()?;
        crate::Grid::create_com(self.host.clone(), self.resources.clone(), columns, rows)
    }

    /// Validates complete checkbox initialization before allocating its COM identity.
    fn create_checkbox(&self, checked: bool, label: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<CheckboxAbi>, NcStatus> {
        self.host.check_mutation()?;
        crate::Checkbox::create_com(self.host.clone(), self.resources.clone(), checked, label)
    }

    /// Validates complete textbox initialization before allocating its COM identity.
    fn create_textbox(&self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<TextboxAbi>, NcStatus> {
        self.host.check_mutation()?;
        crate::Textbox::create_com(self.host.clone(), self.resources.clone(), input)
    }

    /// Validates text and wrapping before allocating the display object's identity.
    fn create_text_block(&self, input: NcCopyFromBufferOffset<'_, u8>, wrapping: TextWrapping) -> Result<ComPtr<TextBlockAbi>, NcStatus> {
        self.host.check_mutation()?;
        crate::TextBlock::create_com(self.host.clone(), self.resources.clone(), input, wrapping)
    }

    /// Validates every arithmetic field before allocating a slider identity.
    fn create_slider(&self, options: SliderOptions) -> Result<ComPtr<SliderAbi>, NcStatus> {
        self.host.check_mutation()?;
        crate::Slider::create_com(self.host.clone(), self.resources.clone(), options)
    }

    /// Validates every arithmetic field before allocating a number identity.
    fn create_number(&self, options: NumberOptions) -> Result<ComPtr<NumberAbi>, NcStatus> {
        self.host.check_mutation()?;
        crate::Number::create_com(self.host.clone(), self.resources.clone(), options)
    }

    /// Validates the label before allocating a color swatch with copied RGBA state.
    fn create_color_swatch(&self, rgba: u32, label: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<ColorSwatchAbi>, NcStatus> {
        self.host.check_mutation()?;
        crate::ColorSwatch::create_com(self.host.clone(), self.resources.clone(), rgba, label)
    }

    /// Validates the label before allocating a standalone list-row identity.
    fn create_list_item(&self, label: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<ListItemAbi>, NcStatus> {
        self.host.check_mutation()?;
        crate::ListItem::create_com(self.host.clone(), self.resources.clone(), label)
    }

    /// Validates the label before allocating a button-like list entry.
    fn create_list_box(&self, label: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<ListBoxAbi>, NcStatus> {
        self.host.check_mutation()?;
        crate::ListBox::create_com(self.host.clone(), self.resources.clone(), label)
    }

    /// Validates axis and range arithmetic before allocating the scrollbar.
    fn create_scrollbar(&self, range: ScrollRange) -> Result<ComPtr<ScrollbarAbi>, NcStatus> {
        self.host.check_mutation()?;
        crate::Scrollbar::create_com(self.host.clone(), self.resources.clone(), range)
    }

    /// Constructs a complete Button in one call through its concrete class implementation.
    fn create_button(&self, label: NcCopyFromBufferOffset<'_, u8>, options: WidgetOptions) -> Result<ComPtr<ButtonAbi>, NcStatus> {
        self.host.check_mutation()?;
        crate::Button::create_com(self.host.clone(), self.resources.clone(), label, options)
    }
}
