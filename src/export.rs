//! Mesh exporters: binary STL and colored OBJ + MTL.

use std::io::{BufWriter, Write};
use std::path::Path;

use crate::error::Result;

/// Triangulate polygons as a fan and return `(a, b, c)` vertex index
/// triples. Polygons with fewer than 3 vertices are skipped.
pub fn polygons_to_triangles(polygons: &[Vec<u32>]) -> Vec<[u32; 3]> {
    let mut tris = Vec::new();
    for poly in polygons {
        let n = poly.len();
        if n < 3 {
            continue;
        }
        for i in 0..n - 2 {
            tris.push([poly[0], poly[i + 1], poly[i + 2]]);
        }
    }
    tris
}

fn f32_le_bytes(buf: &mut Vec<u8>, v: f64) {
    buf.extend_from_slice(&(v as f32).to_le_bytes());
}

/// Write a binary STL file.
///
/// Triangles are generated as a fan over each polygon; the facet normal is
/// computed from the triangle geometry (right-hand rule over the polygon
/// vertex order).
///
/// Returns the number of triangles written.
pub fn write_stl(
    path: impl AsRef<Path>,
    vertices: &[[f64; 3]],
    polygons: &[Vec<u32>],
) -> Result<usize> {
    let tris = polygons_to_triangles(polygons);
    let file = std::fs::File::create(path)?;
    let mut w = BufWriter::new(file);

    let mut header = [0u8; 80];
    let label = b"binary STL, written by xyzcadrs (github.com/TheTesla/xyzcadrs)";
    header[..label.len()].copy_from_slice(label);
    w.write_all(&header)?;
    w.write_all(&(tris.len() as u32).to_le_bytes())?;

    let mut rec = Vec::with_capacity(50);
    for t in &tris {
        let a = vertices[t[0] as usize];
        let b = vertices[t[1] as usize];
        let c = vertices[t[2] as usize];
        let (ux, uy, uz) = (b[0] - a[0], b[1] - a[1], b[2] - a[2]);
        let (vx, vy, vz) = (c[0] - a[0], c[1] - a[1], c[2] - a[2]);
        let mut nx = uy * vz - uz * vy;
        let mut ny = uz * vx - ux * vz;
        let mut nz = ux * vy - uy * vx;
        let len = (nx * nx + ny * ny + nz * nz).sqrt();
        if len > 0.0 {
            nx /= len;
            ny /= len;
            nz /= len;
        } else {
            nx = 0.0;
            ny = 0.0;
            nz = 0.0;
        }
        rec.clear();
        f32_le_bytes(&mut rec, nx);
        f32_le_bytes(&mut rec, ny);
        f32_le_bytes(&mut rec, nz);
        for v in [a, b, c] {
            f32_le_bytes(&mut rec, v[0]);
            f32_le_bytes(&mut rec, v[1]);
            f32_le_bytes(&mut rec, v[2]);
        }
        rec.extend_from_slice(&0u16.to_le_bytes());
        w.write_all(&rec)?;
    }
    w.flush()?;
    Ok(tris.len())
}

/// Deterministic preview color for a material id, ported from
/// `xyzcad.export.create_gen_color`.
///
/// Material 0 maps to black, material 1 to light gray; higher ids spread a
/// bit pattern over the RGB channels so neighbouring ids stay distinguishable.
pub fn gen_color(m: u8) -> [f64; 3] {
    match m {
        0 => [0.0, 0.0, 0.0],
        1 => [0.75, 0.75, 0.75],
        _ => {
            let c = m as u32 - 1;
            let bit = |shift: u32| ((c >> shift) & 1) as f64;
            [
                0.5 * bit(0) + 0.25 * bit(3) + 0.125 * bit(6),
                0.5 * bit(1) + 0.25 * bit(4) + 0.125 * bit(7),
                0.5 * bit(2) + 0.25 * bit(5) + 0.125 * bit(8),
            ]
        }
    }
}

/// Write a triangulated OBJ file with one material group per render group,
/// plus a companion MTL file.
///
/// Unlike the Python original (which emits `Kb` for the diffuse color — a
/// quirk most viewers ignore), the MTL uses the standard `Kd` statement.
///
/// Returns the number of triangles written.
pub fn write_obj(
    path_base: impl AsRef<Path>,
    vertices: &[[f64; 3]],
    groups: &[(u8, Vec<Vec<u32>>)],
) -> Result<usize> {
    let base = path_base.as_ref();
    let obj_path = base.with_extension("obj");
    let mtl_path = base.with_extension("mtl");
    let mtl_name = mtl_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    // MTL companion file.
    {
        let file = std::fs::File::create(&mtl_path)?;
        let mut w = BufWriter::new(file);
        for (m, _) in groups {
            let c = gen_color(*m);
            writeln!(w, "newmtl mtl{m}")?;
            writeln!(w, "Ka {} {} {}", c[0], c[1], c[2])?;
            writeln!(w, "Kd {} {} {}", c[0], c[1], c[2])?;
            writeln!(w, "Ks {} {} {}", c[0], c[1], c[2])?;
            writeln!(w, "d 1.0")?;
            writeln!(w, "illum 1.0")?;
        }
        w.flush()?;
    }

    let file = std::fs::File::create(&obj_path)?;
    let mut w = BufWriter::new(file);
    writeln!(w, "mtllib {mtl_name}")?;
    for v in vertices {
        writeln!(w, "v {:.6} {:.6} {:.6}", v[0], v[1], v[2])?;
    }
    let mut tris_total = 0usize;
    for (m, polys) in groups {
        writeln!(w, "usemtl mtl{m}")?;
        for t in polygons_to_triangles(polys) {
            // OBJ indices are 1-based.
            writeln!(w, "f {} {} {}", t[0] + 1, t[1] + 1, t[2] + 1)?;
            tris_total += 1;
        }
    }
    w.flush()?;
    Ok(tris_total)
}

#[cfg(test)]
mod tests {
    use super::{gen_color, polygons_to_triangles};

    #[test]
    fn fan_triangulation() {
        let polys = vec![vec![0, 1, 2, 3], vec![4, 5, 6], vec![7]];
        let tris = polygons_to_triangles(&polys);
        assert_eq!(tris, vec![[0, 1, 2], [0, 2, 3], [4, 5, 6]]);
    }

    #[test]
    fn gen_color_matches_python_reference() {
        // Values from xyzcad.export.create_gen_color (float-exact):
        // R = 0.5*bit0 + 0.25*bit3 + 0.125*bit6 of (m - 1), etc.
        assert_eq!(gen_color(0), [0.0, 0.0, 0.0]);
        assert_eq!(gen_color(1), [0.75, 0.75, 0.75]);
        assert_eq!(gen_color(2), [0.5, 0.0, 0.0]); // c = 1: bit0 -> R
        assert_eq!(gen_color(3), [0.0, 0.5, 0.0]); // c = 2: bit1 -> G
        assert_eq!(gen_color(4), [0.5, 0.5, 0.0]); // c = 3: bits 0+1 -> R+G
        assert_eq!(gen_color(9), [0.25, 0.0, 0.0]); // c = 8: bit3 -> R*0.25
    }
}
