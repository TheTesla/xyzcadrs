//! Ready-made example fields, used by the CLI, the `examples/` binaries and
//! the documentation images.

use crate::field::Field;
use std::f64::consts::PI;

/// Solid sphere, radius 10 mm, centered at the origin (the canonical example
/// of the Python original).
pub fn sphere() -> impl Field {
    |p: [f64; 3]| {
        if 100.0 > p[0] * p[0] + p[1] * p[1] + p[2] * p[2] {
            1
        } else {
            0
        }
    }
}

/// Rounded box 24 x 16 x 12 mm with 3 mm edge rounding, centered at the
/// origin (classic rounded-cuboid signed distance field).
pub fn rounded_box() -> impl Field {
    let half = [12.0, 8.0, 6.0];
    let r = 3.0;
    move |p: [f64; 3]| {
        let q = [
            (p[0].abs() - half[0] + r),
            (p[1].abs() - half[1] + r),
            (p[2].abs() - half[2] + r),
        ];
        let outside =
            (q[0].max(0.0).powi(2) + q[1].max(0.0).powi(2) + q[2].max(0.0).powi(2)).sqrt();
        let inside = q[0].max(q[1]).max(q[2]).min(0.0);
        if outside + inside - r < 0.0 {
            1
        } else {
            0
        }
    }
}

/// Torus: ring radius 12 mm, tube radius 4 mm, lying in the x-y plane.
pub fn torus() -> impl Field {
    |p: [f64; 3]| {
        let ring = 12.0;
        let tube = 4.0;
        let d = (ring - (p[0] * p[0] + p[1] * p[1]).sqrt()).powi(2) + p[2] * p[2];
        if d < tube * tube {
            1
        } else {
            0
        }
    }
}

/// Threaded rod: 20 mm long single-start sine thread around the z axis
/// (core radius 5.2 mm, crest 6.4 mm, pitch 2.5 mm), ported from the
/// fuzzyometry `fz_thread` recipe of the Python ecosystem. The phase
/// `z/pitch - theta/(2*pi)` makes the crest a true helix.
pub fn threaded_rod() -> impl Field {
    |p: [f64; 3]| {
        let z = p[2];
        if !(0.5..19.5).contains(&z) {
            return 0;
        }
        let radial = (p[0] * p[0] + p[1] * p[1]).sqrt();
        if radial < 5.2 {
            return 1; // core rod
        }
        let theta = p[1].atan2(p[0]);
        // Single-start sine thread: radius oscillates 5.2 .. 6.4 mm.
        let phase = 2.0 * PI * (z / 2.5 - theta / (2.0 * PI));
        if radial < 5.8 + 0.6 * phase.sin() {
            1
        } else {
            0
        }
    }
}

/// Split box 30 x 20 x 20 mm: material 1 for x < 0, material 2 for x >= 0.
/// Demonstrates multi-material rendering and colored OBJ export.
pub fn split_box() -> impl Field {
    let half = [15.0, 10.0, 10.0];
    move |p: [f64; 3]| {
        if p[0].abs() < half[0] && p[1].abs() < half[1] && p[2].abs() < half[2] {
            if p[0] < 0.0 {
                1
            } else {
                2
            }
        } else {
            0
        }
    }
}

/// A printed hex nut: hexagonal prism (16 mm across flats, 8 mm thick) with
/// an 8 mm bore — shows boolean-style field composition (intersection and
/// subtraction written directly into the field function).
pub fn hex_nut() -> impl Field {
    |p: [f64; 3]| {
        if p[2].abs() > 4.0 {
            return 0;
        }
        // Regular hexagon via three axis pairs at 60 degrees.
        let c = 0.866_025_403_784_438_6_f64; // cos(30°)
        let ax = (c * p[0] + 0.5 * p[1]).abs();
        let ay = p[1].abs();
        let az = (c * p[0] - 0.5 * p[1]).abs();
        if ax > 8.0 || ay > 8.0 || az > 8.0 {
            return 0;
        }
        let radial = (p[0] * p[0] + p[1] * p[1]).sqrt();
        if radial < 4.0 {
            return 0; // bore
        }
        1
    }
}

/// Registry entry: description, field constructor, suggested resolution.
pub type ExampleEntry = (&'static str, Box<dyn crate::field::Field + Sync>, f64);

/// Registry of the built-in example fields with their suggested resolution.
pub fn lookup(name: &str) -> Option<ExampleEntry> {
    let entry: ExampleEntry = match name {
        "sphere" => ("solid sphere, radius 10 mm", Box::new(sphere()), 0.3),
        "rounded-box" => (
            "rounded box 24 x 16 x 12 mm, r = 3 mm",
            Box::new(rounded_box()),
            0.3,
        ),
        "torus" => (
            "torus, ring radius 12 mm, tube radius 4 mm",
            Box::new(torus()),
            0.3,
        ),
        "threaded-rod" => (
            "threaded rod, sine thread M12-like, pitch 2.5 mm",
            Box::new(threaded_rod()),
            0.15,
        ),
        "split-box" => (
            "two-material split box 30 x 20 x 20 mm",
            Box::new(split_box()),
            0.4,
        ),
        "hex-nut" => (
            "hex nut, 16 mm across flats, 8 mm bore (boolean composition)",
            Box::new(hex_nut()),
            0.2,
        ),
        _ => return None,
    };
    Some(entry)
}

/// Names of all built-in examples.
pub const NAMES: [&str; 6] = [
    "sphere",
    "rounded-box",
    "torus",
    "threaded-rod",
    "split-box",
    "hex-nut",
];
