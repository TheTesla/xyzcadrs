//! Grid arithmetic shared by all pipeline stages.

/// Quantize a coordinate exactly like the Python original: round half up to
/// 4 decimal places (`floor(10000*x + 0.5) / 10000`).
///
/// All cube and point coordinates in the pipeline are passed through this
/// function, which makes the raster lattice reproducible and lets identical
/// coordinates be deduplicated by exact `f64` equality.
#[inline]
pub fn rnd(x: f64) -> f64 {
    (10000.0 * x + 0.5).floor() / 10000.0
}

#[cfg(test)]
mod tests {
    use super::rnd;

    #[test]
    fn quantizes_to_four_decimals() {
        // Values pinned against numpy: floor(10000*x + 0.5)/10000.
        assert_eq!(rnd(1.95), 1.95);
        assert_eq!(rnd(0.15), 0.15);
        assert_eq!(rnd(0.8999999999), 0.9);
        assert_eq!(rnd(-0.8999999999), -0.9);
        assert_eq!(rnd(2.0 + 0.3 - 0.3 + 0.3), 2.3);
    }

    #[test]
    fn rounds_half_up_like_numpy_floor() {
        // 10000 * 0.00005 + 0.5 = 1.0 -> floor = 1 -> 0.0001
        assert_eq!(rnd(0.00005), 0.0001);
        // 10000 * (-0.00005) + 0.5 = 0.0 -> floor = 0 -> 0.0
        assert_eq!(rnd(-0.00005), 0.0);
    }
}
