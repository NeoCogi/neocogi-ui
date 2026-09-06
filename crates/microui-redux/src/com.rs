//! Typed static clients for dynamically or statically linked COM implementations.
//!
//! Clients keep no shadow widget state or topology. Every edit crosses the
//! checked interface and every returned snapshot owns its storage. The retained implementation
//! lives in the UI plugin; this facade depends only on the contracts and COM
//! object support.

pub use microui_contracts::*;
use nc_com::{ComInterface, ComPtr, ComRef, INcStringComPtrExt, NcCopyFromBufferOffset, NcStatus};
mod containers;
pub use containers::{Linear, Grid, ScrollArea, Disclosure};

/// Generic static proxy for one concrete widget editing capability.
pub struct Widget<I: ComInterface> {
    /// One checked, owned reference to the actual plugin object.
    interface: ComPtr<I>,
}

impl<I: ComInterface> Clone for Widget<I> {
    /// Clones the COM reference without copying widget state or topology.
    fn clone(&self) -> Self {
        Self { interface: self.interface.clone() }
    }
}

impl<I: ComInterface> Widget<I> {
    /// Adopts a reference obtained through checked activation or QueryInterface.
    pub fn from_interface(interface: ComPtr<I>) -> Self {
        Self { interface }
    }

    /// Borrows the exact typed editing interface without changing ownership.
    pub fn as_interface(&self) -> ComRef<'_, I> {
        self.interface.as_com_ref()
    }

    /// Replaces common options through the same widget object used by the runtime.
    pub fn set_options(&self, options: WidgetOptions) -> Result<(), NcStatus> {
        self.node_interface()?.set_options(options)
    }

    /// Obtains the common node capability for attachment; callers cache this at mount.
    pub fn node_interface(&self) -> Result<ComPtr<IWidgetAbi>, NcStatus> {
        // Querying preserves object identity and validates the full projected ABI.
        self.interface.query()
    }
}

/// Button proxy whose concrete methods remain statically discoverable.
pub type Button = Widget<IButtonAbi>;

impl Widget<IButtonAbi> {
    /// Replaces the label with a synchronous UTF-8 copy owned by the plugin.
    pub fn set_label(&self, label: &str) -> Result<(), NcStatus> {
        self.interface.set_label(NcCopyFromBufferOffset::new(0, label.as_bytes()))
    }

    /// Copies an immutable plugin snapshot into an ordinary application String.
    pub fn label(&self) -> Result<String, NcStatus> {
        self.interface.label()?.to_string()
    }

    /// Connects a concrete typed sink and returns the caller-owned subscription.
    /// Retain this lease in the application event session; the runtime never owns
    /// an application handler. A second live subscriber is rejected by the source.
    pub fn subscribe(&self, sink: ComRef<'_, IActivationSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.interface.query::<IActivationEventsAbi>()?.subscribe(sink)
    }
}

/// Static Checkbox proxy; semantic storage belongs to its plugin object.
pub type Checkbox = Widget<ICheckboxAbi>;
/// Static Textbox proxy; semantic storage belongs to its plugin object.
pub type Textbox = Widget<ITextboxAbi>;
/// Static proxy for the fixed scrolling editor; inherited text edits reach its actual content leaf.
pub type TextArea = Widget<ITextAreaAbi>;
/// Static TextBlock proxy; semantic storage belongs to its plugin object.
pub type TextBlock = Widget<ITextBlockAbi>;
/// Static Slider proxy; semantic storage belongs to its plugin object.
pub type Slider = Widget<ISliderAbi>;
/// Static Number proxy; semantic storage belongs to its plugin object.
pub type Number = Widget<INumberAbi>;
/// Static ColorSwatch proxy; semantic storage belongs to its plugin object.
pub type ColorSwatch = Widget<IColorSwatchAbi>;
/// Static ListItem proxy; semantic storage belongs to its plugin object.
pub type ListItem = Widget<IListItemAbi>;
/// Static ListBox proxy; semantic storage belongs to its plugin object.
pub type ListBox = Widget<IListBoxAbi>;
/// Static Scrollbar proxy; semantic storage belongs to its plugin object.
pub type Scrollbar = Widget<IScrollbarAbi>;

