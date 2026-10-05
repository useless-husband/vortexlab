//! Command-line front end. `vortexlab help` lists the commands.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;
use vortexlab::cases::channel::Series;
use vortexlab::cases::tunnel::{Outcome, Tunnel};
use vortexlab::cases::{custom, cylinder};
use vortexlab::geometry::{section, Circle, Corner, Polygon, Rotated};
use vortexlab::suite::{self, CornerStudy, Table, TRT};
use vortexlab::{png, reference, report, units};

const HELP: &str = "vortexlab - a small 2-D lattice-Boltzmann wind tunnel

USAGE: vortexlab <command> [options]

  demo                    cylinder benchmark at low resolution + an animated vortex street
  shape <picture.png>     put your own silhouette in the tunnel (dark shape, light background)
        --re 150  --cells 40  --angle 0  --units 100  --u 0.08  --out <dir>
  validate                run the validation cases, write CSV files
        --quick (small grids, ~1 min)  --only poiseuille,cavity,2d1,2d2  --out results
  corners                 the corner-modification experiment
        --re 200  --d 60  --b 0.1  --units 140  --tag main  --shapes all  --out results
  bench                   speed in million lattice updates per second
        --n 40  --steps 2000  --threads-list 1,2,4
  report                  build the HTML report from the CSV files
        --results results  --out docs/report
  make-shapes <dir>       write the two sample silhouettes

Common options: --threads N (default 4, results do not depend on it).
Set VORTEXLAB_LANG=zh for Traditional Chinese messages in `demo` and `shape`.";

struct Args {
    pos: Vec<String>,
    opts: HashMap<String, String>,
}

impl Args {
    fn parse(raw: &[String]) -> Result<Args, String> {
        const FLAGS: [&str; 2] = ["quick", "no-animation"];
        let mut a = Args { pos: Vec::new(), opts: HashMap::new() };
        let mut i = 0;
        while i < raw.len() {
            if let Some(key) = raw[i].strip_prefix("--") {
                if FLAGS.contains(&key) {
                    a.opts.insert(key.into(), "true".into());
                } else {
                    let v = raw.get(i + 1).ok_or(format!("option --{key} needs a value"))?;
                    a.opts.insert(key.into(), v.clone());
                    i += 1;
                }
            } else {
                a.pos.push(raw[i].clone());
            }
            i += 1;
        }
        Ok(a)
    }
    fn get<T: std::str::FromStr>(&self, key: &str, default: T) -> Result<T, String> {
        match self.opts.get(key) {
            None => Ok(default),
            Some(v) => v.parse().map_err(|_| format!("bad value '{v}' for --{key}")),
        }
    }
    fn flag(&self, key: &str) -> bool {
        self.opts.contains_key(key)
    }
    fn threads(&self) -> Result<usize, String> {
        Ok(self.get("threads", 4usize)?.clamp(1, 64))
    }
    fn out(&self, default: &str) -> Result<PathBuf, String> {
        let p = PathBuf::from(self.get("out", default.to_string())?);
        std::fs::create_dir_all(&p).map_err(|e| format!("cannot create {}: {e}", p.display()))?;
        Ok(p)
    }
}

fn zh() -> bool {
    std::env::var("VORTEXLAB_LANG").map(|v| v.starts_with("zh")).unwrap_or(false)
}

/// Picks the message language.
fn tr<'a>(en: &'a str, zh_text: &'a str) -> &'a str {
    if zh() {
        zh_text
    } else {
        en
    }
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let result = match raw.first().map(String::as_str) {
        None | Some("help" | "--help" | "-h") => {
            println!("{HELP}");
            Ok(())
        }
        Some(cmd) => Args::parse(&raw[1..]).and_then(|a| match cmd {
            "demo" => demo(&a),
            "shape" => shape(&a),
            "validate" => validate(&a),
            "corners" => corners(&a),
            "bench" => bench(&a),
            "report" => {
                let results = PathBuf::from(a.get("results", "results".to_string())?);
                let out = a.out("docs/report")?;
                report::build(&results, &out)?;
                println!("report written to {}", out.join("index.html").display());
                Ok(())
            }
            "make-shapes" => make_shapes(&a),
            other => Err(format!("unknown command '{other}'; try `vortexlab help`")),
        }),
    };
    if let Err(e) = result {
        eprintln!("{}: {e}", tr("error", "錯誤"));
        std::process::exit(1);
    }
}

