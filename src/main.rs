//! Command line interface of xyzcadrs.

use std::time::Instant;

use clap::{Parser, Subcommand};
use xyzcadrs::export::write_obj;
use xyzcadrs::mesh::render;
use xyzcadrs::render::report;
use xyzcadrs::validate::{read_stl, stats};

#[derive(Parser)]
#[command(
    name = "xyzcadrs",
    version,
    about = "Functional CAD: render f(x,y,z) material fields to 3D-printable STL/OBJ",
    long_about = "Rust port of xyzcad. Describe a part as a function from 3D coordinates to a \
                  material id (0 = air) and render it to a watertight STL or colored OBJ."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List the built-in example fields.
    List,
    /// Render a built-in example field to STL or OBJ.
    Render {
        /// Name of the built-in example (see `list`).
        example: String,
        /// Raster resolution in mm (smaller = finer = slower).
        #[arg(short, long)]
        res: Option<f64>,
        /// Output file: `*.stl` (binary STL) or `*.obj` (colored OBJ + MTL).
        #[arg(short, long, default_value = "output.stl")]
        out: String,
    },
    /// Validate an STL file: watertightness, volume, bounding box.
    Validate {
        /// Path to the STL file (binary or ASCII).
        path: String,
        /// Decimal places used for vertex welding before the edge checks.
        #[arg(long, default_value_t = 5)]
        weld_decimals: u32,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::List => {
            println!("built-in examples:");
            for name in xyzcadrs::examples::NAMES {
                let (desc, _, res) = xyzcadrs::examples::lookup(name).unwrap();
                println!("  {name:>14}  res {res:<5} {desc}");
            }
        }
        Command::Render { example, res, out } => {
            let Some((desc, field, default_res)) = xyzcadrs::examples::lookup(&example) else {
                eprintln!("unknown example '{example}' (see `xyzcadrs list`)");
                std::process::exit(2);
            };
            let res = res.unwrap_or(default_res);
            println!("rendering {example}: {desc}, res {res} mm");
            let t0 = Instant::now();
            match render(&field, res, -1000.0, 1000.0, 50_000_000) {
                Ok(mesh) => {
                    report(&mesh.summary);
                    let result = if out.to_ascii_lowercase().ends_with(".obj") {
                        let base = out.trim_end_matches(".obj").trim_end_matches(".OBJ");
                        write_obj(base, &mesh.vertices, &mesh.groups)
                            .map(|n| format!("{} ({} triangles)", out, n))
                    } else {
                        xyzcadrs::export::write_stl(&out, &mesh.vertices, &mesh.polygons)
                            .map(|n| format!("{out} ({n} triangles)"))
                    };
                    match result {
                        Ok(msg) => println!("wrote {msg} in {:.2?}", t0.elapsed()),
                        Err(e) => {
                            eprintln!("export failed: {e}");
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("render failed: {e}");
                    std::process::exit(1);
                }
            }
        }
        Command::Validate {
            path,
            weld_decimals,
        } => match read_stl(&path) {
            Ok(tris) => {
                let s = stats(&tris, weld_decimals);
                println!("file                 {}", path);
                println!("triangles            {}", s.triangles);
                println!("vertices (welded)    {}", s.vertices);
                println!("degenerate triangles {}", s.degenerate_triangles);
                println!("watertight           {}", s.watertight);
                println!("volume       {:>12.3} mm3", s.volume);
                println!("bbox x       {:>9.3} .. {:.3}", s.bbox[0][0], s.bbox[1][0]);
                println!("bbox y       {:>9.3} .. {:.3}", s.bbox[0][1], s.bbox[1][1]);
                println!("bbox z       {:>9.3} .. {:.3}", s.bbox[0][2], s.bbox[1][2]);
            }
            Err(e) => {
                eprintln!("validation failed: {e}");
                std::process::exit(1);
            }
        },
    }
}
