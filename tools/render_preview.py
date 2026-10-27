#!/usr/bin/env python3
"""Render preview PNGs of the example meshes for the documentation.

Usage: python3 tools/render_preview.py <mesh.stl|mesh.obj> <out.png> [elev] [azim]

Requires numpy, numpy-stl (STL input) and matplotlib. For OBJ input the
per-material groups are drawn with their MTL colors (Kd); all triangles go
into ONE Poly3DCollection so per-triangle depth sorting stays correct.
"""
import os
import sys

import numpy as np

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.colors import to_rgb
from mpl_toolkits.mplot3d.art3d import Poly3DCollection


def load_stl(path):
    from stl import mesh

    m = mesh.Mesh.from_file(path)
    return [m.vectors.reshape(-1, 3, 3)], [np.array([0.6, 0.65, 0.72])]


def load_obj(path):
    verts = []
    groups = []  # (material name, [triangle index triples])
    cur = None
    colors = {}
    with open(path) as fh:
        mtl_name = None
        for line in fh:
            parts = line.split()
            if not parts:
                continue
            if parts[0] == "mtllib":
                mtl_name = parts[1]
            elif parts[0] == "v":
                verts.append([float(v) for v in parts[1:4]])
            elif parts[0] == "usemtl":
                cur = parts[1]
                groups.append((cur, []))
            elif parts[0] == "f":
                idx = [int(t.split("/")[0]) - 1 for t in parts[1:]]
                for k in range(len(idx) - 2):
                    groups[-1][1].append([idx[0], idx[k + 1], idx[k + 2]])
    if mtl_name:
        base = os.path.join(os.path.dirname(os.path.abspath(path)), mtl_name)
        try:
            with open(base) as fh:
                name = None
                for line in fh:
                    parts = line.split()
                    if parts and parts[0] == "newmtl":
                        name = parts[1]
                    elif parts and parts[0] == "Kd" and name:
                        colors[name] = np.array([float(c) for c in parts[1:4]])
        except OSError:
            pass
    tris_per_group = []
    color_list = []
    for name, tris in groups:
        tris_per_group.append(verts_arr[np.asarray(tris)] if (verts_arr := np.asarray(verts)) is not None else None)
        color_list.append(to_rgb(colors.get(name, [0.6, 0.65, 0.72])))
    return tris_per_group, color_list


def shaded_color(rgb, normal, light=(0.4, 0.3, 0.85)):
    n = normal / (np.linalg.norm(normal) + 1e-12)
    l = np.asarray(light)
    l = l / np.linalg.norm(l)
    intensity = 0.35 + 0.65 * abs(float(np.dot(n, l)))
    return np.clip(np.asarray(rgb) * intensity, 0, 1)


def render(mesh_path, out_path, elev=22, azim=-55):
    if mesh_path.lower().endswith(".obj"):
        group_tris, group_colors = load_obj(mesh_path)
    else:
        group_tris, group_colors = load_stl(mesh_path)

    all_tris = np.concatenate(group_tris)
    v = all_tris.reshape(-1, 3)
    mins, maxs = v.min(axis=0), v.max(axis=0)
    center = (mins + maxs) / 2
    span = float((maxs - mins).max())

    fig = plt.figure(figsize=(6, 6), dpi=110)
    ax = fig.add_subplot(111, projection="3d")
    polys = []
    pc_colors = []
    for tris, rgb in zip(group_tris, group_colors):
        normals = np.cross(tris[:, 1] - tris[:, 0], tris[:, 2] - tris[:, 0])
        for tri, n in zip(tris, normals):
            polys.append(tri)
            pc_colors.append(shaded_color(rgb, n))
    pc = Poly3DCollection(polys, facecolors=pc_colors, edgecolors="none")
    ax.add_collection3d(pc)
    ax.set_xlim(center[0] - span / 2, center[0] + span / 2)
    ax.set_ylim(center[1] - span / 2, center[1] + span / 2)
    ax.set_zlim(center[2] - span / 2, center[2] + span / 2)
    ax.set_box_aspect((1, 1, 1))
    ax.view_init(elev=elev, azim=azim)
    ax.set_axis_off()
    fig.tight_layout(pad=0)
    fig.savefig(out_path, bbox_inches="tight", pad_inches=0.05)
    print(f"wrote {out_path}")


if __name__ == "__main__":
    render(sys.argv[1], sys.argv[2], *(float(a) for a in sys.argv[3:]))