fn in_interval(v: f64, iv: Option<(f64, f64)>) -> &'static str {
    match iv {
        Some((lo, hi)) if v >= lo && v <= hi => tr("inside", "在區間內"),
        Some((lo, _)) if v < lo => tr("below", "低於區間"),
        Some(_) => tr("above", "高於區間"),
        None => "",
    }
}

fn progress_line(done: f64, s: &Series) {
    let n = s.cd.len();
    let tail = n - (n / 20).max(1);
    println!(
        "  {:3.0}%  t = {:6.1} D/U   Cd = {:6.3}   Cl = {:+6.3}",
        100.0 * done,
        n as f64 * s.dt,
        vortexlab::signal::mean(&s.cd[tail..]),
        s.cl[n - 1]
    );
}

fn print_outcome(t: &Tunnel, o: &Outcome) {
    let s = o.stats;
    println!("{}", tr("Result (after the start-up transient):", "結果（已去掉起動階段）："));
    println!("  {:<34} Cd = {:.3}", tr("mean drag coefficient", "平均阻力係數"), s.cd_mean);
    println!("  {:<34} Cl = {:+.3}", tr("mean lift coefficient", "平均升力係數"), s.cl_mean);
    println!("  {:<34} Cl' = {:.3}", tr("RMS lift fluctuation", "升力擺動的均方根"), s.cl_rms);
    if s.st > 0.0 {
        println!("  {:<34} St = {:.4}  ({} {:.4})", tr("Strouhal number (lift spectrum)", "史特豪數（升力頻譜）"), s.st, tr("from zero crossings:", "由過零點算："), s.st_crossings);
    } else {
        println!("  {}", tr("no vortex shedding detected (the lift does not oscillate)", "沒有偵測到渦流脫落（升力沒有擺動）"));
    }
    // The same flow in physical units, for one example size.
    let re = t.re;
    for (name, nu, rho) in [(tr("air", "空氣"), 1.5e-5, 1.2), (tr("water", "水"), 1.0e-6, 1000.0)] {
        let d = 0.01;
        let u = re * nu / d;
        let conv = units::Units::new(d, t.d as f64, u, t.u, rho);
        let force = conv.force_per_span(0.5 * s.cd_mean * t.u * t.u * t.d as f64);
        println!(
            "  {}: {} {:.3} m/s, {} {:.3e} N/m{}",
            tr("a 1 cm wide body at this Reynolds number", "同樣雷諾數、1 公分寬的物體"),
            name,
            u,
            tr("drag", "阻力"),
            force,
            if s.st > 0.0 { format!(", {} {:.2} Hz", tr("shedding at", "渦流脫落頻率"), s.st * u / d) } else { String::new() }
        );
    }
    println!("  {} {} x {} = {} {}, {} {}, {:.0} s, {:.0} MLUPS", tr("grid", "格點"), t.grid().0, t.grid().1, t.grid().0 * t.grid().1, tr("cells", "格"), o.steps, tr("steps", "步"), o.seconds, o.mlups);
}

fn save_outcome(dir: &Path, o: &Outcome) -> Result<(), String> {
    write(&dir.join("wake.png"), &o.png())?;
    if let Some(g) = o.gif() {
        write(&dir.join("vorticity.gif"), &g)?;
    }
    let mut t = Table::new(&["t", "cd", "cl"]);
    let stride = (o.series.cd.len() / 5000).max(1);
    for i in (0..o.series.cd.len()).step_by(stride) {
        t.push(vec![format!("{:.4}", i as f64 * o.series.dt), format!("{:.6}", o.series.cd[i]), format!("{:.6}", o.series.cl[i])]);
    }
    t.write(&dir.join("forces.csv")).map_err(|e| e.to_string())
}

