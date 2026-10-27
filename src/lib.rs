//! xyzcadrs — functional/implicit CAD in Rust.
//!
//! A Rust port of [xyzcad](https://github.com/TheTesla/xyzcad) (Python +
//! numba): describe a part as a function `f(x, y, z) -> material`, and the
//! renderer produces a watertight, 3D-printable mesh via a flood fill over
//! the isosurface, marching-cubes polygonisation from a 256-entry triangle
//! lookup table, and exact bisection of every cut point onto the surface.
//!
//! # Quick start
//!
//! ```
//! use xyzcadrs::render_mesh;
//!
//! let ball = |p: [f64; 3]| if 100.0 > p[0] * p[0] + p[1] * p[1] + p[2] * p[2] { 1u8 } else { 0u8 };
//! let mesh = render_mesh(&ball, 0.5).unwrap();
//! assert_eq!(mesh.summary.materials, 1);
//! assert_eq!(mesh.summary.repaired_polygons, 0);
//! // mesh.vertices + mesh.groups feed the STL/OBJ exporters
//! ```
//!
//! # Module map
//!
//! * [`field`] — the `Field` trait (`f([x, y, z]) -> u8`)
//! * [`seed`] — find a point on the isosurface
//! * [`flood`] — walk the whole surface cube by cube
//! * [`mesh`] — points, edges, cut points, polygons per material
//! * [`tlt`] — the marching cubes triangle lookup table
//! * [`export`] — binary STL and colored OBJ/MTL writers
//! * [`validate`] — STL reader and mesh statistics (watertightness, volume)
//! * [`render`] — `render_and_save`, the `renderAndSave` counterpart
//! * [`examples`] — built-in demo fields

pub mod error;
pub mod examples;
pub mod export;
pub mod field;
pub mod flood;
pub mod math;
pub mod mesh;
pub mod render;
pub mod seed;
pub mod tlt;
pub mod validate;

pub use error::{Error, Result};
pub use field::Field;
pub use mesh::{render, MeshResult, Summary};
pub use render::{render_and_save, render_mesh};
pub use validate::MeshStats;
