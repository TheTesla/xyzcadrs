//! High-level render + save orchestration, mirroring `xyzcad.render.renderAndSave`.

use std::time::Instant;

use crate::error::Result;
use crate::export::{write_obj, write_stl};
use crate::field::Field;
use crate::mesh::{render, MeshResult, Summary};

/// Default search volume for the initial surface point (matches the Python
/// original's `-1000..1000`).
pub const DEFAULT_MIN_VAL: f64 = -1000.0;
/// See [`DEFAULT_MIN_VAL`].
pub const DEFAULT_MAX_VAL: f64 = 1000.0;

/// Default evaluation budget for the seed search. Fields whose origin lies
/// in air need a deep hierarchical scan before material is found (tens of
/// millions of evaluations are normal there — same as the Python original).
/// The budget only guards against rendering a field without any material at
/// all. `0` disables the guard.
pub const DEFAULT_EVAL_LIMIT: usize = 1_000_000_000;

/// Render the field and write the mesh to `filename`.
///
/// The export format follows the file extension, like the Python original:
///
/// * `*.stl` — binary STL (all materials merged),
/// * `*.obj` — triangulated OBJ with per-material groups and a companion
///   MTL file,
/// * anything else — binary STL with `.stl` appended.
///
/// Progress lines are printed to stdout with elapsed seconds, matching the
/// original's log format. For silent rendering use [`render`] and the
/// exporters directly. Returns the render [`Summary`].
pub fn render_and_save<F: Field + Sync>(f: &F, filename: &str, res: f64) -> Result<Summary> {
    let t0 = Instant::now();
    let log = |elapsed: f64, msg: &str| println!("[{elapsed:9.4}] {msg}");
    log(
        0.0,
        &format!("running xyzcadrs (Rust port of xyzcad), res {res}"),
    );

    let result = render(f, res, DEFAULT_MIN_VAL, DEFAULT_MAX_VAL, DEFAULT_EVAL_LIMIT)?;
    log(t0.elapsed().as_secs_f64(), "meshing done");
    report(&result.summary);

    let lower = filename.to_ascii_lowercase();
    if lower.ends_with(".obj") {
        let base = &filename[..filename.len() - 4];
        let tris = write_obj(base, &result.vertices, &result.groups)?;
        log(
            t0.elapsed().as_secs_f64(),
            &format!("wrote {base}.obj (+ .mtl), {tris} triangles"),
        );
    } else {
        let stem = filename.strip_suffix(".stl").unwrap_or(filename);
        let out = format!("{stem}.stl");
        let tris = write_stl(&out, &result.vertices, &result.polygons)?;
        log(
            t0.elapsed().as_secs_f64(),
            &format!("wrote {out}, {tris} triangles"),
        );
    }
    log(t0.elapsed().as_secs_f64(), "Done.");
    Ok(result.summary)
}

/// Print the summary counters in the column layout of the original.
pub fn report(summary: &Summary) {
    let rows = [
        ("cubes", summary.cubes),
        ("materials", summary.materials),
        ("cube points", summary.cube_points),
        ("cube edges", summary.cube_edges),
        ("surface points", summary.surface_points),
        ("filtered cubes", summary.filtered_cubes),
        ("polygons", summary.polygons),
        ("repaired polygons", summary.repaired_polygons),
    ];
    for (k, v) in rows {
        println!("{k:>36}{v:>28}");
    }
}

/// Convenience: render and hand back the mesh without writing anything.
pub fn render_mesh<F: Field + Sync>(f: &F, res: f64) -> Result<MeshResult> {
    render(f, res, DEFAULT_MIN_VAL, DEFAULT_MAX_VAL, DEFAULT_EVAL_LIMIT)
}
