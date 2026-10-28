/* Example: render a sphere through the xyzcadrs C API and validate the STL.
 *
 * Build (from the repository root, after `cargo build --release -p xyzcadrs-capi`):
 *   cc -O2 -I c-api/include examples/render_sphere.c \
 *      -L c-api/target/release -lxyzcadrs_c -Wl,-rpath,$PWD/c-api/target/release \
 *      -o /tmp/xyzcadrs_c_example
 *   /tmp/xyzcadrs_c_example /tmp/xyzcadrs_c_sphere.stl
 */
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "xyzcadrs.h"

static uint8_t sphere(double x, double y, double z, void *user_data) {
    (void)user_data;
    return 100.0 > x * x + y * y + z * z ? 1 : 0;
}

int main(int argc, char **argv) {
    const char *out = argc > 1 ? argv[1] : "/tmp/xyzcadrs_c_sphere.stl";
    char err[512] = {0};
    xyzcadrs_summary summary = {0};

    printf("xyzcadrs C example, renderer version %s\n", xyzcadrs_version());

    int rc = xyzcadrs_render_stl(sphere, NULL, 0.3, -1000.0, 1000.0, 0,
                                 out, &summary, err, sizeof(err));
    if (rc != 0) {
        fprintf(stderr, "render failed: %s\n", err);
        return 1;
    }
    printf("rendered %s: cubes=%" PRId64 " surface_points=%" PRId64
           " polygons=%" PRId64 " repaired=%" PRId64 "\n",
           out, summary.cubes, summary.surface_points, summary.polygons,
           summary.repaired_polygons);

    xyzcadrs_mesh_stats stats = {0};
    rc = xyzcadrs_validate(out, 5, &stats, err, sizeof(err));
    if (rc != 0) {
        fprintf(stderr, "validate failed: %s\n", err);
        return 1;
    }
    printf("validate: triangles=%" PRId64 " watertight=%d volume=%.3f mm3\n",
           stats.triangles, stats.watertight, stats.volume);

    /* Sanity: the reference counters of the Python original at res 0.3. */
    if (summary.cubes != 20912 || summary.repaired_polygons != 0) {
        fprintf(stderr, "unexpected summary counters\n");
        return 1;
    }
    if (!stats.watertight) {
        fprintf(stderr, "mesh is not watertight\n");
        return 1;
    }
    printf("OK\n");
    return 0;
}
