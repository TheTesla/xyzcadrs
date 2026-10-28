"""Tests for the xyzcadrs Cython bindings (requires the C library built)."""

import math

import pytest

import xyzcadrs_cython as xc


def test_version():
    assert xc.version() == "0.2.0"


def test_c_level_field_sphere_parity(tmp_path):
    # Pinned against the Python original (sphere r=10, res 0.3).
    s = xc.demo_sphere(str(tmp_path / "sphere.stl"), 0.3)
    assert s["cubes"] == 20912
    assert s["surface_points"] == 20910
    assert s["polygons"] == 20912
    assert s["repaired_polygons"] == 0


def test_python_callable_field(tmp_path):
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

    s = xc.render_and_save(hex_nut, str(tmp_path / "nut.stl"), 0.2)
    # Pinned against the Python original (hex nut, res 0.2).
    assert s["cubes"] == 28724
    assert s["surface_points"] == 28724
    assert s["repaired_polygons"] == 0
    # Analytic nut volume (hexagon across flats 16, thickness 8, bore r=4):
    # used by the Python/Go binding tests through their validators.
    hex_area = 2.0 * math.sqrt(3.0) * 64.0
    assert hex_area * 8.0 - math.pi * 16.0 * 8.0 == pytest.approx(1371.4961, abs=0.01)


def test_error_propagation(tmp_path):
    def empty(p):
        return 0

    with pytest.raises(RuntimeError, match="no solid found"):
        xc.render_and_save(empty, str(tmp_path / "empty.stl"), 1.0, eval_limit=10_000_000)
