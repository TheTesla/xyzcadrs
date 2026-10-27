//! Renders a hex nut: hexagonal prism (16 mm across flats, 8 mm thick) with
//! an 8 mm bore — boolean composition written directly into the field
//! function.
//!
//! Run: `cargo run --release --example hex_nut`

fn main() {
    let field = xyzcadrs::examples::hex_nut();
    xyzcadrs::render_and_save(&field, "hex_nut.stl", 0.2).expect("render failed");
}
