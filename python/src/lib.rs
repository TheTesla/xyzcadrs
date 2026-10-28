//! Python bindings for xyzcadrs (`xyzcadrs._native`).
//!
//! Fields are accepted in two forms:
//!
//! 1. **Native**: any object exposing an integer attribute `address` holding
//!    the address of a C function `uint8_t f(double x, double y, double z)`.
//!    The object is kept alive for the duration of the render. The Python
//!    package builds such an object around numba `njit` dispatchers via
//!    `numba.cfunc`, so compiled field functions run at machine speed with
//!    no GIL interaction and no per-point Python overhead.
//! 2. **Callable**: any Python callable `f(x, y, z)` returning `bool` or an
//!    `int` in `0..=255`. Called once per field probe with the GIL
//!    re-acquired per call; Python exceptions abort the render with the
//!    original exception.
//!
//! Material values are clamped to `0..=255` (values `<= 0` mean air).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use xyzcadrs::field::Field;
use xyzcadrs::mesh::{render as render_field, MeshResult};
use xyzcadrs::validate::{read_stl, stats};

type RawFieldFn = extern "C" fn(f64, f64, f64) -> u8;

/// Field source accepted from Python.
enum PyField {
    /// C-level field: call through the machine-code pointer, no GIL.
    Native { f: RawFieldFn, _keepalive: PyObject },
    /// Python callable: re-acquire the GIL per probe.
    Python { obj: PyObject },
}

fn adapt_field(py: Python, field: PyObject) -> PyResult<PyField> {
    match field.getattr(py, "address") {
        Ok(addr) => {
            let addr: usize = addr.extract(py)?;
            if addr == 0 {
                return Err(PyValueError::new_err(
                    "field.address is 0 (native code not compiled yet?)",
                ));
            }
            let f: RawFieldFn = unsafe { std::mem::transmute(addr) };
            Ok(PyField::Native {
                f,
                _keepalive: field,
            })
        }
        Err(_) => Ok(PyField::Python { obj: field }),
    }
}

/// Run `body` with the adapted field. The GIL is released for the duration;
/// the Python-callable path re-acquires it per probe.
fn with_field<T: Send>(
    py: Python,
    field: PyObject,
    body: impl FnOnce(&(dyn Field + Sync)) -> PyResult<T> + Send,
) -> PyResult<T> {
    match adapt_field(py, field)? {
        PyField::Native { f, _keepalive } => {
            let closure = move |p: [f64; 3]| f(p[0], p[1], p[2]);
            py.allow_threads(|| body(&closure))
        }
        PyField::Python { obj } => {
            let errored = Arc::new(AtomicBool::new(false));
            let err_slot: Arc<Mutex<Option<PyErr>>> = Arc::new(Mutex::new(None));
            let errored2 = Arc::clone(&errored);
            let err_slot2 = Arc::clone(&err_slot);
            let closure = move |p: [f64; 3]| -> u8 {
                if errored2.load(Ordering::Relaxed) {
                    return 0;
                }
                Python::with_gil(|py| {
                    // Original xyzcad calling convention: the callable
                    // receives ONE 3-tuple argument (functions use `p[:3]`).
                    match obj
                        .call1(py, ((p[0], p[1], p[2]),))
                        .and_then(|v| v.extract::<i64>(py))
                    {
                        Ok(v) => v.clamp(0, 255) as u8,
                        Err(e) => {
                            errored2.store(true, Ordering::Relaxed);
                            let mut slot = err_slot2.lock().unwrap();
                            slot.get_or_insert(e);
                            0
                        }
                    }
                })
            };
            let result = py.allow_threads(|| body(&closure));
            if let Some(e) = err_slot.lock().unwrap().take() {
                return Err(e);
            }
            result
        }
    }
}

fn mesh_to_dict(py: Python, mesh: &MeshResult) -> PyResult<PyObject> {
    let dict = PyDict::new_bound(py);
    let vertices: Vec<[f64; 3]> = mesh.vertices.clone();
    let vertex_list = PyList::new_bound(py, vertices.iter().map(|v| vec![v[0], v[1], v[2]]));
    dict.set_item("vertices", vertex_list)?;
    let groups = PyList::empty_bound(py);
    for (mat, polys) in &mesh.groups {
        let group = PyDict::new_bound(py);
        group.set_item("material", *mat)?;
        group.set_item("polygons", polys)?;
        groups.append(group)?;
    }
    dict.set_item("groups", groups)?;
    dict.set_item("polygons", &mesh.polygons)?;
    let summary = summary_to_dict(py, mesh, 0)?;
    dict.set_item("summary", summary)?;
    Ok(dict.into())
}

fn summary_to_dict(py: Python, mesh: &MeshResult, triangles: usize) -> PyResult<PyObject> {
    let dict = PyDict::new_bound(py);
    let s = &mesh.summary;
    dict.set_item("cubes", s.cubes)?;
    dict.set_item("materials", s.materials)?;
    dict.set_item("cube_points", s.cube_points)?;
    dict.set_item("cube_edges", s.cube_edges)?;
    dict.set_item("surface_points", s.surface_points)?;
    dict.set_item("filtered_cubes", s.filtered_cubes)?;
    dict.set_item("polygons", s.polygons)?;
    dict.set_item("repaired_polygons", s.repaired_polygons)?;
    dict.set_item("triangles_written", triangles)?;
    Ok(dict.into())
}

