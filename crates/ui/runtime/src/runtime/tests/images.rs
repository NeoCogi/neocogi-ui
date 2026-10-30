//! Image controls preserve immutable ownership, paint ordering, and clipped source coordinates.
use super::*;

#[derive(Default)]
/// Dimensions and lifecycle observations stay outside the deliberately hostile COM object.
struct ImageProbe {
    /// Reads happen only during installation; ordinary updates, measurement and paint must not query.
    reads: Cell<usize>,
    /// Reports one invalid candidate without altering the previously installed image.
    invalid: Cell<bool>,
    /// Final native-resource destruction occurs once after the last owning projection disappears.
    dropped: Cell<bool>,
    /// Weak test slot avoids an image-to-widget ownership cycle during reentry checks.
    target: std::rc::Weak<RefCell<Option<WeakComBox<ButtonAbi>>>>,
}
#[com_object(interfaces(ITexture))]
/// A texture deliberately reenters image editing while its immutable dimensions are validated.
struct ImageTexture {
    /// Test-only observations; the COM allocation itself owns no UI state.
    probe: Rc<ImageProbe>,
}
impl ITexture for ImageTexture {
    /// Nested editing sees BUSY, while a failed dimension read publishes no new widget content.
    fn size(&self) -> Result<Size, NcStatus> {
        self.probe.reads.set(self.probe.reads.get() + 1);
        if let Some(slot) = self.probe.target.upgrade() {
            let target = slot.borrow().clone();
            if let Some(mut target) = target {
                assert_eq!(target.clear_image(), Err(NC_E_BUSY));
            }
        }
        Ok(Size {
            width: if self.probe.invalid.get() { -1 } else { 64 },
            height: 32,
        })
    }
}
impl Drop for ImageTexture {
    /// The fixture observes actual COM destruction, not logical widget closure.
    fn drop(&mut self) {
        assert!(!self.probe.dropped.replace(true));
    }
}

/// Direct external painting checks semantic image ownership and ordering before final expansion.
#[test]
fn image_controls_keep_owned_pixels_and_forward_ordered_semantic_paint_after_errors() {
    let atlas = crate::test_support::test_atlas();
    let mut skin = crate::Skin::from_atlas(atlas);
    let palette = crate::FlatPalette {
        text: crate::color(0, 0, 0, 255),
        ..crate::FlatPalette::default()
    };
    skin.apply_flat_palette(palette);
    let assets = crate::test_support::assets::export_bundle(&crate::SkinBundle::from_skin(skin)).unwrap();
    let ui = nc_ui_nodes::bind_builtin_nodes(crate::test_support::runtime_from_assets(assets.as_com_ref()).unwrap()).unwrap();
    let mut button = ui.widgets().button_unobserved("AB").unwrap();
    button.replace_flags(WidgetOptions::NONE).unwrap();
    button.set_fill_states(FillStates::NONE).unwrap();
    let mut common = button.node_interface().unwrap();
    let target = Rc::new(RefCell::new(Some(button.as_interface().downgrade())));
    let image_probe = Rc::new(ImageProbe {
        target: Rc::downgrade(&target),
        ..ImageProbe::default()
    });
    let texture: ComPtr<ITextureAbi> = ImageTexture::from_value(ImageTexture { probe: image_probe.clone() }).query().unwrap();
    button.set_image(texture.as_com_ref(), false).unwrap();
    assert_eq!(image_probe.reads.get(), 1);
    assert_eq!(common.take_change().unwrap(), GeometryChange::Changed);
    button.set_image(texture.as_com_ref(), false).unwrap();
    assert_eq!(common.take_change().unwrap(), GeometryChange::NoChange);
    image_probe.invalid.set(true);
    assert_eq!(button.set_image(texture.as_com_ref(), true), Err(NC_E_INVALID_ARG));
    assert_eq!(common.take_change().unwrap(), GeometryChange::NoChange);
    image_probe.invalid.set(false);
    let paint_probe = Rc::new(BuiltinPaintProbe::default());
    let painter: ComPtr<IPainterAbi> = BuiltinPainter::from_value(BuiltinPainter {
        button: common.query().unwrap(),
        probe: paint_probe.clone(),
    })
    .query()
    .unwrap();
    let rect = Rect { x: 0, y: 0, width: 100, height: 40 };
    let clip = Rect { x: 12, y: 4, width: 16, height: 32 };
    let state = WidgetPaint {
        rect,
        clip,
        enabled: true,
        window_active: true,
        ..WidgetPaint::default()
    };
    let scope = PaintScope { invocation: 1, clip };
    paint_probe.fail_image.set(true);
    assert_eq!(common.paint(scope, state, painter.as_com_ref()), Err(UI_E_RENDER));
    paint_probe.images.borrow_mut().clear();
    paint_probe.order.borrow_mut().clear();
    common.paint(scope, state, painter.as_com_ref()).unwrap();
    let images = paint_probe.images.borrow();
    assert_eq!(images.len(), 1);
    let (destination, source, tint) = images[0];
    assert_eq!(destination, Rect { x: 5, y: 4, width: 64, height: 32 });
    assert_eq!(source, TextureRegion::FULL);
    assert_eq!(tint, u32::MAX, "theme black text cannot recolor external RGBA pixels");
    assert_eq!(image_probe.reads.get(), 3, "painting never calls texture size");
    drop(images);
    // A complete allocation must preserve the semantic text-before-image order.
    paint_probe.order.borrow_mut().clear();
    common
        .paint(PaintScope { clip: rect, ..scope }, WidgetPaint { clip: rect, ..state }, painter.as_com_ref())
        .unwrap();
    assert_eq!(&*paint_probe.order.borrow(), &[false, true]);
    // The inherited list-box editing path retains exactly the same immutable resource.
    target.borrow_mut().take();
    let mut list = ui.widgets().list_box_unobserved("list image").unwrap();
    list.set_image(texture.as_com_ref(), true).unwrap();
    drop(texture);
    button.clear_image().unwrap();
    assert!(!image_probe.dropped.get());
    list.clear_image().unwrap();
    assert!(image_probe.dropped.get());
    assert_eq!(button.label().unwrap(), "AB");
    assert_eq!(list.text().unwrap(), "list image");
}

