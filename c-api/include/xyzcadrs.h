/*
 * xyzcadrs C API — functional/implicit CAD renderer.
 *
 * Render a material field f(x, y, z) to watertight STL / colored OBJ.
 * A port of xyzcad (github.com/TheTesla/xyzcad) implemented in Rust.
 *
 * The field is a C callback:
 *
 *     uint8_t field(double x, double y, double z, void *user_data);
 *
 * returning the material id (0 = air, 1..=255 = material). The callback may
 * be invoked concurrently from multiple threads and must be thread-safe.
 *
 * All functions return 0 on success and 1 on error, with a message written
 * to the caller-provided err buffer (NUL-terminated, truncated if needed).
 */
#ifndef XYZCADRS_H
#define XYZCADRS_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef uint8_t (*xyzcadrs_field_fn)(double x, double y, double z, void *user_data);

typedef struct {
    int64_t cubes;             /* raster cubes visited by the flood fill */
    int64_t materials;         /* non-air material count (original convention) */
    int64_t cube_points;       /* distinct raster corner points */
    int64_t cube_edges;        /* unique cube edges */
    int64_t surface_points;    /* edges cut by the surface = mesh vertices */
    int64_t filtered_cubes;    /* cubes considered per material, summed */
    int64_t polygons;          /* polygons emitted (before triangulation) */
    int64_t repaired_polygons; /* boundary-closing repair polygons */
} xyzcadrs_summary;

typedef struct {
    int64_t triangles;
    int64_t vertices;             /* after welding */
    int64_t degenerate_triangles;
    int watertight;               /* 1 = watertight, 0 = not */
    double volume;                /* mm^3 */
    double bbox_min[3];
    double bbox_max[3];
} xyzcadrs_mesh_stats;

/* Render the field to a binary STL file. */
int xyzcadrs_render_stl(xyzcadrs_field_fn field, void *user_data,
                        double res, double min_val, double max_val,
                        uint64_t eval_limit,
                        const char *filename,
                        xyzcadrs_summary *summary_out,
                        char *err, size_t err_len);

/* Render the field to a colored OBJ + MTL pair (per-material groups).
 * filename_base is the path WITHOUT extension; writes <base>.obj/.mtl. */
int xyzcadrs_render_obj(xyzcadrs_field_fn field, void *user_data,
                        double res, double min_val, double max_val,
                        uint64_t eval_limit,
                        const char *filename_base,
                        xyzcadrs_summary *summary_out,
                        char *err, size_t err_len);

/* Validate an STL file: watertightness, volume, bounding box. */
int xyzcadrs_validate(const char *path, int weld_decimals,
                      xyzcadrs_mesh_stats *stats_out,
                      char *err, size_t err_len);

/* Renderer version string (static storage). */
const char *xyzcadrs_version(void);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* XYZCADRS_H */
