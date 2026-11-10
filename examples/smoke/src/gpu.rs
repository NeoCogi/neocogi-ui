//! Independent host widget inserts real native 3D work among ordinary UI and texture draws.
use neocogi_ui::*;
use neocogi_platform_contracts::render::{
    source::{Bytes, PipelineDesc},
};
use neocogi_ui::com::widget::WidgetState;
use neocogi_com::{ComBox, ComPtr, ComRef, NcCopyFromBufferOffset, NcStatus, NC_E_INVALID_ARG, com_object};

#[com_object(interfaces(IWidget, ILeafLayout), ownership = exclusive)]
/// Static host implementation owns one common widget state and typed dynamic GPU capabilities.
struct SceneView {
    /// Mounting, mutation and lifetime use the same authority as built-in controls.
    state: WidgetState,
    /// Immutable image belongs to the renderer used for this UI submission.
    texture: ComPtr<ITextureAbi>,
    /// Restricted creation capability; no native renderer or host controller enters this widget.
    resources: ComPtr<IRenderResourcesAbi>,
    /// Client-owned vertex bytes and explicit role-specific indices.
    vertices: ComBox<IGpuBufferAbi>,
    /// Exclusive index ownership is observed weakly at every consuming draw.
    indices: ComBox<IGpuBufferAbi>,
    /// This widget's own programmable pipeline.
    pipeline: ComBox<IGpuPipelineAbi>,
}
impl IWidget for SceneView {
    /// Authorizes this independent class to join the selected UI's tree.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        self.state.host().try_add_ref()
    }
    /// The single common state rejects duplicate tree attachment.
    fn mount(&mut self) -> Result<(), NcStatus> {
        self.state.mount()
    }
    /// Logical closure remains independent of the native resource's final release.
    fn properties(&self) -> Result<WidgetProperties, NcStatus> {
        Ok(WidgetProperties {
            options: self.state.requested_flags()?,
            keyboard: 0,
            frame: WidgetFrame::Generic,
        })
    }
    /// Reads the requested configuration without including transient runtime policy.
    fn requested_flags(&self) -> Result<WidgetOptions, NcStatus> {
        self.state.requested_flags()
    }
    /// Common presentation changes invalidate measurement consistently with every other widget.
    fn replace_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.state.replace_flags(flags)
    }
    /// Sets current requested bits through the same widget-owned mutation authority.
    fn set_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.state.set_flags(flags)
    }
    /// Clears requested bits without copying effective properties back into settings.
    fn clear_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.state.clear_flags(flags)
    }
    /// Resource changes affect paint without inventing a renderer-owned scene model.
    fn update(&mut self, _state: WidgetUpdate, event: WidgetEvent, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
        self.state.writable()?;
        decode_widget_event(event, text)?;
        Ok(GeometryChange::NoChange)
    }
    /// Independent setter invalidation is sticky even when ordinary update was skipped.
    fn take_change(&mut self) -> Result<GeometryChange, NcStatus> {
        self.state.take_change()
    }
    /// These draws must reach the GPU in fill/image/generic-graphics/fill order, with clipping owned by the UI.
    fn paint(&mut self, scope: PaintScope, state: WidgetPaint, painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
        self.state.readable()?;
        let mut painter = Painter::from_call(painter, scope);
        painter.fill(state.rect, 0xff201810)?;
        painter.texture(state.rect, 0xffffffff, self.texture.as_com_ref())?;
        let mut commands = painter.begin_render_view(self.resources.as_com_ref())?;
        commands.bind_pipeline(self.pipeline.as_com_ref())?;
        commands.bind_vertex_buffer(0, self.vertices.as_com_ref(), 0)?;
        commands.bind_index_buffer(self.indices.as_com_ref(), GpuIndexFormat::U16, 0)?;
        commands.draw_indexed(3, 1, 0)?;
        commands.finish()?;
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
            width: if bounds.width.kind == LimitKind::Finite {
                320.min(bounds.width.extent)
            } else {
                320
            },
            height: if bounds.height.kind == LimitKind::Finite {
                200.min(bounds.height.extent)
            } else {
                200
            },
        })
    }
}

