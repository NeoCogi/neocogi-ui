//! Typed static clients for dynamically or statically linked COM implementations.
//!
//! Clients keep no shadow widget state or topology. Every edit crosses the
//! checked interface and every returned snapshot owns its storage. The retained implementation
//! lives in the UI plugin; this facade depends only on the contracts and COM
//! object support.

pub use neocogi_renderer_contracts::{
    Color, IRenderDiagnostics, IRenderDiagnosticsAbi, IRenderItem, IRenderItemAbi, IRenderStream, IRenderStreamAbi, IRenderer, IRendererAbi, IRgbaImage,
    IRgbaImageAbi, ITexture, ITextureAbi, ITextureFactory, ITextureFactoryAbi, MAX_IMAGE_BYTES, MAX_RENDER_BATCHES, MAX_RENDER_VERTICES, Rect, RenderBatch,
    RenderBatchAbi, RenderBatchTag, RenderInfo, RenderVertex, Size, TextureRegion, color, rgba_bytes,
};
pub use neocogi_platform_contracts::{
    IInputSink, IInputSinkAbi, IPlatformWindow, IPlatformWindowAbi, KeyCode, KeyStroke, Platform, PlatformAbi, Point, PollStatus, TextInputArea,
    TextInputAreaAbi, TextInputAreaTag, UiInput, UiInputAbi, UiInputTag,
};
pub use neocogi_ui_core_contracts::*;
/// Complete generated built-in node contract namespace, including its distinct module metadata.
pub mod nodes {
    pub use neocogi_ui_nodes_contracts::*;
}
pub use neocogi_ui_nodes_contracts::{
    ButtonSinkAbi, CheckboxSinkAbi, TextboxSinkAbi, TextAreaSinkAbi, SliderSinkAbi, NumberSinkAbi, ListItemSinkAbi, ListBoxSinkAbi, ScrollbarSinkAbi,
    SplitViewSinkAbi, Button, ButtonAbi, Checkbox, CheckboxAbi, ChildCrossSize, ColorSwatch, ColorSwatchAbi, Combo, ComboAbi, ContentIcon, CrossSize,
    CrossSizeAbi, Disclosure, DisclosureAbi, DisclosureStyle, FillStates, FlowDirection, Grid, GridAbi, GridExtent, LayoutTrack, LayoutTrackAbi, Linear,
    LinearAbi, LinearEdge, ListBox, ListBoxAbi, ListItem, ListItemAbi, Number, NumberAbi, NumberOptions, ScrollArea, ScrollAreaAbi, ScrollAxis, ScrollRange,
    Scrollbar, ScrollbarAbi, Slider, FormAbi, SliderAbi, SliderOptions, SplitAxis, SplitChanged, SplitOptions, SplitView, SplitViewAbi, TextArea, TextAreaAbi,
    TextBlock, TextBlockAbi, Textbox, TextboxAbi, UiFactory, UiFactoryAbi,
};
pub use neocogi_gfx_contracts::{
    AssetLoader, AssetLoaderAbi, DrawPoint, GfxPluginFactory, GfxPluginFactoryAbi, IAssetBuilder, IAssetBuilderAbi, IGfxDiagnostics, IGfxDiagnosticsAbi,
    GfxCompositor, GfxCompositorAbi, PngDecoder, PngDecoderAbi, SolidVertex,
};
#[cfg(feature = "skin")]
pub use neocogi_skin_contracts::{
    ISkinDiagnostics, ISkinDiagnosticsAbi, PaletteEdit, PaletteEditAbi, PaletteEditTag, SkinPluginFactory, SkinPluginFactoryAbi, StyleCompiler,
    StyleCompilerAbi, ThemeLoader, ThemeLoaderAbi,
};
use neocogi_com::{ComInterface, ComPtr, ComRef, INcStringComPtrExt, IUnknown, NcCopyFromBufferOffset, NcStatus};
mod containers;
pub use containers::{DisclosureWidget, FormWidget, GridWidget, LinearWidget, ScrollAreaWidget, SplitViewWidget};
mod mount;
pub use mount::{Component, Mountable};

/// Reusable state composed into concrete Rust widget implementations.
pub mod widget;

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
    pub fn set_options(&mut self, options: WidgetOptions) -> Result<(), NcStatus> {
        self.node_interface()?.set_options(options)
    }

    /// Obtains the common node capability for attachment; callers cache this at mount.
    pub fn node_interface(&self) -> Result<ComPtr<IWidgetAbi>, NcStatus> {
        // Querying preserves object identity and validates the full projected ABI.
        self.interface.query()
    }
}

/// Static convenience wrapper around the dynamically constructed Button class projection.
pub type ButtonWidget = Widget<ButtonAbi>;

impl Widget<ButtonAbi> {
    /// Replaces the label with a synchronous UTF-8 copy owned by the plugin.
    pub fn set_label(&mut self, label: &str) -> Result<(), NcStatus> {
        self.interface.set_label(NcCopyFromBufferOffset::new(0, label.as_bytes()))
    }

    /// Copies an immutable plugin snapshot into an ordinary application String.
    pub fn label(&self) -> Result<String, NcStatus> {
        self.interface.label()?.to_string()
    }
}