/// Shares inherited text convenience calls while keeping every proxy concrete.
macro_rules! text_proxy {
    ($interface:ty) => {
        impl Widget<$interface> {
            /// Copies the current immutable text snapshot into application storage.
            pub fn text(&self) -> Result<String, NcStatus> {
                self.interface.text()?.to_string()
            }
            /// Replaces text through the control's documented normalization policy.
            pub fn set_text(&self, text: &str) -> Result<(), NcStatus> {
                self.interface.set_text(NcCopyFromBufferOffset::new(0, text.as_bytes()))
            }
            /// Clears text using the same cursor and invalidation rules as replacement.
            pub fn clear(&self) -> Result<(), NcStatus> {
                self.set_text("")
            }
        }
    };
}
text_proxy!(ITextContentAbi);
text_proxy!(ITextboxAbi);
text_proxy!(ITextAreaAbi);
text_proxy!(IMultilineEditorAbi);
text_proxy!(ITextBlockAbi);
text_proxy!(IColorSwatchAbi);
text_proxy!(IListItemAbi);
text_proxy!(IListBoxAbi);

impl Widget<ICheckboxAbi> {
    /// Reads the current checked value.
    pub fn checked(&self) -> Result<bool, NcStatus> {
        self.interface.checked()
    }
    /// Changes checked state silently without invalidating fixed checkbox geometry.
    pub fn set_checked(&self, checked: bool) -> Result<(), NcStatus> {
        self.interface.set_checked(checked)
    }
    /// Copies the current label snapshot.
    pub fn label(&self) -> Result<String, NcStatus> {
        self.interface.label()?.to_string()
    }
    /// Replaces label text and records a sticky preferred-size change when needed.
    pub fn set_label(&self, label: &str) -> Result<(), NcStatus> {
        self.interface.set_label(NcCopyFromBufferOffset::new(0, label.as_bytes()))
    }
}
/// Shares byte-cursor editing conveniences across concrete single-line and multiline capabilities.
macro_rules! editor_proxy {
    ($interface:ty) => {
        impl Widget<$interface> {
            /// Returns the Unicode scalar-boundary byte cursor.
            pub fn cursor(&self) -> Result<u64, NcStatus> {
                self.interface.cursor()
            }
            /// Clamps to text length and the preceding Unicode scalar boundary.
            pub fn set_cursor(&self, cursor: u64) -> Result<(), NcStatus> {
                self.interface.set_cursor(cursor)
            }
        }
    };
}
editor_proxy!(ITextboxAbi);
editor_proxy!(ITextAreaAbi);
editor_proxy!(IMultilineEditorAbi);
/// Shares explicit wrapping calls without inferring policy from the resulting geometry.
macro_rules! wrapping_proxy {
    ($interface:ty) => {
        impl Widget<$interface> {
            /// Returns the current explicit wrapping policy.
            pub fn wrapping(&self) -> Result<TextWrapping, NcStatus> {
                self.interface.wrapping()
            }
            /// Updates wrapping and invalidates preferred size when the policy changes.
            pub fn set_wrapping(&self, wrapping: TextWrapping) -> Result<(), NcStatus> {
                self.interface.set_wrapping(wrapping)
            }
        }
    };
}
wrapping_proxy!(ITextBlockAbi);
wrapping_proxy!(ITextAreaAbi);
wrapping_proxy!(IMultilineEditorAbi);
impl Widget<ITextAreaAbi> {
    /// Queries scrolling on this same root identity; it cannot replace the fixed editor content.
    pub fn scrolling(&self) -> Result<Widget<IScrollingAbi>, NcStatus> {
        Ok(Widget::from_interface(self.interface.query()?))
    }
}
/// Shares numeric convenience methods while each plugin retains its arithmetic rules.
macro_rules! numeric_proxy {
    ($interface:ty) => {
        impl Widget<$interface> {
            /// Reads the current finite numeric value.
            pub fn value(&self) -> Result<f32, NcStatus> {
                self.interface.value()
            }
            /// Replaces a finite value silently, applying slider bounds where relevant.
            pub fn set_value(&self, value: f32) -> Result<(), NcStatus> {
                self.interface.set_value(value)
            }
            /// Reports the inline edit mode which controls routed text input.
            pub fn is_editing(&self) -> Result<bool, NcStatus> {
                self.interface.is_editing()
            }
        }
    };
}
numeric_proxy!(ISliderAbi);
numeric_proxy!(INumberAbi);
impl Widget<IColorSwatchAbi> {
    /// Reads the packed RGBA paint color.
    pub fn fill(&self) -> Result<u32, NcStatus> {
        self.interface.fill()
    }
    /// Replaces color without publishing a user event or changing preferred size.
    pub fn set_fill(&self, rgba: u32) -> Result<(), NcStatus> {
        self.interface.set_fill(rgba)
    }
}
impl Widget<IScrollbarAbi> {
    /// Reads a coherent range and offset snapshot.
    pub fn range(&self) -> Result<ScrollRange, NcStatus> {
        self.interface.range()
    }
    /// Replaces nonnegative range lengths atomically.
    pub fn set_lengths(&self, viewport: i32, content: i32) -> Result<(), NcStatus> {
        self.interface.set_lengths(viewport, content)
    }
    /// Silently clamps a requested offset to the current range.
    pub fn set_offset(&self, offset: i32) -> Result<(), NcStatus> {
        self.interface.set_offset(offset)
    }
}

