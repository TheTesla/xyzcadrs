//! C ABI for xyzcadrs.
//!
//! The field is supplied as a C function pointer plus an optional
//! `user_data` context pointer:
//!
//! ```c
//! uint8_t field(double x, double y, double z, void *user_data);
//! ```
//!
//! The callback may be invoked concurrently from multiple threads and must
//! therefore be thread-safe (pure computation; no thread-local state).
//!
//! Every function returns `0` on success and `1` on error; a human-readable
//! message is written to the caller-provided `err` buffer (NUL-terminated,
//! truncated if too small). Panics are caught and reported as errors.

use std::ffi::{c_char, c_int, c_void, CStr};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;

use xyzcadrs::mesh::{render, MeshResult};
use xyzcadrs::validate::{read_stl, stats};

/// Field callback: material id at `(x, y, z)`; `0` = air, `1..=255` =
/// material id (values are truncated to `uint8_t` by the caller side).
pub type XyzcadrsFieldFn = extern "C" fn(f64, f64, f64, *mut c_void) -> u8;

/// Render summary counters (mirror of `xyzcadrs::mesh::Summary`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct XyzcadrsSummary {
    /// Raster cubes visited by the flood fill.
    pub cubes: i64,
    /// Distinct non-air material count per the original's convention.
    pub materials: i64,
    /// Distinct raster corner points.
    pub cube_points: i64,
    /// Unique cube edges.
    pub cube_edges: i64,
    /// Edges cut by the isosurface (= number of mesh vertices).
    pub surface_points: i64,
    /// Sum over materials of the cubes considered per material.
    pub filtered_cubes: i64,
    /// Total polygons emitted (before triangulation).
    pub polygons: i64,
    /// Boundary-closing repair polygons added.
    pub repaired_polygons: i64,
}

/// Mesh statistics (mirror of `xyzcadrs::validate::MeshStats`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct XyzcadrsMeshStats {
    /// Triangles in the file.
    pub triangles: i64,
    /// Distinct vertices after welding.
    pub vertices: i64,
    /// Triangles with repeated vertices.
    pub degenerate_triangles: i64,
    /// 1 = watertight, 0 = not.
    pub watertight: c_int,
    /// Signed mesh volume in mm³.
    pub volume: f64,
    /// Bounding box minimum (x, y, z).
    pub bbox_min: [f64; 3],
    /// Bounding box maximum (x, y, z).
    pub bbox_max: [f64; 3],
}

fn write_err(err: *mut c_char, err_len: usize, msg: &str) {
    if err.is_null() || err_len == 0 {
        return;
    }
    let bytes = msg.as_bytes();
    let n = bytes.len().min(err_len - 1);
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr() as *const c_char, err, n);
        *err.add(n) = 0;
    }
}

fn panic_message(p: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        format!("panic: {s}")
    } else if let Some(s) = p.downcast_ref::<String>() {
        format!("panic: {s}")
    } else {
        "panic: unknown".to_string()
    }
}

/// Adapter: the C callback + context travel as a closure. A function
/// pointer and a `usize` are both `Send + Sync`, so the closure satisfies
/// the renderer's `Fn + Sync` bound without unsafe impls; the C contract
/// requires the callback itself to be thread-safe.
fn run_render(
    fp: XyzcadrsFieldFn,
    user_data: *mut c_void,
    res: f64,
    min_val: f64,
    max_val: f64,
    eval_limit: u64,
) -> Result<MeshResult, String> {
    let user_data_addr = user_data as usize;
    let f = move |p: [f64; 3]| -> u8 {
        fp(p[0], p[1], p[2], user_data_addr as *mut c_void)
    };
    catch_unwind(AssertUnwindSafe(|| {
        render(&f, res, min_val, max_val, eval_limit as usize).map_err(|e| e.to_string())
    }))
    .map_err(|p| panic_message(p))
    .and_then(|r| r)
}

fn fill_summary(out: *mut XyzcadrsSummary, mesh: &MeshResult) {
    if out.is_null() {
        return;
    }
    let s = &mesh.summary;
    unsafe {
        (*out) = XyzcadrsSummary {
            cubes: s.cubes as i64,
            materials: s.materials as i64,
            cube_points: s.cube_points as i64,
            cube_edges: s.cube_edges as i64,
            surface_points: s.surface_points as i64,
            filtered_cubes: s.filtered_cubes as i64,
            polygons: s.polygons as i64,
            repaired_polygons: s.repaired_polygons as i64,
        };
    }
}

