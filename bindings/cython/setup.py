"""Build the xyzcadrs Cython extension against the prebuilt C library.

Requires the C library to be built first:
    cargo build --release -p xyzcadrs-capi   (in c-api/, or from the repo
    root: `cd c-api && cargo build --release`)

Install (into an activated environment):
    pip install .

The library search path and rpath are embedded as absolute paths computed
at build time — fine for development installs; for relocatable packaging,
adjust `library_dirs`/`runtime_library_dirs`.
"""
import os

from setuptools import Extension, setup
from Cython.Build import cythonize

LIB_DIR = os.path.abspath("../../c-api/target/release")

ext = Extension(
    "xyzcadrs_cython",
    ["xyzcadrs_cython.pyx"],
    include_dirs=["../../c-api/include"],
    libraries=["xyzcadrs_c"],
    library_dirs=[LIB_DIR],
    runtime_library_dirs=[LIB_DIR],
    extra_compile_args=["-O2"],
)

setup(
    ext_modules=cythonize([ext], language_level=3),
)

