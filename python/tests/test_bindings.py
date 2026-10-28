"""Tests for the xyzcadrs Python bindings (pytest).

Parity counters are pinned against the Python original xyzcad
(github.com/TheTesla/xyzcad, numba 0.61, numpy 2.2).
"""

import math
import os

import pytest

import xyzcadrs


@ pytest.fixture
def tmp_stl(tmp_path):
    return tmp_path / "out.stl"


def njit_sphere():
    from numba import njit

    @njit
    def sphere(p):
        x, y, z = p[:3]
        return 1 if 100.0 > x * x + y * y + z * z else 0

    return sphere


def njit_hex_nut():
    from numba import njit

    @njit
    def hex_nut(p):
        x, y, z = p[:3]
        if abs(z) > 4.0:
            return 0
        c = 0.8660254037844386
        ax = abs(c * x + 0.5 * y)
        ay = abs(y)
        az = abs(c * x - 0.5 * y)
        if ax > 8.0 or ay > 8.0 or az > 8.0:
            return 0
        if (x * x + y * y) ** 0.5 < 4.0:
            return 0
        return 1

    return hex_nut


def assert_summary_parity(summary, expected):
    got = [
        summary["cubes"],
        summary["materials"],
        summary["cube_points"],
        summary["cube_edges"],
        summary["surface_points"],
        summary["polygons"],
        summary["repaired_polygons"],
    ]
    assert got == expected


def test_njit_sphere_parity(tmp_stl):
    sphere = njit_sphere()
    summary = xyzcadrs.render_and_save(sphere, str(tmp_stl), 2.0)
    assert_summary_parity(summary, [452, 1, 907, 2253, 450, 452, 0])
    assert summary["triangles_written"] > 0
    assert tmp_stl.exists()

    stats = xyzcadrs.validate(str(tmp_stl))
    assert stats["watertight"] is True
    assert stats["degenerate_triangles"] == 0


def test_njit_sphere_cached_wrapper_is_reused(tmp_stl):
    sphere = njit_sphere()
    xyzcadrs.render_and_save(sphere, str(tmp_stl), 2.0)
    native = sphere._xyzcadrs_native
    xyzcadrs.render_and_save(sphere, str(tmp_stl), 2.0)
    assert sphere._xyzcadrs_native is native


def test_njit_hex_nut_parity_and_volume(tmp_stl):
    hex_nut = njit_hex_nut()
    summary = xyzcadrs.render_and_save(hex_nut, str(tmp_stl), 0.2)
    assert_summary_parity(summary, [28724, 1, 57448, 143620, 28724, 28724, 0])
    stats = xyzcadrs.validate(str(tmp_stl))
    assert stats["watertight"] is True
    # Analytic: hexagon (across flats 16) * 8 - bore (r=4, h=8).
    hex_area = 2.0 * math.sqrt(3.0) * 64.0
    expected = hex_area * 8.0 - math.pi * 16.0 * 8.0
    assert abs(stats["volume"] - expected) / expected < 0.02


def test_bool_returning_njit_field(tmp_path):
    from numba import njit

    @njit
    def sphere_bool(p):
        x, y, z = p[:3]
        return 100.0 > x * x + y * y + z * z

    summary = xyzcadrs.render_and_save(sphere_bool, str(tmp_path / "b.stl"), 2.0)
    assert_summary_parity(summary, [452, 1, 907, 2253, 450, 452, 0])


def test_plain_python_callable_same_calling_convention(tmp_path):
    def sphere(p):
        x, y, z = p[:3]
        return 1 if 100.0 > x * x + y * y + z * z else 0

    summary = xyzcadrs.render_and_save(sphere, str(tmp_path / "c.stl"), 2.0)
    assert_summary_parity(summary, [452, 1, 907, 2253, 450, 452, 0])


def test_render_returns_arrays():
    result = xyzcadrs.render(njit_sphere(), 2.0)
    assert len(result["vertices"]) == 450
    assert len(result["polygons"]) == 452
    assert result["groups"][0]["material"] == 1
    assert result["summary"]["cubes"] == 452
    xs = [v[0] for v in result["vertices"]]
    assert max(xs) < 10.001 and min(xs) > -10.001


def test_exception_in_python_field_propagates(tmp_path):
    def bad(p):
        raise RuntimeError("boom")

    with pytest.raises(RuntimeError, match="boom"):
        xyzcadrs.render_and_save(bad, str(tmp_path / "x.stl"), 2.0)


def test_two_material_interface_semantics():
    from numba import njit

    @njit
    def two_mat(p):
        x, y, z = p[:3]
        r2 = x * x + y * y + z * z
        if r2 < 64.0:
            return 1
        if r2 < 144.0:
            return 2
        return 0

    result = xyzcadrs.render(two_mat, 1.0)
    # Python-verified: only the 1|2 interface is walked, attributed to
    # material 2 (mats=[2], one group, counter 1).
    assert result["summary"]["materials"] == 1
    assert result["groups"][0]["material"] == 2
    assert_summary_parity(result["summary"], [1208, 1, 2416, 6030, 1206, 1208, 0])


def test_obj_export_roundtrip(tmp_path):
    hex_nut = njit_hex_nut()
    base = tmp_path / "nut"
    summary = xyzcadrs.render_and_save(hex_nut, str(base) + ".obj", 0.3)
    assert (tmp_path / "nut.obj").exists()
    assert (tmp_path / "nut.mtl").exists()
    mtl = (tmp_path / "nut.mtl").read_text()
    assert "newmtl mtl1" in mtl and "Kd " in mtl
    assert summary["triangles_written"] > 0


def test_write_helpers_roundtrip(tmp_path):
    result = xyzcadrs.render(njit_sphere(), 2.0)
    stl_path = tmp_path / "manual.stl"
    tris = xyzcadrs.write_stl(str(stl_path), result["vertices"], result["polygons"])
    assert tris == sum(len(p) - 2 for p in result["polygons"])
    stats = xyzcadrs.validate(str(stl_path))
    assert stats["watertight"] is True

    result2 = xyzcadrs.render(njit_hex_nut(), 0.3)
    obj_path = tmp_path / "manual"
    tris2 = xyzcadrs.write_obj(str(obj_path), result2["vertices"], result2["groups"])
    assert tris2 > 0
    assert (tmp_path / "manual.obj").exists()


def test_empty_field_raises(tmp_path):
    def flat(p):
        return 0

    with pytest.raises(ValueError, match="no solid found"):
        xyzcadrs.render_and_save(flat, str(tmp_path / "e.stl"), 1.0, eval_limit=1000)