/// Runtime-bound factory client; cloning retains its interface and copies no widget state.
#[derive(Clone)]
pub struct WidgetFactory {
    /// The plugin's factory on the same identity as its runtime/window manager.
    interface: ComPtr<IUiFactoryAbi>,
}

impl WidgetFactory {
    /// Supplies runtime authority for an independent COM widget's inline WidgetState.
    pub fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        self.interface.host()
    }

    /// Creates a fixed multiline editor and its real scroll children with one shared offset model.
    pub fn text_area(&self, text: &str, wrapping: TextWrapping, scrolling: bool) -> Result<TextArea, NcStatus> {
        Ok(TextArea::from_interface(self.interface.create_text_area(
            NcCopyFromBufferOffset::new(0, text.as_bytes()),
            wrapping,
            scrolling,
        )?))
    }
    /// Constructs an initialized, unmounted checkbox in this runtime.
    pub fn checkbox(&self, label: &str, checked: bool) -> Result<Checkbox, NcStatus> {
        Ok(Checkbox::from_interface(
            self.interface.create_checkbox(checked, NcCopyFromBufferOffset::new(0, label.as_bytes()))?,
        ))
    }
    /// Constructs an initialized, unmounted textbox in this runtime.
    pub fn textbox(&self, text: &str) -> Result<Textbox, NcStatus> {
        Ok(Textbox::from_interface(
            self.interface.create_textbox(NcCopyFromBufferOffset::new(0, text.as_bytes()))?,
        ))
    }
    /// Constructs an initialized, unmounted text block in this runtime.
    pub fn text_block(&self, text: &str, wrapping: TextWrapping) -> Result<TextBlock, NcStatus> {
        Ok(TextBlock::from_interface(
            self.interface.create_text_block(NcCopyFromBufferOffset::new(0, text.as_bytes()), wrapping)?,
        ))
    }
    /// Constructs an initialized, unmounted slider in this runtime.
    pub fn slider(&self, options: SliderOptions) -> Result<Slider, NcStatus> {
        Ok(Slider::from_interface(self.interface.create_slider(options)?))
    }
    /// Constructs an initialized, unmounted number in this runtime.
    pub fn number(&self, options: NumberOptions) -> Result<Number, NcStatus> {
        Ok(Number::from_interface(self.interface.create_number(options)?))
    }
    /// Constructs an initialized, unmounted color swatch in this runtime.
    pub fn color_swatch(&self, rgba: u32, label: &str) -> Result<ColorSwatch, NcStatus> {
        Ok(ColorSwatch::from_interface(
            self.interface.create_color_swatch(rgba, NcCopyFromBufferOffset::new(0, label.as_bytes()))?,
        ))
    }
    /// Constructs an initialized, unmounted list item in this runtime.
    pub fn list_item(&self, label: &str) -> Result<ListItem, NcStatus> {
        Ok(ListItem::from_interface(
            self.interface.create_list_item(NcCopyFromBufferOffset::new(0, label.as_bytes()))?,
        ))
    }
    /// Constructs an initialized, unmounted list box in this runtime.
    pub fn list_box(&self, label: &str) -> Result<ListBox, NcStatus> {
        Ok(ListBox::from_interface(
            self.interface.create_list_box(NcCopyFromBufferOffset::new(0, label.as_bytes()))?,
        ))
    }
    /// Constructs an initialized, unmounted scrollbar in this runtime.
    pub fn scrollbar(&self, range: ScrollRange) -> Result<Scrollbar, NcStatus> {
        Ok(Scrollbar::from_interface(self.interface.create_scrollbar(range)?))
    }

    /// Adopts a typed factory obtained from UI runtime activation.
    pub fn from_interface(interface: ComPtr<IUiFactoryAbi>) -> Self {
        Self { interface }
    }

    /// Creates and initializes an unmounted button in one concrete API operation.
    pub fn button(&self, label: &str) -> Result<Button, NcStatus> {
        let button = Button::from_interface(self.interface.create_button()?);
        button.set_label(label)?;
        Ok(button)
    }
}