/// Render the field and write a binary STL to `filename`.
///
/// Returns 0 on success, 1 on error. `eval_limit` = 0 disables the seed
/// search budget guard.
///
/// # Safety
///
/// `filename` must be a valid NUL-terminated C string; `field` must be a
/// thread-safe callback (or NULL, which is an error); `err`/`err_len`
/// describe a writable buffer of `err_len` bytes (or NULL/0).
#[no_mangle]
pub unsafe extern "C" fn xyzcadrs_render_stl(
    field: Option<XyzcadrsFieldFn>,
    user_data: *mut c_void,
    res: f64,
    min_val: f64,
    max_val: f64,
    eval_limit: u64,
    filename: *const c_char,
    summary_out: *mut XyzcadrsSummary,
    err: *mut c_char,
    err_len: usize,
) -> c_int {
    let Some(fp) = field else {
        write_err(err, err_len, "field callback is NULL");
        return 1;
    };
    let filename = if filename.is_null() {
        write_err(err, err_len, "filename is NULL");
        return 1;
    } else {
        match CStr::from_ptr(filename).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => {
                write_err(err, err_len, "filename is not valid UTF-8");
                return 1;
            }
        }
    };
    match run_render(fp, user_data, res, min_val, max_val, eval_limit) {
        Ok(mesh) => {
            fill_summary(summary_out, &mesh);
            match xyzcadrs::export::write_stl(&filename, &mesh.vertices, &mesh.polygons) {
                Ok(_) => 0,
                Err(e) => {
                    write_err(err, err_len, &format!("STL export failed: {e}"));
                    1
                }
            }
        }
        Err(msg) => {
            write_err(err, err_len, &msg);
            1
        }
    }
}

/// Render the field and write a colored OBJ + MTL pair (groups per
/// material) with the given base file name (without extension).
///
/// # Safety
///
/// Same contract as [`xyzcadrs_render_stl`].
#[no_mangle]
pub unsafe extern "C" fn xyzcadrs_render_obj(
    field: Option<XyzcadrsFieldFn>,
    user_data: *mut c_void,
    res: f64,
    min_val: f64,
    max_val: f64,
    eval_limit: u64,
    filename_base: *const c_char,
    summary_out: *mut XyzcadrsSummary,
    err: *mut c_char,
    err_len: usize,
) -> c_int {
    let Some(fp) = field else {
        write_err(err, err_len, "field callback is NULL");
        return 1;
    };
    let base = if filename_base.is_null() {
        write_err(err, err_len, "filename_base is NULL");
        return 1;
    } else {
        match CStr::from_ptr(filename_base).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => {
                write_err(err, err_len, "filename_base is not valid UTF-8");
                return 1;
            }
        }
    };
    match run_render(fp, user_data, res, min_val, max_val, eval_limit) {
        Ok(mesh) => {
            fill_summary(summary_out, &mesh);
            let path = PathBuf::from(&base);
            match xyzcadrs::export::write_obj(path, &mesh.vertices, &mesh.groups) {
                Ok(_) => 0,
                Err(e) => {
                    write_err(err, err_len, &format!("OBJ export failed: {e}"));
                    1
                }
            }
        }
        Err(msg) => {
            write_err(err, err_len, &msg);
            1
        }
    }
}

/// Validate an STL file and fill `stats_out`.
///
/// # Safety
///
/// `path` must be a valid NUL-terminated C string; `stats_out` must point to
/// a writable `XyzcadrsMeshStats` (or be NULL to skip the output).
#[no_mangle]
pub unsafe extern "C" fn xyzcadrs_validate(
    path: *const c_char,
    weld_decimals: c_int,
    stats_out: *mut XyzcadrsMeshStats,
    err: *mut c_char,
    err_len: usize,
) -> c_int {
    let path = if path.is_null() {
        write_err(err, err_len, "path is NULL");
        return 1;
    } else {
        match CStr::from_ptr(path).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => {
                write_err(err, err_len, "path is not valid UTF-8");
                return 1;
            }
        }
    };
    let result = catch_unwind(AssertUnwindSafe(|| {
        read_stl(&path)
            .map(|tris| stats(&tris, weld_decimals.max(0) as u32))
            .map_err(|e| e.to_string())
    }))
    .map_err(|p| panic_message(p))
    .and_then(|r| r);
    match result {
        Ok(s) => {
            if !stats_out.is_null() {
                *stats_out = XyzcadrsMeshStats {
                    triangles: s.triangles as i64,
                    vertices: s.vertices as i64,
                    degenerate_triangles: s.degenerate_triangles as i64,
                    watertight: s.watertight as c_int,
                    volume: s.volume,
                    bbox_min: s.bbox[0],
                    bbox_max: s.bbox[1],
                };
            }
            0
        }
        Err(msg) => {
            write_err(err, err_len, &msg);
            1
        }
    }
}

/// Renderer version string (static, valid for the program lifetime).
#[no_mangle]
pub extern "C" fn xyzcadrs_version() -> *const c_char {
    static VERSION: &[u8] = concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes();
    VERSION.as_ptr() as *const c_char
}
