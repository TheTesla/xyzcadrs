//! End-to-end parity and quality tests: render fields, export STL, re-read
//! and validate the mesh.

use std::f64::consts::PI;

use xyzcadrs::mesh::render;
use xyzcadrs::validate::{read_stl, stats};

fn sphere(p: [f64; 3]) -> u8 {
    if 100.0 > p[0] * p[0] + p[1] * p[1] + p[2] * p[2] {
        1
    } else {
        0
    }
}

/// Two nested material regions: inner sphere r = 8 (material 1), shell
/// 8 <= r < 12 (material 2). Only ONE connected surface exists between
/// materials 1 and 2, and the flood fill renders exactly that one — the
/// documented single-surface limitation of the original renderer.
fn two_mat(p: [f64; 3]) -> u8 {
    let r2 = p[0] * p[0] + p[1] * p[1] + p[2] * p[2];
    if r2 < 64.0 {
        1
    } else if r2 < 144.0 {
        2
    } else {
        0
    }
}

/// Pinned against `mesh_surface_function` of the Python original
/// (github.com/TheTesla/xyzcad, numba 0.61, numpy 2.2, 2026-10).
#[test]
fn parity_with_python_original() {
    type Case = (fn([f64; 3]) -> u8, f64, [usize; 7]);
    let cases: [Case; 5] = [
        // field, res, [cubes, materials, cube_points, cube_edges, surface_points, polygons, repaired]
        (sphere, 2.0, [452, 1, 907, 2253, 450, 452, 0]),
        (sphere, 1.0, [1874, 1, 3754, 9372, 1872, 1874, 0]),
        (sphere, 0.5, [7538, 1, 15088, 37704, 7536, 7538, 0]),
        (two_mat, 1.0, [1208, 1, 2416, 6030, 1206, 1208, 0]),
        (two_mat, 0.5, [4832, 1, 9676, 24174, 4830, 4832, 0]),
    ];
    for (i, (field, res, expected)) in cases.into_iter().enumerate() {
        let r = render(&field, res, -1000.0, 1000.0, 0).unwrap();
        let s = r.summary;
        let got = [
            s.cubes,
            s.materials,
            s.cube_points,
            s.cube_edges,
            s.surface_points,
            s.polygons,
            s.repaired_polygons,
        ];
        assert_eq!(got, expected, "parity case {i} (res {res})");
    }
}

#[test]
fn every_cut_edge_is_used_by_exactly_four_cube_polygons() {
    // Signature of a correct single-material surface: each cut edge has 4
    // adjacent cubes (verified against the Python original with the same
    // probe).
    let r = render(&sphere, 1.0, -1000.0, 1000.0, 0).unwrap();
    let mut use_count: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    for poly in &r.polygons {
        for &v in poly {
            *use_count.entry(v).or_default() += 1;
        }
    }
    assert_eq!(use_count.len(), r.summary.surface_points);
    assert!(
        use_count.values().all(|&n| n == 4),
        "cut edges not used exactly 4 times: {:?}",
        use_count
            .values()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
    );
}

#[test]
fn sphere_stl_is_watertight_with_correct_volume_and_bbox() {
    let res = 0.5;
    let r = render(&sphere, res, -1000.0, 1000.0, 0).unwrap();
    let dir = std::env::temp_dir().join("xyzcadrs_it_sphere.stl");
    let tris_written = xyzcadrs::export::write_stl(&dir, &r.vertices, &r.polygons).unwrap();
    // Fan triangulation: each polygon with n vertices contributes n - 2.
    let expected: usize = r.polygons.iter().map(|p| p.len() - 2).sum();
    assert_eq!(tris_written, expected);

    let tris = read_stl(&dir).unwrap();
    assert_eq!(tris.len(), tris_written);
    let s = stats(&tris, 5);
    assert_eq!(s.degenerate_triangles, 0, "degenerate facets");
    assert!(s.watertight, "mesh is not watertight");
    let sphere_volume = 4.0 / 3.0 * PI * 1000.0;
    let rel = (s.volume - sphere_volume).abs() / sphere_volume;
    assert!(
        rel < 0.01,
        "volume off by {}% ({:.2} vs {:.2})",
        rel * 100.0,
        s.volume,
        sphere_volume
    );
    for axis in 0..3 {
        assert!(
            s.bbox[1][axis] > 9.9 && s.bbox[1][axis] < 10.001,
            "bbox max[{axis}] = {}",
            s.bbox[1][axis]
        );
        assert!(
            s.bbox[0][axis] < -9.9 && s.bbox[0][axis] > -10.001,
            "bbox min[{axis}] = {}",
            s.bbox[0][axis]
        );
    }
    std::fs::remove_file(&dir).ok();
}

