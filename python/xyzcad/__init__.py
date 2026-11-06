"""Drop-in compatibility layer: ``xyzcad`` backed by the Rust renderer.

This package mirrors the import surface of the original xyzcad (Python +
numba, github.com/TheTesla/xyzcad) so existing programs keep working
unchanged:

    from xyzcad import render
    render.renderAndSave(fun, fn, params["resolution"], partpl)

Internally everything is rendered by the Rust pipeline of xyzcadrs — the
field function is called with the same 4-tuple ``(x, y, z, params)`` and the
resulting meshes are bit-identical (verified against the original on real
projects). Install both with::

    pip install xyzcadrs

Note: this shim and the original xyzcad package cannot be installed into
the same virtualenv (both provide the ``xyzcad`` import name — whichever is
installed last overwrites the other's files).
"""

from xyzcad._version import __version__

__all__ = ["__version__", "render"]
