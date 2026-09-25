//! Concrete factory mappings and ownership cleanup for unpublished candidates.
use crate::model::*;
use crate::{Resources, wiring::{Message, PublicEmitter, WidgetEvent}};
use neocogi_com::{ComPtr, ComInterface, NcStatus, NcCopyFromBufferOffset, NC_E_INVALID_ARG, NC_E_NOT_FOUND};
use neocogi_ui::*;
use std::{collections::BTreeMap, sync::mpsc::Sender};

/// Owns all created nodes until construction transfers them to the loaded component.
pub(crate) struct Construction<'a> {
    /// Optional authored aliases for a widget event stream.
    pub declarations: &'a [crate::model::Output],
    /// Each observed child sends its complete typed event to the form's one inbox.
    pub sender: Sender<Message>,
    /// Form-owned public relays; none is passed to a child widget.
    pub public: BTreeMap<String, PublicEmitter>,
    /// Component-owned application bindings, separate from widget lifetimes.
    pub outputs: neocogi_com::events::NamedOutputs,
    /// Existing runtime and independently deployed built-in factory.
    pub ui: &'a Ui,
    /// Required only by window-owned combo popups.
    pub parent: Option<&'a ComPtr<IWindowAbi>>,
    /// Explicit logical image directory supplied by the caller.
    pub resources: &'a Resources,
    /// Name-to-capability index exists only inside the component implementation.
    pub nodes: BTreeMap<String, Widget<IWidgetAbi>>,
}
impl Drop for Construction<'_> {
    /// Failed builds close retained partial topology, including window-owned popup controls.
    fn drop(&mut self) {
        for node in self.nodes.values() {
            if let Ok(mut node) = node.node_interface() {
                let _ = node.close();
            }
        }
    }
}
/// Erases only the concrete widget projection through checked QueryInterface.
fn root<T: ComInterface>(widget: Widget<T>) -> Result<Widget<IWidgetAbi>, NcStatus> {
    Ok(Widget::from_interface(widget.node_interface()?))
}
/// Turns a persistent text policy into the existing runtime enum.
fn wrapping(value: &Wrapping) -> TextWrapping {
    match value {
        Wrapping::None => TextWrapping::None,
        Wrapping::Word => TextWrapping::Word,
        Wrapping::Character => TextWrapping::WordOrCharacter,
    }
}
/// Converts optional selection after the document's range validation.
pub(crate) fn selection(value: Option<u32>) -> ChoiceSelection {
    match value {
        Some(ordinal) => ChoiceSelection::Selected(u64::from(ordinal)),
        None => ChoiceSelection::None,
    }
}
impl Construction<'_> {
    /// Constructs descendants first and then adopts each into its actual parent.
    pub fn build(&mut self, node: &Node) -> Result<Widget<IWidgetAbi>, NcStatus> {
        let children = node.children.iter().map(|child| self.build(&child.node)).collect::<Result<Vec<_>, _>>()?;
        let factory = self.ui.widgets();
        // Each emitting child receives one adapter at construction. Application
        // handlers run later, after the runtime traversal releases widget borrows.
        macro_rules! create {
            ($factory:expr, $method:ident, $event:ty, $kind:ident, $($argument:expr),* $(,)?) => {{
                let exported = self.declarations.iter().find(|output| output.node == node.key);
                let observer = {
                    let sender = self.sender.clone();
                    let source = node.key.clone();
                    Some(<$event>::observer(move |event| {
                        let event = WidgetEvent::$kind(event);
                        event.validate()?;
                        // A retired form may disconnect normally; no child call becomes an error.
                        let _ = sender.send(Message { source: source.clone(), event });
                        Ok(())
                    })?)
                };
                let (proxy, bindings) = <$event>::exported_observer()?;
                let name = exported.map(|output| output.name.as_str()).unwrap_or(node.key.as_str());
                self.outputs.export_component(name, bindings, node.widget.outputs()[0].name)?;
                self.public.insert(node.key.clone(), PublicEmitter::$kind(Some(proxy)));
                $factory.$method($($argument,)* observer.as_ref().map(|value| value.as_com_ref()))?
            }};
        }
        let mut widget = match &node.widget {
            WidgetSpec::Button { text } => root(create!(factory, button_with_observer, nodes::ButtonEvent, Button, text))?,
            WidgetSpec::Checkbox { text, checked } => root(create!(factory, checkbox_with_observer, nodes::CheckboxEvent, Checkbox, text, *checked))?,
            WidgetSpec::Textbox { text } => root(create!(factory, textbox_with_observer, nodes::TextboxEvent, Textbox, text))?,
            WidgetSpec::TextBlock { text, wrapping: policy } => root(factory.text_block(text, wrapping(policy))?)?,
            WidgetSpec::TextArea { text, wrapping: policy, scrolling } => root(create!(
                factory,
                text_area_with_observer,
                nodes::TextAreaEvent, TextArea,
                text,
                wrapping(policy),
                *scrolling
            ))?,
            WidgetSpec::Slider { value, low, high, step, precision } => root(create!(
                factory,
                slider_with_observer,
                nodes::SliderEvent, Slider,
                SliderOptions {
                    value: *value,
                    low: *low,
                    high: *high,
                    step: *step,
                    precision: *precision,
                }
            ))?,
            WidgetSpec::Number { value, step, precision } => root(create!(
                factory,
                number_with_observer,
                nodes::NumberEvent, Number,
                NumberOptions {
                    value: *value,
                    step: *step,
                    precision: *precision,
                }
            ))?,
            WidgetSpec::ColorSwatch { text, rgba } => root(factory.color_swatch(*rgba, text)?)?,
            WidgetSpec::ListItem { text } => root(create!(factory, list_item_with_observer, nodes::ListItemEvent, ListItem, text))?,
            WidgetSpec::ListBox { text } => root(create!(factory, list_box_with_observer, nodes::ListBoxEvent, ListBox, text))?,
            WidgetSpec::Scrollbar { vertical, viewport, content, offset } => root(create!(
                factory,
                scrollbar_with_observer,
                nodes::ScrollbarEvent, Scrollbar,
                ScrollRange {
                    axis: if *vertical { ScrollAxis::Vertical } else { ScrollAxis::Horizontal },
                    viewport: *viewport,
                    content: *content,
                    offset: *offset,
                }
            ))?,
            WidgetSpec::Combo { labels, selected, placeholder } => {
                let parent = self.parent.ok_or(NC_E_INVALID_ARG)?;
                let mut combo = create!(
                    self.ui,
                    combo_box_with_observer,
                    IChoiceMenuEvent, Combo,
                    parent.as_com_ref(),
                    labels,
                    selection(*selected)
                );
                // Retain the root before further fallible setters so cleanup also removes its popup.
                let widget = root(combo.clone())?;
                self.nodes.insert(node.key.clone(), widget.clone());
                combo.set_placeholder(placeholder)?;
                widget
            }
            WidgetSpec::Form { width, height } => root(factory.form(Size { width: *width, height: *height })?)?,
            WidgetSpec::Linear { direction, cross } => {
                let direction = match direction {
                    Direction::Right => FlowDirection::LeftToRight,
                    Direction::Left => FlowDirection::RightToLeft,
                    Direction::Down => FlowDirection::TopToBottom,
                    Direction::Up => FlowDirection::BottomToTop,
                };
                let cross = match cross {
                    Cross::Content => CrossSize::Content,
                    Cross::Stretch => CrossSize::Stretch,
                    Cross::Fixed { pixels } => CrossSize::Fixed { extent: *pixels },
                };
                root(factory.linear(direction, cross)?)?
            }
            WidgetSpec::Grid { columns, rows } => root(factory.grid(
                &columns.iter().map(Track::layout).collect::<Vec<_>>(),
                &rows.iter().map(Track::layout).collect::<Vec<_>>(),
            )?)?,
            WidgetSpec::Split {
                vertical,
                position,
                minimum_first,
                minimum_second,
                handle,
            } => root(create!(
                factory,
                split_view_with_observer,
                nodes::SplitViewEvent, Split,
                SplitOptions {
                    axis: if *vertical { SplitAxis::Vertical } else { SplitAxis::Horizontal },
                    position: *position,
                    minimum_first: *minimum_first,
                    minimum_second: *minimum_second,
                    handle_width: *handle,
                }
            ))?,
            WidgetSpec::Scroll { enabled } => root(factory.scroll_area(children.first().ok_or(NC_E_INVALID_ARG)?, *enabled)?)?,
            WidgetSpec::Disclosure { text, tree, expanded } => {
                root(factory.disclosure(text, if *tree { DisclosureStyle::Tree } else { DisclosureStyle::Header }, *expanded)?)?
            }
        };
        self.nodes.insert(node.key.clone(), widget.clone());
        for (index, (child, relationship)) in children.iter().zip(&node.children).enumerate() {
            let index = index as u32;
            match (&node.widget, &relationship.edge) {
                (WidgetSpec::Linear { .. } | WidgetSpec::Disclosure { .. }, Edge::Linear { main, cross }) => {
                    let mut parent = match node.widget {
                        WidgetSpec::Disclosure { .. } => widget.as_interface().add_ref().query::<DisclosureAbi>()?.body()?,
                        _ => widget.as_interface().add_ref().query::<LinearAbi>()?,
                    };
                    let edge = LinearEdge {
                        main: main.layout().into(),
                        cross: match cross {
                            Some(extent) => ChildCrossSize::Fixed { extent: *extent },
                            None => ChildCrossSize::Stretch,
                        }
                        .into(),
                    };
                    parent
                        .insert(index, neocogi_ui::Mount::widget(child.as_interface()).as_message(), edge)?
                        .finish()?;
                }
                (WidgetSpec::Grid { .. }, Edge::Grid { columns, rows }) => widget
                    .as_interface()
                    .add_ref()
                    .query::<GridAbi>()?
                    .insert(
                        index,
                        neocogi_ui::Mount::widget(child.as_interface()).as_message(),
                        GridExtent { columns: *columns, rows: *rows },
                    )?
                    .finish()?,
                (WidgetSpec::Form { .. }, Edge::Position { x, y, width, height }) => widget
                    .as_interface()
                    .add_ref()
                    .query::<FormAbi>()?
                    .insert(
                        index,
                        neocogi_ui::Mount::widget(child.as_interface()).as_message(),
                        Rect {
                            x: *x,
                            y: *y,
                            width: *width,
                            height: *height,
                        },
                    )?
                    .finish()?,
                (WidgetSpec::Split { .. }, Edge::Pane) => widget
                    .as_interface()
                    .add_ref()
                    .query::<SplitViewAbi>()?
                    .insert(index, neocogi_ui::Mount::widget(child.as_interface()).as_message())?
                    .finish()?,
                (WidgetSpec::Scroll { .. }, Edge::Pane) => {} // The constructor already adopted its sole child.
                _ => return Err(NC_E_INVALID_ARG),
            }
        }
        widget.set_options(node.style.options.map(WidgetOptions::from_raw).unwrap_or_else(|| node.widget.default_options()))?;
        apply_style(&widget, node, self.resources)?;
        Ok(widget)
    }
}