fn demo(a: &Args) -> Result<(), String> {
    let threads = a.threads()?;
    let out = a.out("results/demo")?;
    println!("{}", tr("[1/2] Benchmark: flow past a cylinder in a channel, Schafer & Turek (1996) case 2D-2, Re = 100", "[1/2] 標準考題：管道裡的圓柱繞流，Schäfer 與 Turek（1996）的 2D-2 題，雷諾數 100"));
    println!("{}", tr("      20 cells per diameter (coarse on purpose, so it finishes in seconds).", "      圓柱直徑只切成 20 格（故意用粗網格，幾秒就跑完）。"));
    let o = cylinder::run_2d2(cylinder::Setup { n: 20, u_mean: 0.05, collision: TRT, threads }, 100.0, 20.0)?;
    println!("      {:<10} {:>10} {:>22}", tr("quantity", "物理量"), tr("computed", "算出來"), tr("published interval", "論文給的參考區間"));
    for (name, v, key) in [("c_D max", o.cd_max, "cd_max"), ("c_L max", o.cl_max, "cl_max"), ("St", o.st, "st"), ("dP", o.dp, "dp")] {
        let iv = reference::st_interval("2D-2", key);
        let (lo, hi) = iv.unwrap();
        println!("      {:<10} {:>10.4} {:>12.4} - {:.4}   {}", name, v, lo, hi, in_interval(v, iv));
    }
    println!("      ({:.0} s; {})", o.seconds, tr("finer grids land closer: see the report", "網格越細越接近，完整結果在報告裡"));
    println!();
    println!("{}", tr("[2/2] Vortex street behind a circular cylinder in a uniform stream, Re = 150", "[2/2] 均勻氣流中圓柱後面的渦街，雷諾數 150"));
    let t = Tunnel { d: 30, re: 150.0, u: 0.08, upstream: 6.0, downstream: 16.0, height: 10.0, length: 1.0, collision: TRT, threads, open_sides: true };
    let (cx, cy) = t.centre();
    let o = t.experiment(Box::new(Circle { cx, cy, r: 15.0 }), 90.0, 50.0, true, &mut |d, s| {
        if (d * 20.0).round() as usize % 4 == 0 {
            progress_line(d, s)
        }
    })?;
    print_outcome(&t, &o);
    save_outcome(&out, &o)?;
    println!("{} {}", tr("animation and picture written to", "動畫和圖片存到"), out.display());
    Ok(())
}

fn shape(a: &Args) -> Result<(), String> {
    let file = a.pos.first().ok_or(tr("give the picture file: vortexlab shape <picture.png>", "請指定圖片檔：vortexlab shape <圖片.png>"))?;
    let bytes = std::fs::read(file).map_err(|e| format!("{} {file}: {e}", tr("cannot read", "讀不到檔案")))?;
    let image = png::decode(&bytes)?;
    let (re, cells, angle, units_total, u) = (a.get("re", 150.0)?, a.get("cells", 40usize)?, a.get("angle", 0.0)?, a.get("units", 100.0)?, a.get("u", 0.08)?);
    if !(re > 0.0 && re <= 5000.0) || !(0.005..=0.15).contains(&u) || !(8..=400).contains(&cells) || !(units_total >= 10.0) {
        return Err("--re must be in (0, 5000], --u in [0.005, 0.15], --cells in [8, 400], --units at least 10".into());
    }
    let (tunnel, body) = custom::place(&image, cells, angle, re, u, TRT, a.threads()?)?;
    let tau = vortexlab::lattice::tau_for(u * tunnel.d as f64 / re);
    println!(
        "{} {}x{} px -> {} {} {}, {} {:.2} D; Re = {re}, Ma = {:.3}, tau = {:.4}",
        tr("picture", "圖片"),
        image.w,
        image.h,
        tr("body", "物體迎風寬度"),
        tunnel.d,
        tr("cells across the flow", "格"),
        tr("length", "長度"),
        tunnel.length,
        units::mach(u),
        tau
    );
    if tau < 0.51 {
        println!("{}", tr("warning: tau is very close to 0.5; the run may be under-resolved or unstable. Use more --cells or a lower --re.", "注意：tau 太接近 0.5，解析度可能不夠、甚至會算爆。請加大 --cells 或降低 --re。"));
    }
    let o = tunnel.experiment(body, units_total, 0.5 * units_total, !a.flag("no-animation"), &mut |d, s| {
        if (d * 20.0).round() as usize % 2 == 0 {
            progress_line(d, s)
        }
    })?;
    print_outcome(&tunnel, &o);
    let stem = Path::new(file).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "shape".into());
    let default_out = Path::new(file).parent().unwrap_or(Path::new(".")).join("輸出").join(stem);
    let out = a.out(&default_out.to_string_lossy())?;
    save_outcome(&out, &o)?;
    println!("{} {}", tr("wake.png, vorticity.gif and forces.csv written to", "wake.png、vorticity.gif、forces.csv 存到"), out.display());
    Ok(())
}