#[test]
fn two_material_render_has_two_groups_and_inner_surface_only() {
    let r = render(&two_mat, 1.0, -1000.0, 1000.0, 0).unwrap();
    assert_eq!(r.groups.len(), 1); // only the 1|2 interface is walked
    assert_eq!(r.groups[0].0, 2); // ...and it belongs to material 2
                                  // OBJ export carries the group and its material id.
    let base = std::env::temp_dir().join("xyzcadrs_it_two_mat");
    let tris = xyzcadrs::export::write_obj(&base, &r.vertices, &r.groups).unwrap();
    assert!(tris > 0);
    let obj = std::fs::read_to_string(base.with_extension("obj")).unwrap();
    assert!(obj.contains("usemtl mtl2"));
    let mtl = std::fs::read_to_string(base.with_extension("mtl")).unwrap();
    // Only material 2 receives the interface sheet (Python-verified: mats=[2]).
    assert!(mtl.contains("newmtl mtl2"));
    assert!(!mtl.contains("newmtl mtl1"));
    assert!(
        mtl.contains("Kd "),
        "MTL must carry the standard Kd statement"
    );
    assert!(
        !mtl.contains("Kb "),
        "the Python original's Kb quirk must not appear"
    );
    for ext in ["obj", "mtl"] {
        std::fs::remove_file(base.with_extension(ext)).ok();
    }
}

#[test]
fn split_box_renders_both_materials_each_shell_watertight() {
    let field = xyzcadrs::examples::split_box();
    let r = render(&field, 2.0, -1000.0, 1000.0, 0).unwrap();
    assert_eq!(r.summary.materials, 2);
    assert_eq!(r.groups.len(), 2);
    assert_eq!(r.summary.repaired_polygons, 0);
    // Each material's shell is a closed box half (5 outer faces + the shared
    // interface sheet). Validate them separately — the merged mesh carries
    // the interface sheet twice (once per material, opposite orientation),
    // exactly like the Python original's merged STL.
    for (m, polys) in &r.groups {
        let dir = std::env::temp_dir().join(format!("xyzcadrs_it_split_m{m}.stl"));
        xyzcadrs::export::write_stl(&dir, &r.vertices, polys).unwrap();
        let tris = read_stl(&dir).unwrap();
        let s = stats(&tris, 5);
        assert!(s.watertight, "material {m} shell not watertight");
        assert_eq!(s.degenerate_triangles, 0);
        assert!((s.bbox[1][1] - 10.0).abs() < 2.1);
        assert!((s.bbox[1][2] - 10.0).abs() < 2.1);
        std::fs::remove_file(&dir).ok();
    }
    // Material 1 spans the negative x half, material 2 the positive one;
    // both meet at the interface sheet (x = 0).
    let x_ranges: Vec<(u8, f64, f64)> = r
        .groups
        .iter()
        .map(|(m, polys)| {
            let mut lo = f64::INFINITY;
            let mut hi = f64::NEG_INFINITY;
            for poly in polys {
                for &v in poly {
                    lo = lo.min(r.vertices[v as usize][0]);
                    hi = hi.max(r.vertices[v as usize][0]);
                }
            }
            (*m, lo, hi)
        })
        .collect();
    for (m, lo, hi) in &x_ranges {
        if *m == 1 {
            assert!(*lo > -15.5 && *lo < -14.5, "mat1 lo {lo}");
            assert!(*hi > -0.5 && *hi < 0.5, "mat1 hi {hi}");
        } else {
            assert!(*lo > -0.5 && *lo < 0.5, "mat2 lo {lo}");
            assert!(*hi > 14.5 && *hi < 15.5, "mat2 hi {hi}");
        }
    }
}

#[test]
fn threaded_rod_is_watertight() {
    let field = xyzcadrs::examples::threaded_rod();
    let r = render(&field, 0.2, -1000.0, 1000.0, 0).unwrap();
    assert_eq!(r.summary.repaired_polygons, 0);
    let dir = std::env::temp_dir().join("xyzcadrs_it_thread.stl");
    xyzcadrs::export::write_stl(&dir, &r.vertices, &r.polygons).unwrap();
    let tris = read_stl(&dir).unwrap();
    let s = stats(&tris, 5);
    assert!(s.watertight, "threaded rod mesh not watertight");
    assert_eq!(s.degenerate_triangles, 0);
    std::fs::remove_file(&dir).ok();
}

#[test]
fn hex_nut_is_watertight_and_matches_python() {
    let field = xyzcadrs::examples::hex_nut();
    let r = render(&field, 0.2, -1000.0, 1000.0, 0).unwrap();
    // Pinned against the Python original (mesh_surface_function, res 0.2).
    assert_eq!(r.summary.cubes, 28724);
    assert_eq!(r.summary.cube_points, 57448);
    assert_eq!(r.summary.cube_edges, 143620);
    assert_eq!(r.summary.surface_points, 28724);
    assert_eq!(r.summary.polygons, 28724);
    assert_eq!(r.summary.repaired_polygons, 0);
    let dir = std::env::temp_dir().join("xyzcadrs_it_hexnut.stl");
    xyzcadrs::export::write_stl(&dir, &r.vertices, &r.polygons).unwrap();
    let tris = read_stl(&dir).unwrap();
    let s = stats(&tris, 5);
    assert!(s.watertight);
    assert_eq!(s.degenerate_triangles, 0);
    // Analytic volume: hexagon (across flats 16) * 8 - bore (r=4, h=8).
    let hex_area = 2.0 * 3.0f64.sqrt() * 64.0; // = perimeter * apothem / 2
    let nut = hex_area * 8.0 - PI * 16.0 * 8.0;
    let rel = (s.volume - nut).abs() / nut;
    assert!(
        rel < 0.02,
        "nut volume off by {}% ({:.2} vs {:.2})",
        rel * 100.0,
        s.volume,
        nut
    );
    std::fs::remove_file(&dir).ok();
}
