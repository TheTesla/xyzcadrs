//! Renders a two-material split box as a colored OBJ + MTL pair.
//!
//! Material 1 (x < 0) and material 2 (x >= 0) each become one `usemtl`
//! group; colors come from the MTL file.
//!
//! Run: `cargo run --release --example split_box`

fn main() {
    let field = xyzcadrs::examples::split_box();
    xyzcadrs::render_and_save(&field, "split_box.obj", 0.4).expect("render failed");
}
