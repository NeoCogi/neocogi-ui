//! Concrete factory mappings and ownership cleanup for unpublished candidates.
use crate::model::*;
use crate::Resources;
use neocogi_com::{ComPtr, ComInterface, NcStatus, NcCopyFromBufferOffset, NC_E_INVALID_ARG, NC_E_NOT_FOUND};
use neocogi_ui::*;
use std::collections::BTreeMap;

/// Owns all created nodes until construction transfers them to the loaded component.
/// Rejected candidates are created by this builder and cannot own its call receiver;
/// discarding their returned owners abandons unpublished construction safely.
pub(crate) struct Construction<'a> {
    /// Explicitly registered custom COM factories.
    pub custom: &'a crate::CustomWidgetRegistry,
    /// Existing runtime and independently deployed built-in factory.
    pub ui: &'a Ui,
    /// Required only by window-owned combo popups.
    pub parent: Option<&'a ComPtr<IWindowAbi>>,
    /// Explicit logical image directory supplied by the caller.
    pub resources: &'a Resources,
    /// Name-to-capability index exists only inside the component implementation.
    pub nodes: BTreeMap<String, Widget<IWidgetAbi>>,
    /// Receivers are consumed exactly once before each node is published.
    pub events: crate::EventBindings,
}
/// Erases only the concrete widget projection through checked QueryInterface.
fn root<T: ComInterface>(widget: OwnedWidget<T>) -> Result<OwnedWidget<IWidgetAbi>, NcStatus> {
    Ok(OwnedWidget::from_interface(widget.into_interface().query().map_err(|(status, _unpublished)| status)?))
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
    pub fn build(&mut self, node: &Node) -> Result<OwnedWidget<IWidgetAbi>, NcStatus> {
        let mut children = node.children.iter().map(|child| self.build(&child.node)).collect::<Result<Vec<_>, _>>()?;
        let factory = self.ui.widgets();
        let queue = self.events.take(&node.key);
        let mut widget = match &node.widget {
            WidgetSpec::Button { text } => {
                root(factory.button_with_queue(text, queue.as_ref().map(|queue| queue.query()).transpose()?.as_ref().map(ComPtr::as_com_ref))?)?
            }
            WidgetSpec::Checkbox { text, checked } => root(factory.checkbox_with_queue(
                text,
                *checked,
                queue.as_ref().map(|queue| queue.query()).transpose()?.as_ref().map(ComPtr::as_com_ref),
            )?)?,
            WidgetSpec::Textbox { text } => {
                root(factory.textbox_with_queue(text, queue.as_ref().map(|queue| queue.query()).transpose()?.as_ref().map(ComPtr::as_com_ref))?)?
            }
            WidgetSpec::TextBlock { text, wrapping: policy } => root(factory.text_block(text, wrapping(policy))?)?,
            WidgetSpec::TextArea { text, wrapping: policy, scrolling } => root(factory.text_area_with_queue(
                text,
                wrapping(policy),
                *scrolling,
                queue.as_ref().map(|queue| queue.query()).transpose()?.as_ref().map(ComPtr::as_com_ref),
            )?)?,
            WidgetSpec::Slider { value, low, high, step, precision } => root(factory.slider_with_queue(
                SliderOptions {
                    value: *value,
                    low: *low,
                    high: *high,
                    step: *step,
                    precision: *precision,
                },
                queue.as_ref().map(|queue| queue.query()).transpose()?.as_ref().map(ComPtr::as_com_ref),
            )?)?,
            WidgetSpec::Number { value, step, precision } => root(factory.number_with_queue(
                NumberOptions {
                    value: *value,
                    step: *step,
                    precision: *precision,
                },
                queue.as_ref().map(|queue| queue.query()).transpose()?.as_ref().map(ComPtr::as_com_ref),
            )?)?,
            WidgetSpec::ColorSwatch { text, rgba } => root(factory.color_swatch(*rgba, text)?)?,
            WidgetSpec::ListItem { text } => {
                root(factory.list_item_with_queue(text, queue.as_ref().map(|queue| queue.query()).transpose()?.as_ref().map(ComPtr::as_com_ref))?)?
            }
            WidgetSpec::ListBox { text } => {
                root(factory.list_box_with_queue(text, queue.as_ref().map(|queue| queue.query()).transpose()?.as_ref().map(ComPtr::as_com_ref))?)?
            }
            WidgetSpec::Scrollbar { vertical, viewport, content, offset } => root(factory.scrollbar_with_queue(
                ScrollRange {
                    axis: if *vertical { ScrollAxis::Vertical } else { ScrollAxis::Horizontal },
                    viewport: *viewport,
                    content: *content,
                    offset: *offset,
                },
                queue.as_ref().map(|queue| queue.query()).transpose()?.as_ref().map(ComPtr::as_com_ref),
            )?)?,
            WidgetSpec::Combo { labels, selected, placeholder } => {
                let parent = self.parent.ok_or(NC_E_INVALID_ARG)?;
                let mut combo = self.ui.combo_box_with_queue(
                    parent.as_com_ref(),
                    labels,
                    selection(*selected),
                    queue.as_ref().map(|queue| queue.query()).transpose()?.as_ref().map(ComPtr::as_com_ref),
                )?;
                combo.set_placeholder(placeholder)?;
                root(combo)?
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
            } => root(factory.split_view_with_queue(
                SplitOptions {
                    axis: if *vertical { SplitAxis::Vertical } else { SplitAxis::Horizontal },
                    position: *position,
                    minimum_first: *minimum_first,
                    minimum_second: *minimum_second,
                    handle_width: *handle,
                },
                queue.as_ref().map(|queue| queue.query()).transpose()?.as_ref().map(ComPtr::as_com_ref),
            )?)?,
            WidgetSpec::Scroll { enabled } => {
                // The scroll constructor consumes its sole child before ordinary
                // indexed container insertion below; no second owner is retained.
                if children.len() != 1 {
                    return Err(NC_E_INVALID_ARG);
                }
                root(factory.scroll_area(children.remove(0), *enabled).map_err(|(status, _unpublished)| status)?)?
            },
            WidgetSpec::Disclosure { text, tree, expanded } => {
                root(factory.disclosure(text, if *tree { DisclosureStyle::Tree } else { DisclosureStyle::Header }, *expanded)?)?
            }
            WidgetSpec::Custom { kind, properties, .. } => self.custom.create(kind, self.ui, properties, queue)?,
        };
        self.nodes.insert(node.key.clone(), widget.handle());
        for (index, (child, relationship)) in children.into_iter().zip(&node.children).enumerate() {
            let index = index as u32;
            match (&node.widget, &relationship.edge) {
                (WidgetSpec::Linear { .. } | WidgetSpec::Disclosure { .. }, Edge::Linear { main, cross }) => {
                    let mut parent = match node.widget {
                        WidgetSpec::Disclosure { .. } => widget.as_interface().observe::<DisclosureAbi>()?.body()?,
                        _ => widget.as_interface().observe::<LinearAbi>()?,
                    };
                    let edge = LinearEdge {
                        main: main.layout().into(),
                        cross: match cross {
                            Some(extent) => ChildCrossSize::Fixed { extent: *extent },
                            None => ChildCrossSize::Stretch,
                        }
                        .into(),
                    };
                    parent.insert(index, child.into_interface(), edge).map_err(|(status, _unpublished)| status)?;
                }
                (WidgetSpec::Grid { .. }, Edge::Grid { columns, rows }) => widget
                    .as_interface()
                    .observe::<GridAbi>()?
                    .insert(index, child.into_interface(), GridExtent { columns: *columns, rows: *rows }).map_err(|(status, _unpublished)| status)?,
                (WidgetSpec::Form { .. }, Edge::Position { x, y, width, height }) => widget
                    .as_interface()
                    .observe::<FormAbi>()?
                    .insert(
                        index,
                        child.into_interface(),
                        Rect {
                            x: *x,
                            y: *y,
                            width: *width,
                            height: *height,
                        },
                    ).map_err(|(status, _unpublished)| status)?,
                (WidgetSpec::Split { .. }, Edge::Pane) => widget
                    .as_interface()
                    .observe::<SplitViewAbi>()?
                    .insert(index, child.into_interface()).map_err(|(status, _unpublished)| status)?,
                (WidgetSpec::Scroll { .. }, Edge::Pane) => {} // The constructor already adopted its sole child.
                _ => return Err(NC_E_INVALID_ARG),
            }
        }
        widget.replace_flags(node.style.options.map(WidgetOptions::from_raw).unwrap_or_else(|| node.widget.default_options()))?;
        apply_style(&widget, node, self.resources)?;
        Ok(widget)
    }
}

/// Applies logical resources through the concrete class that owns each property.
fn apply_style(widget: &OwnedWidget<IWidgetAbi>, node: &Node, resources: &Resources) -> Result<(), NcStatus> {
    let source = widget.as_interface().downgrade();
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
