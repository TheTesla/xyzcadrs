# Changelog

All notable changes to this project are documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the versioning aims at [Semantic Versioning](https://semver.org/).

## [0.2.0] - 2026-10-27

### Added

- **Python bindings** (`python/`, PyO3 + maturin): render fields written as
  numba `njit` dispatchers — the compiled machine code is called directly
  through a `numba.cfunc` trampoline (no GIL, no per-point Python
  overhead); plain Python callables use the original xyzcad calling
  convention (single 3-tuple argument). Includes `render`, `render_and_save`,
  `validate`, `write_stl`, `write_obj`, a pytest suite pinned to the
  original's counters, and a benchmark script (`python/benchmarks/`).
  Warm renders are ~4.1–4.3x faster than the original numba renderer; cold
  starts drop from ~25 s of renderer JIT to well under a second.
- **C ABI** (`c-api/`): `libxyzcadrs_c` (shared + static) exposing render
  (STL/OBJ), validation and summary/stat structs with a thread-safe field
  callback `uint8_t (*)(double, double, double, void*)`. Panics are caught
  at the boundary; errors are reported through caller-provided buffers.
- **C++ header** (`c-api/include/xyzcadrs.hpp`): RAII wrapper with
  `std::function` fields and exception-based error reporting.
- **Cython bindings** (`bindings/cython/`): Python-callable trampoline plus
  zero-overhead C-level fields passed as PyCapsules.
- **Go bindings** (`bindings/go/`): cgo package with an exported trampoline
  and a field registry (concurrent fields supported).
- `docs/BINDINGS.md`: build/run instructions and parity matrix for all
  bindings.
- The Rust entry points now accept `&dyn Field` trait objects, which makes
  embedding (FFI, interpreters) straightforward without generics.

### Changed

- Renderer entry points (`render`, `render_and_save`, `render_mesh`, seed
  and flood functions) take `&dyn Field` instead of generic `F: Field`.

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