fn validate(a: &Args) -> Result<(), String> {
    let (quick, threads, out) = (a.flag("quick"), a.threads()?, a.out("results")?);
    let only = a.get("only", "poiseuille,cavity,2d1,2d2".to_string())?;
    let want = |k: &str| only.split(',').any(|s| s == k);
    let start = Instant::now();
    let save = |t: &Table, name: &str| t.write(&out.join(name)).map_err(|e| e.to_string());
    if want("poiseuille") {
        let t = suite::poiseuille_study(if quick { &[8, 16, 32] } else { &[8, 16, 32, 64, 128] });
        for r in &t.rows {
            println!("poiseuille  {:<28} rows {:>3}  L2 error {}  order {}", r[0], r[1], r[3], r[4]);
        }
        save(&t, "poiseuille.csv")?;
    }
    if want("cavity") {
        let mut t = Table::new(&suite::CAVITY_HEADER);
        let runs: &[(usize, u32)] = if quick { &[(33, 100), (65, 1000)] } else { &[(65, 100), (129, 100), (65, 1000), (129, 1000), (257, 1000)] };
        for &(n, re) in runs {
            let r = suite::cavity_run(n, re, 0.1, threads);
            println!("cavity      Re {re:>4}  n {n:>3}  max dev from Ghia: u {} v {}  u_min {}  ({} s)", r.summary[6], r.summary[7], r.summary[9], r.summary[5]);
            suite::cavity_profile_table(&r.outcome).write(&out.join(format!("cavity_profile_re{re}_n{n}.csv"))).map_err(|e| e.to_string())?;
            t.push(r.summary);
        }
        save(&t, "cavity.csv")?;
    }
    if want("2d1") {
        let mut t = Table::new(&suite::STEADY_HEADER);
        let runs: &[(usize, f64)] = if quick { &[(10, 0.04)] } else { &[(10, 0.04), (20, 0.04), (40, 0.04), (80, 0.04), (20, 0.02)] };
        for &(n, u) in runs {
            let r = suite::steady_row(n, u, threads);
            println!("2D-1        n {n:>3} u {u}  cD {}  cL {}  dP {}  La {}  ({} s)", r[2], r[3], r[4], r[5], r[9]);
            t.push(r);
        }
        save(&t, "cylinder_2d1.csv")?;
    }
    if want("2d2") {
        let mut t = Table::new(&suite::UNSTEADY_HEADER);
        let runs: &[(usize, f64)] = if quick { &[(20, 0.05)] } else { &[(20, 0.05), (40, 0.05), (80, 0.05), (20, 0.025)] };
        for &(n, u) in runs {
            let (r, o) = suite::unsteady_run(n, u, 110.0, threads)?;
            println!("2D-2        n {n:>3} u {u}  cDmax {}  cLmax {}  St {}  dP {}  ({} s)", r[2], r[4], r[6], r[8], r[12]);
            suite::unsteady_series_table(&o).write(&out.join(format!("cylinder_2d2_series_n{n}_u{u}.csv"))).map_err(|e| e.to_string())?;
            t.push(r);
        }
        save(&t, "cylinder_2d2.csv")?;
    }
    let secs = start.elapsed().as_secs_f64();
    println!("validation finished in {secs:.0} s with {threads} threads; CSV files in {}", out.display());
    let mut t = Table::new(&["set", "cases", "threads", "seconds"]);
    t.push(vec![if quick { "quick" } else { "full" }.into(), only.replace(',', "+"), threads.to_string(), format!("{secs:.0}")]);
    save(&t, &format!("validate_time_{}.csv", only.replace(',', "_")))
}