/// Render a material field and write the mesh to `filename`.
///
/// `field` is either a numba `njit` dispatcher (wrapped automatically to a
/// native machine-code callback by the Python package), an object with an
/// `address` attribute pointing to a `uint8_t f(double, double, double)` C
/// function, or any Python callable receiving a single 3-tuple argument
/// `(x, y, z)` — the original xyzcad calling convention. The export format
/// follows the extension (`.stl` / `.obj`). Returns the render summary as a
/// dict.
#[pyfunction]
#[pyo3(signature = (field, filename, res, *, min_val = -1000.0, max_val = 1000.0, eval_limit = 0))]
fn render_and_save(
    py: Python,
    field: PyObject,
    filename: &str,
    res: f64,
    min_val: f64,
    max_val: f64,
    eval_limit: usize,
) -> PyResult<PyObject> {
    let filename = filename.to_string();
    let (mesh, written) = with_field(py, field, move |f| {
        let mesh = render_field(f, res, min_val, max_val, eval_limit)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        let lower = filename.to_ascii_lowercase();
        let written = if lower.ends_with(".obj") {
            let base = &filename[..filename.len() - 4];
            xyzcadrs::export::write_obj(base, &mesh.vertices, &mesh.groups)
                .map_err(|e| PyValueError::new_err(e.to_string()))?
        } else {
            let stem = filename.strip_suffix(".stl").unwrap_or(&filename);
            xyzcadrs::export::write_stl(format!("{stem}.stl"), &mesh.vertices, &mesh.polygons)
                .map_err(|e| PyValueError::new_err(e.to_string()))?
        };
        Ok((mesh, written))
    })?;
    summary_to_dict(py, &mesh, written)
}

/// Render a material field and return the mesh as a dict with
/// `vertices`, `groups` (one dict per material with `material` and
/// `polygons`), `polygons` and `summary`.
#[pyfunction]
#[pyo3(signature = (field, res, *, min_val = -1000.0, max_val = 1000.0, eval_limit = 0))]
fn render(
    py: Python,
    field: PyObject,
    res: f64,
    min_val: f64,
    max_val: f64,
    eval_limit: usize,
) -> PyResult<PyObject> {
    let mesh = with_field(py, field, move |f| {
        render_field(f, res, min_val, max_val, eval_limit)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    })?;
    mesh_to_dict(py, &mesh)
}

/// Validate an STL file. Returns a dict with `triangles`, `vertices`,
/// `degenerate_triangles`, `watertight`, `volume`, `bbox_min`, `bbox_max`.
#[pyfunction]
#[pyo3(signature = (path, weld_decimals = 5))]
fn validate(py: Python, path: &str, weld_decimals: u32) -> PyResult<PyObject> {
    let tris = read_stl(path).map_err(|e| PyValueError::new_err(e.to_string()))?;
    let s = stats(&tris, weld_decimals);
    let dict = PyDict::new_bound(py);
    dict.set_item("triangles", s.triangles)?;
    dict.set_item("vertices", s.vertices)?;
    dict.set_item("degenerate_triangles", s.degenerate_triangles)?;
    dict.set_item("watertight", s.watertight)?;
    dict.set_item("volume", s.volume)?;
    dict.set_item("bbox_min", vec![s.bbox[0][0], s.bbox[0][1], s.bbox[0][2]])?;
    dict.set_item("bbox_max", vec![s.bbox[1][0], s.bbox[1][1], s.bbox[1][2]])?;
    Ok(dict.into())
}

/// Write a binary STL from vertices and polygons (as returned by `render`).
#[pyfunction]
fn write_stl(path: &str, vertices: Vec<Vec<f64>>, polygons: Vec<Vec<u32>>) -> PyResult<usize> {
    let verts = convert_vertices(vertices)?;
    xyzcadrs::export::write_stl(path, &verts, &polygons)
        .map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Write a colored OBJ + MTL pair from vertices and groups (as returned by
/// `render`).
#[pyfunction]
fn write_obj(
    path_base: &str,
    vertices: Vec<Vec<f64>>,
    groups: Vec<(u8, Vec<Vec<u32>>)>,
) -> PyResult<usize> {
    let verts = convert_vertices(vertices)?;
    xyzcadrs::export::write_obj(path_base, &verts, &groups)
        .map_err(|e| PyValueError::new_err(e.to_string()))
}

fn convert_vertices(vertices: Vec<Vec<f64>>) -> PyResult<Vec<[f64; 3]>> {
    vertices
        .into_iter()
        .map(|v| {
            if v.len() == 3 {
                Ok([v[0], v[1], v[2]])
            } else {
                Err(PyValueError::new_err(format!(
                    "vertex must have 3 coordinates, got {}",
                    v.len()
                )))
            }
        })
        .collect()
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(render_and_save, m)?)?;
    m.add_function(wrap_pyfunction!(render, m)?)?;
    m.add_function(wrap_pyfunction!(validate, m)?)?;
    m.add_function(wrap_pyfunction!(write_stl, m)?)?;
    m.add_function(wrap_pyfunction!(write_obj, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