/// Static convenience wrapper around the dynamically constructed Checkbox class projection.
pub type CheckboxWidget = Widget<CheckboxAbi>;
/// Static convenience wrapper around the dynamically constructed Textbox class projection.
pub type TextboxWidget = Widget<TextboxAbi>;
/// Static proxy for the fixed scrolling editor; inherited text edits reach its actual content leaf.
pub type TextAreaWidget = Widget<TextAreaAbi>;
/// Static TextBlock proxy; semantic storage belongs to its plugin object.
pub type TextBlockWidget = Widget<TextBlockAbi>;
/// Static convenience wrapper around the dynamically constructed Slider class projection.
pub type SliderWidget = Widget<SliderAbi>;
/// Static convenience wrapper around the dynamically constructed Number class projection.
pub type NumberWidget = Widget<NumberAbi>;
/// Static convenience wrapper around the dynamically constructed ColorSwatch class projection.
pub type ColorSwatchWidget = Widget<ColorSwatchAbi>;
/// Static convenience wrapper around the dynamically constructed ListItem class projection.
pub type ListItemWidget = Widget<ListItemAbi>;
/// Static convenience wrapper around the dynamically constructed ListBox class projection.
pub type ListBoxWidget = Widget<ListBoxAbi>;
/// Static convenience wrapper around the dynamically constructed Scrollbar class projection.
pub type ScrollbarWidget = Widget<ScrollbarAbi>;

/// Shares inherited text convenience calls while keeping every proxy concrete.
macro_rules! text_proxy {
    ($interface:ty) => {
        impl Widget<$interface> {
            /// Copies the current immutable text snapshot into application storage.
            pub fn text(&self) -> Result<String, NcStatus> {
                self.interface.text()?.to_string()
            }
            /// Replaces text through the control's documented normalization policy.
            pub fn set_text(&mut self, text: &str) -> Result<(), NcStatus> {
                self.interface.set_text(NcCopyFromBufferOffset::new(0, text.as_bytes()))
            }
            /// Clears text using the same cursor and invalidation rules as replacement.
            pub fn clear(&mut self) -> Result<(), NcStatus> {
                self.set_text("")
            }
        }
    };
}
text_proxy!(ITextContentAbi);
text_proxy!(TextboxAbi);
text_proxy!(TextAreaAbi);
text_proxy!(IMultilineEditorAbi);
text_proxy!(TextBlockAbi);
text_proxy!(ColorSwatchAbi);
text_proxy!(ListItemAbi);
text_proxy!(ListBoxAbi);

impl Widget<CheckboxAbi> {
    /// Reads the current checked value.
    pub fn checked(&self) -> Result<bool, NcStatus> {
        self.interface.checked()
    }
    /// Changes checked state silently without invalidating fixed checkbox geometry.
    pub fn set_checked(&mut self, checked: bool) -> Result<(), NcStatus> {
        self.interface.set_checked(checked)
    }
    /// Copies the current label snapshot.
    pub fn label(&self) -> Result<String, NcStatus> {
        self.interface.label()?.to_string()
    }
    /// Replaces label text and records a sticky preferred-size change when needed.
    pub fn set_label(&mut self, label: &str) -> Result<(), NcStatus> {
        self.interface.set_label(NcCopyFromBufferOffset::new(0, label.as_bytes()))
    }
    /// Selects typography directly through the Checkbox class contract.
    pub fn set_font_role(&mut self, role: FontRole) -> Result<(), NcStatus> {
        self.interface.set_font_role(role)
    }
    /// Resolves a named font directly through the Checkbox class contract.
    pub fn set_named_font(&mut self, name: &str) -> Result<(), NcStatus> {
        self.interface.set_named_font(NcCopyFromBufferOffset::new(0, name.as_bytes()))
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
            pub fn set_cursor(&mut self, cursor: u64) -> Result<(), NcStatus> {
                self.interface.set_cursor(cursor)
            }
        }
    };
}
editor_proxy!(TextboxAbi);
editor_proxy!(TextAreaAbi);
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
            pub fn set_wrapping(&mut self, wrapping: TextWrapping) -> Result<(), NcStatus> {
                self.interface.set_wrapping(wrapping)
            }
        }
    };
}
wrapping_proxy!(TextBlockAbi);
wrapping_proxy!(TextAreaAbi);
wrapping_proxy!(IMultilineEditorAbi);
impl Widget<TextAreaAbi> {
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
            pub fn set_value(&mut self, value: f32) -> Result<(), NcStatus> {
                self.interface.set_value(value)
            }
            /// Reports the inline edit mode which controls routed text input.
            pub fn is_editing(&self) -> Result<bool, NcStatus> {
                self.interface.is_editing()
            }
        }
    };
}
numeric_proxy!(SliderAbi);
numeric_proxy!(NumberAbi);
impl Widget<ColorSwatchAbi> {
    /// Reads the packed RGBA paint color.
    pub fn fill(&self) -> Result<u32, NcStatus> {
        self.interface.fill()
    }
    /// Replaces color without publishing a user event or changing preferred size.
    pub fn set_fill(&mut self, rgba: u32) -> Result<(), NcStatus> {
        self.interface.set_fill(rgba)
    }
}
impl Widget<ScrollbarAbi> {
    /// Reads a coherent range and offset snapshot.
    pub fn range(&self) -> Result<ScrollRange, NcStatus> {
        self.interface.range()
    }
    /// Replaces nonnegative range lengths atomically.
    pub fn set_lengths(&mut self, viewport: i32, content: i32) -> Result<(), NcStatus> {
        self.interface.set_lengths(viewport, content)
    }
    /// Silently clamps a requested offset to the current range.
    pub fn set_offset(&mut self, offset: i32) -> Result<(), NcStatus> {
        self.interface.set_offset(offset)
    }
}

/// Runtime-bound factory client; cloning retains its interface and copies no widget state.
#[derive(Clone)]
pub struct WidgetFactory {
    /// Dynamically implemented shorthand bound to one runtime's widget host.
    interface: ComPtr<UiFactoryAbi>,
}

