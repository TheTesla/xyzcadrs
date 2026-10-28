"""Python bindings for xyzcadrs — functional/implicit CAD in Rust.

Render material fields `f(x, y, z)` to watertight STL / colored OBJ with the
fast Rust marching-cubes renderer (a port of xyzcad). Fields can be:

* **numba ``@njit`` dispatchers** (recommended): the original xyzcad style::

      from numba import njit
      from xyzcadrs import render_and_save

      @njit
      def sphere(p):
          x, y, z = p[:3]
          return 1 if 100.0 > x*x + y*y + z*z else 0

      render_and_save(sphere, "sphere.stl", 0.3)

  The dispatcher is wrapped once via ``numba.cfunc`` so the Rust renderer
  calls the compiled machine code directly — no GIL, no per-point Python
  overhead. The compiled pointer is cached on the dispatcher.

* **plain Python callables** ``f(p)`` with ``p = (x, y, z)`` — the original
  xyzcad calling convention (functions typically do ``x, y, z = p[:3]``).
  They work out of the box (slower: one Python call per probe).

* **any object with an ``address`` attribute** holding the address of a C
  function ``uint8_t f(double x, double y, double z)``: used as a native
  callback (this is what Cython/C-generated helpers can provide).

Return values are material ids: ``<= 0`` is air, values above 255 are
clamped. Plain ``True``/``False`` mean material 1 / air.
"""

from ._native import (
    __version__,
    render as _render_native,
    render_and_save as _render_and_save_native,
    validate as _validate_native,
    write_obj as _write_obj_native,
    write_stl as _write_stl_native,
)

__all__ = [
    "__version__",
    "render",
    "render_and_save",
    "validate",
    "write_stl",
    "write_obj",
    "NumbaField",
]


class NumbaField:
    """Native wrapper around a numba ``njit`` dispatcher.

    Compiles a small trampoline with ``numba.cfunc`` so the Rust renderer
    can call the dispatcher's machine code through a C function pointer.
    The wrapper is cached on the dispatcher, so repeated renders reuse it.
    """

    __slots__ = ("dispatcher", "cfunc", "address")

    def __init__(self, dispatcher):
        import numba
        import numpy as np

        user = dispatcher

        @numba.njit
        def _wrapper(x, y, z):
            # Accept int or bool returns; cast to the u8 material id.
            return np.uint8(user((x, y, z)))

        cfunc = numba.cfunc("uint8(float64, float64, float64)")(_wrapper.py_func)
        self.dispatcher = dispatcher
        self.cfunc = cfunc
        self.address = cfunc.address


def _adapt(field):
    """Return a field object the native module understands."""
    if hasattr(field, "address"):
        return field  # already native
    try:
        from numba.core.dispatcher import Dispatcher
    except ImportError:
        return field
    if isinstance(field, Dispatcher):
        cached = getattr(field, "_xyzcadrs_native", None)
        if cached is not None:
            return cached
        native = NumbaField(field)
        try:
            field._xyzcadrs_native = native
        except AttributeError:
            pass
        return native
    return field


def render_and_save(
    field, filename, res, *, min_val=-1000.0, max_val=1000.0, eval_limit=0
):
    """Render ``field`` and write ``filename`` (``*.stl`` or ``*.obj``).

    Returns a summary dict: ``cubes``, ``materials``, ``cube_points``,
    ``cube_edges``, ``surface_points``, ``filtered_cubes``, ``polygons``,
    ``repaired_polygons`` and ``triangles_written``.
    """
    return _render_and_save_native(
        _adapt(field), filename, res, min_val=min_val, max_val=max_val, eval_limit=eval_limit
    )


def render(field, res, *, min_val=-1000.0, max_val=1000.0, eval_limit=0):
    """Render ``field`` and return a dict with ``vertices``, ``groups``,
    ``polygons`` and ``summary`` (no file is written)."""
    return _render_native(
        _adapt(field), res, min_val=min_val, max_val=max_val, eval_limit=eval_limit
    )


def validate(path, weld_decimals=5):
    """Validate an STL file: watertightness, volume, bounding box."""
    return _validate_native(path, weld_decimals)


def write_stl(path, vertices, polygons):
    """Write a binary STL from the arrays returned by :func:`render`."""
    return _write_stl_native(path, vertices, polygons)


def write_obj(path_base, vertices, groups):
    """Write a colored OBJ + MTL pair from the arrays returned by
    :func:`render`.

    ``groups`` accepts the group dicts returned by :func:`render` (each with
    ``material`` and ``polygons``) as well as ``(material, polygons)``
    tuples."""
    normalized = [
        (g["material"], g["polygons"]) if isinstance(g, dict) else g for g in groups
    ]
    return _write_obj_native(path_base, vertices, normalized)
