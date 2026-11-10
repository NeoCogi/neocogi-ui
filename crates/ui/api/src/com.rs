//! Typed static clients for dynamically or statically linked COM implementations.
//!
//! Clients keep no shadow widget state or topology. Every edit crosses the
//! checked interface and every returned snapshot owns its storage. The retained implementation
//! lives in the UI plugin; this facade depends only on the contracts and COM
//! object support.

pub use neocogi_gfx_support::abi::RenderVertex;
pub use neocogi_platform_contracts::render::abi::{
    Color, IRenderStream, IRenderStreamAbi, IRenderer, IRendererAbi, IRgbaImage, IRgbaImageAbi, ITexture, ITextureAbi, ITextureFactory, ITextureFactoryAbi,
    MAX_IMAGE_BYTES, Rect, RenderInfo, Size, TextureRegion, color, rgba_bytes,
};
pub use neocogi_platform_contracts::render::platform::{
    IInputSink, IInputSinkAbi, IPlatformWindow, IPlatformWindowAbi, KeyAction, KeyCode, KeyStroke, Platform, PlatformAbi, Point, PollStatus, TextInputArea,
    UiInput,
};
pub use neocogi_ui::abi::core::*;
/// Complete generated built-in node contract namespace, including its distinct module metadata.
pub mod nodes {
    pub use neocogi_ui::abi::nodes::*;
}
pub use neocogi_ui::abi::nodes::{
    Button, ButtonAbi, Checkbox, CheckboxAbi, ChildCrossSize, ColorSwatch, ColorSwatchAbi, Combo, ComboAbi, ContentIcon, CrossSize, Disclosure, DisclosureAbi,
    DisclosureStyle, FillStates, FlowDirection, Grid, GridAbi, GridExtent, GridLimits, LayoutTrack, Linear, LinearAbi, LinearEdge, ListBox, ListBoxAbi,
    ListItem, ListItemAbi, Number, NumberAbi, NumberOptions, ScrollArea, ScrollAreaAbi, ScrollAxis, ScrollRange, Scrollbar, ScrollbarAbi, Slider, FormAbi,
    SliderAbi, SliderOptions, SplitAxis, SplitOptions, SplitView, SplitViewAbi, TextArea, TextAreaAbi, TextBlock, TextBlockAbi, Textbox, TextboxAbi, UiFactory,
    UiFactoryAbi,
};
pub use neocogi_gfx_support::abi::{
    AssetLoader, AssetLoaderAbi, DrawPoint, GfxPluginFactory, GfxPluginFactoryAbi, IAssetBuilder, IAssetBuilderAbi, IGfxDiagnostics, IGfxDiagnosticsAbi,
    GfxCompositor, GfxCompositorAbi, PngDecoder, PngDecoderAbi, SolidVertex,
};
#[cfg(feature = "skin")]
pub use neocogi_appearance::skin::{
    ISkinDiagnostics, ISkinDiagnosticsAbi, PaletteEdit, SkinPluginFactory, SkinPluginFactoryAbi, StyleCompiler, StyleCompilerAbi, ThemeLoader, ThemeLoaderAbi,
};
use neocogi_com::{ComBox, WeakComBox, ComInterface, ComPtr, ComRef, INcStringComPtrExt, IUnknown, NcCopyFromBufferOffset, NcStatus};
mod containers;
pub use containers::{DisclosureWidget, FormWidget, GridWidget, LinearWidget, ScrollAreaWidget, SplitViewWidget};
mod mount;
pub use mount::Mountable;

/// Reusable state composed into concrete Rust widget implementations.
pub mod widget;

/// Restricts facade storage to the two COM handles with widget lifetime semantics.
mod storage {
    use super::*;
    /// Prevents arbitrary third-party storage from inventing widget ownership.
    pub trait Sealed {}
    impl<I: ComInterface> Sealed for ComBox<I> {}
    impl<I: ComInterface> Sealed for WeakComBox<I> {}
}

/// Compile-time storage policy for a widget facade; never a runtime trait object.
/// Owners and observers use the same generated client methods. Only observation
/// differs: a query must return a weak handle without consuming the owner.
pub trait WidgetStorage<I: ComInterface>: storage::Sealed {
    /// Selects a validated non-owning projection while preserving current storage.
    fn observe<U: ComInterface>(&self) -> Result<WeakComBox<U>, NcStatus>;
}
impl<I: ComInterface> WidgetStorage<I> for ComBox<I> {
    /// Retains only the returned observer; it cannot create another widget owner.
    fn observe<U: ComInterface>(&self) -> Result<WeakComBox<U>, NcStatus> {
        self.query_weak()
    }
}
impl<I: ComInterface> WidgetStorage<I> for WeakComBox<I> {
    /// Reuses cached object identity and the provider's immutable projection set.
    fn observe<U: ComInterface>(&self) -> Result<WeakComBox<U>, NcStatus> {
        self.query()
    }
}

