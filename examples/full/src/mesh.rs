//
// Copyright 2022-Present (c) Raja Lehtihet & Wael El Oraiby
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
// this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
// this list of conditions and the following disclaimer in the documentation
// and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
// may be used to endorse or promote products derived from this software without
// specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
// LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
// CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
// SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
// CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGE.
//
// -----------------------------------------------------------------------------
// Ported to rust from https://github.com/rxi/microui/ and the original license
//
// Copyright (c) 2020 rxi
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to
// deal in the Software without restriction, including without limitation the
// rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
// sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
// IN THE SOFTWARE.
//
//! Suzanne owns its vertex layout, shaders and camera; the renderer executes only generic commands.
use super::{camera::Camera, graphics::*, obj_loader::Obj, polymesh::PolyMesh, view3d::View3D};
use neocogi_com::ComBox;
use neocogi_render_gpu::{
    source::{Bytes, PipelineDesc},
};

/// This client's explicit GPU byte stride, unrelated to Rust or generated record layout.
const STRIDE: u32 = 32;
/// Portable column-major camera inputs owned entirely by this example's shader.
const SHADER: &str = r#"
struct Camera { pvm:mat4x4<f32>, view_model:mat4x4<f32> }
@group(1) @binding(0) var<uniform> camera:Camera;
struct V {
    @builtin(position) position:vec4<f32>,
    @location(0) normal:vec3<f32>, @location(1) original:vec3<f32>,
    @location(2) light:vec3<f32>, @location(3) uv:vec2<f32>
}
@vertex fn vs(@location(0) position:vec3<f32>, @location(1) normal:vec3<f32>, @location(2) uv:vec2<f32>) -> V {
    var clip = camera.pvm * vec4(position,1.0);
    // rs-math3d's perspective matrix uses [-1,1] depth; the generic profile uses [0,1].
    clip.z = (clip.z + clip.w) * 0.5;
    return V(clip, normalize((camera.view_model * vec4(normal,0.0)).xyz), normal,
        normalize((camera.pvm * vec4(1.0,0.0,0.0,1.0)).xyz), uv);
}
@fragment fn fs(v:V) -> @location(0) vec4<f32> {
    let intensity = max(dot(normalize(v.light), normalize(v.normal)),0.0);
    let base = v.original * 0.9 + vec3(v.uv,0.0) * 0.1;
    return vec4(base * intensity + vec3(0.1),1.0);
}
"#;

