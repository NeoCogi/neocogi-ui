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
//! Suzanne is one local COM widget driving one renderer-owned COM scene.
use super::{camera::Camera, graphics::*, obj_loader::Obj, polymesh::PolyMesh, view3d::View3D};
use microui_demo_contracts::{IDemoScenesAbi, IMeshSceneAbi, MeshVertex, SceneTransforms};

/// Concrete camera behavior and cached interface projections share one renderer scene.
pub(super) struct Suzanne {
    /// Application camera changes only through routed input and committed allocations.
    view: View3D,
    /// Editing capability owns the uploaded mesh and accepts complete camera snapshots.
    scene: ComPtr<IMeshSceneAbi>,
    /// Cached base projection avoids QueryInterface on every paint.
    item: ComPtr<IRenderItemAbi>,
}
impl Suzanne {
    /// Loads the bundled mesh independently of the process working directory and uploads it once.
    pub(super) fn new(renderer: ComRef<'_, IRendererAbi>) -> Result<Self, NcStatus> {
        let mesh = Obj::from_byte_stream(&microui_redux_demo_assets::suzanne_obj().map_err(|_| neocogi_com::NC_E_LOAD_FAILED)?)
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
        let scenes = renderer.add_ref().query::<IDemoScenesAbi>()?;
        let scene = scenes.create_scene()?;
        let (vertices, indices) = mesh_buffers(&mesh);
        scene.set_mesh(NcCopyFromBufferOffset::new(0, &vertices), NcCopyFromBufferOffset::new(0, &indices))?;
        let item = scene.query()?;
        Ok(Self { view, scene, item })
    }
}
impl LeafContent for Suzanne {
    /// Pointer focus, sequential navigation and committed W/S text all belong to this viewport.
    const KEYBOARD: u16 = 1 | 2 | 256;
    /// Flex allocation determines the actual viewport; this minimum keeps it measurable.
    fn measure(&self, _: LayoutLimits, _: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        Ok(Size { width: 80, height: 24 })
    }
    /// Applies orbit/zoom before publishing both matrices in one scene edit.
    fn update(&mut self, state: WidgetUpdate, event: WidgetEvent, text: &str) -> Result<GeometryChange, NcStatus> {
        let rect = state.rect;
        if rect.width <= 0 || rect.height <= 0 {
            return Ok(GeometryChange::NO_CHANGE);
        }
        self.view.set_dimension(Dimensioni::new(rect.width, rect.height));
        match event {
            WidgetEvent::Drag { position, delta, buttons } if buttons & 1 != 0 => {
                let p = point(position);
                self.view.update_drag(p - point(delta), p);
            }
            WidgetEvent::Scroll { delta, .. } => {
                self.view.apply_scroll(if delta.y != 0 { delta.y } else { delta.x } as f32);
            }
            WidgetEvent::Key(key) if key.pressed => {
                let delta = match key.key {
                    KeyCode::ARROW_LEFT => Vec2i::new(-20, 0),
                    KeyCode::ARROW_RIGHT => Vec2i::new(20, 0),
                    KeyCode::ARROW_UP => Vec2i::new(0, -20),
                    KeyCode::ARROW_DOWN => Vec2i::new(0, 20),
                    _ => Vec2i::default(),
                };
                if delta.x != 0 || delta.y != 0 {
                    let center = Vec2i::new(rect.width / 2, rect.height / 2);
                    self.view.update_drag(center, center + delta);
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
        self.scene.set_transforms(SceneTransforms {
            pvm: self.view.pvm(),
            view_model: self.view.view_matrix(),
        })?;
        Ok(GeometryChange::NO_CHANGE)
    }
    /// Rendering observes the scene without a native callback, native handle or borrowed GPU frame.
    fn paint(&self, state: WidgetPaint, painter: &mut Painter<'_>) -> Result<(), NcStatus> {
        painter.item(state.rect, self.item.as_com_ref())
    }
}

/// Triangulates each bundled OBJ face while preserving per-corner positions, normals and UVs.
fn mesh_buffers(mesh: &PolyMesh) -> (Vec<MeshVertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for poly in mesh.polys() {
        let start = vertices.len() as u32;
        let mut count = 0;
        for v in poly {
            vertices.push(MeshVertex {
                position: mesh.vertex_position(v.pos),
                normal: mesh.vertex_normal(v.normal),
                uv: mesh.vertex_uv(v.tex),
            });
            count += 1;
        }
        for i in 2..count {
            indices.extend([start, start + i - 1, start + i]);
        }
    }
    (vertices, indices)
}
