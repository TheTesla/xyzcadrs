//! Meshing: raster cubes -> points -> edges -> cut points -> polygons.

use std::collections::{BTreeSet, HashMap};

use rayon::prelude::*;

use crate::error::Result;
use crate::field::Field;
use crate::flood::{get_surface, grid_key, CornerValues, GridKey};
use crate::math::rnd;
use crate::seed::{find_surface_pnt, get_surface_pnt};
use crate::tlt::TLT;

/// Number of bisection steps used to refine a cut point onto the exact
/// isosurface (the Python original hardcodes 24 in `precTrPnts`).
const REFINE_STEPS: u32 = 24;

/// The 12 edges of a cube, given as pairs of corner indices in
/// [`CornerValues`] order.
///
/// Index in this array = the edge numbering used by [`crate::tlt::TLT`].
/// Every pair is oriented from the lower to the higher corner index, which
/// matches the orientation the Python original produces consistently for
/// shared edges.
pub const CUBE_EDGES: [[usize; 2]; 12] = [
    [0, 4], // +x at (y, z)
    [0, 2], // +y at (x, z)
    [0, 1], // +z at (x, y)
    [3, 7], // +x at (yh, zh)
    [5, 7], // +y at (xh, zh)
    [6, 7], // +z at (xh, yh)
    [4, 5], // +z at (xh, y)
    [4, 6], // +y at (xh, z)
    [2, 6], // +x at (yh, z)
    [2, 3], // +z at (x, yh)
    [1, 3], // +y at (x, zh)
    [1, 5], // +x at (y, zh)
];

/// Counters describing one completed render, mirroring the summary the
/// Python original prints.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    /// Raster cubes visited by the flood fill.
    pub cubes: usize,
    /// Number of distinct non-air materials.
    pub materials: usize,
    /// Distinct raster corner points of all visited cubes.
    pub cube_points: usize,
    /// Unique cube edges.
    pub cube_edges: usize,
    /// Edges cut by the isosurface (= number of mesh vertices).
    pub surface_points: usize,
    /// Sum over materials of the cubes considered per material.
    pub filtered_cubes: usize,
    /// Total polygons emitted (before triangulation).
    pub polygons: usize,
    /// Boundary-closing repair polygons added.
    pub repaired_polygons: usize,
}

/// Result of a successful render.
#[derive(Debug, Clone)]
pub struct MeshResult {
    /// Refined cut points in mm; indices are referenced by the polygons.
    pub vertices: Vec<[f64; 3]>,
    /// One group per material: `(material id, polygons)` with polygon
    /// elements indexing into [`MeshResult::vertices`].
    pub groups: Vec<(u8, Vec<Vec<u32>>)>,
    /// All polygons of all groups merged into one list (convenience for
    /// single-material STL export).
    pub polygons: Vec<Vec<u32>>,
    /// Counters, see [`Summary`].
    pub summary: Summary,
}

