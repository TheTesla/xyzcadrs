//! Surface seed search: locate a point on the isosurface before flooding.

use crate::error::{Error, Result};
use crate::field::Field;

/// Search for two points with different field values on a hierarchically
/// refined grid over `[min_val, max_val]^3`.
///
/// Port of `xyzcad.render.find_init_pnt`. The first point of the pair is the
/// first probed grid point whose value differs from the field value at the
/// origin; the second point is always the origin itself.
///
/// `res_steps` bounds the refinement depth: the finest scanned grid has
/// spacing `(max_val - min_val) / 2^(res_steps - 1)`.
///
/// Returns `None` when no differing point is found (empty field) or when the
/// evaluation budget `eval_limit` (0 = unlimited) is exhausted before a
/// differing point shows up.
///
/// The scan is parallelised per depth level with an order-preserving
/// reduction (lowest scan-order index among the differing probes wins), so
/// the returned point is bit-identical to a sequential scan while the
/// millions of evaluations of a deep level run on all cores.
pub fn find_init_pnt(
    f: &(dyn Field + Sync),
    min_val: f64,
    max_val: f64,
    res_steps: u32,
    eval_limit: usize,
) -> Option<[[f64; 3]; 2]> {
    use rayon::prelude::*;

    let s0 = f([0.0, 0.0, 0.0]);
    let span = max_val - min_val;
    let mut evaluated: u64 = 1; // the origin probe
    for d in 0..res_steps {
        let n = 2u32.pow(d);
        let count = n as u64 * n as u64 * n as u64;
        // Budget guard at depth granularity (the guard only bounds fields
        // without material; a hit inside the level is still reported).
        if eval_limit > 0 && evaluated + count > eval_limit as u64 {
            return None;
        }
        let nf = n as f64;
        // Lowest scan-order index (xi outermost, zi innermost) wins, matching
        // the sequential original exactly.
        let hit = (0..count)
            .into_par_iter()
            .filter_map(|i| {
                let xi = (i / (n as u64 * n as u64)) as u32;
                let yi = ((i / n as u64) % n as u64) as u32;
                let zi = (i % n as u64) as u32;
                let p = [
                    (xi as f64 + 0.5) / nf * span + min_val,
                    (yi as f64 + 0.5) / nf * span + min_val,
                    (zi as f64 + 0.5) / nf * span + min_val,
                ];
                if f(p) != s0 {
                    Some(i)
                } else {
                    None
                }
            })
            .min();
        evaluated += count;
        if let Some(i) = hit {
            let xi = (i / (n as u64 * n as u64)) as u32;
            let yi = ((i / n as u64) % n as u64) as u32;
            let zi = (i % n as u64) as u32;
            let p = [
                (xi as f64 + 0.5) / nf * span + min_val,
                (yi as f64 + 0.5) / nf * span + min_val,
                (zi as f64 + 0.5) / nf * span + min_val,
            ];
            return Some([p, [0.0, 0.0, 0.0]]);
        }
    }
    None
}

/// Walk from `p0` towards `p1` and converge onto the isosurface between
/// them.
///
/// Port of `xyzcad.render.get_surfacePnt`: a chaotic bisection that flips
/// direction whenever the sampled field value changes. With `res_steps`
/// iterations the final step size is `|p1 - p0| / 2^(res_steps - 1)`.
pub fn get_surface_pnt(f: &dyn Field, p0: [f64; 3], p1: [f64; 3], res_steps: u32) -> [f64; 3] {
    let mut s0 = f(p0);
    let mut u = 0.0f64;
    let mut d = 1.0f64;
    let mut p = p0;
    for i in 0..res_steps {
        let one_minus_u = 1.0 - u;
        p = [
            p0[0] * one_minus_u + p1[0] * u,
            p0[1] * one_minus_u + p1[1] * u,
            p0[2] * one_minus_u + p1[2] * u,
        ];
        let s = f(p);
        if s != s0 {
            s0 = s;
            d = -d;
        }
        u += d / 2u64.pow(i) as f64;
    }
    p
}

/// Find a point lying (approximately) on the isosurface.
///
/// Port of `xyzcad.render.find_surface_pnt`. The refinement depth is derived
/// from the requested raster resolution `res`, matching the original:
/// `res_steps = floor(log2((max_val - min_val) / res)) + 2`.
pub fn find_surface_pnt(
    f: &(dyn Field + Sync),
    min_val: f64,
    max_val: f64,
    res: f64,
    eval_limit: usize,
) -> Result<[f64; 3]> {
    let res_steps = ((max_val - min_val) / res).log2().floor() as i64 + 2;
    let (p0, p1) = {
        let pair = find_init_pnt(f, min_val, max_val, res_steps as u32, eval_limit).ok_or(
            Error::NoSolidFound {
                min_val,
                max_val,
                budget: eval_limit,
            },
        )?;
        (pair[0], pair[1])
    };
    Ok(get_surface_pnt(f, p0, p1, res_steps as u32))
}

#[cfg(test)]
mod tests {
    use super::{find_surface_pnt, get_surface_pnt};

    fn sphere(p: [f64; 3]) -> u8 {
        if 100.0 > p[0] * p[0] + p[1] * p[1] + p[2] * p[2] {
            1
        } else {
            0
        }
    }

    #[test]
    fn surface_point_lies_near_sphere() {
        // The chaotic bisection converges to within a fraction of a raster
        // cell (identical to the Python original; the flood fill starts from
        // this seed and reaches the exact surface).
        let p = find_surface_pnt(&sphere, -1000.0, 1000.0, 0.5, 0).unwrap();
        let r = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        assert!((r - 10.0).abs() < 0.5, "radius {r} not near the surface");
    }

    #[test]
    fn empty_field_errors_out() {
        let flat = |_: [f64; 3]| 0u8;
        let err = find_surface_pnt(&flat, -1000.0, 1000.0, 1.0, 1_000_000).unwrap_err();
        assert!(err.to_string().contains("no solid found"));
    }

    #[test]
    fn evaluation_budget_stops_the_search() {
        let sphere_far = |p: [f64; 3]| {
            if 900.0 * 900.0 > p[0].powi(2) + p[1].powi(2) + p[2].powi(2) {
                1
            } else {
                0
            }
        };
        // Budget too small to reach the surface (the scan needs 10 evals for
        // this field) -> guarded error instead of a hang.
        let err = find_surface_pnt(&sphere_far, -1000.0, 1000.0, 1.0, 5).unwrap_err();
        assert!(err.to_string().contains("evaluation budget"));
    }

    #[test]
    fn bisection_flips_across_the_surface() {
        // Direct walk from inside to outside must land on the shell.
        let p = get_surface_pnt(&sphere, [0.0, 0.0, 0.0], [20.0, 0.0, 0.0], 24);
        let r = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        assert!((r - 10.0).abs() < 1e-5, "radius {r}");
    }
}
