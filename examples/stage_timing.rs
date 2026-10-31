//! Stage timing + parallel scaling probe.
//!
//! Run: cargo run --release --example stage_timing -- [res]
//!
//! Measures, for the sphere field:
//! 1. total render wall time at RAYON_NUM_THREADS=1 vs 16 (set via env)
//! 2. the split seed / flood fill / meshing (difference), which reveals how
//!    much of the pipeline is sequential (Amdahl's law).

use std::time::Instant;

use xyzcadrs::flood::get_surface;
use xyzcadrs::mesh::render;
use xyzcadrs::seed::find_surface_pnt;

fn sphere(p: [f64; 3]) -> u8 {
    if 100.0 > p[0] * p[0] + p[1] * p[1] + p[2] * p[2] {
        1
    } else {
        0
    }
}

fn median_of3(mut f: impl FnMut() -> f64) -> f64 {
    let mut xs = [f(), f(), f()];
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs[1]
}

fn main() {
    let res: f64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.2);
    let threads = std::env::var("RAYON_NUM_THREADS")
        .ok()
        .unwrap_or_else(|| "default".to_string());

    let total = median_of3(|| {
        let t0 = Instant::now();
        render(&sphere, res, -1000.0, 1000.0, 0).unwrap();
        t0.elapsed().as_secs_f64()
    });

    let seed = median_of3(|| {
        let t0 = Instant::now();
        let p = find_surface_pnt(&sphere, -1000.0, 1000.0, res, 0).unwrap();
        std::hint::black_box(p);
        t0.elapsed().as_secs_f64()
    });

    let seed_frozen = find_surface_pnt(&sphere, -1000.0, 1000.0, res, 0).unwrap();
    let flood = median_of3(|| {
        let t0 = Instant::now();
        let c = get_surface(&sphere, seed_frozen, res);
        std::hint::black_box(c.len());
        t0.elapsed().as_secs_f64()
    });

    let meshing = total - seed - flood;
    println!(
        "res {res:<6} threads={threads:<4} total={total:.4}s | seed={seed:.4}s flood={flood:.4}s meshing={meshing:.4}s | sequential share={:.1}%",
        (seed + flood) / total * 100.0
    );
}
