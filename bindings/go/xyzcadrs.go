// Package xyzcadrs renders material fields f(x, y, z) to watertight STL /
// colored OBJ files using the xyzcadrs Rust renderer through its C API.
//
// A field is a Go function:
//
//	func(x, y, z float64) uint8
//
// returning the material id (0 = air, 1..=255 = material). It is invoked
// concurrently from multiple render threads and must be thread-safe (no
// shared mutable state).
//
// Build requirement: the C shared library must be built first:
//
//	cd ../../c-api && cargo build --release
//
// The cgo directives below link against ../../c-api/target/release with an
// rpath embedded at build time.
package xyzcadrs

/*
#cgo CFLAGS: -I${SRCDIR}/../../c-api/include
#cgo LDFLAGS: -L${SRCDIR}/../../c-api/target/release -lxyzcadrs_c -Wl,-rpath,${SRCDIR}/../../c-api/target/release
#include <stdlib.h>
#include "xyzcadrs.h"
extern uint8_t xyzcadrs_go_trampoline(double x, double y, double z, void* user_data);
*/
import "C"

import (
	"fmt"
	"sync"
	"unsafe"
)

// Field computes the material id (0 = air, 1..=255 = material) at (x, y, z).
type Field func(x, y, z float64) uint8

// Summary holds the render counters of a completed render.
type Summary struct {
	Cubes            int64
	Materials        int64
	CubePoints       int64
	CubeEdges        int64
	SurfacePoints    int64
	FilteredCubes    int64
	Polygons         int64
	RepairedPolygons int64
}

var (
	regMu    sync.Mutex
	nextID   uintptr = 1
	fieldReg         = make(map[uintptr]Field)
)

func register(f Field) uintptr {
	regMu.Lock()
	defer regMu.Unlock()
	id := nextID
	nextID++
	fieldReg[id] = f
	return id
}

func unregister(id uintptr) {
	regMu.Lock()
	defer regMu.Unlock()
	delete(fieldReg, id)
}

//export xyzcadrs_go_trampoline
func xyzcadrs_go_trampoline(x, y, z C.double, userData unsafe.Pointer) C.uint8_t {
	regMu.Lock()
	f := fieldReg[*(*uintptr)(userData)]
	regMu.Unlock()
	if f == nil {
		return 0
	}
	return C.uint8_t(f(float64(x), float64(y), float64(z)))
}

func cString(s string) (*C.char, func()) {
	cs := C.CString(s)
	return cs, func() { C.free(unsafe.Pointer(cs)) }
}

// RenderSTL renders the field at resolution res (mm) and writes a binary STL
// to path. Returns the render summary.
//
// The seed search runs without an evaluation budget; for fields without any
// material use RenderSTLWithLimits to bound the search.
func RenderSTL(f Field, res float64, path string) (Summary, error) {
	return RenderSTLWithLimits(f, res, -1000.0, 1000.0, 0, path)
}

// RenderSTLWithLimits is RenderSTL with explicit seed-search bounds:
// the search cube [minVal, maxVal]³ and an evaluation budget for the seed
// search (0 = unlimited). The budget guards against long scans on fields
// without material.
func RenderSTLWithLimits(f Field, res, minVal, maxVal float64, evalLimit uint64, path string) (Summary, error) {
	id := register(f)
	defer unregister(id)
	handle := id // passed to C by pointer; alive for the call's duration

	cs, free := cString(path)
	defer free()

	errBuf := make([]C.char, 512)
	var summary C.xyzcadrs_summary
	rc := C.xyzcadrs_render_stl(
		C.xyzcadrs_field_fn(C.xyzcadrs_go_trampoline),
		unsafe.Pointer(&handle),
		C.double(res), C.double(minVal), C.double(maxVal), C.uint64_t(evalLimit),
		cs, &summary, &errBuf[0], C.size_t(len(errBuf)),
	)
	if rc != 0 {
		return Summary{}, fmt.Errorf("xyzcadrs: %s", C.GoString(&errBuf[0]))
	}
	return Summary{
		Cubes:            int64(summary.cubes),
		Materials:        int64(summary.materials),
		CubePoints:       int64(summary.cube_points),
		CubeEdges:        int64(summary.cube_edges),
		SurfacePoints:    int64(summary.surface_points),
		FilteredCubes:    int64(summary.filtered_cubes),
		Polygons:         int64(summary.polygons),
		RepairedPolygons: int64(summary.repaired_polygons),
	}, nil
}

// RenderOBJ renders the field at resolution res (mm) and writes a colored
// OBJ + MTL pair with the given base path (without extension).
func RenderOBJ(f Field, res float64, pathBase string) (Summary, error) {
	id := register(f)
	defer unregister(id)
	handle := id // passed to C by pointer; alive for the call's duration

	cs, free := cString(pathBase)
	defer free()

	errBuf := make([]C.char, 512)
	var summary C.xyzcadrs_summary
	rc := C.xyzcadrs_render_obj(
		C.xyzcadrs_field_fn(C.xyzcadrs_go_trampoline),
		unsafe.Pointer(&handle),
		C.double(res), C.double(-1000.0), C.double(1000.0), C.uint64_t(0),
		cs, &summary, &errBuf[0], C.size_t(len(errBuf)),
	)
	if rc != 0 {
		return Summary{}, fmt.Errorf("xyzcadrs: %s", C.GoString(&errBuf[0]))
	}
	return Summary{
		Cubes:            int64(summary.cubes),
		Materials:        int64(summary.materials),
		CubePoints:       int64(summary.cube_points),
		CubeEdges:        int64(summary.cube_edges),
		SurfacePoints:    int64(summary.surface_points),
		FilteredCubes:    int64(summary.filtered_cubes),
		Polygons:         int64(summary.polygons),
		RepairedPolygons: int64(summary.repaired_polygons),
	}, nil
}

// Version returns the renderer version string.
func Version() string {
	return C.GoString(C.xyzcadrs_version())
}
