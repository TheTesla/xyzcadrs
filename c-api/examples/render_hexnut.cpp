// Example: render a hex nut through the xyzcadrs C++ bindings (std::function
// field with capture) and validate the STL.
//
// Build (from the repository root, after `cargo build --release -p xyzcadrs-capi`):
//   g++ -O2 -std=c++17 -I c-api/include examples/render_hexnut.cpp \
//       -L c-api/target/release -lxyzcadrs_c -Wl,-rpath,$PWD/c-api/target/release \
//       -o /tmp/xyzcadrs_cpp_example
//   /tmp/xyzcadrs_cpp_example /tmp/xyzcadrs_cpp_nut.stl
#include <cmath>
#include <cstdio>
#include <string>

#include "xyzcadrs.hpp"

int main(int argc, char **argv) {
    const std::string out = argc > 1 ? argv[1] : "/tmp/xyzcadrs_cpp_nut.stl";

    std::printf("xyzcadrs C++ example, renderer version %s\n", xyzcadrs::version().c_str());

    const double c = 0.8660254037844386;  // cos(30°)
    auto hex_nut = [c](double x, double y, double z) -> std::uint8_t {
        if (std::abs(z) > 4.0) return 0;
        if (std::abs(c * x + 0.5 * y) > 8.0) return 0;
        if (std::abs(y) > 8.0) return 0;
        if (std::abs(c * x - 0.5 * y) > 8.0) return 0;
        if (std::sqrt(x * x + y * y) < 4.0) return 0;
        return 1;
    };

    try {
        xyzcadrs::Summary s = xyzcadrs::render_stl(hex_nut, 0.2, out);
        std::printf("rendered %s: cubes=%lld surface_points=%lld polygons=%lld repaired=%lld\n",
                    out.c_str(), (long long)s.cubes, (long long)s.surface_points,
                    (long long)s.polygons, (long long)s.repaired_polygons);

        xyzcadrs::MeshStats stats = xyzcadrs::validate(out);
        std::printf("validate: triangles=%lld watertight=%d volume=%.3f mm3\n",
                    (long long)stats.triangles, stats.watertight ? 1 : 0, stats.volume);

        if (s.cubes != 28724 || s.repaired_polygons != 0) {
            std::fprintf(stderr, "unexpected summary counters\n");
            return 1;
        }
        if (!stats.watertight) {
            std::fprintf(stderr, "mesh is not watertight\n");
            return 1;
        }
    } catch (const std::exception &e) {
        std::fprintf(stderr, "failed: %s\n", e.what());
        return 1;
    }
    std::printf("OK\n");
    return 0;
}
