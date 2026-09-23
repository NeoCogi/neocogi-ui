#![allow(dead_code)]
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
//! Simple polygon mesh container used by OBJ loading and 3D demo rendering.

use rs_math3d::*;

#[repr(C)]
pub struct Vertex {
    pub position: Vec3f,
    pub normal: Vec3f,
    pub uv: Vec2f,
}

#[derive(Clone)]
struct Polygon {
    len: usize,
    start: usize,
}

#[derive(Copy, Clone)]
pub struct PolyVertex {
    pub pos: usize,
    pub normal: usize,
    pub tex: usize,
}

#[derive(Copy, Clone)]
pub struct PMVertex {
    position: Vec3f,
    selected: bool,
}

#[derive(Clone)]
pub struct PolyMesh {
    v_positions: Vec<PMVertex>,
    v_normals: Vec<Vec3f>,
    v_tex: Vec<Vec2f>,

    vertices: Vec<PolyVertex>,
    polys: Vec<Polygon>,
}

impl<'a> PolyMesh {
    pub fn new() -> Self {
        Self {
            v_positions: Vec::new(),
            v_normals: Vec::new(),
            v_tex: Vec::new(),

            vertices: Vec::new(),
            polys: Vec::new(),
        }
    }
    pub fn poly_count(&self) -> usize {
        self.polys.len()
    }
    pub fn get_poly(&'a self, f: usize) -> PolygonIterator<'a> {
        PolygonIterator {
            mesh: self,
            poly_id: f,
            v_count: self.polys[f].len,
            v_id: 0,
        }
    }

    /// Bounds only positions referenced by polygons; an empty mesh has a zero-sized origin box.
    pub fn calculate_bounding_box(&self) -> Box3f {
        let mut vertices = self.polys().flatten();
        let Some(first) = vertices.next() else {
            return Box3f::new(&Vec3f::zero(), &Vec3f::zero());
        };
        // Seed from real geometry so one-sided meshes do not accidentally include the origin.
        let point = &self.v_positions[first.pos].position;
        vertices.fold(Box3f::new(point, point), |bounds, vertex| {
            bounds.add(&self.v_positions[vertex.pos].position)
        })
    }

    pub fn set_vertices(&mut self, positions: Vec<Vec3f>, normals: Vec<Vec3f>, tex: Vec<Vec2f>) {
        self.v_positions = positions.into_iter().map(|position| PMVertex { position, selected: false }).collect();
        self.v_normals = normals;
        self.v_tex = tex;
    }

    pub fn add_poly(&mut self, verts: &[PolyVertex]) {
        let len = verts.len();
        let start = self.vertices.len();

        for vertex in verts {
            self.vertices.push(*vertex);
        }

        self.polys.push(Polygon { len, start });
    }

    pub fn polys(&'a self) -> PolyMeshIterator<'a> {
        PolyMeshIterator { mesh: self, poly_id: 0 }
    }

    pub fn vertex_position(&self, index: usize) -> Vec3f {
        self.v_positions[index].position
    }

    pub fn vertex_normal(&self, index: usize) -> Vec3f {
        self.v_normals[index]
    }

    pub fn vertex_uv(&self, index: usize) -> Vec2f {
        if self.v_tex.len() > index { self.v_tex[index] } else { Vec2f::zero() }
    }
}

#[derive(Clone, Copy)]
pub struct PolygonIterator<'a> {
    mesh: &'a PolyMesh,
    poly_id: usize,
    v_count: usize,
    v_id: usize,
}

impl<'a> Iterator for PolygonIterator<'a> {
    type Item = &'a PolyVertex;
    fn next(&mut self) -> Option<Self::Item> {
        if self.v_id < self.v_count {
            let v = &self.mesh.vertices[self.mesh.polys[self.poly_id].start + self.v_id];
            self.v_id += 1;
            Some(v)
        } else {
            None
        }
    }
}

impl<'a> PolygonIterator<'a> {
    pub fn vertex_count(&self) -> usize {
        self.mesh.polys[self.poly_id].len
    }
}

pub struct PolyMeshIterator<'a> {
    mesh: &'a PolyMesh,
    poly_id: usize,
}

impl<'a> Iterator for PolyMeshIterator<'a> {
    type Item = PolygonIterator<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.poly_id < self.mesh.poly_count() {
            let p = self.mesh.get_poly(self.poly_id);
            self.poly_id += 1;
            Some(p)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each polygon vertex refers to the same position ordinal in the test mesh.
    fn mesh(points: Vec<Vec3f>) -> PolyMesh {
        let mut mesh = PolyMesh::new();
        let vertices = (0..points.len()).map(|pos| PolyVertex { pos, normal: 0, tex: 0 }).collect::<Vec<_>>();
        mesh.set_vertices(points, Vec::new(), Vec::new());
        mesh.add_poly(&vertices);
        mesh
    }

    /// Copies components because the math library intentionally gives vectors no equality trait.
    fn coordinates(point: Vec3f) -> (f32, f32, f32) {
        (point.x, point.y, point.z)
    }

    /// Empty and one-sided geometry produce exact bounds without a phantom origin vertex.
    #[test]
    fn polygon_bounds_use_only_referenced_positions() {
        let empty = PolyMesh::new().calculate_bounding_box();
        assert_eq!(coordinates(empty.min), (0.0, 0.0, 0.0));
        assert_eq!(coordinates(empty.max), (0.0, 0.0, 0.0));

        let mut positive = mesh(vec![Vec3f::new(2.0, 3.0, 4.0), Vec3f::new(5.0, 7.0, 9.0)]);
        // A later unused position must not change bounds computed from polygon references.
        positive.set_vertices(
            vec![Vec3f::new(2.0, 3.0, 4.0), Vec3f::new(5.0, 7.0, 9.0), Vec3f::new(-100.0, -100.0, -100.0)],
            Vec::new(),
            Vec::new(),
        );
        let bounds = positive.calculate_bounding_box();
        assert_eq!(coordinates(bounds.min), (2.0, 3.0, 4.0));
        assert_eq!(coordinates(bounds.max), (5.0, 7.0, 9.0));

        let negative = mesh(vec![Vec3f::new(-9.0, -7.0, -5.0), Vec3f::new(-4.0, -3.0, -2.0)]);
        let bounds = negative.calculate_bounding_box();
        assert_eq!(coordinates(bounds.min), (-9.0, -7.0, -5.0));
        assert_eq!(coordinates(bounds.max), (-4.0, -3.0, -2.0));
    }
}