/// Typed editing facade containing exactly one owner or observer handle.
/// The default is a weak form field; `OwnedWidget` selects exclusive storage.
/// Method forwarding is statically resolved and stores no duplicate weak facade.
pub struct Widget<I: ComInterface, P = WeakComBox<I>> {
    /// Sole COM handle: ownership and destruction follow P's ordinary semantics.
    interface: P,
    /// Associates generic storage with its exact contract without runtime data.
    contract: core::marker::PhantomData<fn() -> I>,
}

impl<I: ComInterface, P: core::fmt::Debug> core::fmt::Debug for Widget<I, P> {
    /// Format the handle identity without entering a widget method.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Widget").field(&self.interface).finish()
    }
}

impl<I: ComInterface> Clone for Widget<I> {
    /// Clones weak access without retaining widget resources or duplicating ownership.
    fn clone(&self) -> Self {
        Self::from_interface(self.interface.clone())
    }
}

impl<I: ComInterface, P: WidgetStorage<I>> Widget<I, P> {
    /// Adopts one validated owner or observer without creating any other handle.
    pub fn from_interface(interface: P) -> Self {
        Self {
            interface,
            contract: core::marker::PhantomData,
        }
    }

    /// Borrows the exact stored handle without changing its ownership policy.
    pub fn as_interface(&self) -> &P {
        &self.interface
    }

    /// Borrows the stored handle for custom mutable commands without cloning.
    /// Provider calls still check liveness and the allocation-wide borrow gate.
    pub fn as_interface_mut(&mut self) -> &mut P {
        &mut self.interface
    }

    /// Reads constructor/setter configuration through COM, excluding effective runtime policy.
    /// A newly constructed widget reports its provider's defaults until explicitly edited.
    pub fn requested_flags(&self) -> Result<WidgetOptions, NcStatus> {
        self.node_interface()?.requested_flags()
    }

    /// Replaces all requested flags, clearing every omitted bit through the provider.
    pub fn replace_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.node_interface()?.replace_flags(flags)
    }

    /// Sets selected requested bits while preserving unrelated application settings.
    pub fn set_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.node_interface()?.set_flags(flags)
    }

    /// Clears selected bits without writing effective input policy into settings.
    pub fn clear_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.node_interface()?.clear_flags(flags)
    }

    /// Obtains weak common-widget access; attachment requires transferring an owner.
    pub fn node_interface(&self) -> Result<WeakComBox<IWidgetAbi>, NcStatus> {
        self.interface.observe()
    }
}

/// One unmounted widget owner with the same typed editing methods as a weak field.
/// Insertion consumes it. Explicitly call `handle` if later application edits need
/// an observer; constructing and moving an owner alone creates no weak reference.
///
/// ```compile_fail
/// fn duplicate(owner: neocogi_ui::OwnedWidget<neocogi_ui::ButtonAbi>) {
///     let second_owner = owner.clone(); // exclusive ownership cannot be cloned
/// }
/// ```
pub type OwnedWidget<I> = Widget<I, ComBox<I>>;

impl<I: ComInterface> OwnedWidget<I> {
    /// Creates a non-owning form field before transferring the owner to a container.
    pub fn handle(&self) -> Widget<I> {
        Widget::from_interface(self.interface.downgrade())
    }
    /// Transfers the sole interface owner into a lower-level constructor or container.
    pub fn into_interface(self) -> ComBox<I> {
        self.interface
    }
}

/// Static convenience wrapper around the dynamically constructed Button class projection.
pub type ButtonWidget = Widget<ButtonAbi>;