impl WidgetFactory {
    /// Supplies runtime authority for an independent COM widget's inline WidgetState.
    pub fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        self.interface.query::<IConstructionHostAbi>()?.host()
    }

    /// Creates a fixed multiline editor and its real scroll children with one shared offset model.
    pub fn text_area_with_sink(
        &self,
        text: &str,
        wrapping: TextWrapping,
        scrolling: bool,
        sink: Option<ComRef<'_, TextAreaSinkAbi>>,
    ) -> Result<TextAreaWidget, NcStatus> {
        Ok(TextAreaWidget::from_interface(attach_optional_sink(
            self.interface
                .create_text_area(NcCopyFromBufferOffset::new(0, text.as_bytes()), wrapping, scrolling)?,
            sink,
        )?))
    }
    /// Constructs an initialized, unmounted checkbox in this runtime.
    pub fn checkbox_with_sink(&self, label: &str, checked: bool, sink: Option<ComRef<'_, CheckboxSinkAbi>>) -> Result<CheckboxWidget, NcStatus> {
        Ok(CheckboxWidget::from_interface(attach_optional_sink(
            self.interface.create_checkbox(checked, NcCopyFromBufferOffset::new(0, label.as_bytes()))?,
            sink,
        )?))
    }
    /// Constructs an initialized, unmounted textbox in this runtime.
    pub fn textbox_with_sink(&self, text: &str, sink: Option<ComRef<'_, TextboxSinkAbi>>) -> Result<TextboxWidget, NcStatus> {
        Ok(TextboxWidget::from_interface(attach_optional_sink(
            self.interface.create_textbox(NcCopyFromBufferOffset::new(0, text.as_bytes()))?,
            sink,
        )?))
    }
    /// Constructs an initialized, unmounted text block in this runtime.
    pub fn text_block(&self, text: &str, wrapping: TextWrapping) -> Result<TextBlockWidget, NcStatus> {
        Ok(TextBlockWidget::from_interface(
            self.interface.create_text_block(NcCopyFromBufferOffset::new(0, text.as_bytes()), wrapping)?,
        ))
    }
    /// Constructs an initialized, unmounted slider in this runtime.
    pub fn slider_with_sink(&self, options: SliderOptions, sink: Option<ComRef<'_, SliderSinkAbi>>) -> Result<SliderWidget, NcStatus> {
        Ok(SliderWidget::from_interface(attach_optional_sink(
            self.interface.create_slider(options)?,
            sink,
        )?))
    }
    /// Constructs an initialized, unmounted number in this runtime.
    pub fn number_with_sink(&self, options: NumberOptions, sink: Option<ComRef<'_, NumberSinkAbi>>) -> Result<NumberWidget, NcStatus> {
        Ok(NumberWidget::from_interface(attach_optional_sink(
            self.interface.create_number(options)?,
            sink,
        )?))
    }
    /// Constructs an initialized, unmounted color swatch in this runtime.
    pub fn color_swatch(&self, rgba: u32, label: &str) -> Result<ColorSwatchWidget, NcStatus> {
        Ok(ColorSwatchWidget::from_interface(
            self.interface.create_color_swatch(rgba, NcCopyFromBufferOffset::new(0, label.as_bytes()))?,
        ))
    }
    /// Constructs an initialized, unmounted list item in this runtime.
    pub fn list_item_with_sink(&self, label: &str, sink: Option<ComRef<'_, ListItemSinkAbi>>) -> Result<ListItemWidget, NcStatus> {
        Ok(ListItemWidget::from_interface(attach_optional_sink(
            self.interface.create_list_item(NcCopyFromBufferOffset::new(0, label.as_bytes()))?,
            sink,
        )?))
    }
    /// Constructs an initialized, unmounted list box in this runtime.
    pub fn list_box_with_sink(&self, label: &str, sink: Option<ComRef<'_, ListBoxSinkAbi>>) -> Result<ListBoxWidget, NcStatus> {
        Ok(ListBoxWidget::from_interface(attach_optional_sink(
            self.interface.create_list_box(NcCopyFromBufferOffset::new(0, label.as_bytes()))?,
            sink,
        )?))
    }
    /// Constructs an initialized, unmounted scrollbar in this runtime.
    pub fn scrollbar_with_sink(&self, range: ScrollRange, sink: Option<ComRef<'_, ScrollbarSinkAbi>>) -> Result<ScrollbarWidget, NcStatus> {
        Ok(ScrollbarWidget::from_interface(attach_optional_sink(
            self.interface.create_scrollbar(range)?,
            sink,
        )?))
    }

    /// Adopts a typed factory obtained from UI runtime activation.
    pub fn from_interface(interface: ComPtr<UiFactoryAbi>) -> Self {
        Self { interface }
    }

    /// Creates and initializes an unmounted button in one concrete API operation.
    pub fn button_with_sink(&self, label: &str, sink: Option<ComRef<'_, ButtonSinkAbi>>) -> Result<ButtonWidget, NcStatus> {
        Ok(ButtonWidget::from_interface(attach_optional_sink(
            self.interface
                .create_button(NcCopyFromBufferOffset::new(0, label.as_bytes()), WidgetOptions::FRAME)?,
            sink,
        )?))
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
        if rect.width < 0 || rect.height < 0 {
            return Err(neocogi_com::NC_E_INVALID_ARG);
        }
        let parent = self.scope.clip;
        let x = parent.x.max(rect.x);
        let y = parent.y.max(rect.y);
        let right = (i64::from(parent.x) + i64::from(parent.width)).min(i64::from(rect.x) + i64::from(rect.width));
        let bottom = (i64::from(parent.y) + i64::from(parent.height)).min(i64::from(rect.y) + i64::from(rect.height));
        let clip = Rect {
            x,
            y,
            width: (right - i64::from(x)).max(0) as i32,
            height: (bottom - i64::from(y)).max(0) as i32,
        };
        Ok(Self {
            interface: self.interface,
            scope: PaintScope { invocation: self.scope.invocation, clip },
        })
    }
    /// Records a finite solid line; the UI owns tessellation and the currently installed material.
    pub fn stroke_line(&mut self, from: PaintPoint, to: PaintPoint, width: f32, rgba: u32) -> Result<(), NcStatus> {
        self.interface.stroke_line(self.scope, from, to, width, rgba)
    }
    /// Records finite polygon positions with no application dependence on UI tessellation internals.
    pub fn fill_polygon(&mut self, points: &[PaintPoint], rgba: u32) -> Result<(), NcStatus> {
        self.interface.fill_polygon(self.scope, NcCopyFromBufferOffset::new(0, points), rgba)
    }
    /// Draws body text using the UI's current font and semantic content color.
    pub fn text(&mut self, rect: Rect, text: &str) -> Result<(), NcStatus> {
        self.interface.text(self.scope, rect, NcCopyFromBufferOffset::new(0, text.as_bytes()))
    }

    /// Records solid triangle geometry without exposing atlas packing to the application.
    pub fn colored_triangles(&mut self, vertices: &[ColoredVertex]) -> Result<(), NcStatus> {
        self.interface.colored_triangles(self.scope, NcCopyFromBufferOffset::new(0, vertices))
    }

    /// Records a complete validated atlas triangle batch with one COM copy.
    pub fn triangles(&mut self, vertices: &[RenderVertex]) -> Result<(), NcStatus> {
        self.interface.triangles(self.scope, NcCopyFromBufferOffset::new(0, vertices))
    }

    /// Draws an owned immutable texture with an RGBA tint; its creator must be the submitted renderer.
    pub fn texture(&mut self, rect: Rect, rgba: u32, texture: ComRef<'_, ITextureAbi>) -> Result<(), NcStatus> {
        self.texture_region(rect, TextureRegion::FULL, rgba, texture)
    }

    /// Draws an explicit normalized subregion with destination clipping preserving source mapping.
    pub fn texture_region(&mut self, rect: Rect, source: TextureRegion, rgba: u32, texture: ComRef<'_, ITextureAbi>) -> Result<(), NcStatus> {
        self.interface.texture(self.scope, rect, source, rgba, texture)
    }
    /// Inserts renderer-owned application GPU work at this point in painter order.
    pub fn item(&mut self, rect: Rect, item: ComRef<'_, IRenderItemAbi>) -> Result<(), NcStatus> {
        self.interface.item(self.scope, rect, item)
    }

    /// Records a local rectangle within this invocation.
    pub fn fill(&mut self, rect: Rect, rgba: u32) -> Result<(), NcStatus> {
        self.interface.fill(self.scope, rect, rgba)
    }
}

