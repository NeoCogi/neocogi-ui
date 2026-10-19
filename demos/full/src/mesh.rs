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
use neocogi_render_mesh_contracts::{IMeshScenesAbi, IMeshSceneAbi, MeshVertex, SceneTransforms};

/// Concrete camera behavior and cached interface projections share one renderer scene.
pub(super) struct Suzanne {
    /// Camera uses the visible client area during both input and final paint traversal.
    view: View3D,
    /// Editing capability owns the uploaded mesh and accepts complete camera snapshots.
    scene: ComPtr<IMeshSceneAbi>,
    /// Cached base projection avoids QueryInterface on every paint.
    item: ComPtr<IRenderItemAbi>,
}
impl Suzanne {
    /// Loads the bundled mesh independently of the process working directory and uploads it once.
    pub(super) fn new(renderer: ComRef<'_, IRendererAbi>) -> Result<Self, NcStatus> {
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
        let scenes = renderer.try_add_ref()?.query::<IMeshScenesAbi>()?;
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
    /// Applies orbit/zoom using the visible client; final scene publication belongs to paint.
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
                    self.view.update_drag(center, Vec2i::new(center.x.saturating_add(delta.x), center.y.saturating_add(delta.y)));
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
    /// Publishes the final visible-area projection before recording the renderer-owned scene.
    /// Scene edits do not change UI geometry; this covers layout that settles after input dispatch.
    fn paint(&mut self, state: WidgetPaint, painter: &mut Painter<'_>) -> Result<(), NcStatus> {
        if state.clip.width <= 0 || state.clip.height <= 0 {
            return Ok(());
        }
        self.view.set_viewport(local_rect(state.clip));
        self.scene.set_transforms(SceneTransforms {
            pvm: self.view.pvm().into(),
            view_model: self.view.view_matrix().into(),
        })?;
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
                position: mesh.vertex_position(v.pos).into(),
                normal: mesh.vertex_normal(v.normal).into(),
                uv: mesh.vertex_uv(v.tex).into(),
            });
            count += 1;
        }
        for i in 2..count {
            indices.extend([start, start + i - 1, start + i]);
        }
    }
    (vertices, indices)
}

#[cfg(test)]
mod viewport_tests {
    use super::*;
    use neocogi_com::com_object;
    use neocogi_render_mesh_contracts::{IMeshScene, IMeshSceneVtblFor};
    use std::{cell::RefCell, rc::Rc};

    #[com_object(interfaces(IMeshScene))]
    /// Records actual generated scene edits independently of the application camera.
    struct SceneProbe {
        /// Shared observations remain borrow-free across the exclusive painter call.
        transforms: Rc<RefCell<Vec<SceneTransforms>>>,
    }
    impl IRenderItem for SceneProbe {}
    impl IMeshScene for SceneProbe {
        /// Geometry upload is unnecessary for observing transform publication.
        fn set_mesh(&self, _: NcCopyFromBufferOffset<'_, MeshVertex>, _: NcCopyFromBufferOffset<'_, u32>) -> Result<(), NcStatus> { Ok(()) }
        /// Complete copied snapshots prove paint publishes the final settled aspect through COM.
        fn set_transforms(&self, transforms: SceneTransforms) -> Result<(), NcStatus> {
            self.transforms.borrow_mut().push(transforms); Ok(())
        }
    }

    /// Final paint can have a smaller clip than update; the scene must change before item recording.
    #[test]
    fn paint_publishes_the_final_visible_aspect_and_skips_empty_clients() {
        let transforms = Rc::new(RefCell::new(Vec::new()));
        let scene: ComPtr<IMeshSceneAbi> = SceneProbe::from_value(SceneProbe { transforms: transforms.clone() }).query().unwrap();
        let item = scene.query().unwrap();
        let mut widget = Suzanne { view: crate::grid::create_grid_view_3d(), scene, item };
        let full = Rect { x: 0, y: 0, width: 200, height: 100 };
        widget.update(crate::viewport_test_support::update_state(full, full), WidgetEvent::None, "").unwrap();
        let earlier_projection = widget.view.projection_matrix();
        assert!(transforms.borrow().is_empty(), "input does not publish an intermediate projection");
        let visible = Rect { x: 37, y: 23, width: 91, height: 57 };
        let (recorder, output) = crate::viewport_test_support::recorder();
        let mut painter = Painter::from_call(recorder.as_com_ref(), PaintScope { invocation: 1, clip: visible });
        widget.paint(WidgetPaint { rect: full, clip: visible, ..WidgetPaint::default() }, &mut painter).unwrap();
        let mut reference = crate::grid::create_grid_view_3d();
        reference.set_viewport(local_rect(visible));
        let expected: neocogi_render_mesh_contracts::Matrix4 = reference.pvm().into();
        let edits = transforms.borrow();
        assert_eq!(edits.len(), 1);
        assert_ne!(earlier_projection.col[0].x, widget.view.projection_matrix().col[0].x);
        assert_eq!(edits[0].pvm.m00, expected.m00);
        assert_eq!(edits[0].pvm.m11, expected.m11);
        drop(edits);
        assert_eq!(output.borrow().items.len(), 1);
        assert_eq!(output.borrow().items[0].width, full.width, "UI allocation remains full-sized");
        widget.paint(WidgetPaint { rect: full, clip: Rect::default(), ..WidgetPaint::default() }, &mut painter).unwrap();
        assert_eq!(transforms.borrow().len(), 1);
        assert_eq!(output.borrow().items.len(), 1);

        // A thin visible strip can sit at the far end of an otherwise valid wide allocation.
        // Keyboard orbit must saturate its synthetic endpoint rather than overflow that origin.
        widget.update(crate::viewport_test_support::update_state(
            Rect { x: 0, y: 0, width: i32::MAX, height: 1 },
            Rect { x: i32::MAX - 1, y: 0, width: 1, height: 1 },
        ), WidgetEvent::Key(KeyStroke { key: KeyCode::ArrowRight, value: 0, pressed: true, repeat: false, modifiers: 0 }), "").unwrap();
        assert!(widget.view.pvm().col.iter().all(|column| [column.x, column.y, column.z, column.w].iter().all(|value| value.is_finite())));
    }
}
