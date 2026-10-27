//! Renders a rounded box (24 x 16 x 12 mm, 3 mm edge rounding).
//!
//! Run: `cargo run --release --example rounded_box`

fn main() {
    let field = xyzcadrs::examples::rounded_box();
    xyzcadrs::render_and_save(&field, "rounded_box.stl", 0.3).expect("render failed");
}