/// Retained UI client with cached, concrete factory and window-manager capabilities.
/// Clones retain those same projections so typed app handlers can own runtime authority.
#[derive(Clone)]
pub struct Ui {
    /// Authoritative runtime allocation; all state stays in the implementation plugin.
    runtime: ComPtr<UiRuntimeAbi>,
    /// Factory is a separate dynamically implemented object bound to this runtime's host.
    factory: WidgetFactory,
    /// Window-management projection cached once at construction.
    windows: ComPtr<IWindowManagerAbi>,
}

impl Ui {
    /// Installs a typed host clipboard; existing asynchronous editor requests retain their own receipts.
    pub fn set_clipboard(&mut self, clipboard: ComRef<'_, ITextClipboardAbi>) -> Result<(), NcStatus> {
        self.runtime.set_clipboard(clipboard)
    }
    /// Creates one complete combo box with the contract-defined fallback and selection state.
    pub fn combo_box_with_sink<Item: AsRef<str>>(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        items: &[Item],
        selection: ChoiceSelection,
        sink: Option<ComRef<'_, IChoiceMenuSinkAbi>>,
    ) -> Result<ComboWidget, NcStatus> {
        let (menu_text, entries) = encoded_combo_items(items)?;
        let combo = self.factory.interface.create_combo_box(
            parent,
            NcCopyFromBufferOffset::new(0, &menu_text),
            NcCopyFromBufferOffset::new(0, &entries),
            selection.into(),
        )?;
        Ok(ComboWidget::from_interface(attach_optional_sink(combo, sink)?))
    }

    /// Creates a combo from the generated preorder menu contract and its shared UTF-8 storage.
    pub fn combo_menu_with_sink(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        menu_text: &str,
        entries: &[ChoiceMenuEntry],
        selection: ChoiceSelection,
        sink: Option<ComRef<'_, IChoiceMenuSinkAbi>>,
    ) -> Result<ComboWidget, NcStatus> {
        let entries = combo_menu_abi(entries);
        let combo = self.factory.interface.create_combo_box(
            parent,
            NcCopyFromBufferOffset::new(0, menu_text.as_bytes()),
            NcCopyFromBufferOffset::new(0, &entries),
            selection.into(),
        )?;
        Ok(ComboWidget::from_interface(attach_optional_sink(combo, sink)?))
    }

    /// Creates a hidden standard file picker using the actual runtime's modal and COM controls.
    pub fn file_dialog_with_sink(&self, parent: ComRef<'_, IWindowAbi>, sink: Option<ComRef<'_, IFileDialogSinkAbi>>) -> Result<crate::FileDialog, NcStatus> {
        Ok(crate::FileDialog::from_interface(attach_optional_sink(
            self.factory.interface.query::<IFileDialogsAbi>()?.create(parent)?,
            sink,
        )?))
    }

    /// Queries compact menu construction on this same runtime identity without copying its forest.
    pub fn menus(&self) -> Result<crate::Menus, NcStatus> {
        Ok(crate::Menus::from_interface(self.runtime.query()?))
    }