/// Borrowed painting convenience with the invocation token hidden from each call.
/// The underlying owned service validates expiry even if a direct COM caller
/// retains its interface and token beyond this Rust wrapper's lifetime.
pub struct Painter<'a> {
    /// Borrowed recording capability, never an adapter over stack-owned drawing state.
    interface: ComRef<'a, IPainterAbi>,
    /// Unique invocation authority issued and expired by the UI runtime.
    scope: PaintScope,
}

impl<'a> Painter<'a> {
    /// Wraps the capability and token received by a widget's COM paint method.
    pub fn from_call(interface: ComRef<'a, IPainterAbi>, scope: PaintScope) -> Self {
        Self { interface, scope }
    }

    /// Narrows local clipping by value; no clip stack, callback, extra COM object or Drop-time
    /// restoration is involved. The parent painter remains usable with its original clip.
    pub fn clipped(&self, rect: Rect) -> Result<Self, NcStatus> {
        if rect.width < 0 || rect.height < 0 { return Err(nc_com::NC_E_INVALID_ARG); }
        let parent = self.scope.clip;
        let x = parent.x.max(rect.x);
        let y = parent.y.max(rect.y);
        let right = (i64::from(parent.x) + i64::from(parent.width)).min(i64::from(rect.x) + i64::from(rect.width));
        let bottom = (i64::from(parent.y) + i64::from(parent.height)).min(i64::from(rect.y) + i64::from(rect.height));
        let clip = Rect { x, y, width: (right - i64::from(x)).max(0) as i32, height: (bottom - i64::from(y)).max(0) as i32 };
        Ok(Self { interface: self.interface, scope: PaintScope { invocation: self.scope.invocation, clip } })
    }
    /// Records a finite solid line; the UI owns tessellation and the currently installed material.
    pub fn stroke_line(&self, from: PaintPoint, to: PaintPoint, width: f32, rgba: u32) -> Result<(), NcStatus> {
        self.interface.stroke_line(self.scope, from, to, width, rgba)
    }
    /// Records finite polygon positions with no application dependence on UI tessellation internals.
    pub fn fill_polygon(&self, points: &[PaintPoint], rgba: u32) -> Result<(), NcStatus> {
        self.interface.fill_polygon(self.scope, NcCopyFromBufferOffset::new(0, points), rgba)
    }
    /// Draws body text using the UI's current font and semantic content color.
    pub fn text(&self, rect: Rect, text: &str) -> Result<(), NcStatus> {
        self.interface.text(self.scope, rect, NcCopyFromBufferOffset::new(0, text.as_bytes()))
    }

    /// Records solid triangle geometry without exposing atlas packing to the application.
    pub fn colored_triangles(&self, vertices: &[ColoredVertex]) -> Result<(), NcStatus> {
        self.interface.colored_triangles(self.scope, NcCopyFromBufferOffset::new(0, vertices))
    }