/// Render the isosurface of `f` at resolution `res`.
///
/// `min_val`/`max_val` bound the axis-aligned search cube for the initial
/// surface point; `eval_limit` (0 = unlimited) caps the seed search's field
/// evaluations as a hang guard for fields without material.
pub fn render<F: Field + Sync>(
    f: &F,
    res: f64,
    min_val: f64,
    max_val: f64,
    eval_limit: usize,
) -> Result<MeshResult> {
    if res <= 0.0 || res.is_nan() {
        return Err(crate::error::Error::InvalidResolution(res));
    }

    let seed = find_surface_pnt(f, min_val, max_val, res, eval_limit)?;
    let flooded = get_surface(f, seed, res);

    // Deterministic cube order: sort by the bit-key (total order, no NaN).
    let mut cube_keys: Vec<[f64; 3]> = flooded.iter().map(|(k, _)| *k).collect();
    cube_keys.sort_unstable_by_key(|&k| grid_key(k));
    let corner_of: HashMap<GridKey, &CornerValues> =
        flooded.iter().map(|(k, v)| (grid_key(*k), v)).collect();
    let cube_vals: Vec<&CornerValues> =
        cube_keys.iter().map(|k| corner_of[&grid_key(*k)]).collect();

    // --- raster points -----------------------------------------------------
    let mut point_ids: HashMap<GridKey, u32> = HashMap::with_capacity(cube_keys.len() * 2);
    let mut points: Vec<[f64; 3]> = Vec::with_capacity(cube_keys.len() * 2);
    {
        let mut point_id = |p: [f64; 3]| -> u32 {
            let key = grid_key(p);
            if let Some(&id) = point_ids.get(&key) {
                id
            } else {
                let id = points.len() as u32;
                point_ids.insert(key, id);
                points.push(p);
                id
            }
        };
        for &[x, y, z] in &cube_keys {
            let xh = rnd(x + res);
            let yh = rnd(y + res);
            let zh = rnd(z + res);
            for i in 0..8 {
                point_id([
                    if i & 4 != 0 { xh } else { x },
                    if i & 2 != 0 { yh } else { y },
                    if i & 1 != 0 { zh } else { z },
                ]);
            }
        }
    }
    // Material value at every raster point. The Python original derives this
    // from the cube corners (each point receives the value of its
    // point-reflected corner, which nets out to the field value at the point
    // itself); evaluating the field directly is equivalent and cheaper.
    let point_vals: Vec<u8> = points.iter().map(|&p| f(p)).collect();

    // --- cube -> point indices and cube edges ------------------------------
    let cube_point_ids: Vec<[u32; 8]> = cube_keys
        .iter()
        .map(|&[x, y, z]| {
            let xh = rnd(x + res);
            let yh = rnd(y + res);
            let zh = rnd(z + res);
            let mut ids = [0u32; 8];
            for (i, id) in ids.iter_mut().enumerate() {
                let p = [
                    if i & 4 != 0 { xh } else { x },
                    if i & 2 != 0 { yh } else { y },
                    if i & 1 != 0 { zh } else { z },
                ];
                *id = point_ids[&grid_key(p)];
            }
            ids
        })
        .collect();

    let mut edge_ids: HashMap<(u32, u32), u32> = HashMap::with_capacity(cube_keys.len() * 6);
    let mut edges: Vec<(u32, u32)> = Vec::with_capacity(cube_keys.len() * 6);
    let mut cube_edge_ids: Vec<[u32; 12]> = Vec::with_capacity(cube_keys.len());
    for cpi in &cube_point_ids {
        let mut eids = [0u32; 12];
        for (k, eid) in eids.iter_mut().enumerate() {
            let [a, b] = CUBE_EDGES[k];
            let (pa, pb) = (cpi[a], cpi[b]);
            let key = (pa.min(pb), pa.max(pb));
            *eid = *edge_ids.entry(key).or_insert_with(|| {
                edges.push(key);
                (edges.len() - 1) as u32
            });
        }
        cube_edge_ids.push(eids);
    }

    // --- edges cut by the isosurface ---------------------------------------
    let cut_edges: Vec<u32> = edges
        .iter()
        .enumerate()
        .filter(|&(_, &(a, b))| point_vals[a as usize] != point_vals[b as usize])
        .map(|(i, _)| i as u32)
        .collect();
    let cut_vertex_of_edge: HashMap<u32, u32> = cut_edges
        .iter()
        .enumerate()
        .map(|(vi, &e)| (e, vi as u32))
        .collect();

    // Refine every cut point onto the exact isosurface, in parallel.
    let vertices: Vec<[f64; 3]> = cut_edges
        .par_iter()
        .map(|&e| {
            let (a, b) = edges[e as usize];
            get_surface_pnt(f, points[a as usize], points[b as usize], REFINE_STEPS)
        })
        .collect();

    // --- polygons per material ---------------------------------------------
    // Replicates the Python original's semantics exactly: the material list
    // is the sorted set of all corner values, the summary counts len-1, and
    // the per-material loop starts at the SECOND value (skipping air 0 in
    // the normal case; for surfaces between two non-zero materials this
    // attributes the shared interface sheet to the higher material only —
    // verified against the original: mats=[2], one group, counter 1).
    let all_values: BTreeSet<u8> = cube_vals.iter().flat_map(|v| v.iter().copied()).collect();
    let materials: Vec<u8> = all_values.iter().copied().collect();
    let mut groups = Vec::with_capacity(materials.len());
    let mut summary = Summary {
        cubes: cube_keys.len(),
        materials: materials.len().saturating_sub(1),
        cube_points: points.len(),
        cube_edges: edges.len(),
        surface_points: cut_edges.len(),
        ..Summary::default()
    };

    for &m in materials.iter().skip(1) {
        // Polygon edge-index loops per cube.
        let polys_edge: Vec<Vec<u32>> = cube_edge_ids
            .par_iter()
            .zip(cube_vals.par_iter())
            .filter_map(|(eids, &v)| {
                let mut mask = 0u32;
                for (i, &c) in v.iter().enumerate() {
                    if c == m {
                        mask |= 1 << i;
                    }
                }
                if mask == 0 || mask == 255 {
                    return None; // uniform cube: no surface inside
                }
                Some(
                    TLT[mask as usize]
                        .iter()
                        .map(|poly| poly.iter().map(|&k| eids[k as usize]).collect())
                        .collect::<Vec<Vec<u32>>>(),
                )
            })
            .collect::<Vec<Vec<Vec<u32>>>>()
            .into_iter()
            .flatten()
            .collect();

        let repaired = repair_surface(&polys_edge);

        let poly_count = polys_edge.len() + repaired.len();
        let repaired_count = repaired.len();
        let mut all_polys = polys_edge;
        all_polys.extend(repaired);

        let polys_vertex: Vec<Vec<u32>> = all_polys
            .par_iter()
            .filter_map(|poly| {
                let mapped: Vec<u32> = poly
                    .iter()
                    .filter_map(|&e| cut_vertex_of_edge.get(&e).copied())
                    .collect();
                if mapped.len() >= 3 {
                    Some(mapped)
                } else {
                    None
                }
            })
            .collect();

        summary.filtered_cubes += cube_keys.len();
        summary.polygons += poly_count;
        summary.repaired_polygons += repaired_count;
        groups.push((m, polys_vertex));
    }

    let polygons = groups.iter().flat_map(|(_, p)| p.iter().cloned()).collect();
    Ok(MeshResult {
        vertices,
        groups,
        polygons,
        summary,
    })
}