    /// Binds independently activated core-runtime and concrete-node providers after proving ownership.
    pub fn from_interfaces(runtime: ComPtr<UiRuntimeAbi>, factory: ComPtr<UiFactoryAbi>) -> Result<Self, NcStatus> {
        let runtime_host = runtime.construction_host()?.host()?.query::<IUnknown>()?;
        let factory_host = factory.query::<IConstructionHostAbi>()?.host()?.query::<IUnknown>()?;
        if runtime_host.as_raw().object != factory_host.as_raw().object {
            return Err(UI_E_OWNER);
        }
        let factory = WidgetFactory::from_interface(factory);
        let windows = runtime.query()?;
        Ok(Self { runtime, factory, windows })
    }
    /// Returns the runtime-bound factory without retaining another interface reference.
    pub fn widgets(&self) -> &WidgetFactory {
        &self.factory
    }
    /// Mounts the actual widget object in a new logical window.
    pub fn window(&self, title: &str, rect: Rect, root: &impl Mountable) -> Result<ComPtr<IWindowAbi>, NcStatus> {
        self.windows.create_window(
            NcCopyFromBufferOffset::new(0, title.as_bytes()),
            rect,
            neocogi_com::NcValueRef::new(&root.mount_description()?)?,
        )
    }
    /// Mounts a structural child under a live window from this UI.
    pub fn child_window(&self, parent: ComRef<'_, IWindowAbi>, title: &str, rect: Rect, root: &impl Mountable) -> Result<ComPtr<IWindowAbi>, NcStatus> {
        self.windows.create_child(
            parent,
            NcCopyFromBufferOffset::new(0, title.as_bytes()),
            rect,
            neocogi_com::NcValueRef::new(&root.mount_description()?)?,
        )
    }
    /// Mounts a hidden modal dialog; show it after initialization with set_visibility.
    pub fn dialog(&self, parent: ComRef<'_, IWindowAbi>, title: &str, rect: Rect, root: &impl Mountable) -> Result<ComPtr<IWindowAbi>, NcStatus> {
        self.windows.create_dialog(
            parent,
            NcCopyFromBufferOffset::new(0, title.as_bytes()),
            rect,
            neocogi_com::NcValueRef::new(&root.mount_description()?)?,
        )
    }
    /// Mounts a hidden auto-sized popup with a distinct popup capability.
    pub fn popup(&self, parent: ComRef<'_, IWindowAbi>, title: &str, root: &impl Mountable) -> Result<ComPtr<IPopupAbi>, NcStatus> {
        self.windows.create_popup(
            parent,
            NcCopyFromBufferOffset::new(0, title.as_bytes()),
            neocogi_com::NcValueRef::new(&root.mount_description()?)?,
        )
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
    /// Settles owned application messages between input phases and commits handler edits.
    /// Construct the owner once and retain its checked IEventOwner projection across frames.
    /// This call only borrows that projection; it allocates no callback or revocation state.
    pub fn update_with_events(&self, size: Size, owner: ComRef<'_, IEventOwnerAbi>) -> Result<(), NcStatus> {
        self.runtime.update_with_events(size, owner)
    }
    /// Submits a synchronized UI frame to a native window's renderer capability.
    pub fn render(&self, size: Size, renderer: ComRef<'_, IRendererAbi>) -> Result<(), NcStatus> {
        self.runtime.render(size, renderer)
    }
    /// Retains the current immutable bundle for restoration or another UI/renderer consumer.
    pub fn assets(&self) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        self.runtime.assets()
    }
    /// Replaces this UI's runtime-wide resources through exclusive access; call update after success.
    pub fn install_assets(&mut self, assets: ComRef<'_, IAssetBundleAbi>, renderer: ComRef<'_, IRendererAbi>) -> Result<(), NcStatus> {
        self.runtime.install_assets(assets, renderer)
    }
    /// Closes the runtime's complete retained forest while handles may remain allocated.
    pub fn close(&self) -> Result<(), NcStatus> {
        self.runtime.close()
    }
}

/// Static typography conveniences exist only on text-bearing proxy types.
macro_rules! font_proxy {
    (direct $interface:ty) => {
        impl Widget<$interface> {
            /// Selects typography directly through the concrete class contract.
            pub fn set_font_role(&mut self, role: FontRole) -> Result<(), NcStatus> {
                self.interface.set_font_role(role)
            }
            /// Resolves a named font directly through the concrete class contract.
            pub fn set_named_font(&mut self, name: &str) -> Result<(), NcStatus> {
                self.interface.set_named_font(NcCopyFromBufferOffset::new(0, name.as_bytes()))
            }
        }
    };
    ($interface:ty) => {
        impl Widget<$interface> {
            /// Selects typography through this same widget object's optional capability.
            pub fn set_font_role(&mut self, role: FontRole) -> Result<(), NcStatus> {
                self.interface.query::<IFontStyleAbi>()?.set_font_role(role)
            }
            /// Resolves a named font once in this widget's resource bundle.
            pub fn set_named_font(&mut self, name: &str) -> Result<(), NcStatus> {
                self.interface
                    .query::<IFontStyleAbi>()?
                    .set_named_font(NcCopyFromBufferOffset::new(0, name.as_bytes()))
            }
        }
    };
}
font_proxy!(direct TextboxAbi);
font_proxy!(direct TextAreaAbi);
font_proxy!(IMultilineEditorAbi);
font_proxy!(direct TextBlockAbi);
font_proxy!(direct SliderAbi);
font_proxy!(direct NumberAbi);
font_proxy!(direct ColorSwatchAbi);
font_proxy!(direct ListBoxAbi);
font_proxy!(direct ListItemAbi);

