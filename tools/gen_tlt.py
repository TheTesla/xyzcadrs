#!/usr/bin/env python3
"""Generate the Rust TLT table from the Python original's xyzcad.tlt.

Usage: python3 tools/gen_tlt.py [path-to-xyzcad-repo]
(default: ../xyzcad relative to this repository)
"""
import os
import sys

default_repo = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "xyzcad")
sys.path.insert(0, os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else default_repo))

try:
    from xyzcad.tlt import TLT as py_tlt
except ImportError as exc:  # pragma: no cover
    raise SystemExit(
        "Could not import xyzcad. Pass the path to a checkout of the Python "
        "original: python3 tools/gen_tlt.py /path/to/xyzcad"
    ) from exc

table = py_tlt()

lines = []
lines.append("// GENERATED FILE — derived mechanically from the Python original's")
lines.append("// `xyzcad/tlt.py` (github.com/TheTesla/xyzcad). Do not edit by hand;")
lines.append("// regenerate with `tools/gen_tlt.py` instead.")
lines.append("")
lines.append("//! Triangle lookup table for the marching cubes polygonisation.")
lines.append("//!")
lines.append("//! Index = 8-bit corner mask, bit `k` set means cube corner `k` carries the")
lines.append("//! current material (corner bits: `k & 4` -> +x, `k & 2` -> +y, `k & 1` -> +z).")
lines.append("//! Each entry lists, for the given mask, the closed polygon(s) of the")
lines.append("//! isosurface inside the cube. Polygon elements are edge indices 0..12 in")
lines.append("//! [`crate::mesh::CUBE_EDGES`] order. Masks 0 (all air) and 255 (all")
lines.append("//! material) map to the sentinel polygon `[0]`, which the meshing stage")
lines.append("//! filters out because none of a uniform cube's edges are cut.")
lines.append("")
lines.append("pub static TLT: [&[&[u8]]; 256] = [")
for entry in table:
    polys = ", ".join("&[" + ", ".join(str(e) for e in poly) + "]" for poly in entry)
    lines.append(f"    &[{polys}],")
lines.append("];")
lines.append("")
lines.append("#[cfg(test)]")
lines.append("mod tests {")
lines.append("    use super::TLT;")
lines.append("")
lines.append("    #[test]")
lines.append("    fn single_corner_masks_match_verified_convention() {")
lines.append("        // Empirically verified against the Python original on a rendered")
lines.append("        // sphere: single-corner masks must cut exactly the three edges")
lines.append("        // incident to that corner (CUBE_EDGES order).")
lines.append("        let expected: [&[&[u8]]; 8] = [")
lines.append("            &[&[0, 1, 2]],   // bit 0 -> corner 0")
lines.append("            &[&[2, 10, 11]], // bit 1 -> corner 4 (z edge)")
lines.append("            &[&[1, 8, 9]],   // bit 2 -> corner 2 (y edge)")
lines.append("            &[&[3, 10, 9]],  // bit 3 -> corner 6 (y+z)")
lines.append("            &[&[0, 6, 7]],   // bit 4 -> corner 1 (x edge)")
lines.append("            &[&[6, 11, 4]],  // bit 5 -> corner 5 (x+z)")
lines.append("            &[&[5, 8, 7]],   // bit 6 -> corner 3 (x+y)")
lines.append("            &[&[3, 5, 4]],   // bit 7 -> corner 7 (x+y+z)")
lines.append("        ];")
lines.append("        for (bit, exp) in expected.iter().enumerate() {")
lines.append("            assert_eq!(TLT[1 << bit], *exp, \"mask bit {}\", bit);")
lines.append("        }")
lines.append("    }")
lines.append("")
lines.append("    #[test]")
lines.append("    fn uniform_masks_are_sentinels() {")
lines.append("        assert_eq!(TLT[0], &[&[0]]);")
lines.append("        assert_eq!(TLT[255], &[&[0]]);")
lines.append("    }")
lines.append("")
lines.append("    #[test]")
lines.append("    fn all_edge_indices_in_range() {")
lines.append("        for (mask, polys) in TLT.iter().enumerate() {")
lines.append("            for poly in polys.iter() {")
lines.append("                assert!(!poly.is_empty(), \"mask {mask}: empty polygon\");")
lines.append("                for &e in poly.iter() {")
lines.append("                    assert!(e < 12, \"mask {mask}: edge index {e}\");")
lines.append("                }")
lines.append("            }")
lines.append("        }")
lines.append("    }")
lines.append("}")

out = "\n".join(lines) + "\n"
target = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "src", "tlt.rs")
with open(target, "w") as f:
    f.write(out)
print(f"wrote {os.path.normpath(target)} ({len(out)} bytes, {len(table)} masks)")
