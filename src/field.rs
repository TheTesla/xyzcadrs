//! Material field definition.

/// A material field function `f([x, y, z]) -> u8`.
///
/// The return value encodes the material at the probed point:
///
/// * `0` — air (no material),
/// * `1..=255` — material id. Points with the same non-zero id belong to the
///   same material; the isosurface between two different values is meshed.
///
/// Plain boolean fields simply return `0` or `1`. Any closure, function item
/// or callable mapping `[f64; 3]` to `u8` implements this trait
/// automatically:
///
/// ```
/// use xyzcadrs::Field;
///
/// let ball = |p: [f64; 3]| if 100.0 > p[0] * p[0] + p[1] * p[1] + p[2] * p[2] { 1 } else { 0 };
/// assert_eq!(ball([0.0, 0.0, 0.0]), 1);
/// assert_eq!(ball([20.0, 0.0, 0.0]), 0);
/// ```
pub trait Field: Fn([f64; 3]) -> u8 {}
impl<T: Fn([f64; 3]) -> u8> Field for T {}
