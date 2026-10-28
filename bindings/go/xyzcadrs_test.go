package xyzcadrs

import (
	"math"
	"os"
	"path/filepath"
	"testing"
)

func sphereField(x, y, z float64) uint8 {
	if 100.0 > x*x+y*y+z*z {
		return 1
	}
	return 0
}

// Pinned against the Python original (sphere r=10, res 0.3):
// cubes=20912 surface_points=20910 polygons=20912 repaired=0.
func TestRenderSTLSphereParity(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "sphere.stl")
	s, err := RenderSTL(sphereField, 0.3, path)
	if err != nil {
		t.Fatalf("render failed: %v", err)
	}
	if s.Cubes != 20912 || s.SurfacePoints != 20910 || s.Polygons != 20912 || s.RepairedPolygons != 0 {
		t.Fatalf("unexpected counters: %+v", s)
	}
	info, err := os.Stat(path)
	if err != nil {
		t.Fatalf("STL not written: %v", err)
	}
	if info.Size() != 84+50*41816 {
		t.Fatalf("unexpected STL size %d", info.Size())
	}
}

func TestRenderOBJHexNutWatertightVolume(t *testing.T) {
	const c = 0.8660254037844386
	hexNut := func(x, y, z float64) uint8 {
		if math.Abs(z) > 4.0 {
			return 0
		}
		if math.Abs(c*x+0.5*y) > 8.0 || math.Abs(y) > 8.0 || math.Abs(c*x-0.5*y) > 8.0 {
			return 0
		}
		if math.Sqrt(x*x+y*y) < 4.0 {
			return 0
		}
		return 1
	}

	dir := t.TempDir()
	base := filepath.Join(dir, "nut")
	s, err := RenderOBJ(hexNut, 0.2, base)
	if err != nil {
		t.Fatalf("render failed: %v", err)
	}
	// Pinned against the Python original (hex nut, res 0.2).
	if s.Cubes != 28724 || s.RepairedPolygons != 0 {
		t.Fatalf("unexpected counters: %+v", s)
	}
	if _, err := os.Stat(base + ".obj"); err != nil {
		t.Fatalf("OBJ not written: %v", err)
	}
	if _, err := os.Stat(base + ".mtl"); err != nil {
		t.Fatalf("MTL not written: %v", err)
	}
}

func TestErrorPropagation(t *testing.T) {
	empty := func(x, y, z float64) uint8 { return 0 }
	_, err := RenderSTLWithLimits(empty, 1.0, -1000.0, 1000.0, 10_000_000,
		filepath.Join(t.TempDir(), "empty.stl"))
	if err == nil {
		t.Fatal("expected an error for a field without material")
	}
}
