# cython: language_level=3
"""Cython bindings for the xyzcadrs C API.

Two ways to supply a field:

1. **C-level field (zero callback overhead)** — write a ``cdef`` field
   function in your own Cython module and pass it as a PyCapsule::

       from cpython.pycapsule cimport PyCapsule_New

       cdef uint8_t my_field(double x, double y, double z, void* ud) noexcept nogil:
           ...  # pure C computation

       capsule = PyCapsule_New(<void*>my_field, NULL, NULL)
       xyzcadrs_cython.render_and_save_fn(capsule, 0, "out.stl", 0.3)

2. **Python callable** — pass any Python function ``f((x, y, z))`` (original
   xyzcad convention: a single 3-tuple argument). It is invoked through a
   GIL-reacquiring trampoline; simple but slower.

The renderer itself is the Rust xyzcadrs pipeline (watertight STL / OBJ).
"""

from libc.stdint cimport int64_t, uint8_t
from cpython.pycapsule cimport PyCapsule_GetPointer

cdef extern from "xyzcadrs.h":
    ctypedef uint8_t (*xyzcadrs_field_fn)(double x, double y, double z,
                                          void* user_data)

    ctypedef struct xyzcadrs_summary:
        int64_t cubes
        int64_t materials
        int64_t cube_points
        int64_t cube_edges
        int64_t surface_points
        int64_t filtered_cubes
        int64_t polygons
        int64_t repaired_polygons

    int xyzcadrs_render_stl(xyzcadrs_field_fn field, void* user_data,
                            double res, double min_val, double max_val,
                            unsigned long long eval_limit,
                            const char* filename,
                            xyzcadrs_summary* summary_out,
                            char* err, size_t err_len) nogil

    const char* xyzcadrs_version()


cdef uint8_t _py_trampoline(double x, double y, double z, void* user_data) noexcept nogil:
    """Call the Python object held in user_data with the original xyzcad
    convention (a single 3-tuple), re-acquiring the GIL."""
    with gil:
        obj = <object>user_data
        return <uint8_t> obj((x, y, z))


cdef uint8_t _sphere_field(double x, double y, double z, void* user_data) noexcept nogil:
    cdef double r2 = x * x + y * y + z * z
    if 100.0 > r2:
        return 1
    return 0


cdef _summary_to_dict(xyzcadrs_summary* s):
    return {
        "cubes": s.cubes,
        "materials": s.materials,
        "cube_points": s.cube_points,
        "cube_edges": s.cube_edges,
        "surface_points": s.surface_points,
        "filtered_cubes": s.filtered_cubes,
        "polygons": s.polygons,
        "repaired_polygons": s.repaired_polygons,
    }


cdef int _render(xyzcadrs_field_fn fp, void* user_data, double res,
                 double min_val, double max_val, unsigned long long eval_limit,
                 str filename, xyzcadrs_summary* out_summary):
    cdef bytes fn_bytes = filename.encode("utf-8")
    cdef const char* fn_ptr = fn_bytes
    cdef char err[512]
    cdef int rc
    with nogil:
        rc = xyzcadrs_render_stl(fp, user_data, res, min_val, max_val,
                                 eval_limit, fn_ptr, out_summary,
                                 err, sizeof(err))
    if rc != 0:
        raise RuntimeError(err.decode("utf-8", "replace"))
    return rc


def version():
    return xyzcadrs_version().decode("ascii")


def render_and_save_fn(capsule, unsigned long long user_data, str filename,
                       double res, double min_val=-1000.0,
                       double max_val=1000.0, eval_limit=0):
    """Render using a C field callback given as a PyCapsule.

    The capsule must hold a ``uint8_t (*)(double, double, double, void*)``
    function pointer (see the module docstring for how to create it from
    Cython). ``user_data`` is passed through as the context pointer.
    """
    cdef xyzcadrs_summary s
    cdef xyzcadrs_field_fn fp = <xyzcadrs_field_fn> PyCapsule_GetPointer(
        capsule, NULL)
    cdef void* ud = <void*> <size_t> user_data
    if fp == NULL:
        raise ValueError("capsule does not contain a field function pointer")
    _render(fp, ud, res, min_val, max_val, eval_limit, filename, &s)
    return _summary_to_dict(&s)


def render_and_save(field, str filename, double res, double min_val=-1000.0,
                    double max_val=1000.0, eval_limit=0):
    """Render a Python callable field (single 3-tuple argument, original
    xyzcad convention) and write ``filename`` as binary STL. Returns the
    summary dict."""
    cdef xyzcadrs_summary s
    cdef object keepalive = field
    _render(_py_trampoline, <void*> keepalive, res, min_val, max_val,
            eval_limit, filename, &s)
    return _summary_to_dict(&s)


def demo_sphere(str filename, double res=0.3):
    """Render the canonical sphere example with a true C-level field
    (zero callback overhead) and return the summary dict."""
    cdef xyzcadrs_summary s
    _render(_sphere_field, NULL, res, -1000.0, 1000.0, 0, filename, &s)
    return _summary_to_dict(&s)