/// One client's resource collection; no corresponding semantic object exists in the renderer.
struct Geometry {
    /// Persistent restricted factory, safe to use during ordinary widget updates.
    resources: ComPtr<IRenderResourcesAbi>,
    /// Exclusive owners define public lifetime; draws capture private immutable versions.
    vertices: ComBox<IGpuBufferAbi>,
    /// Separate index role remains portable to WebGL 2.
    indices: ComBox<IGpuBufferAbi>,
    /// Explicit encoded layout and this example's programmable shading.
    pipeline: ComBox<IGpuPipelineAbi>,
    /// Complete triangle-list index extent.
    index_count: u32,
}
impl Geometry {
    /// Uploads explicit byte arrays and declares their exact shader interpretation.
    fn new(resources: ComRef<'_, IRenderResourcesAbi>, vertices: Vec<u8>, indices: Vec<u32>) -> Result<Self, NcStatus> {
        let vertex = resources.create_buffer(
            GpuBufferDesc {
                role: GpuBufferRole::Vertex,
                byte_len: vertices.len() as u64,
            },
            neocogi_render_core::source::supplied(Bytes::source(vertices).as_com_ref())?.borrow()?,
        )?;
        let index_count = indices.len().try_into().map_err(|_| neocogi_com::NC_E_OUT_OF_RANGE)?;
        let indices: Vec<_> = indices.into_iter().flat_map(u32::to_le_bytes).collect();
        let index = resources.create_buffer(
            GpuBufferDesc {
                role: GpuBufferRole::Index,
                byte_len: indices.len() as u64,
            },
            neocogi_render_core::source::supplied(Bytes::source(indices).as_com_ref())?.borrow()?,
        )?;
        let code = Bytes::source(SHADER.as_bytes().to_vec());
        let vs = resources.create_shader(GpuShaderStage::Vertex, code.as_com_ref(), NcCopyFromBufferOffset::new(0, b"vs"))?;
        let fs = resources.create_shader(GpuShaderStage::Fragment, code.as_com_ref(), NcCopyFromBufferOffset::new(0, b"fs"))?;
        let descriptor = PipelineDesc {
            state: GpuPipelineInfo {
                depth_compare: GpuCompare::LessEqual,
                depth_write: true,
                ..GpuPipelineInfo::default()
            },
            vertex_buffers: vec![GpuVertexBufferLayout {
                slot: 0,
                stride: STRIDE,
                step: GpuStepMode::Vertex,
            }],
            attributes: vec![
                GpuVertexAttribute {
                    location: 0,
                    slot: 0,
                    format: GpuVertexFormat::Float32x3,
                    offset: 0,
                },
                GpuVertexAttribute {
                    location: 1,
                    slot: 0,
                    format: GpuVertexFormat::Float32x3,
                    offset: 12,
                },
                GpuVertexAttribute {
                    location: 2,
                    slot: 0,
                    format: GpuVertexFormat::Float32x2,
                    offset: 24,
                },
            ],
            bindings: vec![GpuShaderBinding {
                stage: GpuShaderStage::Vertex,
                slot: 0,
                kind: GpuBindingKind::Uniform,
                byte_len: 128,
            }],
        }
        .source()?;
        let pipeline = resources.create_pipeline(descriptor.as_com_ref(), vs.as_com_ref(), fs.as_com_ref())?;
        Ok(Self {
            resources: resources.try_add_ref()?,
            vertices: vertex,
            indices: index,
            pipeline,
            index_count,
        })
    }
}