impl Widget<ListItemAbi> {
    /// Selects an icon role directly through the concrete ListItem class contract.
    pub fn set_icon_role(&mut self, icon: ContentIcon) -> Result<(), NcStatus> {
        self.interface.set_icon_role(icon)
    }
    /// Resolves a named icon directly through the concrete ListItem class contract.
    pub fn set_named_icon(&mut self, name: &str) -> Result<(), NcStatus> {
        self.interface.set_named_icon(NcCopyFromBufferOffset::new(0, name.as_bytes()))
    }
}
impl Widget<ButtonAbi> {
    /// Selects typography directly through the Button class contract.
    pub fn set_font_role(&mut self, role: FontRole) -> Result<(), NcStatus> {
        self.interface.set_font_role(role)
    }
    /// Resolves a named font directly through the Button class contract.
    pub fn set_named_font(&mut self, name: &str) -> Result<(), NcStatus> {
        self.interface.set_named_font(NcCopyFromBufferOffset::new(0, name.as_bytes()))
    }
    /// Selects an adjacent semantic icon directly through the Button class contract.
    pub fn set_icon_role(&mut self, icon: ContentIcon) -> Result<(), NcStatus> {
        self.interface.set_icon_role(icon)
    }
    /// Resolves a named adjacent icon directly through the Button class contract.
    pub fn set_named_icon(&mut self, name: &str) -> Result<(), NcStatus> {
        self.interface.set_named_icon(NcCopyFromBufferOffset::new(0, name.as_bytes()))
    }
    /// Reads the current mask controlling center fill on an unframed button.
    pub fn fill_states(&self) -> Result<FillStates, NcStatus> {
        self.interface.fill_states()
    }
    /// Changes the validated paint-only fill mask.
    pub fn set_fill_states(&mut self, states: FillStates) -> Result<(), NcStatus> {
        self.interface.set_fill_states(states)
    }
    /// Installs a renderer-owned image directly through the Button class contract.
    pub fn set_image(&mut self, image: ComRef<'_, ITextureAbi>, fit_width: bool) -> Result<(), NcStatus> {
        self.interface.set_image(image, fit_width)
    }
    /// Returns to label-only content without generating an activation event.
    pub fn clear_image(&mut self) -> Result<(), NcStatus> {
        self.interface.clear_image()
    }
}

/// Typed static wrapper for the concrete Combo class and its inherited widget state.
pub type ComboWidget = Widget<ComboAbi>;
impl Widget<ComboAbi> {
    /// Returns the NCIDL-defined selection value without a parallel facade record.
    pub fn selection(&self) -> Result<ChoiceSelection, NcStatus> {
        ChoiceSelection::try_from(self.interface.selection()?).map_err(|_| neocogi_com::NC_E_INVALID_ARG)
    }
    /// Copies the exact selected label or visible placeholder currently painted by the header.
    pub fn display_text(&self) -> Result<String, NcStatus> {
        self.interface.display_text()?.to_string()
    }
    /// Returns the current number of retained choice labels.
    pub fn choice_count(&self) -> Result<usize, NcStatus> {
        usize::try_from(self.interface.choice_count()?).map_err(|_| neocogi_com::NC_E_INVALID_ARG)
    }
    /// Copies one exact leaf label by its preorder ordinal after strict range validation.
    pub fn choice_label(&self, ordinal: usize) -> Result<String, NcStatus> {
        let ordinal = u64::try_from(ordinal).map_err(|_| neocogi_com::NC_E_INVALID_ARG)?;
        self.interface.choice_label(ordinal)?.to_string()
    }
    /// Atomically replaces all labels and the explicit optional selection.
    pub fn replace_items<Item: AsRef<str>>(&mut self, items: &[Item], selection: ChoiceSelection) -> Result<(), NcStatus> {
        let (menu_text, entries) = encoded_combo_items(items)?;
        self.interface.replace_menu(
            NcCopyFromBufferOffset::new(0, &menu_text),
            NcCopyFromBufferOffset::new(0, &entries),
            selection.into(),
        )
    }
    /// Atomically replaces the generated preorder hierarchy, shared text, and selection.
    pub fn replace_menu(&mut self, menu_text: &str, entries: &[ChoiceMenuEntry], selection: ChoiceSelection) -> Result<(), NcStatus> {
        let entries = combo_menu_abi(entries);
        self.interface.replace_menu(
            NcCopyFromBufferOffset::new(0, menu_text.as_bytes()),
            NcCopyFromBufferOffset::new(0, &entries),
            selection.into(),
        )
    }
    /// Silently replaces the committed optional selection without clamping invalid indices.
    pub fn set_selection(&mut self, selection: ChoiceSelection) -> Result<(), NcStatus> {
        self.interface.set_selection(selection.into())
    }
    /// Copies the fallback retained independently from the current selection.
    pub fn placeholder(&self) -> Result<String, NcStatus> {
        self.interface.placeholder()?.to_string()
    }
    /// Replaces the fallback without changing selection or publishing a user event.
    pub fn set_placeholder(&mut self, placeholder: &str) -> Result<(), NcStatus> {
        self.interface.set_placeholder(NcCopyFromBufferOffset::new(0, placeholder.as_bytes()))
    }
    /// Selects typography directly through the Combo class contract.
    pub fn set_font_role(&mut self, role: FontRole) -> Result<(), NcStatus> {
        self.interface.set_font_role(role)
    }
    /// Resolves a named font directly through the Combo class contract.
    pub fn set_named_font(&mut self, name: &str) -> Result<(), NcStatus> {
        self.interface.set_named_font(NcCopyFromBufferOffset::new(0, name.as_bytes()))
    }
}

/// Packs a flat convenience list into the same generated menu contract used by nested choices.
fn encoded_combo_items<Item: AsRef<str>>(items: &[Item]) -> Result<(Vec<u8>, Vec<ChoiceMenuEntryAbi>), NcStatus> {
    let mut text = Vec::new();
    let mut entries = Vec::with_capacity(items.len());
    for (id, item) in items.iter().enumerate() {
        let id = u64::try_from(id).map_err(|_| neocogi_com::NC_E_INVALID_ARG)?;
        let label = combo_label_range(&mut text, item.as_ref())?;
        entries.push(ChoiceMenuEntry::Choice { id, label }.into());
    }
    Ok((text, entries))
}

/// Converts generated values to their fixed-layout call representation without another model.
fn combo_menu_abi(entries: &[ChoiceMenuEntry]) -> Vec<ChoiceMenuEntryAbi> {
    entries.iter().copied().map(Into::into).collect()
}

