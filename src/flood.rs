//! Flood fill over the isosurface: discover all raster cubes touched by the
//! surface, starting from a single seed point.

use std::collections::HashMap;

use crate::field::Field;
use crate::math::rnd;

/// Field values at the 8 corners of a raster cube.
///
/// Corner `i` sits at coordinate `(x + r if i & 4 != 0 else x,
/// y + r if i & 2 != 0 else y, z + r if i & 1 != 0 else z)` for a cube with
/// low corner `(x, y, z)` and edge length `r`. I.e. the corner index bits
/// map as `i & 4` -> +x, `i & 2` -> +y, `i & 1` -> +z.
pub type CornerValues = [u8; 8];

/// Quantized bit-key for a raster coordinate: `f64::to_bits` per component.
///
/// All pipeline coordinates are produced by [`rnd`], so equal coordinates
/// are bit-identical and this key is an exact deduplication key.
pub type GridKey = [u64; 3];

/// Bit-key of a coordinate triple.
#[inline]
pub fn grid_key(p: [f64; 3]) -> GridKey {
    [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]
}

/// Walk the entire isosurface and collect the material values at the corners
/// of every raster cube the surface passes through.
///
/// Port of `xyzcad.render.get_surface`. Starting from the cube below
/// `start_pnt`, neighbouring cubes are visited whenever the shared face
/// carries differing values; corner values of the face the walk entered
/// through are reused instead of re-evaluating the field (exactly like the
/// original). Cube coordinates are quantized with [`rnd`] at edge length
/// `res`, so identical cubes deduplicate by exact `f64` equality.
///
/// Returns one `(cube low corner, corner values)` pair per discovered cube
/// (unordered; sort for deterministic downstream output).
pub fn get_surface<F: Field>(
    f: &F,
    start_pnt: [f64; 3],
    res: f64,
) -> Vec<([f64; 3], CornerValues)> {
    let r = res;
    let start = [
        rnd(start_pnt[0] - r / 2.0),
        rnd(start_pnt[1] - r / 2.0),
        rnd(start_pnt[2] - r / 2.0),
    ];
    // Stack entries: cube low corner, entry direction, reusable face values.
    let mut stack: Vec<([f64; 3], i32, [u8; 4])> = vec![(start, 0, [0; 4])];
    let mut corners: HashMap<GridKey, CornerValues> = HashMap::new();

    while let Some(([x, y, z], d, vo)) = stack.pop() {
        let xh = rnd(x + r);
        let yh = rnd(y + r);
        let zh = rnd(z + r);

        // Evaluate the 8 corner values, reusing the values of the face the
        // walk entered through (Python: `(0 == (i & abs(d))) == (1 == sign(d))`).
        // d == 0 (seed cube) reuses nothing; the match arms below reproduce
        // the original condition for d in {+4, +2, +1, -4, -2, -1}.
        let mut v: CornerValues = [0; 8];
        let mut j = 0usize;
        // The loop index IS the corner bit pattern (x = 4, y = 2, z = 1);
        // enumerate() would obscure that mapping.
        #[allow(clippy::needless_range_loop)]
        for i in 0..8usize {
            let reuse = match d {
                4 => i & 4 == 0,
                2 => i & 2 == 0,
                1 => i & 1 == 0,
                -4 => i & 4 != 0,
                -2 => i & 2 != 0,
                -1 => i & 1 != 0,
                _ => false,
            };
            if reuse {
                v[i] = vo[j];
                j += 1;
            } else {
                v[i] = f([
                    if i & 4 != 0 { xh } else { x },
                    if i & 2 != 0 { yh } else { y },
                    if i & 1 != 0 { zh } else { z },
                ]);
            }
        }

        // Skip pushing across uniform faces; the direction we came from is
        // never revisited.
        if d != -4 && !(v[4] == v[6] && v[4] == v[5] && v[4] == v[7]) {
            let key = [xh, y, z];
            if !corners.contains_key(&grid_key(key)) {
                stack.push((key, 4, [v[4], v[5], v[6], v[7]]));
            }
        }
        if d != -2 && !(v[2] == v[6] && v[2] == v[3] && v[2] == v[7]) {
            let key = [x, yh, z];
            if !corners.contains_key(&grid_key(key)) {
                stack.push((key, 2, [v[2], v[3], v[6], v[7]]));
            }
        }
        if d != -1 && !(v[1] == v[5] && v[1] == v[3] && v[1] == v[7]) {
            let key = [x, y, zh];
            if !corners.contains_key(&grid_key(key)) {
                stack.push((key, 1, [v[1], v[3], v[5], v[7]]));
            }
        }
        if d != 4 && !(v[0] == v[2] && v[0] == v[1] && v[0] == v[3]) {
            let key = [rnd(x - r), y, z];
            if !corners.contains_key(&grid_key(key)) {
                stack.push((key, -4, [v[0], v[1], v[2], v[3]]));
            }
        }
        if d != 2 && !(v[0] == v[4] && v[0] == v[1] && v[0] == v[5]) {
            let key = [x, rnd(y - r), z];
            if !corners.contains_key(&grid_key(key)) {
                stack.push((key, -2, [v[0], v[1], v[4], v[5]]));
            }
        }
        if d != 1 && !(v[0] == v[4] && v[0] == v[2] && v[0] == v[6]) {
            let key = [x, y, rnd(z - r)];
            if !corners.contains_key(&grid_key(key)) {
                stack.push((key, -1, [v[0], v[2], v[4], v[6]]));
            }
        }

        corners.insert(grid_key([x, y, z]), v);
    }

    corners
        .into_iter()
        .map(|(k, v)| {
            (
                [
                    f64::from_bits(k[0]),
                    f64::from_bits(k[1]),
                    f64::from_bits(k[2]),
                ],
                v,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::get_surface;
    use crate::math::rnd;

    fn sphere(p: [f64; 3]) -> u8 {
        if 100.0 > p[0] * p[0] + p[1] * p[1] + p[2] * p[2] {
            1
        } else {
            0
        }
    }

    #[test]
    fn corner_values_equal_field_at_corner_coordinates() {
        // Reference (Python original, same seed/res): 480 cubes, every stored
        // corner value equals the field value at the corner's coordinates.
        let res = 2.0;
        let cubes = get_surface(&sphere, [9.9, 0.5, 0.5], res);
        assert_eq!(cubes.len(), 480);
        for (cube, v) in cubes.iter() {
            let [x, y, z] = *cube;
            let xh = rnd(x + res);
            let yh = rnd(y + res);
            let zh = rnd(z + res);
            // Index = corner bit pattern (x = 4, y = 2, z = 1).
            #[allow(clippy::needless_range_loop)]
            for i in 0..8 {
                let p = [
                    if i & 4 != 0 { xh } else { x },
                    if i & 2 != 0 { yh } else { y },
                    if i & 1 != 0 { zh } else { z },
                ];
                assert_eq!(v[i], sphere(p), "corner {i} of cube ({x},{y},{z})");
            }
        }
    }
}