impl<P> Widget<ButtonAbi, P>
where
    P: WidgetStorage<ButtonAbi> + core::ops::DerefMut<Target = <ButtonAbi as neocogi_com::ComClientInterface>::Client<neocogi_com::ThreadLocal>>,
{
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
        impl<P> Widget<$interface, P>
        where
            P: WidgetStorage<$interface> + core::ops::DerefMut<Target = <$interface as neocogi_com::ComClientInterface>::Client<neocogi_com::ThreadLocal>>,
        {
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

impl<P> Widget<CheckboxAbi, P>
where
    P: WidgetStorage<CheckboxAbi> + core::ops::DerefMut<Target = <CheckboxAbi as neocogi_com::ComClientInterface>::Client<neocogi_com::ThreadLocal>>,
{
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
        impl<P> Widget<$interface, P>
        where
            P: WidgetStorage<$interface> + core::ops::DerefMut<Target = <$interface as neocogi_com::ComClientInterface>::Client<neocogi_com::ThreadLocal>>,
        {
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
        impl<P> Widget<$interface, P>
        where
            P: WidgetStorage<$interface> + core::ops::DerefMut<Target = <$interface as neocogi_com::ComClientInterface>::Client<neocogi_com::ThreadLocal>>,
        {
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
impl<P> Widget<TextAreaAbi, P>
where
    P: WidgetStorage<TextAreaAbi> + core::ops::DerefMut<Target = <TextAreaAbi as neocogi_com::ComClientInterface>::Client<neocogi_com::ThreadLocal>>,
{
    /// Queries scrolling on this same root identity; it cannot replace the fixed editor content.
    pub fn scrolling(&self) -> Result<Widget<IScrollingAbi>, NcStatus> {
        Ok(Widget::from_interface(self.interface.observe()?))
    }
}
/// Shares numeric convenience methods while each plugin retains its arithmetic rules.
macro_rules! numeric_proxy {
    ($interface:ty) => {
        impl<P> Widget<$interface, P>
        where
            P: WidgetStorage<$interface> + core::ops::DerefMut<Target = <$interface as neocogi_com::ComClientInterface>::Client<neocogi_com::ThreadLocal>>,
        {
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
impl<P> Widget<ColorSwatchAbi, P>
where
    P: WidgetStorage<ColorSwatchAbi> + core::ops::DerefMut<Target = <ColorSwatchAbi as neocogi_com::ComClientInterface>::Client<neocogi_com::ThreadLocal>>,
{
    /// Reads the packed RGBA paint color.
    pub fn fill(&self) -> Result<u32, NcStatus> {
        self.interface.fill()
    }
    /// Replaces color without publishing a user event or changing preferred size.
    pub fn set_fill(&mut self, rgba: u32) -> Result<(), NcStatus> {
        self.interface.set_fill(rgba)
    }
}
impl<P> Widget<ScrollbarAbi, P>
where
    P: WidgetStorage<ScrollbarAbi> + core::ops::DerefMut<Target = <ScrollbarAbi as neocogi_com::ComClientInterface>::Client<neocogi_com::ThreadLocal>>,
{
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
    /// Lends this runtime's concrete construction capability to an independent plugin.
    /// The call-scoped borrow cannot retain the facade or change its runtime binding.
    pub fn as_com_ref(&self) -> ComRef<'_, UiFactoryAbi> {
        self.interface.as_com_ref()
    }

    /// Supplies runtime authority for an independent COM widget's inline WidgetState.
    pub fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        self.interface.query::<IConstructionHostAbi>()?.host()
    }

    /// Creates a fixed multiline editor and its real scroll children with one shared offset model.
    pub fn text_area_with_queue(
        &self,
        text: &str,
        wrapping: TextWrapping,
        scrolling: bool,
        queue: Option<ComRef<'_, crate::events::IEventReceiverAbi>>,
    ) -> Result<OwnedWidget<TextAreaAbi>, NcStatus> {
        Ok(OwnedWidget::from_interface(self.interface.create_text_area(
            NcCopyFromBufferOffset::new(0, text.as_bytes()),
            wrapping,
            scrolling,
            neocogi_com::NcValueRef::new(&crate::events::EventBinding::from_queue(queue)?)?,
        )?))
    }
    /// Constructs an initialized, unmounted checkbox in this runtime.
    pub fn checkbox_with_queue(
        &self,
        label: &str,
        checked: bool,
        queue: Option<ComRef<'_, crate::events::IEventReceiverAbi>>,
    ) -> Result<OwnedWidget<CheckboxAbi>, NcStatus> {
        Ok(OwnedWidget::from_interface(self.interface.create_checkbox(
            checked,
            NcCopyFromBufferOffset::new(0, label.as_bytes()),
            neocogi_com::NcValueRef::new(&crate::events::EventBinding::from_queue(queue)?)?,
        )?))
    }
    /// Constructs an initialized, unmounted textbox in this runtime.
    pub fn textbox_with_queue(&self, text: &str, queue: Option<ComRef<'_, crate::events::IEventReceiverAbi>>) -> Result<OwnedWidget<TextboxAbi>, NcStatus> {
        Ok(OwnedWidget::from_interface(self.interface.create_textbox(
            NcCopyFromBufferOffset::new(0, text.as_bytes()),
            neocogi_com::NcValueRef::new(&crate::events::EventBinding::from_queue(queue)?)?,
        )?))
    }
    /// Constructs an initialized, unmounted text block in this runtime.
    pub fn text_block(&self, text: &str, wrapping: TextWrapping) -> Result<OwnedWidget<TextBlockAbi>, NcStatus> {
        Ok(OwnedWidget::from_interface(
            self.interface.create_text_block(NcCopyFromBufferOffset::new(0, text.as_bytes()), wrapping)?,
        ))
    }
    /// Constructs an initialized, unmounted slider in this runtime.
    pub fn slider_with_queue(
        &self,
        options: SliderOptions,
        queue: Option<ComRef<'_, crate::events::IEventReceiverAbi>>,
    ) -> Result<OwnedWidget<SliderAbi>, NcStatus> {
        Ok(OwnedWidget::from_interface(self.interface.create_slider(
            options,
            neocogi_com::NcValueRef::new(&crate::events::EventBinding::from_queue(queue)?)?,
        )?))
    }
    /// Constructs an initialized, unmounted number in this runtime.
    pub fn number_with_queue(
        &self,
        options: NumberOptions,
        queue: Option<ComRef<'_, crate::events::IEventReceiverAbi>>,
    ) -> Result<OwnedWidget<NumberAbi>, NcStatus> {
        Ok(OwnedWidget::from_interface(self.interface.create_number(
            options,
            neocogi_com::NcValueRef::new(&crate::events::EventBinding::from_queue(queue)?)?,
        )?))
    }
    /// Constructs an initialized, unmounted color swatch in this runtime.
    pub fn color_swatch(&self, rgba: u32, label: &str) -> Result<OwnedWidget<ColorSwatchAbi>, NcStatus> {
        Ok(OwnedWidget::from_interface(
            self.interface.create_color_swatch(rgba, NcCopyFromBufferOffset::new(0, label.as_bytes()))?,
        ))
    }
    /// Constructs an initialized, unmounted list item in this runtime.
    pub fn list_item_with_queue(&self, label: &str, queue: Option<ComRef<'_, crate::events::IEventReceiverAbi>>) -> Result<OwnedWidget<ListItemAbi>, NcStatus> {
        Ok(OwnedWidget::from_interface(self.interface.create_list_item(
            NcCopyFromBufferOffset::new(0, label.as_bytes()),
            neocogi_com::NcValueRef::new(&crate::events::EventBinding::from_queue(queue)?)?,
        )?))
    }
    /// Constructs an initialized, unmounted list box in this runtime.
    pub fn list_box_with_queue(&self, label: &str, queue: Option<ComRef<'_, crate::events::IEventReceiverAbi>>) -> Result<OwnedWidget<ListBoxAbi>, NcStatus> {
        Ok(OwnedWidget::from_interface(self.interface.create_list_box(
            NcCopyFromBufferOffset::new(0, label.as_bytes()),
            neocogi_com::NcValueRef::new(&crate::events::EventBinding::from_queue(queue)?)?,
        )?))
    }
    /// Constructs an initialized, unmounted scrollbar in this runtime.
    pub fn scrollbar_with_queue(
        &self,
        range: ScrollRange,
        queue: Option<ComRef<'_, crate::events::IEventReceiverAbi>>,
    ) -> Result<OwnedWidget<ScrollbarAbi>, NcStatus> {
        Ok(OwnedWidget::from_interface(self.interface.create_scrollbar(
            range,
            neocogi_com::NcValueRef::new(&crate::events::EventBinding::from_queue(queue)?)?,
        )?))
    }

    /// Adopts a typed factory obtained from UI runtime activation.
    pub fn from_interface(interface: ComPtr<UiFactoryAbi>) -> Self {
        Self { interface }
    }

    /// Creates and initializes an unmounted button in one concrete API operation.
    pub fn button_with_queue(&self, label: &str, queue: Option<ComRef<'_, crate::events::IEventReceiverAbi>>) -> Result<OwnedWidget<ButtonAbi>, NcStatus> {
        Ok(OwnedWidget::from_interface(self.interface.create_button(
            NcCopyFromBufferOffset::new(0, label.as_bytes()),
            neocogi_com::NcValueRef::new(&crate::events::EventBinding::from_queue(queue)?)?,
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
        let requested = crate::geometry::virtual_rectangle(rect)?;
        let parent = crate::geometry::virtual_rectangle(self.scope.clip)?;
        let result = parent.checked_intersect(&requested).map_err(crate::geometry::status)?;
        let clip = if let Some(r) = result {
            Rect {
                x: i32::try_from(r.x.to_bits() / 4096).map_err(|_| neocogi_com::NC_E_OUT_OF_RANGE)?,
                y: i32::try_from(r.y.to_bits() / 4096).map_err(|_| neocogi_com::NC_E_OUT_OF_RANGE)?,
                width: i32::try_from(r.width.to_bits() / 4096).map_err(|_| neocogi_com::NC_E_OUT_OF_RANGE)?,
                height: i32::try_from(r.height.to_bits() / 4096).map_err(|_| neocogi_com::NC_E_OUT_OF_RANGE)?,
            }
        } else {
            Rect {
                x: self.scope.clip.x.max(rect.x),
                y: self.scope.clip.y.max(rect.y),
                width: 0,
                height: 0,
            }
        };
        Ok(Self {
            interface: self.interface,
            scope: PaintScope { invocation: self.scope.invocation, clip },
        })
    }
    /// Begins generic graphics recording in this widget's committed visible client rectangle.
    pub fn begin_render_view(
        &mut self,
        resources: ComRef<'_, neocogi_platform_contracts::render::abi::IRenderResourcesAbi>,
    ) -> Result<neocogi_com::ComBox<neocogi_platform_contracts::render::abi::IRenderViewAbi>, NcStatus> {
        self.interface.begin_render_view(self.scope, resources)
    }

    /// Records bounded integer pixel coordinates through the shared fixed-point UI implementation.
    pub fn stroke_line(&mut self, from: Point, to: Point, width: i32, rgba: u32) -> Result<(), NcStatus> {
        self.interface.stroke_line(self.scope, from, to, width, rgba)
    }
    /// Records bounded integer polygon coordinates in declaration order.
    pub fn fill_polygon(&mut self, points: &[Point], rgba: u32) -> Result<(), NcStatus> {
        self.interface.fill_polygon(self.scope, NcCopyFromBufferOffset::new(0, points), rgba)
    }

    /// Records a finite solid line; the UI owns tessellation and the currently installed material.
    pub fn stroke_line_f32(&mut self, from: PaintPoint, to: PaintPoint, width: f32, rgba: u32) -> Result<(), NcStatus> {
        self.interface.stroke_line_f32(self.scope, from, to, width, rgba)
    }
    /// Records finite polygon positions with no application dependence on UI tessellation internals.
    pub fn fill_polygon_f32(&mut self, points: &[PaintPoint], rgba: u32) -> Result<(), NcStatus> {
        self.interface.fill_polygon_f32(self.scope, NcCopyFromBufferOffset::new(0, points), rgba)
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
    pub fn combo_box_with_queue<Item: AsRef<str>>(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        items: &[Item],
        selection: ChoiceSelection,
        queue: Option<ComRef<'_, crate::events::IEventReceiverAbi>>,
    ) -> Result<OwnedWidget<ComboAbi>, NcStatus> {
        let (menu_text, entries) = encoded_combo_items(items)?;
        let combo = self.factory.interface.create_combo_box(
            parent,
            NcCopyFromBufferOffset::new(0, &menu_text),
            NcCopyFromBufferOffset::new(0, &entries),
            selection,
            neocogi_com::NcValueRef::new(&crate::events::EventBinding::from_queue(queue)?)?,
        )?;
        Ok(OwnedWidget::from_interface(combo))
    }

    /// Creates a combo from the generated preorder menu contract and its shared UTF-8 storage.
    pub fn combo_menu_with_queue(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        menu_text: &str,
        entries: &[ChoiceMenuEntry],
        selection: ChoiceSelection,
        queue: Option<ComRef<'_, crate::events::IEventReceiverAbi>>,
    ) -> Result<OwnedWidget<ComboAbi>, NcStatus> {
        let combo = self.factory.interface.create_combo_box(
            parent,
            NcCopyFromBufferOffset::new(0, menu_text.as_bytes()),
            NcCopyFromBufferOffset::new(0, entries),
            selection,
            neocogi_com::NcValueRef::new(&crate::events::EventBinding::from_queue(queue)?)?,
        )?;
        Ok(OwnedWidget::from_interface(combo))
    }

    /// Creates a hidden standard file picker using the actual runtime's modal and COM controls.
    pub fn file_dialog_with_queue(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        queue: Option<ComRef<'_, crate::events::IEventReceiverAbi>>,
    ) -> Result<crate::FileDialog, NcStatus> {
        Ok(crate::FileDialog::from_interface(self.factory.interface.query::<IFileDialogsAbi>()?.create(
            parent,
            neocogi_com::NcValueRef::new(&crate::events::EventBinding::from_queue(queue)?)?,
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
    pub fn window<W: Mountable>(&self, title: &str, rect: Rect, root: W) -> Result<ComPtr<IWindowAbi>, (NcStatus, W)> {
        self.window_with_queue(title, rect, root, None)
    }
    /// Constructs with its final typed receiver before returning the surface.
    pub fn window_with_queue<W: Mountable>(
        &self,
        title: &str,
        rect: Rect,
        root: W,
        queue: Option<ComRef<'_, crate::events::IEventReceiverAbi>>,
    ) -> Result<ComPtr<IWindowAbi>, (NcStatus, W)> {
        let queue_binding = match crate::events::EventBinding::from_queue(queue) {
            Ok(binding) => binding,
            Err(status) => return Err((status, root)),
        };
        let binding = match neocogi_com::NcValueRef::new(&queue_binding) {
            Ok(binding) => binding,
            Err(status) => return Err((status, root)),
        };

        root.with_mount(|root| {
            self.windows
                .create_window(NcCopyFromBufferOffset::new(0, title.as_bytes()), rect, root, binding)
        })
    }
    /// Mounts a structural child under a live window from this UI.
    pub fn child_window<W: Mountable>(&self, parent: ComRef<'_, IWindowAbi>, title: &str, rect: Rect, root: W) -> Result<ComPtr<IWindowAbi>, (NcStatus, W)> {
        self.child_window_with_queue(parent, title, rect, root, None)
    }
    /// Constructs with its final typed receiver before returning the surface.
    pub fn child_window_with_queue<W: Mountable>(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        title: &str,
        rect: Rect,
        root: W,
        queue: Option<ComRef<'_, crate::events::IEventReceiverAbi>>,
    ) -> Result<ComPtr<IWindowAbi>, (NcStatus, W)> {
        let queue_binding = match crate::events::EventBinding::from_queue(queue) {
            Ok(binding) => binding,
            Err(status) => return Err((status, root)),
        };
        let binding = match neocogi_com::NcValueRef::new(&queue_binding) {
            Ok(binding) => binding,
            Err(status) => return Err((status, root)),
        };

        root.with_mount(|root| {
            self.windows
                .create_child(parent, NcCopyFromBufferOffset::new(0, title.as_bytes()), rect, root, binding)
        })
    }
    /// Mounts a hidden modal dialog; show it after initialization with set_visibility.
    pub fn dialog<W: Mountable>(&self, parent: ComRef<'_, IWindowAbi>, title: &str, rect: Rect, root: W) -> Result<ComPtr<IWindowAbi>, (NcStatus, W)> {
        self.dialog_with_queue(parent, title, rect, root, None)
    }
    /// Constructs with its final typed receiver before returning the surface.
    pub fn dialog_with_queue<W: Mountable>(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        title: &str,
        rect: Rect,
        root: W,
        queue: Option<ComRef<'_, crate::events::IEventReceiverAbi>>,
    ) -> Result<ComPtr<IWindowAbi>, (NcStatus, W)> {
        let queue_binding = match crate::events::EventBinding::from_queue(queue) {
            Ok(binding) => binding,
            Err(status) => return Err((status, root)),
        };
        let binding = match neocogi_com::NcValueRef::new(&queue_binding) {
            Ok(binding) => binding,
            Err(status) => return Err((status, root)),
        };

        root.with_mount(|root| {
            self.windows
                .create_dialog(parent, NcCopyFromBufferOffset::new(0, title.as_bytes()), rect, root, binding)
        })
    }
    /// Mounts a hidden auto-sized popup with a distinct popup capability.
    pub fn popup<W: Mountable>(&self, parent: ComRef<'_, IWindowAbi>, title: &str, root: W) -> Result<ComPtr<IPopupAbi>, (NcStatus, W)> {
        self.popup_with_queue(parent, title, root, None)
    }
    /// Constructs with its final typed receiver before returning the surface.
    pub fn popup_with_queue<W: Mountable>(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        title: &str,
        root: W,
        queue: Option<ComRef<'_, crate::events::IEventReceiverAbi>>,
    ) -> Result<ComPtr<IPopupAbi>, (NcStatus, W)> {
        let queue_binding = match crate::events::EventBinding::from_queue(queue) {
            Ok(binding) => binding,
            Err(status) => return Err((status, root)),
        };
        let binding = match neocogi_com::NcValueRef::new(&queue_binding) {
            Ok(binding) => binding,
            Err(status) => return Err((status, root)),
        };

        root.with_mount(|root| {
            self.windows
                .create_popup(parent, NcCopyFromBufferOffset::new(0, title.as_bytes()), root, binding)
        })
    }
    /// Queues a typed host event while keeping its text buffer local to this call.
    pub fn input(&self, event: UiInput, text: &str) -> Result<(), NcStatus> {
        self.runtime.input(event, NcCopyFromBufferOffset::new(0, text.as_bytes()))
    }
    /// Retains the runtime's input projection once for a host loop or concrete input interceptor.
    pub fn input_sink(&self) -> Result<ComPtr<IInputSinkAbi>, NcStatus> {
        self.runtime.query()
    }
    /// Reads the committed editor area used by the platform's native text input service.
    pub fn text_input_area(&self) -> Result<TextInputArea, NcStatus> {
        self.runtime.text_input_area()
    }
    /// Processes input and synchronizes independently recorded geometry changes.
    pub fn update(&self, size: Size) -> Result<(), NcStatus> {
        self.runtime.update(size)
    }
    /// Settles owned application messages between input phases and commits handler edits.
    /// Construct the owner once and retain its checked IApplicationDispatcher projection across frames.
    /// This call only borrows that projection; it allocates no callback or revocation state.
    pub fn update_with_events(&self, size: Size, owner: ComRef<'_, IApplicationDispatcherAbi>) -> Result<(), NcStatus> {
        self.runtime.update_with_events(size, owner)
    }
    /// Submits synchronized UI geometry to the renderer for its bound framebuffer or image surface.
    /// The returned failure owns the original status and text independently of the renderer.
    pub fn render(&self, size: Size, renderer: ComRef<'_, IRendererAbi>) -> Result<(), RenderError> {
        self.runtime.render(size, renderer)
    }
    /// Retains the current immutable bundle for restoration or another UI/renderer consumer.
    pub fn assets(&self) -> Result<ComPtr<IAssetBundleAbi>, NcStatus> {
        self.runtime.assets()
    }
    /// Replaces this UI's resources through exclusive access; call update after success.
    /// Failed native preparation returns its owned error and preserves the installed revision.
    pub fn install_assets(&mut self, assets: ComRef<'_, IAssetBundleAbi>) -> Result<(), RenderError> {
        self.runtime.install_assets(assets)
    }
    /// Terminates this UI context, synchronously dropping mounted trees and private resources.
    /// Does not publish window/popup lifecycle events or dispatch pending application work.
    /// Settle application requests first; facts already in application queues are not retracted.
    /// Busy rejection leaves the context live. Success is terminal and repeated calls succeed.
    /// Retained window services are closed; weak widget handles observe dropped owners.
    pub fn close(&self) -> Result<(), NcStatus> {
        self.runtime.close()
    }
}

/// Static typography conveniences exist only on text-bearing proxy types.
macro_rules! font_proxy {
    (direct $interface:ty) => {
        impl<P> Widget<$interface, P>
        where
            P: WidgetStorage<$interface> + core::ops::DerefMut<Target = <$interface as neocogi_com::ComClientInterface>::Client<neocogi_com::ThreadLocal>>,
        {
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
        impl<P> Widget<$interface, P>
        where
            P: WidgetStorage<$interface> + core::ops::DerefMut<Target = <$interface as neocogi_com::ComClientInterface>::Client<neocogi_com::ThreadLocal>>,
        {
            /// Selects typography through this same widget object's optional capability.
            pub fn set_font_role(&mut self, role: FontRole) -> Result<(), NcStatus> {
                self.interface.observe::<IFontStyleAbi>()?.set_font_role(role)
            }
            /// Resolves a named font once in this widget's resource bundle.
            pub fn set_named_font(&mut self, name: &str) -> Result<(), NcStatus> {
                self.interface
                    .observe::<IFontStyleAbi>()?
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

impl<P> Widget<ListItemAbi, P>
where
    P: WidgetStorage<ListItemAbi> + core::ops::DerefMut<Target = <ListItemAbi as neocogi_com::ComClientInterface>::Client<neocogi_com::ThreadLocal>>,
{
    /// Selects an icon role directly through the concrete ListItem class contract.
    pub fn set_icon_role(&mut self, icon: ContentIcon) -> Result<(), NcStatus> {
        self.interface.set_icon_role(icon)
    }
    /// Resolves a named icon directly through the concrete ListItem class contract.
    pub fn set_named_icon(&mut self, name: &str) -> Result<(), NcStatus> {
        self.interface.set_named_icon(NcCopyFromBufferOffset::new(0, name.as_bytes()))
    }
}
impl<P> Widget<ButtonAbi, P>
where
    P: WidgetStorage<ButtonAbi> + core::ops::DerefMut<Target = <ButtonAbi as neocogi_com::ComClientInterface>::Client<neocogi_com::ThreadLocal>>,
{
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
impl<P> Widget<ComboAbi, P>
where
    P: WidgetStorage<ComboAbi> + core::ops::DerefMut<Target = <ComboAbi as neocogi_com::ComClientInterface>::Client<neocogi_com::ThreadLocal>>,
{
    /// Returns the positive per-panel row limit; initially ten.
    pub fn max_visible_items(&self) -> Result<u32, NcStatus> {
        self.interface.max_visible_items()
    }
    /// Silently changes the positive per-panel limit without changing selection.
    pub fn set_max_visible_items(&mut self, count: u32) -> Result<(), NcStatus> {
        self.interface.set_max_visible_items(count)
    }
    /// Returns the NCIDL-defined selection value without a parallel facade record.
    pub fn selection(&self) -> Result<ChoiceSelection, NcStatus> {
        self.interface.selection()
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
        self.interface
            .replace_menu(NcCopyFromBufferOffset::new(0, &menu_text), NcCopyFromBufferOffset::new(0, &entries), selection)
    }
    /// Atomically replaces the generated preorder hierarchy, shared text, and selection.
    pub fn replace_menu(&mut self, menu_text: &str, entries: &[ChoiceMenuEntry], selection: ChoiceSelection) -> Result<(), NcStatus> {
        self.interface.replace_menu(
            NcCopyFromBufferOffset::new(0, menu_text.as_bytes()),
            NcCopyFromBufferOffset::new(0, entries),
            selection,
        )
    }
    /// Silently replaces the committed optional selection without clamping invalid indices.
    pub fn set_selection(&mut self, selection: ChoiceSelection) -> Result<(), NcStatus> {
        self.interface.set_selection(selection)
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
fn encoded_combo_items<Item: AsRef<str>>(items: &[Item]) -> Result<(Vec<u8>, Vec<ChoiceMenuEntry>), NcStatus> {
    let mut text = Vec::new();
    let mut entries = Vec::with_capacity(items.len());
    for (id, item) in items.iter().enumerate() {
        let id = u64::try_from(id).map_err(|_| neocogi_com::NC_E_INVALID_ARG)?;
        let label = combo_label_range(&mut text, item.as_ref())?;
        entries.push(ChoiceMenuEntry::Choice { id, label });
    }
    Ok((text, entries))
}

/// Appends one label and returns its checked range in the shared operation buffer.
fn combo_label_range(text: &mut Vec<u8>, label: &str) -> Result<MenuTextRange, NcStatus> {
    let first = u32::try_from(text.len()).map_err(|_| neocogi_com::NC_E_INVALID_ARG)?;
    let count = u32::try_from(label.len()).map_err(|_| neocogi_com::NC_E_INVALID_ARG)?;
    text.extend_from_slice(label.as_bytes());
    Ok(MenuTextRange { first, count })
}

impl<P> Widget<ListBoxAbi, P>
where
    P: WidgetStorage<ListBoxAbi> + core::ops::DerefMut<Target = <ListBoxAbi as neocogi_com::ComClientInterface>::Client<neocogi_com::ThreadLocal>>,
{
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
    /// Creates an explicitly unobserved button without allocating a destination.
    pub fn button_unobserved(&self, label: &str) -> Result<OwnedWidget<ButtonAbi>, NcStatus> {
        self.button_with_queue(label, None)
    }

    /// Creates an explicitly unobserved checkbox without allocating a destination.
    pub fn checkbox_unobserved(&self, label: &str, checked: bool) -> Result<OwnedWidget<CheckboxAbi>, NcStatus> {
        self.checkbox_with_queue(label, checked, None)
    }

    /// Creates an explicitly unobserved textbox without allocating a destination.
    pub fn textbox_unobserved(&self, text: &str) -> Result<OwnedWidget<TextboxAbi>, NcStatus> {
        self.textbox_with_queue(text, None)
    }

    /// Creates an explicitly unobserved text area without allocating a destination.
    pub fn text_area_unobserved(&self, text: &str, wrapping: TextWrapping, scrolling: bool) -> Result<OwnedWidget<TextAreaAbi>, NcStatus> {
        self.text_area_with_queue(text, wrapping, scrolling, None)
    }

    /// Creates an explicitly unobserved slider without allocating a destination.
    pub fn slider_unobserved(&self, options: SliderOptions) -> Result<OwnedWidget<SliderAbi>, NcStatus> {
        self.slider_with_queue(options, None)
    }

    /// Creates an explicitly unobserved number without allocating a destination.
    pub fn number_unobserved(&self, options: NumberOptions) -> Result<OwnedWidget<NumberAbi>, NcStatus> {
        self.number_with_queue(options, None)
    }

    /// Creates an explicitly unobserved list item without allocating a destination.
    pub fn list_item_unobserved(&self, label: &str) -> Result<OwnedWidget<ListItemAbi>, NcStatus> {
        self.list_item_with_queue(label, None)
    }

    /// Creates an explicitly unobserved list box without allocating a destination.
    pub fn list_box_unobserved(&self, label: &str) -> Result<OwnedWidget<ListBoxAbi>, NcStatus> {
        self.list_box_with_queue(label, None)
    }

    /// Creates an explicitly unobserved scrollbar without allocating a destination.
    pub fn scrollbar_unobserved(&self, range: ScrollRange) -> Result<OwnedWidget<ScrollbarAbi>, NcStatus> {
        self.scrollbar_with_queue(range, None)
    }

    /// Creates an explicitly unobserved split view without allocating a destination.
    pub fn split_view_unobserved(&self, settings: SplitOptions) -> Result<OwnedWidget<SplitViewAbi>, NcStatus> {
        self.split_view_with_queue(settings, None)
    }
}

impl Ui {
    /// Constructs an unobserved choice header without a dummy destination allocation.
    pub fn combo_box_unobserved<Item: AsRef<str>>(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        items: &[Item],
        selection: ChoiceSelection,
    ) -> Result<OwnedWidget<ComboAbi>, NcStatus> {
        self.combo_box_with_queue(parent, items, selection, None)
    }

    /// Constructs an unobserved hierarchical choice header.
    pub fn combo_menu_unobserved(
        &self,
        parent: ComRef<'_, IWindowAbi>,
        text: &str,
        entries: &[ChoiceMenuEntry],
        selection: ChoiceSelection,
    ) -> Result<OwnedWidget<ComboAbi>, NcStatus> {
        self.combo_menu_with_queue(parent, text, entries, selection, None)
    }
}

impl Ui {
    /// Creates an explicitly unobserved picker without allocating a destination.
    pub fn file_dialog_unobserved(&self, parent: ComRef<'_, IWindowAbi>) -> Result<crate::FileDialog, NcStatus> {
        self.file_dialog_with_queue(parent, None)
    }
}

/// Concrete immutable control events queried on the common owned IEvent value.
pub use neocogi_ui::abi::nodes::{
    ICheckboxChangedEvent, ICheckboxChangedEventAbi, INumericChangedEvent, INumericChangedEventAbi, IScrollbarChangedEvent, IScrollbarChangedEventAbi,
    ISplitChangedEvent, ISplitChangedEventAbi,
};
