# Algorithm and conventions

This document describes the rendering pipeline of xyzcadrs, the exact
conventions it uses, and how behaviour parity with the Python original
([TheTesla/xyzcad](https://github.com/TheTesla/xyzcad)) was established and
verified.

## Pipeline

```
field f([x,y,z]) -> u8
        │
        ▼
[1] seed search          find_surface_pnt      hierarchical grid + chaotic bisection
        │
        ▼
[2] flood fill           get_surface           walk the isosurface cube by cube
        │                                       → Vec<(cube low corner, 8 corner values)>
        ▼
[3] meshing              mesh::render          points → edges → cut edges → refined vertices
        │                                       → per-material polygons (TLT) → repair
        ▼
[4] export               export::write_stl     binary STL / grouped OBJ + MTL
                         export::write_obj
```

### 1. Seed search

Port of `xyzcad.render.find_init_pnt` / `get_surfacePnt` /
`find_surface_pnt`.

- The value at the origin defines the "inside" reference `s0`.
- A hierarchical grid over `[-1000, 1000]³` is probed: at depth `d` the
  grid has `2^d` points per axis at `(xi + 0.5) / 2^d` offsets; the scan
  depth is `floor(log2(2000 / res)) + 2`.
- The first probe whose value differs from `s0` becomes `p0`; `p1` is
  always the origin.
- The chaotic bisection walks from `p0` towards `p1`: `u += ±1/2^i`,
  flipping direction whenever the sampled value changes, for
  `res_steps` iterations. The result lands within a fraction of a raster
  cell of the surface — close enough for the flood fill, which then finds
  the exact surface.

Fields whose origin lies in air (e.g. a part with a bore at the origin)
need this scan to reach a fine grid before material shows up — tens of
millions of evaluations are normal. `xyzcadrs` caps the scan with an
evaluation budget (`render::DEFAULT_EVAL_LIMIT`, 0 = unlimited) and
returns `Error::NoSolidFound` instead of hanging.

### 2. Flood fill

Port of `xyzcad.render.get_surface`. Coordinates are quantized with
`rnd(x) = floor(10000·x + 0.5) / 10000` (round half up to 4 decimals),
exactly like the original, so identical coordinates are bit-identical and
deduplicate by equality (the Rust implementation hashes `f64::to_bits`).

Corner indexing: cube corner `i` sits at
`(x + r·[i&4], y + r·[i&2], z + r·[i&1])` — **x is bit 4, y is bit 2, z is
bit 1** (note: this differs from the corner indexing of the edge stage
below; the original mixes two conventions and compensates in
`convert_corners2pts`).

Starting cube: low corner `rnd(seed − r/2)`, entry direction 0 (no reuse).
Neighbours are pushed in the order +x, +y, +z, −x, −y, −z, skipping:

- the direction the walk came from,
- neighbours behind a uniform face (all four face corner values equal),
- already-stored cubes.

The four corner values of the entered face are carried in the stack entry
and reused (`(0 == (i & |d|)) == (1 == sign(d))` in the original; spelled
out per direction in `flood.rs`). A memoized probe verified that every
stored corner value equals the field value at the corner's true
coordinates, and that face reuse never introduces a wrong value.

### 3. Meshing

Port of `convert_corners2pts`, `coords2relations`, `cutCedgeIdx`,
`precTrPnts`, `calc_polygons` / `calc_closed_surface` (TLT) and
`repair_surface`.

- **Raster points**: the 8 corners of every visited cube, deduplicated.
  The Python original assigns each point the value of its
  point-reflected cube corner — a systematic half-cube displacement that
  nets out to the field value at the point itself (verified: 966/966
  points match the true field on a reference sphere). xyzcadrs therefore
  evaluates the field directly at each raster point; the resulting cut
  edge sets are identical.
- **Cube edges**: 12 edges per cube, indexed as in
  [`mesh::CUBE_EDGES`](../src/mesh.rs) — in *flood-fill corner indexing*,
  each oriented low→high corner index. This orientation matches the
  original's edge list exactly (verified: shared edges are listed in the
  same direction from both adjacent cubes, so the Python set-deduplication
  equals the Rust one).
- **Cut edges**: edges whose two point values differ.
- **Vertex refinement**: every cut point is refined onto the exact
  isosurface with the original's 24-step chaotic bisection between the two
  raster points (`precTrPnts`), parallelised with rayon.
- **Corner masks and the TLT**: for material `m`, a cube's mask is
  `Σ (v[i] == m) << i` over the flood-fill corner values `v`. The
  256-entry triangle lookup table (`tlt.rs`, generated mechanically from
  the original's `xyzcad/tlt.py`) maps masks to polygons of edge indices
  in `CUBE_EDGES` numbering. This convention was verified against the
  original on a rendered sphere: with it, zero polygons reference uncut
  edges and every cut edge is used exactly 4× (once per adjacent cube).
- **Materials**: the sorted set of all corner values. The summary counts
  `len − 1`, and the per-material loop starts at the *second* value —
  replicating the original's `materials[1:]` quirk. For a normal field
  (values `{0, 1, ...}`) this skips air; for a surface between two
  non-zero materials the shared interface sheet is attributed to the
  higher material only (verified against the original: `mats=[2]`, one
  group, counter 1).
- **Repair**: polygons are loops of directed edges. Directed edges whose
  reversal never appears are chained into loops (consuming entries as the
  walk proceeds, like the original's `del single_edge_dict[e]`), which are
  appended as repair polygons. Start order is deterministic (sorted) in
  xyzcadrs; the original's depends on Python set order, so the *number* of
  repair loops can differ slightly on degenerate fields (same total
  boundary). Well-behaved fields produce 0 repairs.

### 4. Export

- **Binary STL**: fan triangulation per polygon (n − 2 triangles), facet
  normals from the triangle geometry, little-endian f32 coordinates.
- **OBJ + MTL**: `v` lines at 1e-6 precision, one `usemtl mtl<id>` group
  per material, fan-triangulated faces, MTL with `Ka`/`Kd`/`Ks`/`d`/
  `illum` and colors from the ported `create_gen_color` bit pattern
  (material 0 = black, 1 = 0.75 gray). The original emitted `Kb` for the
  diffuse color; xyzcadrs uses the standard `Kd`.

## Parity verification

Reference counters were measured with `mesh_surface_function` of the
Python original (numba 0.61, numpy 2.2) using the original's own seed
search, and are pinned in `tests/integration.rs`:

| Field | res | cubes | points | edges | surface points | polygons | repaired |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| sphere r=10 | 2.0 | 452 | 907 | 2253 | 450 | 452 | 0 |
| sphere r=10 | 1.0 | 1874 | 3754 | 9372 | 1872 | 1874 | 0 |
| sphere r=10 | 0.5 | 7538 | 15088 | 37704 | 7536 | 7538 | 0 |
| two-material nested spheres | 1.0 | 1208 | 2416 | 6030 | 1206 | 1208 | 0 |
| two-material nested spheres | 0.5 | 4832 | 9676 | 24174 | 4830 | 4832 | 0 |
| hex nut | 0.2 | 28724 | 57448 | 143620 | 28724 | 28724 | 0 |

The Rust port reproduces all counters exactly (IEEE-754 f64 arithmetic in
the same operation order as the numpy scalar code makes the seed and the
raster bit-identical).

Additional structural invariants tested:

- every cut edge is referenced by exactly 4 cube polygons (single-material
  surface signature),
- exported STLs re-read from disk are watertight with 0 degenerate facets,
- volumes match the analytic values within 0.2 %,
- MTL output contains `Kd` and never `Kb`.

## Known limitations (inherited from the original)

- **One connected surface per render**: the flood fill starts at one seed
  and covers exactly the connected surface component it lands on. Two
  disjoint solids render only the first one found; the outer shell of a
  nested material arrangement is a separate surface and is not rendered.
- The raster lattice is anchored at the seed cube (`rnd(seed − r/2)`), so
  cube counts vary slightly with the seed position.
- Degenerate geometry (zero-thickness slits, tangential contacts) creates
  non-manifold corner configurations that the lookup table cannot
  represent; the repair stage closes the resulting boundary loops, but
  heavily degenerate fields keep defects. Avoid zero-width features or
  lower the resolution.