    /// Records a complete validated atlas triangle batch with one COM copy.
    pub fn triangles(&self, vertices: &[RenderVertex]) -> Result<(), NcStatus> {
        self.interface.triangles(self.scope, NcCopyFromBufferOffset::new(0, vertices))
    }

    /// Draws an owned immutable texture with an RGBA tint; its creator must be the submitted renderer.
    pub fn texture(&self, rect: Rect, rgba: u32, texture: ComRef<'_, ITextureAbi>) -> Result<(), NcStatus> {
        self.texture_region(rect, TextureRegion::FULL, rgba, texture)
    }

    /// Draws an explicit normalized subregion with destination clipping preserving source mapping.
    pub fn texture_region(&self, rect: Rect, source: TextureRegion, rgba: u32, texture: ComRef<'_, ITextureAbi>) -> Result<(), NcStatus> {
        self.interface.texture(self.scope, rect, source, rgba, texture)
    }
    /// Inserts renderer-owned application GPU work at this point in painter order.
    pub fn item(&self, rect: Rect, item: ComRef<'_, IRenderItemAbi>) -> Result<(), NcStatus> {
        self.interface.item(self.scope, rect, item)
    }

    /// Records a local rectangle within this invocation.
    pub fn fill(&self, rect: Rect, rgba: u32) -> Result<(), NcStatus> {
        self.interface.fill(self.scope, rect, rgba)
    }
}

/// Retained UI client with cached, concrete factory and window-manager capabilities.
/// Clones retain those same projections so typed app handlers can own runtime authority.
#[derive(Clone)]
pub struct Ui {
    /// Authoritative runtime allocation; all state stays in the implementation plugin.
    runtime: ComPtr<IUiRuntimeAbi>,
    /// Factory shares the runtime's COM identity.
    factory: WidgetFactory,
    /// Window-management projection cached once at construction.
    windows: ComPtr<IWindowManagerAbi>,
}

impl Ui {
    /// Creates a hidden standard file picker using the actual runtime's modal and COM controls.
    pub fn file_dialog(&self, parent: ComRef<'_, IWindowAbi>) -> Result<crate::FileDialog, NcStatus> {
        Ok(crate::FileDialog::from_interface(self.runtime.query::<IFileDialogsAbi>()?.create(parent)?))
    }

    /// Queries compact menu construction on this same runtime identity without copying its forest.
    pub fn menus(&self) -> Result<crate::Menus, NcStatus> {
        Ok(crate::Menus::from_interface(self.runtime.query()?))
    }