/// Exercises actual separately loaded texture/resource creation and ordered native GPU execution.
pub(super) fn smoke(ui: &Ui, size: Size, renderer: ComRef<'_, IRendererAbi>) -> Result<(), RenderError> {
    let factory: ComPtr<ITextureFactoryAbi> = renderer.try_add_ref()?.query()?;
    let pixels = [255u8, 32, 16, 255, 16, 255, 32, 255, 32, 16, 255, 255, 255, 255, 255, 255];
    let image_size = Size { width: 2, height: 2 };
    assert_eq!(
        factory
            .create_texture(
                image_size,
                neocogi_platform_contracts::render::source::initial_bytes(pixels[..15].to_vec()).borrow()?
            )
            .unwrap_err()
            .status,
        NC_E_INVALID_ARG
    );
    let texture = factory.create_texture(
        image_size,
        neocogi_platform_contracts::render::source::initial_bytes((NcCopyFromBufferOffset::new(0, &pixels)).as_slice().to_vec()).borrow()?,
    )?;
    assert_eq!(texture.size()?, image_size);
    let clients: ComPtr<IRenderClientFactoryAbi> = renderer.try_add_ref()?.query()?;
    let surface = renderer
        .try_add_ref()?
        .query::<IRenderSurfaceFactoryAbi>()?
        .create_surface(RenderSurfaceDesc::Framebuffer)?;
    let controller = clients.create_client(RenderClientLimits::default(), surface.as_com_ref(), RenderClientPermissions::default())?;
    let resources = controller.resources()?;
    assert!(resources.query::<IRendererAbi>().is_err());
    let code = Bytes::source(
        br#"
        struct V { @builtin(position) position:vec4<f32>, @location(0) color:vec3<f32> }
        @vertex fn vs(@location(0) position:vec2<f32>, @location(1) color:vec3<f32>) -> V {
            return V(vec4(position,0.5,1.0),color);
        }
        @fragment fn fs(v:V) -> @location(0) vec4<f32> { return vec4(v.color,1.0); }
    "#
        .to_vec(),
    );
    let vs = resources.create_shader(GpuShaderStage::Vertex, code.as_com_ref(), NcCopyFromBufferOffset::new(0, b"vs"))?;
    let fs = resources.create_shader(GpuShaderStage::Fragment, code.as_com_ref(), NcCopyFromBufferOffset::new(0, b"fs"))?;
    let descriptor = PipelineDesc {
        vertex_buffers: vec![GpuVertexBufferLayout {
            slot: 0,
            stride: 20,
            step: GpuStepMode::Vertex,
        }],
        attributes: vec![
            GpuVertexAttribute {
                location: 0,
                slot: 0,
                format: GpuVertexFormat::Float32x2,
                offset: 0,
            },
            GpuVertexAttribute {
                location: 1,
                slot: 0,
                format: GpuVertexFormat::Float32x3,
                offset: 8,
            },
        ],
        ..PipelineDesc::default()
    }
    .source()?;
    let pipeline = resources.create_pipeline(descriptor.as_com_ref(), vs.as_com_ref(), fs.as_com_ref())?;
    let payload: Vec<_> = [-0.8f32, -0.8, 1., 0., 0., 0.8, -0.8, 0., 1., 0., 0., 0.8, 0., 0., 1.]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect();
    let vertices = resources.create_static_buffer(GpuBufferDesc::Vertex { byte_len: payload.len() as u64 }, Bytes::source(payload).as_com_ref())?;
    let mut indices = resources.create_dynamic_buffer(GpuBufferDesc::Index { byte_len: 6 })?;
    let invalid_indices: Vec<_> = [0u16, 1, 3].into_iter().flat_map(u16::to_le_bytes).collect();
    indices.upload(NcCopyFromBufferOffset::new(0, &invalid_indices))?;
    // Raw bytes have no semantic meaning until draw consumption validates the complete footprint.
    let host = renderer.try_add_ref()?.query::<IPlatformWindowAbi>()?.graphics_host()?;
    let mut frame = host.recording_broker()?.begin_frame()?;
    let bounds = Rect { x: 0, y: 0, width: 16, height: 16 };
    let mut session = frame.begin_view(RenderViewRoot { client: bounds, target: bounds }, resources.as_com_ref())?;
    let mut check = session.view()?;
    check.bind_pipeline(pipeline.as_com_ref())?;
    check.bind_vertex_buffer(0, vertices.as_com_ref(), 0)?;
    check.bind_index_buffer(indices.as_com_ref(), GpuIndexFormat::U16, 0)?;
    assert!(check.draw_indexed(3, 1, 0).is_err());
    check.cancel()?;
    // Cancellation cannot erase the rejected draw; abort this deliberately failed CPU frame.
    assert_eq!(session.state()?, RenderViewState::Rejected);
    assert_eq!(frame.finish(), Err(RENDER_E_STATE));
    frame.abort()?;
    let valid_indices: Vec<_> = [0u16, 1, 2].into_iter().flat_map(u16::to_le_bytes).collect();
    indices.upload(NcCopyFromBufferOffset::new(0, &valid_indices))?;
    let root = OwnedWidget::<IWidgetAbi>::from_interface(
        SceneView::into_box(SceneView {
            state: WidgetState::new(ui.widgets().host()?, WidgetOptions::NONE),
            texture,
            resources,
            vertices,
            indices,
            pipeline,
        })
        .query()
        .map_err(|(status, _candidate)| status)?,
    );
    let window = ui
        .window("Native COM graphics and image", Rect { x: 20, y: 20, width: 560, height: 320 }, root)
        .map_err(|(status, _candidate)| status)?;
    ui.update(size)?;
    ui.render(size, renderer)?;
    window.close()?;
    Ok(())
}
