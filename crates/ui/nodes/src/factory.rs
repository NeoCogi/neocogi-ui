//! Dynamically linked widget-construction shorthand over concrete COM contracts.

use neocogi_ui_core_contracts::*;
use neocogi_ui_nodes_contracts::*;
use neocogi_com::{ComPtr, ComRef, NcCopyFromBufferOffset, NcStatus, com_factory};
use std::rc::Rc;

/// Public-COM services and one shared imported resource cache used by all standard node products.
pub(crate) struct NodeServices {
    /// Non-owning tree authority used by every concrete node allocation.
    pub(crate) host: ComPtr<IWidgetHostAbi>,
    /// One imported asset cache shared across products from this bound factory.
    pub(crate) resources: crate::widget_object::SharedAssets,
    /// Core-owned selection-menu construction used by Combo without private runtime access.
    pub(crate) choices: ComPtr<IChoiceMenusAbi>,
    /// Public window capability used by standard mounted compositions such as FileDialog.
    pub(crate) windows: ComPtr<IWindowManagerAbi>,
}

impl NodeServices {
    /// Imports one initial resource revision and captures every public service required by built-ins.
    pub(crate) fn bind(runtime: ComRef<'_, UiRuntimeAbi>) -> Result<Rc<Self>, NcStatus> {
        let construction = runtime.construction_host()?;
        let host = construction.host()?;
        host.check_open()?;
        let source = host.assets()?;
        let resources = Rc::new(std::cell::RefCell::new(crate::assets::ImportedAssets::read(source.as_com_ref(), None)?));
        Ok(Rc::new(Self {
            host,
            resources,
            choices: runtime.add_ref().query()?,
            windows: runtime.add_ref().query()?,
        }))
    }
}

#[com_factory(neocogi_ui_nodes_contracts::UiFactory)]
/// Dynamically linked construction shorthand retaining its products' host and one asset value.
pub struct UiFactory {
    /// Cohesive typed dependencies captured once when this provider binds to a runtime.
    services: Rc<NodeServices>,
}

impl UiFactory {
    /// Binds the independently deployed node provider through public core COM capabilities only.
    pub fn bind(runtime: ComRef<'_, UiRuntimeAbi>) -> Result<ComPtr<UiFactoryAbi>, NcStatus> {
        let services = NodeServices::bind(runtime)?;
        let value = Self { services };
        Ok(Self::from_value(value).query().expect("UiFactory declares its callable class contract"))
    }
}

impl IConstructionHost for UiFactory {
    /// Returns the exact host used by every product without retaining a UI tree.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        self.services.host.check_open()?;
        Ok(self.services.host.clone())
    }
}

impl IFileDialogs for UiFactory {
    /// Creates the standard picker as an eventful mounted composite owned by the nodes plugin.
    fn create(&self, parent: ComRef<'_, IWindowAbi>, events: neocogi_com::NcValueRef<'_, IFileDialogEventBinding>) -> Result<neocogi_com::WeakComBox<IFileDialogAbi>, NcStatus> {
        crate::dialogs::create(self.services.clone(), parent, events.to_owned()?.into_queue())
    }
}

impl neocogi_ui_nodes_contracts::UiFactory for UiFactory {
    /// Validates complete text and wrapping before constructing the fixed scroll/editor composition.
    fn create_text_area(
        &self,
        input: NcCopyFromBufferOffset<'_, u8>,
        wrapping: TextWrapping,
        scrolling: bool,
        events: neocogi_com::NcValueRef<'_, TextAreaEventBinding>,
    ) -> Result<neocogi_com::ComBox<TextAreaAbi>, NcStatus> {
        crate::TextArea::create_com(
            self.services.host.clone(),
            self.services.resources.clone(),
            input,
            wrapping,
            scrolling,
            events.to_owned()?.into_queue(),
        )
    }

    /// Validates text and presentation before constructing either fixed structural child.
    fn create_disclosure(&self, label: NcCopyFromBufferOffset<'_, u8>, style: DisclosureStyle, expanded: bool) -> Result<neocogi_com::ComBox<DisclosureAbi>, NcStatus> {
        crate::Disclosure::create_com(self.services.host.clone(), self.services.resources.clone(), label, style, expanded)
    }