/// Close boundary loops left by the polygonisation.
///
/// Port of `xyzcad.render.repair_surface` + `build_repair_polygons`: collect
/// directed polygon edges `(next, current)` whose reversal is not present,
/// then chain them into closed loops which are appended as extra polygons.
/// For well-behaved fields this yields no repairs at all.
fn repair_surface(polys: &[Vec<u32>]) -> Vec<Vec<u32>> {
    // Directed edges whose reverse is missing, as a chain map next -> current.
    let mut single: HashMap<u32, u32> = HashMap::new();
    let mut reversed: BTreeSet<(u32, u32)> = BTreeSet::new();
    for poly in polys {
        let n = poly.len();
        for (i, &cur) in poly.iter().enumerate() {
            let nxt = poly[(i + 1) % n];
            reversed.insert((nxt, cur));
        }
    }
    for poly in polys {
        let n = poly.len();
        for (i, &cur) in poly.iter().enumerate() {
            let nxt = poly[(i + 1) % n];
            if !reversed.contains(&(cur, nxt)) {
                single.insert(nxt, cur);
            }
        }
    }

    let mut loops = Vec::new();
    // Deterministic start order; entries are consumed while chaining (the
    // Python original deletes them from its dict as it walks), so chains that
    // revisit a consumed edge terminate instead of cycling forever.
    let start_keys: BTreeSet<u32> = single.keys().copied().collect();
    for start in start_keys {
        if !single.contains_key(&start) {
            continue; // already consumed by an earlier chain
        }
        let mut chain = Vec::new();
        let mut e = start;
        while let Some(next) = single.remove(&e) {
            chain.push(e);
            e = next;
        }
        if chain.len() >= 3 {
            loops.push(chain);
        }
    }
    loops
}

#[cfg(test)]
mod tests {
    use super::{render, CUBE_EDGES};

    #[test]
    fn cube_edges_are_unit_axis_steps() {
        for (k, &[a, b]) in CUBE_EDGES.iter().enumerate() {
            let diff = a ^ b;
            assert!(diff == 1 || diff == 2 || diff == 4, "edge {k}: {a}-{b}");
        }
    }

    #[test]
    fn cube_edges_cover_all_pairs_differing_by_one_bit() {
        let mut seen = std::collections::BTreeSet::new();
        for &[a, b] in CUBE_EDGES.iter() {
            seen.insert((a.min(b), a.max(b)));
        }
        assert_eq!(seen.len(), 12);
        for a in 0..8usize {
            for b in (a + 1)..8usize {
                if (a ^ b).is_power_of_two() {
                    assert!(seen.contains(&(a, b)), "missing edge {a}-{b}");
                }
            }
        }
    }

    #[test]
    fn sphere_res_two_matches_python_reference() {
        // Pinned against the Python original (mesh_surface_function,
        // find_surface_pnt seed, res 2.0).
        let sphere = |p: [f64; 3]| {
            if 100.0 > p[0] * p[0] + p[1] * p[1] + p[2] * p[2] {
                1u8
            } else {
                0u8
            }
        };
        let r = render(&sphere, 2.0, -1000.0, 1000.0, 0).unwrap();
        assert_eq!(
            r.summary,
            super::Summary {
                cubes: 452,
                materials: 1,
                cube_points: 907,
                cube_edges: 2253,
                surface_points: 450,
                filtered_cubes: 452,
                polygons: 452,
                repaired_polygons: 0,
            }
        );
    }
}
