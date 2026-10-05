use vortexlab::cases::poiseuille;
use vortexlab::lattice::Collision;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("poiseuille") => {
            for (name, col, tau, q) in [
                ("bgk tau=0.8 halfway", Collision::Bgk, 0.8, 0.5),
                ("trt 3/16 halfway", Collision::Trt { magic: 0.1875 }, 0.8, 0.5),
                ("trt 1/4 halfway", Collision::Trt { magic: 0.25 }, 0.8, 0.5),
                ("trt 3/16 q=0.25", Collision::Trt { magic: 0.1875 }, 0.8, 0.25),
                ("trt 3/16 q=0.8", Collision::Trt { magic: 0.1875 }, 0.8, 0.8),
            ] {
                let out: Vec<_> = [8, 16, 32, 64].iter().map(|&h| poiseuille::run(h, tau, col, q, 1)).collect();
                let e: Vec<String> = out.iter().map(|o| format!("{:.3e}", o.l2)).collect();
                let fb: Vec<String> = out.iter().map(|o| format!("{:.12}", o.force_balance)).collect();
                println!("{name}: l2 {e:?} orders {:?} balance {fb:?} steps {}", poiseuille::observed_orders(&out), out[3].steps);
            }
        }
        Some("st1") => {
            let n: usize = args[2].parse().unwrap();
            let u: f64 = args[3].parse().unwrap();
            let th: usize = args.get(4).map(|s| s.parse().unwrap()).unwrap_or(1);
            let o = vortexlab::cases::cylinder::run_2d1(vortexlab::cases::cylinder::Setup { n, u_mean: u, collision: Collision::Trt { magic: 0.1875 }, threads: th }, 400.0, 1e-6);
            println!("{o:?}");
        }
        Some("st2") => {
            let n: usize = args[2].parse().unwrap();
            let u: f64 = args[3].parse().unwrap();
            let th: usize = args.get(4).map(|s| s.parse().unwrap()).unwrap_or(1);
            let mut o = vortexlab::cases::cylinder::run_2d2(vortexlab::cases::cylinder::Setup { n, u_mean: u, collision: Collision::Trt { magic: 0.1875 }, threads: th }, std::env::var("UNITS").ok().and_then(|s| s.parse().ok()).unwrap_or(60.0), 15.0);
            let nodes = (22 * n * 41 * n / 10) as f64;
            println!("mlups {:.1}", nodes * o.steps as f64 / o.seconds / 1e6);
            if let Ok(path) = std::env::var("DUMP") {
                let mut out = String::new();
                for i in (0..o.series.cd.len()).step_by(4) {
                    out += &format!("{} {} {} {}\n", i as f64 * o.series.dt, o.series.cd[i], o.series.cl[i], o.dp_series[i]);
                }
                std::fs::write(path, out).unwrap();
            }
            o.series = Default::default(); o.dp_series.clear();
            println!("{o:?}");
        }
        Some("cavity") => {
            let n: usize = args[2].parse().unwrap();
            let re: f64 = args[3].parse().unwrap();
            let o = vortexlab::cases::cavity::run(n, re, 0.1, Collision::Trt { magic: 0.1875 }, 1e-8, 2_000_000, 1);
            let umin = o.u_vertical.iter().fold(0.0f64, |a, p| a.min(p.1));
            println!("steps {} conv {} sec {:.1} umin {umin}", o.steps, o.converged, o.seconds);
        }
        Some("sq") => {
            use vortexlab::geometry::{section, Corner};
            let d: usize = args[2].parse().unwrap();
            let re: f64 = args[3].parse().unwrap();
            let b: f64 = args[5].parse().unwrap();
            let corner = match args[4].as_str() { "square" => Corner::Sharp, "chamfer" => Corner::Chamfered(b), "round" => Corner::Rounded(b), "recess" => Corner::Recessed(b), _ => Corner::DoubleRecessed(b) };
            let units: f64 = args[6].parse().unwrap();
            let t = vortexlab::cases::tunnel::Tunnel { d, re, u: 0.1, upstream: 8.0, downstream: 16.0, height: 12.0, collision: Collision::Trt { magic: 0.1875 }, threads: 4 };
            let (cx, cy) = t.centre();
            let mut ch = t.build(section(corner, cx, cy, d as f64, 0.0));
            let t0 = std::time::Instant::now();
            let s = t.run(&mut ch, units, 0, &mut |_| {}).unwrap();
            let st = s.stats(units * 0.5);
            println!("{} {:?} sec {:.0}", corner.name(), st, t0.elapsed().as_secs_f64());
            let (nx, ny) = t.grid();
            let view = vortexlab::render::View { x0: 0, x1: nx, y0: 0, y1: ny, zoom: 0.75 };
            let (w, h) = view.size();
            let px = vortexlab::render::vorticity_frame(&ch.sim, view, 3.0 * 0.1 / d as f64);
            std::fs::write(format!("/private/tmp/claude-501/portfolio/vortexlab/{}.png", corner.name()), vortexlab::png::encode(w, h, vortexlab::png::Pixels::Indexed(&px, &vortexlab::render::palette()))).unwrap();
            let mut out = String::new();
            for i in (0..s.cd.len()).step_by(20) { out += &format!("{} {} {}\n", i as f64 * s.dt, s.cd[i], s.cl[i]); }
            std::fs::write(format!("/private/tmp/claude-501/portfolio/vortexlab/{}.txt", corner.name()), out).unwrap();
        }
        _ => eprintln!("usage: vortexlab <command>"),
    }
}