    /// Composes real bars and a transform container around validated application content.
    fn create_scroll_area(
        &self,
        content: neocogi_com::ComBoxInput<'_, IWidgetAbi>,
        enabled: bool,
    ) -> Result<neocogi_com::ComBox<ScrollAreaAbi>, NcStatus> {
        crate::ScrollArea::create_com(self.services.host.clone(), self.services.resources.clone(), content, enabled)
    }

    /// Validates both policies before creating a container and its host-owned child store.
    fn create_linear(&self, direction: FlowDirection, cross: CrossSize) -> Result<neocogi_com::ComBox<LinearAbi>, NcStatus> {
        self.services.host.check_mutation()?;
        crate::Linear::create_com(self.services.host.clone(), self.services.resources.clone(), direction, cross)
    }

    /// Validates sizing before allocating the split container and its sole child editor.
    fn create_split_view(&self, settings: SplitOptions, events: neocogi_com::NcValueRef<'_, SplitViewEventBinding>) -> Result<neocogi_com::ComBox<SplitViewAbi>, NcStatus> {
        self.services.host.check_mutation()?;
        crate::SplitView::create_com(
            self.services.host.clone(),
            self.services.resources.clone(),
            settings,
            events.to_owned()?.into_queue(),
        )
    }

    /// Creates the fixed-coordinate container through the same host as other nodes.
    fn create_form(&self, size: Size) -> Result<neocogi_com::ComBox<FormAbi>, NcStatus> {
        self.services.host.check_mutation()?;
        crate::Form::create_com(self.services.host.clone(), self.services.resources.clone(), size)
    }

    /// Validates complete typed track sequences before allocating Grid state.
    fn create_grid(
        &self,
        columns: NcCopyFromBufferOffset<'_, LayoutTrack>,
        rows: NcCopyFromBufferOffset<'_, LayoutTrack>,
    ) -> Result<neocogi_com::ComBox<GridAbi>, NcStatus> {
        self.services.host.check_mutation()?;
        crate::Grid::create_com(self.services.host.clone(), self.services.resources.clone(), columns, rows)
    }

    /// Validates complete checkbox initialization before allocating its COM identity.
    fn create_checkbox(
        &self,
        checked: bool,
        label: NcCopyFromBufferOffset<'_, u8>,
        events: neocogi_com::NcValueRef<'_, CheckboxEventBinding>,
    ) -> Result<neocogi_com::ComBox<CheckboxAbi>, NcStatus> {
        self.services.host.check_mutation()?;
        crate::Checkbox::create_com(
            self.services.host.clone(),
            self.services.resources.clone(),
            checked,
            label,
            events.to_owned()?.into_queue(),
        )
    }

    /// Validates complete textbox initialization before allocating its COM identity.
    fn create_textbox(
        &self,
        input: NcCopyFromBufferOffset<'_, u8>,
        events: neocogi_com::NcValueRef<'_, TextboxEventBinding>,
    ) -> Result<neocogi_com::ComBox<TextboxAbi>, NcStatus> {
        self.services.host.check_mutation()?;
        crate::Textbox::create_com(
            self.services.host.clone(),
            self.services.resources.clone(),
            input,
            events.to_owned()?.into_queue(),
        )
    }

    /// Validates text and wrapping before allocating the display object's identity.
    fn create_text_block(&self, input: NcCopyFromBufferOffset<'_, u8>, wrapping: TextWrapping) -> Result<neocogi_com::ComBox<TextBlockAbi>, NcStatus> {
        self.services.host.check_mutation()?;
        crate::TextBlock::create_com(self.services.host.clone(), self.services.resources.clone(), input, wrapping)
    }

    /// Validates every arithmetic field before allocating a slider identity.
    fn create_slider(&self, options: SliderOptions, events: neocogi_com::NcValueRef<'_, SliderEventBinding>) -> Result<neocogi_com::ComBox<SliderAbi>, NcStatus> {
        self.services.host.check_mutation()?;
        crate::Slider::create_com(
            self.services.host.clone(),
            self.services.resources.clone(),
            options,
            events.to_owned()?.into_queue(),
        )
    }

