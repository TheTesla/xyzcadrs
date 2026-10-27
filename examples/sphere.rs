//! Renders the solid sphere (radius 10 mm) — the canonical xyzcad example.
//!
//! Run: `cargo run --release --example sphere`

fn main() {
    let field = xyzcadrs::examples::sphere();
    let summary = xyzcadrs::render_and_save(&field, "sphere.stl", 0.3).expect("render failed");
    assert!(summary.repaired_polygons == 0);
}
