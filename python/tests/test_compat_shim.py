"""Tests: the bundled ``xyzcad`` compatibility shim.

Old programs keep importing ``from xyzcad import render`` and calling
``render.renderAndSave(fun, fn, res, partpl)`` — the Rust renderer runs
underneath. Counters are pinned against the Python original.
"""

import math

import pytest


def test_import_surface():
    from xyzcad import __version__, render

    assert __version__ == "0.3.0"
    assert hasattr(render, "renderAndSave")
    assert hasattr(render, "mesh_surface_function")
    assert hasattr(render, "get_installed_version")


def test_render_and_save_original_call_shape(tmp_path):
    from xyzcad import render

    from numba import njit

    @njit
    def sphere(p):
        x, y, z = p[:3]
        return 1 if 100.0 > x * x + y * y + z * z else 0

    fn = str(tmp_path / "sphere.stl")
    result = render.renderAndSave(sphere, fn, 2.0)
    assert result["cubes"] == 452
    assert result["surface_points"] == 450


def test_mesh_surface_function_return_shape(tmp_path):
    import numpy as np

    from xyzcad import render

    from numba import njit

    @njit
    def hex_nut(p):
        x, y, z = p[:3]
        if abs(z) > 4.0:
            return 0
        c = 0.8660254037844386
        if abs(c * x + 0.5 * y) > 8.0 or abs(y) > 8.0 or abs(c * x - 0.5 * y) > 8.0:
            return 0
        if (x * x + y * y) ** 0.5 < 4.0:
            return 0
        return 1

    partpl = (1.0, 2.0)  # arbitrary params, unused by this field
    vertices, faces_grpd, mats, summary = render.mesh_surface_function(
        hex_nut, 0.2, partpl
    )
    assert isinstance(vertices, np.ndarray) and vertices.shape[1] == 3
    assert mats == [1]
    assert len(faces_grpd[0]) == 28724
    assert summary["cubes"] == 28724
    assert summary["repaired_polygons"] == 0
    # Polygons index into vertices.
    max_idx = max(max(poly) for poly in faces_grpd[0])
    assert max_idx < len(vertices)