    /// Validates every arithmetic field before allocating a number identity.
    fn create_number(&self, options: NumberOptions, events: neocogi_com::NcValueRef<'_, NumberEventBinding>) -> Result<neocogi_com::ComBox<NumberAbi>, NcStatus> {
        self.services.host.check_mutation()?;
        crate::Number::create_com(
            self.services.host.clone(),
            self.services.resources.clone(),
            options,
            events.to_owned()?.into_queue(),
        )
    }

    /// Validates the label before allocating a color swatch with copied RGBA state.
    fn create_color_swatch(&self, rgba: u32, label: NcCopyFromBufferOffset<'_, u8>) -> Result<neocogi_com::ComBox<ColorSwatchAbi>, NcStatus> {
        self.services.host.check_mutation()?;
        crate::ColorSwatch::create_com(self.services.host.clone(), self.services.resources.clone(), rgba, label)
    }

    /// Validates the label before allocating a standalone list-row identity.
    fn create_list_item(
        &self,
        label: NcCopyFromBufferOffset<'_, u8>,
        events: neocogi_com::NcValueRef<'_, ListItemEventBinding>,
    ) -> Result<neocogi_com::ComBox<ListItemAbi>, NcStatus> {
        self.services.host.check_mutation()?;
        crate::ListItem::create_com(
            self.services.host.clone(),
            self.services.resources.clone(),
            label,
            events.to_owned()?.into_queue(),
        )
    }

    /// Validates the label before allocating a button-like list entry.
    fn create_list_box(
        &self,
        label: NcCopyFromBufferOffset<'_, u8>,
        events: neocogi_com::NcValueRef<'_, ListBoxEventBinding>,
    ) -> Result<neocogi_com::ComBox<ListBoxAbi>, NcStatus> {
        self.services.host.check_mutation()?;
        crate::ListBox::create_com(
            self.services.host.clone(),
            self.services.resources.clone(),
            label,
            events.to_owned()?.into_queue(),
        )
    }

    /// Validates axis and range arithmetic before allocating the scrollbar.
    fn create_scrollbar(&self, range: ScrollRange, events: neocogi_com::NcValueRef<'_, ScrollbarEventBinding>) -> Result<neocogi_com::ComBox<ScrollbarAbi>, NcStatus> {
        self.services.host.check_mutation()?;
        crate::Scrollbar::create_com(
            self.services.host.clone(),
            self.services.resources.clone(),
            range,
            events.to_owned()?.into_queue(),
        )
    }

    /// Constructs a complete Button in one call through its concrete class implementation.
    fn create_button(
        &self,
        label: NcCopyFromBufferOffset<'_, u8>,
        options: WidgetOptions,
        events: neocogi_com::NcValueRef<'_, ButtonEventBinding>,
    ) -> Result<neocogi_com::ComBox<ButtonAbi>, NcStatus> {
        self.services.host.check_mutation()?;
        crate::Button::create_com(
            self.services.host.clone(),
            self.services.resources.clone(),
            label,
            options,
            events.to_owned()?.into_queue(),
        )
    }

    /// Binds the concrete combo model to this factory's live window-owning runtime.
    fn create_combo_box(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        menu_text: NcCopyFromBufferOffset<'_, u8>,
        entries: NcCopyFromBufferOffset<'_, ChoiceMenuEntry>,
        selection: ChoiceSelection,
        events: neocogi_com::NcValueRef<'_, ComboEventBinding>,
    ) -> Result<neocogi_com::ComBox<ComboAbi>, NcStatus> {
        self.services.host.check_mutation()?;
        let forwarder = events
            .to_owned()?
            .into_queue()
            .map(|queue| crate::widgets::ComboEventForwarder::into_local(crate::widgets::ComboEventForwarder { queue, origin: None }));
        let receiver = forwarder.as_ref().map(|forwarder| forwarder.query::<IChoiceMenuEventQueueAbi>()).transpose()?;
        let binding = IChoiceMenuEventBinding::from_queue(receiver.as_ref().map(ComPtr::as_com_ref));
        let menu = self
            .services
            .choices
            .create_choice_menu(parent, menu_text, entries, selection, neocogi_com::NcValueRef::new(&binding)?)?;
        crate::Combo::create_com(
            self.services.host.clone(),
            self.services.resources.clone(),
            menu,
            crate::widgets::DEFAULT_COMBO_PLACEHOLDER.to_owned(),
            forwarder,
        )
    }
}
