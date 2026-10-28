# xyzcadrs — Python bindings

Python bindings for the [xyzcadrs](https://github.com/TheTesla/xyzcadrs)
Rust renderer: write your material fields exactly like in the original
[xyzcad](https://github.com/TheTesla/xyzcad) — as numba `@njit` functions —
and render them watertight to STL/OBJ with the fast Rust marching-cubes
pipeline.

```python
from numba import njit
from xyzcadrs import render_and_save

@njit
def sphere(p):
    x, y, z = p[:3]
    return 1 if 100.0 > x*x + y*y + z*z else 0

render_and_save(sphere, "sphere.stl", 0.3)
```

## Install (from source)

```console
cd python
python3 -m venv ../.venv && source ../.venv/bin/activate
pip install maturin pytest numba numpy
maturin develop --release
pytest          # runs the parity + watertightness test suite
```

Requires the Rust toolchain (cargo). The njit path additionally needs numba
and numpy; plain Python callables work without them.

## Measured performance (sphere r = 10 mm, incl. STL writing)

| res | original xyzcad (warm numba) | xyzcadrs binding | speed-up |
| --- | --- | --- | --- |
| 0.3 mm | 0.444 s | 0.109 s | 4.1x |
| 0.15 mm | 2.130 s | 0.500 s | 4.3x |

Cold start: the original compiles its whole renderer with numba (~25 s per
new field); the binding compiles only your field (as the original does) and
renders immediately — total first-call time ≈ 0.5 s.

See [`docs/BINDINGS.md`](../docs/BINDINGS.md) for API details and the
benchmark script in [`benchmarks/`](benchmarks/).