/// Appends one label and returns its checked range in the shared operation buffer.
fn combo_label_range(text: &mut Vec<u8>, label: &str) -> Result<MenuTextRange, NcStatus> {
    let first = u32::try_from(text.len()).map_err(|_| neocogi_com::NC_E_INVALID_ARG)?;
    let count = u32::try_from(label.len()).map_err(|_| neocogi_com::NC_E_INVALID_ARG)?;
    text.extend_from_slice(label.as_bytes());
    Ok(MenuTextRange { first, count })
}

impl ListBoxWidget {
    /// Installs this renderer-owned image directly through the concrete class contract.
    pub fn set_image(&mut self, image: ComRef<'_, ITextureAbi>, fit_width: bool) -> Result<(), NcStatus> {
        self.interface.set_image(image, fit_width)
    }
    /// Returns to label-only content without generating an activation event.
    pub fn clear_image(&mut self) -> Result<(), NcStatus> {
        self.interface.clear_image()
    }
}

impl WidgetFactory {
    /// Creates an unmounted button with one lifetime-bound sink.
    /// The generated adapter owns the sender and mapper; construction failure releases both.
    pub fn button<M: 'static>(
        &self,
        label: &str,
        sender: std::sync::mpsc::Sender<M>,
        map: impl FnMut(crate::button::Event) -> M + 'static,
    ) -> Result<ButtonWidget, NcStatus> {
        let sink = crate::button::sink(sender, map)?;
        self.button_with_sink(label, Some(sink.as_com_ref()))
    }

    /// Creates an explicitly unobserved button without allocating a destination.
    pub fn button_unobserved(&self, label: &str) -> Result<ButtonWidget, NcStatus> {
        self.button_with_sink(label, None)
    }
    /// Creates an unmounted checkbox with one lifetime-bound sink.
    /// The generated adapter owns the sender and mapper; construction failure releases both.
    pub fn checkbox<M: 'static>(
        &self,
        label: &str,
        checked: bool,
        sender: std::sync::mpsc::Sender<M>,
        map: impl FnMut(crate::checkbox::Event) -> M + 'static,
    ) -> Result<CheckboxWidget, NcStatus> {
        let sink = crate::checkbox::sink(sender, map)?;
        self.checkbox_with_sink(label, checked, Some(sink.as_com_ref()))
    }

    /// Creates an explicitly unobserved checkbox without allocating a destination.
    pub fn checkbox_unobserved(&self, label: &str, checked: bool) -> Result<CheckboxWidget, NcStatus> {
        self.checkbox_with_sink(label, checked, None)
    }
    /// Creates an unmounted textbox with one lifetime-bound sink.
    /// The generated adapter owns the sender and mapper; construction failure releases both.
    pub fn textbox<M: 'static>(
        &self,
        text: &str,
        sender: std::sync::mpsc::Sender<M>,
        map: impl FnMut(crate::textbox::Event) -> M + 'static,
    ) -> Result<TextboxWidget, NcStatus> {
        let sink = crate::textbox::sink(sender, map)?;
        self.textbox_with_sink(text, Some(sink.as_com_ref()))
    }

    /// Creates an explicitly unobserved textbox without allocating a destination.
    pub fn textbox_unobserved(&self, text: &str) -> Result<TextboxWidget, NcStatus> {
        self.textbox_with_sink(text, None)
    }
    /// Creates an unmounted text area with one lifetime-bound sink.
    /// The generated adapter owns the sender and mapper; construction failure releases both.
    pub fn text_area<M: 'static>(
        &self,
        text: &str,
        wrapping: TextWrapping,
        scrolling: bool,
        sender: std::sync::mpsc::Sender<M>,
        map: impl FnMut(crate::text_area::Event) -> M + 'static,
    ) -> Result<TextAreaWidget, NcStatus> {
        let sink = crate::text_area::sink(sender, map)?;
        self.text_area_with_sink(text, wrapping, scrolling, Some(sink.as_com_ref()))
    }

    /// Creates an explicitly unobserved text area without allocating a destination.
    pub fn text_area_unobserved(&self, text: &str, wrapping: TextWrapping, scrolling: bool) -> Result<TextAreaWidget, NcStatus> {
        self.text_area_with_sink(text, wrapping, scrolling, None)
    }
    /// Creates an unmounted slider with one lifetime-bound sink.
    /// The generated adapter owns the sender and mapper; construction failure releases both.
    pub fn slider<M: 'static>(
        &self,
        options: SliderOptions,
        sender: std::sync::mpsc::Sender<M>,
        map: impl FnMut(crate::slider::Event) -> M + 'static,
    ) -> Result<SliderWidget, NcStatus> {
        let sink = crate::slider::sink(sender, map)?;
        self.slider_with_sink(options, Some(sink.as_com_ref()))
    }

    /// Creates an explicitly unobserved slider without allocating a destination.
    pub fn slider_unobserved(&self, options: SliderOptions) -> Result<SliderWidget, NcStatus> {
        self.slider_with_sink(options, None)
    }
    /// Creates an unmounted number with one lifetime-bound sink.
    /// The generated adapter owns the sender and mapper; construction failure releases both.
    pub fn number<M: 'static>(
        &self,
        options: NumberOptions,
        sender: std::sync::mpsc::Sender<M>,
        map: impl FnMut(crate::number::Event) -> M + 'static,
    ) -> Result<NumberWidget, NcStatus> {
        let sink = crate::number::sink(sender, map)?;
        self.number_with_sink(options, Some(sink.as_com_ref()))
    }

    /// Creates an explicitly unobserved number without allocating a destination.
    pub fn number_unobserved(&self, options: NumberOptions) -> Result<NumberWidget, NcStatus> {
        self.number_with_sink(options, None)
    }
    /// Creates an unmounted list item with one lifetime-bound sink.
    /// The generated adapter owns the sender and mapper; construction failure releases both.
    pub fn list_item<M: 'static>(
        &self,
        label: &str,
        sender: std::sync::mpsc::Sender<M>,
        map: impl FnMut(crate::list_item::Event) -> M + 'static,
    ) -> Result<ListItemWidget, NcStatus> {
        let sink = crate::list_item::sink(sender, map)?;
        self.list_item_with_sink(label, Some(sink.as_com_ref()))
    }

    /// Creates an explicitly unobserved list item without allocating a destination.
    pub fn list_item_unobserved(&self, label: &str) -> Result<ListItemWidget, NcStatus> {
        self.list_item_with_sink(label, None)
    }
    /// Creates an unmounted list box with one lifetime-bound sink.
    /// The generated adapter owns the sender and mapper; construction failure releases both.
    pub fn list_box<M: 'static>(
        &self,
        label: &str,
        sender: std::sync::mpsc::Sender<M>,
        map: impl FnMut(crate::list_box::Event) -> M + 'static,
    ) -> Result<ListBoxWidget, NcStatus> {
        let sink = crate::list_box::sink(sender, map)?;
        self.list_box_with_sink(label, Some(sink.as_com_ref()))
    }

    /// Creates an explicitly unobserved list box without allocating a destination.
    pub fn list_box_unobserved(&self, label: &str) -> Result<ListBoxWidget, NcStatus> {
        self.list_box_with_sink(label, None)
    }
    /// Creates an unmounted scrollbar with one lifetime-bound sink.
    /// The generated adapter owns the sender and mapper; construction failure releases both.
    pub fn scrollbar<M: 'static>(
        &self,
        range: ScrollRange,
        sender: std::sync::mpsc::Sender<M>,
        map: impl FnMut(crate::scrollbar::Event) -> M + 'static,
    ) -> Result<ScrollbarWidget, NcStatus> {
        let sink = crate::scrollbar::sink(sender, map)?;
        self.scrollbar_with_sink(range, Some(sink.as_com_ref()))
    }

    /// Creates an explicitly unobserved scrollbar without allocating a destination.
    pub fn scrollbar_unobserved(&self, range: ScrollRange) -> Result<ScrollbarWidget, NcStatus> {
        self.scrollbar_with_sink(range, None)
    }
    /// Creates an unmounted split view with one lifetime-bound sink.
    /// The generated adapter owns the sender and mapper; construction failure releases both.
    pub fn split_view<M: 'static>(
        &self,
        settings: SplitOptions,
        sender: std::sync::mpsc::Sender<M>,
        map: impl FnMut(crate::split_view::Event) -> M + 'static,
    ) -> Result<SplitViewWidget, NcStatus> {
        let sink = crate::split_view::sink(sender, map)?;
        self.split_view_with_sink(settings, Some(sink.as_com_ref()))
    }

    /// Creates an explicitly unobserved split view without allocating a destination.
    pub fn split_view_unobserved(&self, settings: SplitOptions) -> Result<SplitViewWidget, NcStatus> {
        self.split_view_with_sink(settings, None)
    }
}

