//
// Copyright (C) 2020 Raja Lehtihet & Wael El Oraiby
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
//! Checked decoder for the demo's positive absolute-index OBJ subset.
use rs_math3d::{Box3f, Vec2f, Vec3f, Vector};

/// Fully resolved GPU input; absent attributes have already acquired their per-corner defaults.
pub(super) struct MeshVertex {
    /// Finite position from a checked reference.
    pub(super) position: Vec3f,
    /// Unit supplied or triangle-flat normal.
    pub(super) normal: Vec3f,
    /// Finite supplied UV, or zero for this particular missing attribute.
    pub(super) uv: Vec2f,
}

/// Final triangle list. Syntax indices never escape this module's admission boundary.
pub(super) struct MeshData {
    /// Three finalized corners per emitted fan triangle, preserving winding.
    vertices: Vec<MeshVertex>,
    /// Checked u32 indices into finalized vertices.
    indices: Vec<u32>,
    /// Bounds seeded from referenced geometry, excluding unused source positions.
    bounds: Box3f,
}

/// Temporary syntax reference; absence is distinct from explicit index zero.
#[derive(Clone, Copy)]
struct Corner {
    /// Positive absolute source position, converted to zero-based indexing.
    position: usize,
    /// Optional source normal.
    normal: Option<usize>,
    /// Optional source UV.
    uv: Option<usize>,
}

/// Temporary face keeps source context until final-array reference resolution.
struct Face {
    /// One-based source line for resolution and geometry errors.
    line: usize,
    /// Ordered polygon corners; finalization emits a triangle fan.
    corners: Vec<Corner>,
}

/// Adds stable human-readable source context without imposing a new error hierarchy.
fn at(line: usize, detail: impl std::fmt::Display) -> String {
    format!("line {line}: {detail}")
}

/// Parses exactly the supported finite components, rejecting extra geometry fields.
fn components<const N: usize>(parts: &[&str], line: usize) -> Result<[f32; N], String> {
    if parts.len() != N {
        return Err(at(line, format!("expected {N} components, got {}", parts.len())));
    }
    let mut values = [0.; N];
    for (value, token) in values.iter_mut().zip(parts) {
        *value = token
            .parse::<f32>()
            .ok()
            .filter(|v| v.is_finite())
            .ok_or_else(|| at(line, format!("invalid finite coordinate '{token}'")))?;
    }
    Ok(values)
}

/// Converts only supported positive absolute OBJ indices; negatives and zero are explicit errors.
fn index(token: &str, line: usize) -> Result<usize, String> {
    token
        .parse::<usize>()
        .ok()
        .and_then(|n| n.checked_sub(1))
        .ok_or_else(|| at(line, format!("invalid positive absolute index '{token}'")))
}

/// Parses one face token, keeping missing fields absent until finalization.
fn corner(token: &str, line: usize) -> Result<Corner, String> {
    let parts: Vec<_> = token.split('/').collect();
    if parts.len() > 3 {
        return Err(at(line, format!("invalid corner '{token}'")));
    }
    let optional = |part: Option<&&str>| match part {
        Some(part) if !part.is_empty() => index(part, line).map(Some),
        _ => Ok(None),
    };
    Ok(Corner {
        position: index(parts[0], line)?,
        uv: optional(parts.get(1))?,
        normal: optional(parts.get(2))?,
    })
}

/// Normalizes in f64 so finite f32 geometry cannot overflow/underflow the cross-product length.
fn unit(value: [f64; 3], line: usize) -> Result<Vec3f, String> {
    let length = value.iter().map(|v| v * v).sum::<f64>().sqrt();
    if !length.is_finite() || length == 0. {
        return Err(at(line, "zero or nonfinite normal/triangle area"));
    }
    Ok(Vec3f::new((value[0] / length) as f32, (value[1] / length) as f32, (value[2] / length) as f32))
}

/// Checks triangle and explicit byte counts before any output or GPU allocation.
fn output_count(triangles: usize) -> Result<usize, String> {
    let count = triangles
        .checked_mul(3)
        .filter(|n| u32::try_from(*n).is_ok())
        .ok_or_else(|| "triangle count exceeds u32 indices".to_owned())?;
    count
        .checked_mul(32)
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or_else(|| "vertex byte count exceeds addressable storage".to_owned())?;
    Ok(count)
}