#[com_object(interfaces(IRenderer))]
/// Consumes actual final image batches without a GPU, after both widget and runtime clipping.
struct ImageFrameRenderer {
    /// Copied final quads outlive the invocation without retaining renderer resources.
    quads: Rc<RefCell<Vec<[RenderVertex; 4]>>>,
}
impl IRenderer for ImageFrameRenderer {
    /// The CPU sink has no upload phase; the runtime already validated the immutable image.
    fn prepare_atlas(&self, _: ComRef<'_, IRgbaImageAbi>) -> Result<(), NcStatus> {
        Ok(())
    }
    /// Resolves each texture only during the active stream, then immediately releases the projection.
    fn render(&self, info: RenderInfo, _: ComRef<'_, IRgbaImageAbi>, stream: ComRef<'_, IRenderStreamAbi>) -> Result<(), NcStatus> {
        let (vertices, batches) = neocogi_gfx_support::testing::read(info, stream)?;
        let quads = batches
            .into_iter()
            .filter_map(|batch| {
                if let RenderBatch::Texture { first } = batch {
                    Some(vertices[first as usize..first as usize + 4].try_into().unwrap())
                } else {
                    None
                }
            })
            .collect();
        *self.quads.borrow_mut() = quads;
        Ok(())
    }
}

/// Rendering a narrow real window must retain the cropped image's source coordinates and ownership.
#[test]
fn image_buttons_survive_both_builtin_and_runtime_clipping_in_a_complete_frame() {
    let ui = nc_ui_nodes::bind_builtin_nodes(runtime()).unwrap();
    let button_owner = ui.widgets().button_unobserved("AB").unwrap();
    let mut button = button_owner.handle();
    button.replace_flags(WidgetOptions::NONE).unwrap();
    let image_probe = Rc::new(ImageProbe::default());
    let image: ComPtr<ITextureAbi> = ImageTexture::from_value(ImageTexture { probe: image_probe.clone() }).query().unwrap();
    neocogi_gfx_support::testing::register_image(image.as_com_ref(), Size { width: 64, height: 32 });
    button.set_image(image.as_com_ref(), false).unwrap();
    drop(image);
    let background = ui.widgets().text_block("", TextWrapping::None).unwrap();
    let parent = ui.window("clip owner", Rect { x: 20, y: 20, width: 32, height: 40 }, background).unwrap();
    parent.set_options(WindowOptions::NO_TITLE | WindowOptions::NO_PADDING).unwrap();
    parent.set_child_clip(ChildClip::Content).unwrap();
    let window = ui
        .child_window(parent.as_com_ref(), "image", Rect { x: 20, y: 20, width: 100, height: 40 }, button_owner)
        .unwrap();
    window.set_options(WindowOptions::NO_TITLE | WindowOptions::NO_PADDING).unwrap();
    let quads = Rc::new(RefCell::new(Vec::new()));
    let renderer: ComPtr<IRendererAbi> = ImageFrameRenderer::from_value(ImageFrameRenderer { quads: quads.clone() }).query().unwrap();
    ui.update(SIZE).unwrap();
    ui.render(SIZE, renderer.as_com_ref()).unwrap();
    assert_eq!(image_probe.reads.get(), 1, "measurement and drawing use immutable cached dimensions");
    let output = quads.borrow();
    assert_eq!(output.len(), 1);
    assert!(
        output[0]
            .iter()
            .all(|v| v.x >= 20. && v.x <= 52. && v.y >= 20. && v.y <= 60. && v.rgba == u32::MAX)
    );
    assert!(
        output[0][2].u > 0. && output[0][2].u < 1.,
        "nested clipping preserves the cropped source region"
    );
    drop(output);
    button.clear_image().unwrap();
    assert!(image_probe.dropped.get(), "no scratch recording retains the image after the frame");
    window.close().unwrap();
}