impl Ui {
    /// Constructs a choice header with one sink owned by its core choice model.
    pub fn combo_box<Item: AsRef<str>, M: 'static>(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        items: &[Item],
        selection: ChoiceSelection,
        sender: std::sync::mpsc::Sender<M>,
        map: impl FnMut(crate::combo::Event) -> M + 'static,
    ) -> Result<ComboWidget, NcStatus> {
        let sink = crate::combo::sink(sender, map)?;
        self.combo_box_with_sink(parent, items, selection, Some(sink.as_com_ref()))
    }
    /// Constructs an unobserved choice header without a dummy destination allocation.
    pub fn combo_box_unobserved<Item: AsRef<str>>(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        items: &[Item],
        selection: ChoiceSelection,
    ) -> Result<ComboWidget, NcStatus> {
        self.combo_box_with_sink(parent, items, selection, None)
    }
    /// Constructs a hierarchical choice header with one constructor-bound sink.
    pub fn combo_menu<M: 'static>(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        text: &str,
        entries: &[ChoiceMenuEntry],
        selection: ChoiceSelection,
        sender: std::sync::mpsc::Sender<M>,
        map: impl FnMut(crate::combo::Event) -> M + 'static,
    ) -> Result<ComboWidget, NcStatus> {
        let sink = crate::combo::sink(sender, map)?;
        self.combo_menu_with_sink(parent, text, entries, selection, Some(sink.as_com_ref()))
    }
    /// Constructs an unobserved hierarchical choice header.
    pub fn combo_menu_unobserved(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        text: &str,
        entries: &[ChoiceMenuEntry],
        selection: ChoiceSelection,
    ) -> Result<ComboWidget, NcStatus> {
        self.combo_menu_with_sink(parent, text, entries, selection, None)
    }
}

impl Ui {
    /// Creates the hidden picker with one upward sink retained for its lifetime.
    pub fn file_dialog<M: 'static>(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        sender: std::sync::mpsc::Sender<M>,
        map: impl FnMut(crate::file_dialog::Event) -> M + 'static,
    ) -> Result<crate::FileDialog, NcStatus> {
        let sink = crate::file_dialog::sink(sender, map)?;
        self.file_dialog_with_sink(parent, Some(sink.as_com_ref()))
    }
    /// Creates an explicitly unobserved picker without allocating a destination.
    pub fn file_dialog_unobserved(&self, parent: ComRef<'_, IWindowAbi>) -> Result<crate::FileDialog, NcStatus> {
        self.file_dialog_with_sink(parent, None)
    }
}

/// Completes parent binding before a newly constructed capability becomes public.
fn attach_optional_sink<C: neocogi_com::events::EventSource, S: neocogi_com::events::MessageSinkInterface<Message = neocogi_com::events::EventOf<C>>>(
    source: ComPtr<C>,
    sink: Option<ComRef<'_, S>>,
) -> Result<ComPtr<C>, NcStatus> {
    if let Some(sink) = sink {
        neocogi_com::events::attach_source(source.as_com_ref(), sink)?;
    }
    Ok(source)
}