    /// Checks the complete runtime capability set before exposing a usable client.
    pub fn from_interface(runtime: ComPtr<IUiRuntimeAbi>) -> Result<Self, NcStatus> {
        let factory = WidgetFactory::from_interface(runtime.query()?);
        let windows = runtime.query()?;
        Ok(Self { runtime, factory, windows })
    }
    /// Returns the runtime-bound factory without retaining another interface reference.
    pub fn widgets(&self) -> &WidgetFactory {
        &self.factory
    }
    /// Mounts the actual widget object in a new logical window.
    pub fn window<I: ComInterface>(&self, title: &str, rect: Rect, root: &Widget<I>) -> Result<ComPtr<IWindowAbi>, NcStatus> {
        self.windows
            .create_window(NcCopyFromBufferOffset::new(0, title.as_bytes()), rect, root.node_interface()?.as_com_ref())
    }
    /// Mounts a structural child under a live window from this UI.
    pub fn child_window<I: ComInterface>(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        title: &str,
        rect: Rect,
        root: &Widget<I>,
    ) -> Result<ComPtr<IWindowAbi>, NcStatus> {
        self.windows.create_child(
            parent,
            NcCopyFromBufferOffset::new(0, title.as_bytes()),
            rect,
            root.node_interface()?.as_com_ref(),
        )
    }
    /// Mounts a hidden modal dialog; show it after initialization with set_visibility.
    pub fn dialog<I: ComInterface>(&self, parent: ComRef<'_, IWindowAbi>, title: &str, rect: Rect, root: &Widget<I>) -> Result<ComPtr<IWindowAbi>, NcStatus> {
        self.windows.create_dialog(
            parent,
            NcCopyFromBufferOffset::new(0, title.as_bytes()),
            rect,
            root.node_interface()?.as_com_ref(),
        )
    }
    /// Mounts a hidden auto-sized popup with a distinct popup capability.
    pub fn popup<I: ComInterface>(&self, parent: ComRef<'_, IWindowAbi>, title: &str, root: &Widget<I>) -> Result<ComPtr<IPopupAbi>, NcStatus> {
        self.windows
            .create_popup(parent, NcCopyFromBufferOffset::new(0, title.as_bytes()), root.node_interface()?.as_com_ref())
    }
    /// Queues a typed host event while keeping its text buffer local to this call.
    pub fn input(&self, event: UiInput, text: &str) -> Result<(), NcStatus> {
        self.runtime.input(event.into(), NcCopyFromBufferOffset::new(0, text.as_bytes()))
    }
    /// Retains the runtime's input projection once for a host loop or concrete input interceptor.
    pub fn input_sink(&self) -> Result<ComPtr<IInputSinkAbi>, NcStatus> {
        self.runtime.query()
    }
    /// Reads the committed editor area used by the platform's native text input service.
    pub fn text_input_area(&self) -> Result<TextInputAreaAbi, NcStatus> {
        self.runtime.text_input_area()
    }
    /// Processes input and synchronizes independently recorded geometry changes.
    pub fn update(&self, size: Size) -> Result<(), NcStatus> {
        self.runtime.update(size)
    }
    /// Dispatches typed application events between input phases and commits handler edits before routing subsequent input.
    pub fn update_with_events<State: 'static>(&self, size: Size, events: &crate::EventSession<State>) -> Result<(), NcStatus> {
        self.runtime.update_with_events(size, events.as_dispatcher())
    }
    /// Submits a synchronized UI frame to a native window's renderer capability.
    pub fn render(&self, size: Size, renderer: ComRef<'_, IRendererAbi>) -> Result<(), NcStatus> {
        self.runtime.render(size, renderer)
    }
    /// Retains the current immutable bundle for restoration or another UI/renderer consumer.
    pub fn assets(&self) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        self.runtime.assets()
    }
    /// Validates compatible resources and uploads before publication; call update after success.
    pub fn install_assets(&self, assets: ComRef<'_, IAssetBundleAbi>, renderer: ComRef<'_, IRendererAbi>) -> Result<(), NcStatus> {
        self.runtime.install_assets(assets, renderer)
    }
    /// Closes the runtime's complete retained forest while handles may remain allocated.
    pub fn close(&self) -> Result<(), NcStatus> {
        self.runtime.close()
    }
}

/// Static typography conveniences exist only on text-bearing proxy types.
macro_rules! font_proxy {
    ($interface:ty) => {
        impl Widget<$interface> {
            /// Selects typography through this same widget object's optional capability.
            pub fn set_font_role(&self, role: TextFontRole) -> Result<(), NcStatus> {
                self.interface.query::<IFontStyleAbi>()?.set_font_role(role)
            }
            /// Resolves a named font once in this widget's resource bundle.
            pub fn set_named_font(&self, name: &str) -> Result<(), NcStatus> {
                self.interface
                    .query::<IFontStyleAbi>()?
                    .set_named_font(NcCopyFromBufferOffset::new(0, name.as_bytes()))
            }
        }
    };
}
font_proxy!(IButtonAbi);
font_proxy!(ICheckboxAbi);
font_proxy!(ITextboxAbi);
font_proxy!(ITextAreaAbi);
font_proxy!(IMultilineEditorAbi);
font_proxy!(ITextBlockAbi);
font_proxy!(ISliderAbi);
font_proxy!(INumberAbi);
font_proxy!(IColorSwatchAbi);
font_proxy!(IListBoxAbi);
font_proxy!(IListItemAbi);

