//! Mesh validation: parse STL files and check watertightness, volume and
//! bounding box. Used by the integration tests, the CLI and the docs
//! workflow.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use crate::error::{Error, Result};

/// Statistics of a triangle mesh, computed after vertex welding.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshStats {
    /// Number of triangles in the file.
    pub triangles: usize,
    /// Distinct vertices after welding at the given tolerance.
    pub vertices: usize,
    /// Triangles with repeated vertices (degenerate).
    pub degenerate_triangles: usize,
    /// `true` when every undirected edge is used by exactly two triangles and
    /// the directed edges are balanced (consistent orientation).
    pub watertight: bool,
    /// Signed mesh volume in mm³ (positive when the facet orientation points
    /// outwards).
    pub volume: f64,
    /// Axis-aligned bounding box `[min, max]`.
    pub bbox: [[f64; 3]; 2],
}

/// Load an STL file (binary or ASCII) and return its triangles as vertex
/// coordinate triples.
pub fn read_stl(path: impl AsRef<Path>) -> Result<Vec<[[f64; 3]; 3]>> {
    let mut buf = Vec::new();
    std::fs::File::open(path)?.read_to_end(&mut buf)?;
    if buf.len() < 15 {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "file too small to be an STL",
        )));
    }
    let is_ascii = buf.starts_with(b"solid")
        && buf.iter().take(1024).position(|&b| b == b'f').is_some()
        && std::str::from_utf8(&buf[..buf.len().min(1024)])
            .map(|s| s.contains("facet"))
            .unwrap_or(false);
    if is_ascii {
        parse_ascii_stl(&buf)
    } else {
        parse_binary_stl(&buf)
    }
}

fn parse_binary_stl(buf: &[u8]) -> Result<Vec<[[f64; 3]; 3]>> {
    if buf.len() < 84 {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "binary STL shorter than its header",
        )));
    }
    let n = u32::from_le_bytes(buf[80..84].try_into().unwrap()) as usize;
    if buf.len() < 84 + n * 50 {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "binary STL truncated",
        )));
    }
    let mut tris = Vec::with_capacity(n);
    for i in 0..n {
        let mut off = 84 + i * 50 + 12; // skip the stored normal
        let mut tri = [[0.0f64; 3]; 3];
        for v in &mut tri {
            for c in v {
                *c = f32::from_le_bytes(buf[off..off + 4].try_into().unwrap()) as f64;
                off += 4;
            }
        }
        tris.push(tri);
    }
    Ok(tris)
}

fn parse_ascii_stl(buf: &[u8]) -> Result<Vec<[[f64; 3]; 3]>> {
    let text = std::str::from_utf8(buf).map_err(|_| {
        Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "ASCII STL is not UTF-8",
        ))
    })?;
    let mut tris = Vec::new();
    let mut cur: Vec<[f64; 3]> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("vertex ") {
            let nums: Vec<f64> = rest
                .split_whitespace()
                .filter_map(|t| t.parse().ok())
                .collect();
            if nums.len() == 3 {
                cur.push([nums[0], nums[1], nums[2]]);
            }
            if cur.len() == 3 {
                tris.push([cur[0], cur[1], cur[2]]);
                cur.clear();
            }
        }
    }
    Ok(tris)
}

/// Compute mesh statistics: weld vertices at `weld_tol` (decimal places of
/// the coordinate rounding), then check edges, volume and bounding box.
pub fn stats(tris: &[[[f64; 3]; 3]], weld_decimals: u32) -> MeshStats {
    let factor = 10f64.powi(weld_decimals as i32);
    let mut weld: HashMap<[i64; 3], u32> = HashMap::new();
    let mut idx_tris: Vec<[u32; 3]> = Vec::with_capacity(tris.len());
    let mut coords: Vec<[f64; 3]> = Vec::new();
    let mut degenerate = 0usize;

    for tri in tris {
        let mut ids = [0u32; 3];
        for (k, v) in tri.iter().enumerate() {
            let key = [
                (v[0] * factor).round() as i64,
                (v[1] * factor).round() as i64,
                (v[2] * factor).round() as i64,
            ];
            let next = coords.len() as u32;
            let id = *weld.entry(key).or_insert(next);
            if id == next {
                coords.push(*v);
            }
            ids[k] = id;
        }
        if ids[0] == ids[1] || ids[1] == ids[2] || ids[0] == ids[2] {
            degenerate += 1;
            continue;
        }
        idx_tris.push(ids);
    }

    // Directed edge census.
    let mut directed: HashMap<(u32, u32), usize> = HashMap::new();
    let mut undirected: HashMap<(u32, u32), usize> = HashMap::new();
    for &[a, b, c] in &idx_tris {
        for (p, q) in [(a, b), (b, c), (c, a)] {
            *directed.entry((p, q)).or_default() += 1;
            *undirected.entry((p.min(q), p.max(q))).or_default() += 1;
        }
    }
    let watertight = undirected.values().all(|&n| n == 2)
        && directed
            .iter()
            .all(|(&(p, q), &n)| n == 1 && directed.get(&(q, p)) == Some(&1));

    // Volume via the divergence theorem.
    let mut volume = 0.0f64;
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for tri in tris {
        let [a, b, c] = *tri;
        volume += dot(a, cross(sub(b, a), sub(c, a))) / 6.0;
        for v in [a, b, c] {
            for k in 0..3 {
                min[k] = min[k].min(v[k]);
                max[k] = max[k].max(v[k]);
            }
        }
    }

    MeshStats {
        triangles: tris.len(),
        vertices: coords.len(),
        degenerate_triangles: degenerate,
        watertight,
        volume,
        bbox: [min, max],
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[cfg(test)]
mod tests {
    use super::{read_stl, stats};

    #[test]
    fn two_triangles_share_an_edge() {
        let tris = vec![
            [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            [[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
        ];
        let s = stats(&tris, 6);
        assert_eq!(s.vertices, 4);
        assert_eq!(s.triangles, 2);
        assert!(!s.watertight); // open boundary along the diagonal square
        assert!((s.volume - 0.0).abs() < 1e-12); // planar -> zero volume
    }

    #[test]
    fn tetrahedron_is_watertight_with_positive_volume() {
        // Outward-oriented unit tetrahedron, volume = 1/6.
        let tris = vec![
            [[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]], // z = 0, normal -z
            [[0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]], // x = 0, normal -x
            [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]], // y = 0, normal -y
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], // slanted face, outward
        ];
        let s = stats(&tris, 6);
        assert!(s.watertight);
        assert!((s.volume - 1.0 / 6.0).abs() < 1e-12);
    }

    #[test]
    fn binary_reader_roundtrip_smoke() {
        // A minimal valid binary STL: header, count, 1 triangle record.
        let mut buf = Vec::new();
        buf.extend_from_slice(&[0u8; 80]);
        buf.extend_from_slice(&1u32.to_le_bytes());
        buf.extend_from_slice(&[0u8; 12]); // facet normal (zeros)
        for v in [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
            for c in v {
                buf.extend_from_slice(&(c as f32).to_le_bytes());
            }
        }
        buf.extend_from_slice(&0u16.to_le_bytes());
        assert_eq!(buf.len(), 84 + 50);
        let path = std::env::temp_dir().join("xyzcadrs_test_smoke.stl");
        std::fs::write(&path, &buf).unwrap();
        let tris = read_stl(&path).unwrap();
        assert_eq!(tris.len(), 1);
        std::fs::remove_file(&path).ok();
    }
}
