//! Renders a torus (ring radius 12 mm, tube radius 4 mm).
//!
//! Run: `cargo run --release --example torus`

fn main() {
    let field = xyzcadrs::examples::torus();
    xyzcadrs::render_and_save(&field, "torus.stl", 0.3).expect("render failed");
}