/// Concrete camera behavior and GPU owners stay with the example's widget.
pub(super) struct Suzanne {
    /// The same final visible client drives pointer mapping and camera projection.
    view: View3D,
    /// Restricted GPU objects; this value has no renderer, window or host controller capability.
    geometry: Geometry,
}
impl Suzanne {
    /// The trusted application grants a factory; the widget never receives the common renderer.
    pub(super) fn new(resources: ComRef<'_, IRenderResourcesAbi>) -> Result<Self, NcStatus> {
        let mesh = Obj::from_byte_stream(&neocogi_demo_assets::suzanne_obj().map_err(|_| neocogi_com::NC_E_LOAD_FAILED)?)
            .map_err(|_| neocogi_com::NC_E_INVALID_ARG)?
            .to_polymesh();
        let bounds = mesh.calculate_bounding_box();
        let view = View3D::new(
            Camera::new(
                bounds.center(),
                bounds.max.length() * 3.,
                Quatf::identity(),
                PI / 4.,
                1.,
                0.1,
                bounds.max.length() * 10.,
            ),
            Dimensioni::new(1, 1),
            bounds,
        );
        let (vertices, indices) = mesh_buffers(&mesh);
        Ok(Self {
            view,
            geometry: Geometry::new(resources, vertices, indices)?,
        })
    }
}
impl LeafContent for Suzanne {
    /// Pointer focus, navigation and W/S text all belong to this viewport.
    const KEYBOARD: u16 = 1 | 2 | 256;
    /// Flex allocation determines the actual viewport; this minimum keeps it measurable.
    fn measure(&self, _: LayoutLimits, _: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        Ok(Size { width: 80, height: 24 })
    }
    /// Applies orbit/zoom using the visible client; final projection capture belongs to paint.
    fn update(&mut self, state: WidgetUpdate, event: WidgetEvent, text: &str) -> Result<GeometryChange, NcStatus> {
        let rect = state.clip;
        if rect.width <= 0 || rect.height <= 0 {
            return Ok(GeometryChange::NoChange);
        }
        self.view.set_viewport(local_rect(rect));
        match event {
            WidgetEvent::Drag { position, delta, buttons } if buttons & 1 != 0 => {
                let p = point(position);
                self.view.update_drag(Vec2i::new(p.x.saturating_sub(delta.x), p.y.saturating_sub(delta.y)), p);
            }
            WidgetEvent::Scroll { delta, .. } => {
                self.view.apply_scroll(if delta.y != 0 { delta.y } else { delta.x } as f32);
            }
            WidgetEvent::Key(key) if key.pressed => {
                let delta = match key.key {
                    KeyCode::ArrowLeft => Vec2i::new(-20, 0),
                    KeyCode::ArrowRight => Vec2i::new(20, 0),
                    KeyCode::ArrowUp => Vec2i::new(0, -20),
                    KeyCode::ArrowDown => Vec2i::new(0, 20),
                    _ => Vec2i::default(),
                };
                if delta.x != 0 || delta.y != 0 {
                    let center = Vec2i::new(rect.x + rect.width / 2, rect.y + rect.height / 2);
                    self.view
                        .update_drag(center, Vec2i::new(center.x.saturating_add(delta.x), center.y.saturating_add(delta.y)));
                }
            }
            WidgetEvent::Text => {
                for ch in text.chars() {
                    match ch {
                        'w' | 'W' => {
                            self.view.apply_scroll(-0.5);
                        }
                        's' | 'S' => {
                            self.view.apply_scroll(0.5);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        Ok(GeometryChange::NoChange)
    }

    /// Captures this widget's own shader inputs using the renderer view's committed client.
    fn paint(&mut self, state: WidgetPaint, painter: &mut Painter<'_>) -> Result<(), NcStatus> {
        if state.clip.width <= 0 || state.clip.height <= 0 {
            return Ok(());
        }
        let mut commands = painter.begin_render_view(self.geometry.resources.as_com_ref())?;
        self.view.set_viewport(local_rect(commands.info()?.client));
        let uniforms = camera_bytes(&self.view.pvm(), &self.view.view_matrix());
        commands.bind_pipeline(self.geometry.pipeline.as_com_ref())?;
        commands.bind_vertex_buffer(0, self.geometry.vertices.as_com_ref(), 0)?;
        commands.bind_index_buffer(self.geometry.indices.as_com_ref(), GpuIndexFormat::U32, 0)?;
        commands.push_uniforms(GpuShaderStage::Vertex, 0, NcCopyFromBufferOffset::new(0, &uniforms))?;
        commands.draw_indexed(self.geometry.index_count, 1, 0)?;
        commands.finish()
    }
}

/// Explicitly encodes this client's eight floats; no semantic record is borrowed as GPU bytes.
fn vertex_bytes(output: &mut Vec<u8>, position: Vec3f, normal: Vec3f, uv: Vec2f) {
    output.extend(
        [position.x, position.y, position.z, normal.x, normal.y, normal.z, uv.x, uv.y]
            .into_iter()
            .flat_map(f32::to_le_bytes),
    );
}
/// Triangulates OBJ corners while keeping the exact position/normal/UV interpretation local.
fn mesh_buffers(mesh: &PolyMesh) -> (Vec<u8>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for poly in mesh.polys() {
        let first = (vertices.len() / STRIDE as usize) as u32;
        let mut count = 0;
        for vertex in poly {
            vertex_bytes(
                &mut vertices,
                mesh.vertex_position(vertex.pos),
                mesh.vertex_normal(vertex.normal),
                mesh.vertex_uv(vertex.tex),
            );
            count += 1;
        }
        for index in 2..count {
            indices.extend([first, first + index - 1, first + index]);
        }
    }
    (vertices, indices)
}
/// The shader consumes two column-major matrices with sixteen-byte column alignment.
fn camera_bytes(projection: &Mat4f, view: &Mat4f) -> Vec<u8> {
    [projection, view]
        .into_iter()
        .flat_map(|matrix| matrix.col.iter())
        .flat_map(|column| [column.x, column.y, column.z, column.w])
        .flat_map(f32::to_le_bytes)
        .collect()
}

#[cfg(test)]
mod viewport_tests {
    use super::*;
    use neocogi_render_gpu::{GpuDevice, command::Command};

    /// Distinct named values pin the layout read by all three declared shader attributes.
    #[test]
    fn explicit_vertex_encoding_has_no_rust_layout_dependency() {
        let mut bytes = Vec::new();
        vertex_bytes(&mut bytes, Vec3f::new(1., 2., 3.), Vec3f::new(4., 5., 6.), Vec2f::new(7., 8.));
        assert_eq!(bytes.len(), 32);
        let values: Vec<_> = bytes.as_chunks::<4>().0.iter().map(|value| f32::from_le_bytes(*value)).collect();
        assert_eq!(values, [1., 2., 3., 4., 5., 6., 7., 8.]);
    }

    /// Final paint uses a real scoped view and captures the late visible aspect as generic uniforms.
    #[test]
    fn paint_captures_the_final_visible_aspect_and_skips_empty_clients() {
        let device = GpuDevice::new(
            nc_shader_compiler::CompilerObject::from_value(nc_shader_compiler::CompilerObject)
                .query()
                .unwrap(),
        );
        let domain = device.create_client(RenderClientLimits::default()).unwrap();
        let resources = domain.resources().unwrap();
        let mut bytes = Vec::new();
        for position in [Vec3f::new(-1., -1., 0.), Vec3f::new(1., -1., 0.), Vec3f::new(0., 1., 0.)] {
            vertex_bytes(&mut bytes, position, Vec3f::new(0., 0., 1.), Vec2f::new(0., 0.));
        }
        let mut widget = Suzanne {
            view: crate::grid::create_grid_view_3d(),
            geometry: Geometry::new(resources.as_com_ref(), bytes, vec![0, 1, 2]).unwrap(),
        };
        let full = Rect { x: 0, y: 0, width: 200, height: 100 };
        widget
            .update(crate::viewport_test_support::update_state(full, full), WidgetEvent::None, "")
            .unwrap();
        let earlier = widget.view.projection_matrix();
        let visible = Rect { x: 37, y: 23, width: 91, height: 57 };
        let (recorder, output) = crate::viewport_test_support::gpu_recorder(&device);
        let mut painter = Painter::from_call(recorder.as_com_ref(), PaintScope { invocation: 1, clip: visible });
        widget
            .paint(
                WidgetPaint {
                    rect: full,
                    clip: visible,
                    ..WidgetPaint::default()
                },
                &mut painter,
            )
            .unwrap();
        let mut reference = crate::grid::create_grid_view_3d();
        reference.set_viewport(local_rect(visible));
        assert_ne!(earlier.col[0].x, widget.view.projection_matrix().col[0].x);
        let mut session = output.borrow_mut().views.pop().unwrap();
        let owner = session.take_block().unwrap();
        output.borrow_mut().views.push(session);
        let block = device.block_snapshot(owner.as_com_ref()).unwrap();
        let Command::Draw(draw) = &block.commands()[1] else {
            panic!("missing generic draw");
        };
        assert_eq!(block.root().client, visible);
        assert_eq!(draw.uniforms[0].data.bytes(), camera_bytes(&reference.pvm(), &reference.view_matrix()));
        widget
            .paint(
                WidgetPaint {
                    rect: full,
                    clip: Rect::default(),
                    ..WidgetPaint::default()
                },
                &mut painter,
            )
            .unwrap();
        assert_eq!(output.borrow().views.len(), 1);
        widget
            .update(
                crate::viewport_test_support::update_state(
                    Rect { x: 0, y: 0, width: i32::MAX, height: 1 },
                    Rect {
                        x: i32::MAX - 1,
                        y: 0,
                        width: 1,
                        height: 1,
                    },
                ),
                WidgetEvent::Key(KeyStroke {
                    key: KeyCode::ArrowRight,
                    value: 0,
                    pressed: true,
                    repeat: false,
                    modifiers: 0,
                }),
                "",
            )
            .unwrap();
        assert!(
            widget
                .view
                .pvm()
                .col
                .iter()
                .all(|column| [column.x, column.y, column.z, column.w].iter().all(|value| value.is_finite()))
        );
    }
}
