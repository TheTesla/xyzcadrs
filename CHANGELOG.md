# Changelog

All notable changes to this project are documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the versioning aims at [Semantic Versioning](https://semver.org/).

## [0.1.0] - 2026-10-27

Initial release: a from-scratch Rust port of
[xyzcad](https://github.com/TheTesla/xyzcad) (Python + numba).

### Added

- `Field` trait: parts are plain Rust functions `f([x, y, z]) -> u8`
  (0 = air, 1..=255 = material id) — no JIT warm-up, no macro machinery.
- Full rendering pipeline ported 1:1 from the original: hierarchical seed
  search, chaotic-bisection surface refinement, surface flood fill with
  corner-value reuse, marching cubes polygonisation from the original
  256-entry triangle lookup table, boundary-loop repair.
- Bit-exact behaviour parity with the Python original, pinned by tests
  (cube/point/edge/surface-point/polygon counters for five field/resolution
  combinations).
- Multi-material support with the original's group semantics.
- Binary STL export, colored OBJ + MTL export (uses the standard `Kd`
  diffuse statement; the original emitted the non-standard `Kb`).
- Built-in STL reader and mesh statistics (watertightness, volume, bbox,
  degenerate facets) used by tests and the `validate` CLI command.
- CLI with `render`, `validate` and `list` subcommands.
- Seed-search evaluation budget (guards against hangs on fields without
  material; 0 disables it).
- Parallel cut-point refinement and per-material meshing via rayon.
- GitHub Actions CI (rustfmt, clippy `-D warnings`, tests, release build).
