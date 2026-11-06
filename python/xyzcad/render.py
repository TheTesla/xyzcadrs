"""``xyzcad.render`` shim: same API as the original, Rust renderer inside.

Provided functions (subset chosen by real-world usage, e.g. SpiroBrixx's
``spirostd.make_model``):

* ``renderAndSave(func, filename, res=1, params=())`` — identical signature
  and argument order to the original; returns the render summary dict
  (the original returns None; extra return values are additive and
  harmless).
* ``mesh_surface_function(func, res, params, t0=None)`` — returns the
  original's 4-tuple ``(vertices, faces_grpd, mats, summary)`` with plain
  numpy/list types instead of numba typed lists.

The field function contract is unchanged: a numba ``njit`` function (or
plain callable) receiving ``(x, y, z, params)`` and returning bool/int.
"""

import numpy as np

from xyzcad._version import __version__ as _SHIM_VERSION
from xyzcadrs import __version__ as _renderer_version
from xyzcadrs import render as _render_mesh
from xyzcadrs import render_and_save as _render_and_save

# Version string of the original xyzcad this shim replaces, as it appears
# in the original's startup line ("running xyzcad version 0.7.3").
_ORIGINAL_VERSION = "0.7.3"

__all__ = ["renderAndSave", "mesh_surface_function", "get_installed_version"]


def get_installed_version():
    """Version of the renderer powering this shim (xyzcadrs)."""
    return _renderer_version


def renderAndSave(func, filename, res=1, params=()):
    """Render ``func`` and write ``filename`` (``*.stl`` / ``*.obj``).

    Same call shape as ``xyzcad.render.renderAndSave`` of the original.
    The field is invoked with ``(x, y, z, params)``. Returns the summary
    dict (additive; the original returns None).
    """
    import time

    t0 = time.time()
    # Startup line like the original's, stating which renderer is actually
    # running and what it replaces (the original would print
    # "running xyzcad version 0.7.3").
    print(f"running rust implementation xyzcadrs {_renderer_version} "
          f"(replacing xyzcad version {_ORIGINAL_VERSION})")
    result = _render_and_save(func, filename, res, params)
    print(f"[{time.time() - t0:9.4f}] Done.")
    return result


def mesh_surface_function(func, res, params, t0=None):
    """Render ``func`` and return ``(vertices, faces_grpd, mats, summary)``.

    Mirrors the original's return shape with plain numpy/list types instead
    of numba typed lists: ``vertices`` is an ``(N, 3)`` float array,
    ``faces_grpd`` a list with one polygon list per material, ``mats`` the
    material id per group and ``summary`` the counter dict.
    """
    mesh = _render_mesh(func, res, params)
    vertices = np.asarray(mesh["vertices"], dtype=np.float64)
    faces_grpd = [list(map(list, g["polygons"])) for g in mesh["groups"]]
    mats = [g["material"] for g in mesh["groups"]]
    summary = dict(mesh["summary"])
    return vertices, faces_grpd, mats, summary
