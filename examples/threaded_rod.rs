//! Renders a threaded rod (M12-like sine thread, pitch 2.5 mm).
//!
//! Demonstrates that fine helical geometry survives the raster + refinement
//! pipeline watertight.
//!
//! Run: `cargo run --release --example threaded_rod`

fn main() {
    let field = xyzcadrs::examples::threaded_rod();
    xyzcadrs::render_and_save(&field, "threaded_rod.stl", 0.15).expect("render failed");
}
