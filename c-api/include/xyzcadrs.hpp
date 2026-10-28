// xyzcadrs C++ bindings — thin RAII wrapper over the C ABI (xyzcadrs.h).
//
// The field is a std::function; each render call copies it into
// callback-owned storage, so concurrent renders with different fields are
// safe. Errors are reported as std::runtime_error.
//
// Example:
//
//     xyzcadrs::Summary s = xyzcadrs::render_stl(
//         [](double x, double y, double z) -> uint8_t {
//             return 100.0 > x*x + y*y + z*z ? 1 : 0;
//         },
//         0.3, "sphere.stl");
//     std::cout << s.polygons << " polygons\n";
#ifndef XYZCADRS_HPP
#define XYZCADRS_HPP

#include "xyzcadrs.h"

#include <cstdint>
#include <functional>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

namespace xyzcadrs {

using FieldFn = std::function<std::uint8_t(double, double, double)>;

struct Summary {
    std::int64_t cubes;
    std::int64_t materials;
    std::int64_t cube_points;
    std::int64_t cube_edges;
    std::int64_t surface_points;
    std::int64_t filtered_cubes;
    std::int64_t polygons;
    std::int64_t repaired_polygons;
};

struct MeshStats {
    std::int64_t triangles;
    std::int64_t vertices;
    std::int64_t degenerate_triangles;
    bool watertight;
    double volume;
    std::vector<double> bbox_min;  // 3 entries
    std::vector<double> bbox_max;  // 3 entries
};

namespace detail {

inline std::string last_error(const char *err_buf) {
    return err_buf != nullptr ? std::string(err_buf) : std::string("unknown error");
}

inline std::unique_ptr<FieldFn> leak_field(const FieldFn &field) {
    return std::make_unique<FieldFn>(field);
}

extern "C" inline std::uint8_t trampoline(double x, double y, double z, void *user_data) {
    auto *f = static_cast<const FieldFn *>(user_data);
    return (*f)(x, y, z);
}

inline void check(int rc, const char *err_buf) {
    if (rc != 0) {
        throw std::runtime_error("xyzcadrs: " + last_error(err_buf));
    }
}

}  // namespace detail

/// Render `field` to a binary STL at `filename` (resolution `res` mm).
inline Summary render_stl(const FieldFn &field, double res, const std::string &filename,
                          double min_val = -1000.0, double max_val = 1000.0,
                          std::uint64_t eval_limit = 0) {
    auto owned = detail::leak_field(field);
    xyzcadrs_summary s{};
    std::vector<char> err(512);
    detail::check(
        xyzcadrs_render_stl(&detail::trampoline, owned.get(), res, min_val, max_val,
                            eval_limit, filename.c_str(), &s, err.data(), err.size()),
        err.data());
    return Summary{s.cubes, s.materials, s.cube_points, s.cube_edges,
                   s.surface_points, s.filtered_cubes, s.polygons, s.repaired_polygons};
}

/// Render `field` to a colored OBJ + MTL pair with base name `filename_base`.
inline Summary render_obj(const FieldFn &field, double res, const std::string &filename_base,
                          double min_val = -1000.0, double max_val = 1000.0,
                          std::uint64_t eval_limit = 0) {
    auto owned = detail::leak_field(field);
    xyzcadrs_summary s{};
    std::vector<char> err(512);
    detail::check(
        xyzcadrs_render_obj(&detail::trampoline, owned.get(), res, min_val, max_val,
                            eval_limit, filename_base.c_str(), &s, err.data(), err.size()),
        err.data());
    return Summary{s.cubes, s.materials, s.cube_points, s.cube_edges,
                   s.surface_points, s.filtered_cubes, s.polygons, s.repaired_polygons};
}

/// Validate an STL file.
inline MeshStats validate(const std::string &path, int weld_decimals = 5) {
    xyzcadrs_mesh_stats s{};
    std::vector<char> err(512);
    detail::check(
        xyzcadrs_validate(path.c_str(), weld_decimals, &s, err.data(), err.size()),
        err.data());
    MeshStats out;
    out.triangles = s.triangles;
    out.vertices = s.vertices;
    out.degenerate_triangles = s.degenerate_triangles;
    out.watertight = s.watertight != 0;
    out.volume = s.volume;
    out.bbox_min = {s.bbox_min[0], s.bbox_min[1], s.bbox_min[2]};
    out.bbox_max = {s.bbox_max[0], s.bbox_max[1], s.bbox_max[2]};
    return out;
}

/// Renderer version string.
inline std::string version() {
    return std::string(xyzcadrs_version());
}

}  // namespace xyzcadrs

#endif /* XYZCADRS_HPP */
