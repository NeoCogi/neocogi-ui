//! Independent host widget inserts real native 3D work among ordinary UI and texture draws.
use neocogi_ui::*;
use neocogi_demo_contracts::*;
use neocogi_ui::com::widget::WidgetState;
use neocogi_com::{ComPtr, ComRef, NcCopyFromBufferOffset, NcStatus, NC_E_INVALID_ARG, com_object};
use rs_math3d::{Mat4f, Vec2f, Vec3f};

#[com_object(interfaces(IWidget, ILeafLayout))]
/// Static host implementation owns one common widget state and typed dynamic GPU capabilities.
struct SceneView {
    /// Mounting, mutation and lifetime use the same authority as built-in controls.
    state: WidgetState,
    /// Immutable image belongs to the renderer used for this UI submission.
    texture: ComPtr<ITextureAbi>,
    /// Inherited render-item projection of the concrete editable mesh scene.
    scene: ComPtr<IRenderItemAbi>,
}
impl IWidget for SceneView {
    /// Authorizes this independent class to join the selected UI's tree.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        Ok(self.state.host().add_ref())
    }
    /// The single common state rejects duplicate tree attachment.
    fn mount(&mut self) -> Result<(), NcStatus> {
        self.state.mount()
    }
    /// Logical closure remains independent of the native resource's final release.
    fn close(&mut self) -> Result<(), NcStatus> {
        self.state.close().map(|_| ())
    }
    /// The scene is a passive fixed-preference surface, with no synthetic keyboard target.
    fn properties(&self) -> Result<WidgetProperties, NcStatus> {
        Ok(WidgetProperties {
            options: self.state.options()?,
            keyboard: 0,
            frame: WidgetFrame::GENERIC,
        })
    }
    /// Common presentation changes invalidate measurement consistently with every other widget.
    fn set_options(&mut self, options: WidgetOptions) -> Result<(), NcStatus> {
        self.state.set_options(options)
    }
    /// Scene data affects paint only; the host explicitly updates transforms outside UI observation.
    fn update(&mut self, _state: WidgetUpdate, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
        self.state.writable()?;
        decode_widget_event(event, text)?;
        Ok(GeometryChange::NO_CHANGE)
    }
    /// Independent setter invalidation is sticky even when ordinary update was skipped.
    fn take_change(&mut self) -> Result<GeometryChange, NcStatus> {
        self.state.take_change()
    }
    /// These draws must reach the GPU in fill/image/mesh/fill order, with clipping owned by the UI.
    fn paint(&mut self, scope: PaintScope, state: WidgetPaint, painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
        self.state.readable()?;
        let mut painter = Painter::from_call(painter, scope);
        painter.fill(state.rect, 0xff201810)?;
        painter.texture(state.rect, 0xffffffff, self.texture.as_com_ref())?;
        painter.item(state.rect, self.scene.as_com_ref())?;
        painter.fill(Rect { x: 4, y: 4, width: 12, height: 12 }, 0xff00ffff)
    }
}
impl ILeafLayout for SceneView {
    /// Preferred scene extent obeys bounded zero and leaves unbounded axes unconstrained.
    fn measure(&mut self, bounds: LayoutLimits, _metrics: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        self.state.readable()?;
        bounds.width.validate()?;
        bounds.height.validate()?;
        Ok(Size {
            width: if bounds.width.kind == LimitKind::FINITE {
                320.min(bounds.width.extent)
            } else {
                320
            },
            height: if bounds.height.kind == LimitKind::FINITE {
                200.min(bounds.height.extent)
            } else {
                200
            },
        })
    }
}

/// Exercises actual separately loaded texture/scene creation and ordered native GPU execution.
pub(super) fn smoke(ui: &Ui, size: Size, renderer: ComRef<'_, IRendererAbi>) -> Result<(), NcStatus> {
    let factory: ComPtr<ITextureFactoryAbi> = renderer.add_ref().query()?;
    let pixels = [255u8, 32, 16, 255, 16, 255, 32, 255, 32, 16, 255, 255, 255, 255, 255, 255];
    let image_size = Size { width: 2, height: 2 };
    assert_eq!(
        factory.create_texture(image_size, NcCopyFromBufferOffset::new(0, &pixels[..15])).unwrap_err(),
        NC_E_INVALID_ARG
    );
    let texture = factory.create_texture(image_size, NcCopyFromBufferOffset::new(0, &pixels))?;
    assert_eq!(texture.size()?, image_size);
    let scenes: ComPtr<IDemoScenesAbi> = renderer.add_ref().query()?;
    let scene = scenes.create_scene()?;
    let vertices = [
        MeshVertex {
            position: Vec3f::new(-0.8, -0.8, 0.0).into(),
            normal: Vec3f::new(0.0, 0.0, 1.0).into(),
            uv: Vec2f::new(0.0, 0.0).into(),
        },
        MeshVertex {
            position: Vec3f::new(0.8, -0.8, 0.0).into(),
            normal: Vec3f::new(0.0, 0.0, 1.0).into(),
            uv: Vec2f::new(1.0, 0.0).into(),
        },
        MeshVertex {
            position: Vec3f::new(0.0, 0.8, 0.0).into(),
            normal: Vec3f::new(0.0, 0.0, 1.0).into(),
            uv: Vec2f::new(0.5, 1.0).into(),
        },
    ];
    scene.set_mesh(NcCopyFromBufferOffset::new(0, &vertices), NcCopyFromBufferOffset::new(0, &[0, 1, 2]))?;
    assert_eq!(
        scene.set_mesh(NcCopyFromBufferOffset::new(0, &vertices), NcCopyFromBufferOffset::new(0, &[0, 1, 3])),
        Err(NC_E_INVALID_ARG)
    );
    scene.set_transforms(SceneTransforms {
        pvm: Mat4f::identity().into(),
        view_model: Mat4f::identity().into(),
    })?;
    let mut bad = Mat4f::identity();
    bad.col[0].x = f32::NAN;
    assert_eq!(
        scene.set_transforms(SceneTransforms {
            pvm: bad.into(),
            view_model: Mat4f::identity().into(),
        }),
        Err(NC_E_INVALID_ARG)
    );
    // The base projection is the same scene allocation, with no extra widget or application callback object.
    let root = Widget::<IWidgetAbi>::from_interface(
        SceneView::from_value(SceneView {
            state: WidgetState::new(ui.widgets().host()?, WidgetOptions::NONE),
            texture,
            scene: scene.query()?,
        })
        .query()?,
    );
    let window = ui.window("Native COM mesh and image", Rect { x: 20, y: 20, width: 560, height: 320 }, &root)?;
    ui.update(size)?;
    ui.render(size, renderer)?;
    window.close()?;
    Ok(())
}
