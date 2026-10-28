#!/usr/bin/env python3
"""Benchmark: original xyzcad renderer vs. the xyzcadrs Python binding.

Both run the SAME numba njit field function. The original renders with its
own numba-compiled pipeline; the binding hands the compiled field to the
Rust renderer via numba.cfunc. Warm timings (compilation excluded, in
process), total wall time including STL writing.
"""
import os
import subprocess
import sys
import time

XYZCAD_REPO = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
    os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "xyzcad"
)
XYZCAD_REPO = os.path.abspath(XYZCAD_REPO)

# Python interpreter that has the original xyzcad + its dependencies
# (numba, numpy, numpy-stl) installed; defaults to the running interpreter.
ORIGINAL_PYTHON = os.environ.get("XYZCAD_PYTHON", sys.executable)

ORIGINAL_SCRIPT = r'''
import sys, time
sys.path.insert(0, {repo!r})
from numba import njit
from xyzcad import render

@njit
def sphere(p):
    x, y, z = p[:3]
    return 1 if 100.0 > x*x + y*y + z*z else 0

res = float(sys.argv[1])
times = []
for i in range(3):
    t0 = time.perf_counter()
    render.renderAndSave(sphere, "/tmp/bench_orig.stl", res)
    times.append(time.perf_counter() - t0)
print("COLD %.3f WARM %.4f" % (times[0], min(times[1:])))
'''.format(repo=XYZCAD_REPO)


def bench_original(res):
    out = subprocess.run(
        [ORIGINAL_PYTHON, "-c", ORIGINAL_SCRIPT, str(res)],
        capture_output=True, text=True,
    )
    lines = [l for l in out.stdout.splitlines() if l.startswith("COLD")]
    if not lines:
        raise RuntimeError("original benchmark failed:\n" + out.stdout + out.stderr)
    parts = dict(zip(lines[-1].split()[0::2], lines[-1].split()[1::2]))
    return float(parts["COLD"]), float(parts["WARM"])


def njit_sphere():
    from numba import njit

    @njit
    def sphere(p):
        x, y, z = p[:3]
        return 1 if 100.0 > x * x + y * y + z * z else 0

    return sphere


def bench_binding(res, runs=3):
    import xyzcadrs

    field = njit_sphere()
    times = []
    cold = None
    for i in range(runs):
        t0 = time.perf_counter()
        xyzcadrs.render_and_save(field, "/tmp/bench_bind.stl", res)
        dt = time.perf_counter() - t0
        times.append(dt)
        if cold is None:
            cold = dt  # includes the one-time cfunc compilation
    return cold, min(times[1:])


def main():
    print(f"{'res':>6} | {'orig cold':>10} | {'orig warm':>10} | {'bind cold*':>10} | "
          f"{'bind warm':>10} | {'warmup speedup':>14}")
    for res in (0.3, 0.15):
        ocold, owarm = bench_original(res)
        bcold, bwarm = bench_binding(res)
        print(f"{res:>6} | {ocold:>9.2f}s | {owarm:>9.4f}s | {bcold:>9.4f}s | "
              f"{bwarm:>9.4f}s | {owarm / bwarm:>13.1f}x")
    print("* binding 'cold' = first render incl. the one-time numba.cfunc "
          "compilation of the field trampoline (~0.5 s), but with no renderer "
          "JIT at all.")


if __name__ == "__main__":
    main()