fn corners(a: &Args) -> Result<(), String> {
    let (threads, out) = (a.threads()?, a.out("results")?);
    let (d, re, b, u) = (a.get("d", 60usize)?, a.get("re", 200.0)?, a.get("b", 0.1)?, a.get("u", 0.1)?);
    let units_total = a.get("units", 140.0)?;
    let tag = a.get("tag", "main".to_string())?;
    let which = a.get("shapes", "all".to_string())?;
    let tunnel = Tunnel { d, re, u, upstream: a.get("upstream", 10.0)?, downstream: a.get("downstream", 18.0)?, height: a.get("height", 20.0)?, length: 1.0, collision: TRT, threads, open_sides: a.get("sides", "open".to_string())? != "walls" };
    let study = CornerStudy { tunnel, b, units: units_total, discard: a.get("discard", 0.5 * units_total)? };
    let path = out.join(format!("corners_{tag}.csv"));
    // Keep rows of shapes not re-run this time, so sections can be run one at a time.
    let mut table = Table::read(&path).filter(|t| t.header == suite::CORNER_HEADER).unwrap_or_else(|| Table::new(&suite::CORNER_HEADER));
    println!("corner study '{tag}': Re {re}, D = {d} cells, corner size {b} D, grid {} x {}, tau {:.4}", tunnel.grid().0, tunnel.grid().1, vortexlab::lattice::tau_for(u * d as f64 / re));
    for corner in study.shapes() {
        if which != "all" && !which.split(',').any(|s| s == corner.name()) {
            continue;
        }
        let (row, o) = suite::corner_run(&study, corner, !a.flag("no-animation"))?;
        println!("  {:<16} Cd {:.4}  Cl' {:.4}  St {:.4}  ({:.0} s, {:.0} MLUPS)", corner.name(), o.stats.cd_mean, o.stats.cl_rms, o.stats.st, o.seconds, o.mlups);
        write(&out.join(format!("corners_{tag}_{}.png", corner.name())), &o.png())?;
        if let Some(g) = o.gif() {
            write(&out.join(format!("corners_{tag}_{}.gif", corner.name())), &g)?;
        }
        let mut series = Table::new(&["t", "cd", "cl"]);
        let stride = (o.series.cd.len() / 4000).max(1);
        for i in (0..o.series.cd.len()).step_by(stride) {
            series.push(vec![format!("{:.4}", i as f64 * o.series.dt), format!("{:.6}", o.series.cd[i]), format!("{:.6}", o.series.cl[i])]);
        }
        series.write(&out.join(format!("corners_{tag}_{}_series.csv", corner.name()))).map_err(|e| e.to_string())?;
        table.rows.retain(|r| r[0] != corner.name());
        table.push(row);
        table.write(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn bench(a: &Args) -> Result<(), String> {
    let (n, steps) = (a.get("n", 40usize)?, a.get("steps", 2000usize)?);
    let list = a.get("threads-list", "1,2,4".to_string())?;
    let out = a.out("results")?;
    let mut table = Table::new(&["threads", "grid", "fluid_nodes", "steps", "seconds", "mlups", "state_hash"]);
    println!("benchmark: cylinder in a channel (2D-2 set-up), {n} cells per diameter, TRT, {steps} steps");
    for th in list.split(',') {
        let threads: usize = th.parse().map_err(|_| "bad --threads-list")?;
        let (ch, secs) = cylinder::bench(cylinder::Setup { n, u_mean: 0.05, collision: TRT, threads }, steps);
        let nodes = ch.sim.fluid_nodes();
        let mlups = nodes as f64 * steps as f64 / secs / 1e6;
        println!("  {threads} thread(s): {mlups:7.1} MLUPS   ({} x {} grid, {nodes} fluid nodes, {secs:.2} s, state hash {:016x})", ch.sim.nx, ch.sim.ny, ch.sim.state_hash());
        table.push(vec![threads.to_string(), format!("{}x{}", ch.sim.nx, ch.sim.ny), nodes.to_string(), steps.to_string(), format!("{secs:.3}"), format!("{mlups:.1}"), format!("{:016x}", ch.sim.state_hash())]);
    }
    table.write(&out.join(format!("bench_n{n}.csv"))).map_err(|e| e.to_string())
}

fn make_shapes(a: &Args) -> Result<(), String> {
    let dir = PathBuf::from(a.pos.first().ok_or("give the output folder: vortexlab make-shapes <dir>")?);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    // A square tower plan with double-notched corners, 300 px across.
    let tower = section(Corner::DoubleRecessed(0.05), 0.0, 0.0, 300.0, 0.0);
    let gray = custom::rasterize(tower.as_ref(), 400, 400, 1.0);
    write(&dir.join("台北101平面.png"), &png::encode(400, 400, png::Pixels::Gray(&gray)))?;
    // A NACA 0015 aerofoil, nose to the left, pitched 12 degrees nose-up, chord 420 px.
    let foil = Rotated { inner: Box::new(Polygon { pts: custom::naca_symmetric(0.15, 80) }), cx: 0.0, cy: 0.0, angle: (-12.0f64).to_radians() };
    let gray = custom::rasterize(&foil, 520, 220, 420.0);
    write(&dir.join("機翼.png"), &png::encode(520, 220, png::Pixels::Gray(&gray)))?;
    println!("wrote 台北101平面.png and 機翼.png to {}", dir.display());
    Ok(())
}