impl MeshData {
    /// Decodes supported geometry, resolves final-array references (including forward references),
    /// then publishes only finite, nondegenerate finalized triangles. Other OBJ records are ignored.
    pub(super) fn parse(bytes: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(bytes).map_err(|e| {
            let prefix = &bytes[..e.valid_up_to()];
            at(
                prefix.iter().filter(|b| **b == b'\n').count() + 1,
                format!("invalid UTF-8 at byte {}", e.valid_up_to()),
            )
        })?;
        let (mut positions, mut normals, mut uvs, mut faces) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for (line, source) in text.lines().enumerate() {
            let line = line + 1;
            let parts: Vec<_> = source.split('#').next().unwrap_or("").split_whitespace().collect();
            let Some(record) = parts.first() else {
                continue;
            };
            match *record {
                "v" => {
                    let [x, y, z] = components(&parts[1..], line)?;
                    positions.push(Vec3f::new(x, y, z));
                }
                "vn" => {
                    let [x, y, z] = components(&parts[1..], line)?;
                    normals.push(unit([f64::from(x), f64::from(y), f64::from(z)], line)?);
                }
                "vt" => {
                    let [x, y] = components(&parts[1..], line)?;
                    uvs.push(Vec2f::new(x, y));
                }
                "f" => {
                    if parts.len() < 4 {
                        return Err(at(line, "face requires at least three corners"));
                    }
                    faces.push(Face {
                        line,
                        corners: parts[1..].iter().map(|token| corner(token, line)).collect::<Result<_, _>>()?,
                    });
                }
                _ => {}
            }
        }
        let triangles = faces
            .iter()
            .try_fold(0usize, |n, face| n.checked_add(face.corners.len() - 2))
            .ok_or_else(|| "triangle count overflow".to_owned())?;
        let count = output_count(triangles)?;
        if count == 0 {
            return Err("no usable faces".into());
        }
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        vertices.try_reserve_exact(count).map_err(|_| "cannot allocate finalized vertices")?;
        indices.try_reserve_exact(count).map_err(|_| "cannot allocate triangle indices")?;
        let mut bounds: Option<Box3f> = None;
        for face in faces {
            // Resolve every corner before fan expansion; all reported indices name source arrays.
            let resolved = face
                .corners
                .iter()
                .map(|corner| {
                    let position = *positions
                        .get(corner.position)
                        .ok_or_else(|| at(face.line, format!("position index {} out of range", corner.position + 1)))?;
                    let normal = corner
                        .normal
                        .map(|i| {
                            normals
                                .get(i)
                                .copied()
                                .ok_or_else(|| at(face.line, format!("normal index {} out of range", i + 1)))
                        })
                        .transpose()?;
                    let uv = corner
                        .uv
                        .map(|i| uvs.get(i).copied().ok_or_else(|| at(face.line, format!("UV index {} out of range", i + 1))))
                        .transpose()?
                        .unwrap_or_else(Vec2f::zero);
                    Ok((position, normal, uv))
                })
                .collect::<Result<Vec<_>, String>>()?;
            for i in 2..resolved.len() {
                let triangle = [resolved[0], resolved[i - 1], resolved[i]];
                let p = triangle.map(|(p, _, _)| [f64::from(p.x), f64::from(p.y), f64::from(p.z)]);
                let a: [f64; 3] = std::array::from_fn(|j| p[1][j] - p[0][j]);
                let b: [f64; 3] = std::array::from_fn(|j| p[2][j] - p[0][j]);
                let flat = unit([a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]], face.line)?;
                for (position, normal, uv) in triangle {
                    bounds = Some(match bounds {
                        Some(bounds) => bounds.add(&position),
                        None => Box3f::new(&position, &position),
                    });
                    indices.push(u32::try_from(vertices.len()).map_err(|_| "vertex index exceeds u32")?);
                    vertices.push(MeshVertex {
                        position,
                        normal: normal.unwrap_or(flat),
                        uv,
                    });
                }
            }
        }
        Ok(Self {
            vertices,
            indices,
            bounds: bounds.ok_or("no referenced positions")?,
        })
    }

    /// Borrows immutable finalized corners for explicit byte encoding.
    pub(super) fn vertices(&self) -> &[MeshVertex] {
        &self.vertices
    }
    /// Borrows checked triangle indices; callers may copy into their owned GPU payload.
    pub(super) fn indices(&self) -> &[u32] {
        &self.indices
    }
    /// Copies referenced bounds for numeric camera admission.
    pub(super) fn bounds(&self) -> Box3f {
        self.bounds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const TRI: &str = "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";
    /// Invalid storage/geometry is rejected by the production decoder with source context.
    #[test]
    fn malformed_assets_fail_without_unwinding() {
        assert!(MeshData::parse(&[b'#', b'\n', 255]).err().unwrap().contains("line 2"));
        for face in [
            "",
            "f",
            "f 1",
            "f 1 2",
            "f 0 2 3",
            "f -1 2 3",
            "f 1 2 99",
            "f 1/1 2 3",
            "f 1//1 2 3",
            "f 1/0 2 3",
            "f 1 1 1",
            "f 1///2 2 3",
            "f 99999999999999999999999 2 3",
        ] {
            let source = format!("v 0 0 0\nv 1 0 0\nv 0 1 0\n{face}");
            assert!(MeshData::parse(source.as_bytes()).is_err(), "{face}");
        }
        for record in [
            "v NaN 0 0",
            "v inf 0 0",
            "v 1e99 0 0",
            "v 1 2",
            "v 1 2 3 4",
            "vn 0 0 0",
            "vn 1 NaN 0",
            "vt 1 2 3",
        ] {
            assert!(MeshData::parse(format!("{record}\n{TRI}").as_bytes()).is_err(), "{record}");
        }
        assert!(output_count(usize::MAX).is_err());
        assert!(output_count(u32::MAX as usize).is_err());
    }
    /// Forward references and mixed absence preserve per-corner defaults and normalized supplied normals.
    #[test]
    fn forward_references_and_mixed_attributes_are_finalized() {
        let mesh = MeshData::parse(b"f 1/1/1 2 3//1\nv 0 0 0\nv 1 0 0\nv 0 1 0\nvt .4 .7\nvn 0 0 4\n").unwrap();
        assert_eq!(mesh.indices(), [0, 1, 2]);
        assert_eq!(mesh.vertices[0].uv.x, 0.4);
        assert_eq!(mesh.vertices[1].uv.x, 0.);
        for vertex in mesh.vertices() {
            assert_eq!(vertex.normal.z, 1.);
        }
    }
    /// Nonplanar fan triangles get their own geometric normals rather than sharing polygon corners.
    #[test]
    fn nonplanar_fan_has_independent_flat_normals() {
        let mesh = MeshData::parse(b"v 0 0 0\nv 1 0 0\nv 0 1 0\nv 0 0 1\nf 1 2 3 4").unwrap();
        assert_eq!(mesh.indices().len(), 6);
        assert_eq!(mesh.vertices[0].normal.z, 1.);
        assert_eq!(mesh.vertices[3].normal.x, 1.);
    }
    /// Exact one-sided bounds exclude both the origin and an unreferenced outlier.
    #[test]
    fn referenced_bounds_and_finite_extreme_normals() {
        for sign in [-1., 1.] {
            let source = format!(
                "v {} {} {}\nv {} {} {}\nv {} {} {}\nv -10000 -10000 -10000\nf 1 2 3",
                2. * sign,
                3. * sign,
                4. * sign,
                5. * sign,
                3. * sign,
                4. * sign,
                2. * sign,
                7. * sign,
                9. * sign
            );
            let mesh = MeshData::parse(source.as_bytes()).unwrap();
            let bounds = mesh.bounds();
            assert_eq!(bounds.min.x, if sign > 0. { 2. } else { -5. });
            assert_eq!(bounds.max.z, if sign > 0. { 9. } else { -4. });
        }
        for scale in ["1e-30", "2e20"] {
            let source = format!("v 0 0 0\nv {scale} 0 0\nv 0 {scale} 0\nf 1 2 3");
            let mesh = MeshData::parse(source.as_bytes()).unwrap();
            assert_eq!(mesh.vertices[0].normal.z, 1.);
        }
    }
}