/// Adjacent-icon conveniences remain restricted to controls that own such content.
macro_rules! icon_proxy {
    ($interface:ty) => {
        impl Widget<$interface> {
            /// Selects an icon role, or NONE to remove it, without changing the label.
            pub fn set_icon_role(&self, icon: ContentIcon) -> Result<(), NcStatus> {
                self.interface.query::<IIconContentAbi>()?.set_icon_role(icon)
            }
            /// Resolves a named icon in this widget's own resource bundle.
            pub fn set_named_icon(&self, name: &str) -> Result<(), NcStatus> {
                self.interface
                    .query::<IIconContentAbi>()?
                    .set_named_icon(NcCopyFromBufferOffset::new(0, name.as_bytes()))
            }
        }
    };
}
icon_proxy!(IButtonAbi);
icon_proxy!(IListItemAbi);
impl Widget<IButtonAbi> {
    /// Reads the current mask controlling center fill on an unframed button.
    pub fn fill_states(&self) -> Result<FillStates, NcStatus> {
        self.interface.query::<IButtonStyleAbi>()?.fill_states()
    }
    /// Changes the validated paint-only fill mask.
    pub fn set_fill_states(&self, states: FillStates) -> Result<(), NcStatus> {
        self.interface.query::<IButtonStyleAbi>()?.set_fill_states(states)
    }
}

/// Typed static proxy for the concrete combo header and its inherited widget state.
pub type Combo = Widget<IComboAbi>;
impl WidgetFactory {
    /// Creates an empty header; application-local list data never enters the UI plugin.
    pub fn combo(&self) -> Result<Combo, NcStatus> {
        Ok(Combo::from_interface(self.interface.create_combo()?))
    }
}
impl Widget<IComboAbi> {
    /// Reads the selected position without interpreting it as a global identity.
    pub fn selected(&self) -> Result<u64, NcStatus> {
        self.interface.selected()
    }
    /// Copies the current display label independently of generic list values.
    pub fn label(&self) -> Result<String, NcStatus> {
        self.interface.label()?.to_string()
    }
    /// Reads the header's intended popup state.
    pub fn is_open(&self) -> Result<bool, NcStatus> {
        self.interface.is_open()
    }
    /// Synchronizes intent silently after actual popup visibility changes.
    pub fn set_open(&self, open: bool) -> Result<(), NcStatus> {
        self.interface.set_open(open)
    }
    /// Installs an already-resolved application selection without publishing a user event.
    pub fn set_selection(&self, index: u64, label: &str) -> Result<(), NcStatus> {
        self.interface.set_selection(index, NcCopyFromBufferOffset::new(0, label.as_bytes()))
    }
    /// Resolves and clamps local list state, sending only its selected position and label across COM.
    pub fn update_items<Item: AsRef<str>>(&self, items: &[Item]) -> Result<(), NcStatus> {
        let index = usize::try_from(self.selected()?).unwrap_or(usize::MAX).min(items.len().saturating_sub(1));
        self.set_selection(index as u64, items.get(index).map_or("", AsRef::as_ref))
    }
    /// Commits an actual popup submission, closes the header and publishes changed when its snapshot differs.
    pub fn select<Item: AsRef<str>>(&self, index: usize, items: &[Item]) -> Result<Option<String>, NcStatus> {
        let index = index.min(items.len().saturating_sub(1));
        let label = items.get(index).map(AsRef::as_ref);
        self.interface
            .submit_selection(index as u64, NcCopyFromBufferOffset::new(0, label.unwrap_or("").as_bytes()))?;
        Ok(label.map(str::to_owned))
    }
}
font_proxy!(IComboAbi);

/// Shares concrete image-capability calls without introducing an image-only widget wrapper.
macro_rules! image_content {
    ($name:ident) => {
        impl $name {
            /// Installs this renderer-owned image; dimensions are validated before the edit commits.
            pub fn set_image(&self, image: ComRef<'_, ITextureAbi>, fit_width: bool) -> Result<(), NcStatus> {
                self.interface.query::<IImageContentAbi>()?.set_image(image, fit_width)
            }
            /// Returns to label-only content without generating an activation event.
            pub fn clear_image(&self) -> Result<(), NcStatus> {
                self.interface.query::<IImageContentAbi>()?.clear_image()
            }
        }
    };
}
image_content!(Button);
image_content!(ListBox);