/// Applies logical resources through the concrete class that owns each property.
fn apply_style(widget: &Widget<IWidgetAbi>, node: &Node, resources: &Resources) -> Result<(), NcStatus> {
    let source = widget.as_interface().add_ref();
    if let Some(font) = &node.style.font {
        let bytes = NcCopyFromBufferOffset::new(0, font.as_bytes());
        macro_rules! font {
            ($kind:ty) => {
                source.query::<$kind>()?.set_named_font(bytes)?
            };
        }
        match node.widget {
            WidgetSpec::Button { .. } => font!(ButtonAbi),
            WidgetSpec::Checkbox { .. } => font!(CheckboxAbi),
            WidgetSpec::Textbox { .. } => font!(TextboxAbi),
            WidgetSpec::TextBlock { .. } => font!(TextBlockAbi),
            WidgetSpec::TextArea { .. } => font!(TextAreaAbi),
            WidgetSpec::Slider { .. } => font!(SliderAbi),
            WidgetSpec::Number { .. } => font!(NumberAbi),
            WidgetSpec::ColorSwatch { .. } => font!(ColorSwatchAbi),
            WidgetSpec::ListItem { .. } => font!(ListItemAbi),
            WidgetSpec::ListBox { .. } => font!(ListBoxAbi),
            WidgetSpec::Combo { .. } => font!(ComboAbi),
            WidgetSpec::Disclosure { .. } => source.query::<DisclosureAbi>()?.header()?.query::<IFontStyleAbi>()?.set_named_font(bytes)?,
            _ => return Err(NC_E_INVALID_ARG),
        }
    }
    if let Some(icon) = &node.style.icon {
        let bytes = NcCopyFromBufferOffset::new(0, icon.as_bytes());
        match node.widget {
            WidgetSpec::Button { .. } => source.query::<ButtonAbi>()?.set_named_icon(bytes)?,
            WidgetSpec::ListItem { .. } => source.query::<ListItemAbi>()?.set_named_icon(bytes)?,
            _ => return Err(NC_E_INVALID_ARG),
        }
    }
    if let Some(image) = &node.style.image {
        let image = resources.images.get(image).ok_or(NC_E_NOT_FOUND)?;
        match node.widget {
            WidgetSpec::Button { .. } => source.query::<ButtonAbi>()?.set_image(image.as_com_ref(), node.style.fit_image)?,
            WidgetSpec::ListBox { .. } => source.query::<ListBoxAbi>()?.set_image(image.as_com_ref(), node.style.fit_image)?,
            _ => return Err(NC_E_INVALID_ARG),
        }
    }
    Ok(())
}
