# Bindings

xyzcadrs is a Rust library at its core; the renderer is available from
several languages through two layers:

```
                 ┌────────────────────────────┐
   Rust closures │  xyzcadrs (Rust crate)     │  native Rust API
                 └──────────┬─────────────────┘
                            │
        ┌───────────────────┴────────────────────┐
        │ C ABI: libxyzcadrs_c (c-api/)          │  extern "C", function
        │ field = fn(x,y,z, void* ctx) -> u8     │  pointer + context
        └───────────────────┬────────────────────┘
                            │
   ┌──────────┬─────────────┼──────────────┬─────────────┐
   │ Python   │ C / C++     │ Cython       │ Go (cgo)    │
   │ (PyO3 +  │ (direct)    │ (capsule /   │ (exported   │
   │ numba    │             │  trampoline) │  tramp.)    │
   │ cfunc)   │             │              │             │
   └──────────┴─────────────┴──────────────┴─────────────┘
```

All bindings produce bit-identical results to the Rust/Python originals and
are covered by parity tests (see the table at the bottom).

## Python (recommended: `@njit` fields)

Located in [`python/`](../python/), built with [maturin](https://www.maturin.rs)
(PyO3). This is the closest match to the original xyzcad workflow — you keep
writing fields exactly like before:

```python
from numba import njit
from xyzcadrs import render_and_save, validate

@njit
def hex_nut(p):
    x, y, z = p[:3]
    if abs(z) > 4.0:
        return 0
    c = 0.8660254037844386
    if abs(c*x + 0.5*y) > 8.0 or abs(y) > 8.0 or abs(c*x - 0.5*y) > 8.0:
        return 0
    if (x*x + y*y) ** 0.5 < 4.0:
        return 0
    return 1

summary = render_and_save(hex_nut, "hex_nut.stl", 0.2)
print(summary["cubes"], summary["polygons"])
print(validate("hex_nut.stl")["watertight"])
```

How it works: a numba dispatcher is wrapped once via `numba.cfunc`, which
compiles a small trampoline with a C signature; the Rust renderer then calls
the **machine code of your field directly** — no GIL, no per-point Python
overhead, no interpreter round trips. The wrapper is cached on the
dispatcher (`field._xyzcadrs_native`).

Plain Python callables also work (same calling convention: one 3-tuple
argument, `x, y, z = p[:3]`); they are called once per probe with the GIL
re-acquired — convenient for prototyping, roughly 100x slower than `njit`.

API (module `xyzcadrs`):

| Function | Purpose |
| --- | --- |
| `render_and_save(field, filename, res, *, min_val=-1000, max_val=1000, eval_limit=0)` | render + write `.stl`/`.obj`; returns summary dict |
| `render(field, res, ...)` | render; returns dict with `vertices`, `groups`, `polygons`, `summary` |
| `validate(path, weld_decimals=5)` | STL statistics (watertight, volume, bbox) |
| `write_stl(path, vertices, polygons)` / `write_obj(path_base, vertices, groups)` | export previously rendered data |

Build & test (Python ≥ 3.9, numba + numpy for the njit path):

```console
cd python
python3 -m venv ../.venv && source ../.venv/bin/activate
pip install maturin pytest numba numpy
maturin develop --release
pytest
```

Benchmark (same njit field rendered by both pipelines, warm):

| res | original xyzcad | xyzcadrs binding | speed-up |
| --- | --- | --- | --- |
| 0.3 mm | 0.444 s | 0.109 s | 4.1x |
| 0.15 mm | 2.130 s | 0.500 s | 4.3x |

The original additionally pays ~25 s of renderer JIT compilation per new
field on a cold start; the binding has none (first call ≈ 0.45 s including
the one-time `numba.cfunc` compilation). Script:
`python/benchmarks/bench_vs_xyzcad.py <path-to-xyzcad-checkout>`.

## C

Located in [`c-api/`](../c-api/): builds `libxyzcadrs_c` (shared + static)
with a plain C ABI (see [`c-api/include/xyzcadrs.h`](../c-api/include/xyzcadrs.h)).

```c
#include "xyzcadrs.h"

static uint8_t sphere(double x, double y, double z, void *ud) {
    (void)ud;
    return 100.0 > x*x + y*y + z*z ? 1 : 0;
}

int main(void) {
    xyzcadrs_summary s;
    char err[512];
    xyzcadrs_render_stl(sphere, NULL, 0.3, -1000.0, 1000.0, 0,
                        "sphere.stl", &s, err, sizeof(err));
    return 0;
}
```

The field callback may be called from multiple render threads and must be
thread-safe. Errors return `1` with a message in the `err` buffer; panics
are caught at the boundary. Build & run the example:

```console
cd c-api && cargo build --release && cd ..
cc -O2 -I c-api/include c-api/examples/render_sphere.c \
   -L c-api/target/release -lxyzcadrs_c \
   -Wl,-rpath,"$PWD/c-api/target/release" -o /tmp/c_example
/tmp/c_example /tmp/c_sphere.stl
```

## C++

[`c-api/include/xyzcadrs.hpp`](../c-api/include/xyzcadrs.hpp) wraps the C ABI
with `std::function` fields and exceptions:

```cpp
#include "xyzcadrs.hpp"

xyzcadrs::Summary s = xyzcadrs::render_stl(
    [](double x, double y, double z) -> std::uint8_t {
        return 100.0 > x*x + y*y + z*z ? 1 : 0;
    },
    0.3, "sphere.stl");
```

Each call copies the `std::function` into callback-owned storage, so
concurrent renders with different fields are safe. Build & run:

```console
g++ -O2 -std=c++17 -I c-api/include c-api/examples/render_hexnut.cpp \
    -L c-api/target/release -lxyzcadrs_c \
    -Wl,-rpath,"$PWD/c-api/target/release" -o /tmp/cpp_example
/tmp/cpp_example /tmp/cpp_nut.stl
```

## Cython

Located in [`bindings/cython/`](../bindings/cython/). Two field styles:

1. **Python callable** — original xyzcad convention (single 3-tuple), called
   through a GIL-reacquiring trampoline:

   ```python
   import xyzcadrs_cython as xc
   summary = xc.render_and_save(lambda p: 100.0 > p[0]**2 + p[1]**2 + p[2]**2,
                                "sphere.stl", 0.3)
   ```

2. **C-level field (zero callback overhead)** — a `cdef` field in your own
   Cython module, passed as a PyCapsule:

   ```cython
   from cpython.pycapsule cimport PyCapsule_New
   import xyzcadrs_cython as xc

   cdef uint8_t my_field(double x, double y, double z, void* ud) noexcept nogil:
       return 100.0 > x*x + y*y + z*z

   def render():
       capsule = PyCapsule_New(<void*>my_field, NULL, NULL)
       return xc.render_and_save_fn(capsule, 0, "sphere.stl", 0.3)
   ```

Build & test:

```console
cd bindings/cython
pip install .        # requires c-api/target/release/libxyzcadrs_c.so
python -c "import xyzcadrs_cython as xc; print(xc.demo_sphere('/tmp/s.stl', 0.3))"
```

## Go (cgo)

Located in [`bindings/go/`](../bindings/go/):

```go
package main

import (
    "github.com/TheTesla/xyzcadrs/bindings/go/xyzcadrs"
)

func main() {
    summary, err := xyzcadrs.RenderSTL(
        func(x, y, z float64) uint8 {
            if 100.0 > x*x+y*y+z*z { return 1 }
            return 0
        },
        0.3, "sphere.stl")
    ...
}
```

The Go field is registered and invoked through a single exported cgo
trampoline (thread-safe, multiple concurrent fields supported). Build & test:

```console
cd c-api && cargo build --release && cd ../bindings/go
go test ./...      # parity + error-propagation tests
```

For fields without material use `RenderSTLWithLimits(f, res, minVal, maxVal,
evalLimit, path)` to bound the seed search (0 = unlimited); `RenderSTL`
runs without a budget, like the original.

## Parity

All bindings are pinned to the counters measured on the Python original
(github.com/TheTesla/xyzcad):

| Field | res | cubes | surface points | polygons | repaired | verified in |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| sphere r=10 | 0.3 | 20912 | 20910 | 20912 | 0 | C, Cython, Python (`render`), Go test |
| sphere r=10 | 2.0 | 452 | 450 | 452 | 0 | Python, Rust tests |
| hex nut | 0.2 | 28724 | 28724 | 28724 | 0 | Python, C++, Cython, Go tests |
