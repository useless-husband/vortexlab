//! Runs the validation cases and the corner experiment and stores the numbers as CSV files,
//! which the report generator and the tests read back.

use crate::cases::tunnel::Tunnel;
use crate::cases::{cavity, cylinder, poiseuille};
use crate::geometry::{section, Corner};
use crate::lattice::Collision;
use crate::reference;
use std::path::Path;

/// A small CSV table (no quoting: fields never contain commas).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Table {
    pub header: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    pub fn new(header: &[&str]) -> Table {
        Table { header: header.iter().map(|s| s.to_string()).collect(), rows: Vec::new() }
    }
    pub fn push(&mut self, row: Vec<String>) {
        assert_eq!(row.len(), self.header.len(), "row width does not match the header");
        self.rows.push(row);
    }
    pub fn to_csv(&self) -> String {
        let mut s = self.header.join(",") + "\n";
        for r in &self.rows {
            s += &(r.join(",") + "\n");
        }
        s
    }
    pub fn parse(text: &str) -> Table {
        let mut rows = reference::rows(text);
        if rows.is_empty() {
            return Table::default();
        }
        let header = rows.remove(0);
        Table { header, rows }
    }
    pub fn read(path: &Path) -> Option<Table> {
        std::fs::read_to_string(path).ok().map(|t| Table::parse(&t))
    }
    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        std::fs::write(path, self.to_csv())
    }
    pub fn col(&self, name: &str) -> usize {
        self.header.iter().position(|h| h == name).unwrap_or_else(|| panic!("no column '{name}'"))
    }
    pub fn text(&self, row: usize, name: &str) -> &str {
        &self.rows[row][self.col(name)]
    }
    pub fn num(&self, row: usize, name: &str) -> f64 {
        self.text(row, name).parse().unwrap_or(f64::NAN)
    }
}

pub const TRT: Collision = Collision::Trt { magic: 3.0 / 16.0 };

fn f(v: f64) -> String {
    format!("{v:.8e}")
}

/// Poiseuille grid-convergence study: five scheme variants, each on a sequence of grids.
pub fn poiseuille_study(rows: &[usize]) -> Table {
    let mut t = Table::new(&["scheme", "rows", "q", "l2", "order", "balance", "steps"]);
    let variants: [(&str, Collision, f64); 5] = [
        ("BGK; halfway wall", Collision::Bgk, 0.5),
        ("TRT 3/16; halfway wall", TRT, 0.5),
        ("TRT 1/4; halfway wall", Collision::Trt { magic: 0.25 }, 0.5),
        ("TRT 3/16; wall at q = 0.25", TRT, 0.25),
        ("TRT 3/16; wall at q = 0.80", TRT, 0.8),
    ];
    for (name, collision, q) in variants {
        let out: Vec<poiseuille::Outcome> = rows.iter().map(|&h| poiseuille::run(h, 0.8, collision, q, 1)).collect();
        let orders = poiseuille::observed_orders(&out);
        for (k, o) in out.iter().enumerate() {
            let order = if k == 0 { String::new() } else { format!("{:.3}", orders[k - 1]) };
            t.push(vec![
                name.into(),
                o.rows.to_string(),
                q.to_string(),
                f(o.l2),
                order,
                format!("{:.12}", o.force_balance),
                o.steps.to_string(),
            ]);
        }
    }
    t
}

/// Position and value of the extremum of a sampled profile nearest to index `k`, refined by a
/// parabola through three samples (non-uniform spacing allowed).
fn refine_extremum(p: &[(f64, f64)], k: usize) -> (f64, f64) {
    if k == 0 || k + 1 >= p.len() {
        return p[k];
    }
    let ((x0, y0), (x1, y1), (x2, y2)) = (p[k - 1], p[k], p[k + 1]);
    let d1 = (y1 - y0) / (x1 - x0);
    let d2 = (y2 - y1) / (x2 - x1);
    let c = (d2 - d1) / (x2 - x0);
    if c == 0.0 {
        return p[k];
    }
    let x = 0.5 * (x0 + x1) - 0.5 * d1 / c;
    (x, y0 + d1 * (x - x0) + c * (x - x0) * (x - x1))
}

pub struct CavityRun {
    pub summary: Vec<String>,
    pub outcome: cavity::Outcome,
}

pub const CAVITY_HEADER: [&str; 17] = [
    "re",
    "n",
    "u_lid",
    "steps",
    "converged",
    "seconds",
    "max_dev_u",
    "max_dev_v",
    "rms_dev",
    "u_min",
    "u_min_y",
    "v_max",
    "v_max_x",
    "v_min",
    "v_min_x",
    "worst_point",
    "worst_ref",
];

/// One cavity run compared with Ghia's table: maximum and RMS deviation over the 2 x 15
/// interior tabulated points (in units of the lid velocity), plus the profile extrema.
pub fn cavity_run(n: usize, re: u32, u_lid: f64, threads: usize) -> CavityRun {
    let o = cavity::run(n, re as f64, u_lid, TRT, 1e-9, 3_000_000, threads);
    let (mut max_u, mut max_v, mut sq, mut count) = (0.0f64, 0.0f64, 0.0, 0);
    let mut worst = (0.0f64, String::new(), 0.0);
    for (table, profile, max) in [("u", &o.u_vertical, &mut max_u), ("v", &o.v_horizontal, &mut max_v)] {
        for (s, r) in reference::ghia(table, re) {
            if s == 0.0 || s == 1.0 {
                continue;
            }
            let d = cavity::interpolate(profile, s) - r;
            *max = max.max(d.abs());
            sq += d * d;
            count += 1;
            if d.abs() > worst.0 {
                worst = (d.abs(), format!("{table}({s:.4})"), r);
            }
        }
    }
    let arg = |p: &[(f64, f64)], sign: f64| (0..p.len()).max_by(|&a, &b| (sign * p[a].1).total_cmp(&(sign * p[b].1))).unwrap();
    let u_min = refine_extremum(&o.u_vertical, arg(&o.u_vertical, -1.0));
    let v_max = refine_extremum(&o.v_horizontal, arg(&o.v_horizontal, 1.0));
    let v_min = refine_extremum(&o.v_horizontal, arg(&o.v_horizontal, -1.0));
    let summary = vec![
        re.to_string(),
        n.to_string(),
        u_lid.to_string(),
        o.steps.to_string(),
        o.converged.to_string(),
        format!("{:.1}", o.seconds),
        f(max_u),
        f(max_v),
        f((sq / count as f64).sqrt()),
        f(u_min.1),
        f(u_min.0),
        f(v_max.1),
        f(v_max.0),
        f(v_min.1),
        f(v_min.0),
        worst.1,
        worst.2.to_string(),
    ];
    CavityRun { summary, outcome: o }
}

pub fn cavity_profile_table(o: &cavity::Outcome) -> Table {
    let mut t = Table::new(&["line", "s", "value"]);
    for (name, p) in [("u", &o.u_vertical), ("v", &o.v_horizontal)] {
        for (s, v) in p {
            t.push(vec![name.into(), format!("{s:.6}"), f(*v)]);
        }
    }
    t
}

pub const STEADY_HEADER: [&str; 10] = ["n", "u_mean", "cd", "cl", "dp", "la", "residual", "steps", "converged", "seconds"];

pub fn steady_row(n: usize, u_mean: f64, threads: usize) -> Vec<String> {
    let o = cylinder::run_2d1(cylinder::Setup { n, u_mean, collision: TRT, threads }, 300.0, 1e-6);
    vec![
        n.to_string(),
        u_mean.to_string(),
        f(o.cd),
        f(o.cl),
        f(o.dp),
        f(o.la),
        f(o.residual),
        o.steps.to_string(),
        o.converged.to_string(),
        format!("{:.1}", o.seconds),
    ]
}

pub const UNSTEADY_HEADER: [&str; 13] =
    ["n", "u_mean", "cd_max", "cd_min", "cl_max", "cl_min", "st", "st_crossings", "dp", "peak_spread", "periods", "steps", "seconds"];

pub fn unsteady_run(n: usize, u_mean: f64, units: f64, threads: usize) -> Result<(Vec<String>, cylinder::Unsteady), String> {
    let o = cylinder::run_2d2(cylinder::Setup { n, u_mean, collision: TRT, threads }, units, 20.0)?;
    let row = vec![
        n.to_string(),
        u_mean.to_string(),
        f(o.cd_max),
        f(o.cd_min),
        f(o.cl_max),
        f(o.cl_min),
        f(o.st),
        f(o.st_crossings),
        f(o.dp),
        f(o.peak_spread),
        o.periods.to_string(),
        o.steps.to_string(),
        format!("{:.1}", o.seconds),
    ];
    Ok((row, o))
}

/// The analysed window of a 2D-2 run, thinned to at most ~3000 samples: time (in D/U), c_D,
/// c_L and pressure difference.
pub fn unsteady_series_table(o: &cylinder::Unsteady) -> Table {
    let mut t = Table::new(&["t", "cd", "cl", "dp"]);
    let len = o.series.cd.len() - o.window_start;
    let stride = (len / 3000).max(1);
    for i in (o.window_start..o.series.cd.len()).step_by(stride) {
        t.push(vec![format!("{:.5}", i as f64 * o.series.dt), f(o.series.cd[i]), f(o.series.cl[i]), f(o.dp_series[i])]);
    }
    t
}

/// The corner-modification experiment's configuration.
#[derive(Clone, Copy, Debug)]
pub struct CornerStudy {
    pub tunnel: Tunnel,
    /// Corner size as a fraction of the side (chamfer leg, radius, recess side).
    pub b: f64,
    pub units: f64,
    pub discard: f64,
}

impl CornerStudy {
    pub fn shapes(&self) -> Vec<Corner> {
        vec![
            Corner::Sharp,
            Corner::Chamfered(self.b),
            Corner::Rounded(self.b),
            Corner::Recessed(self.b),
            Corner::DoubleRecessed(0.5 * self.b),
        ]
    }
}

pub const CORNER_HEADER: [&str; 15] =
    ["shape", "b", "d", "re", "blockage", "u", "units", "cd_mean", "cd_rms", "cl_rms", "st", "st_crossings", "cl_mean", "steps", "seconds"];

/// Runs one section; returns the table row and the full outcome (pictures, force history).
pub fn corner_run(study: &CornerStudy, corner: Corner, animate: bool) -> Result<(Vec<String>, crate::cases::tunnel::Outcome), String> {
    let t = &study.tunnel;
    let (cx, cy) = t.centre();
    let o = t.experiment(section(corner, cx, cy, t.d as f64, 0.0), study.units, study.discard, animate, &mut |_, _| {})?;
    let s = o.stats;
    let row = vec![
        corner.name().into(),
        study.b.to_string(),
        t.d.to_string(),
        t.re.to_string(),
        format!("{:.4}", 1.0 / t.height),
        t.u.to_string(),
        study.units.to_string(),
        f(s.cd_mean),
        f(s.cd_rms),
        f(s.cl_rms),
        f(s.st),
        f(s.st_crossings),
        f(s.cl_mean),
        o.steps.to_string(),
        format!("{:.1}", o.seconds),
    ];
    Ok((row, o))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_round_trip() {
        let mut t = Table::new(&["name", "value"]);
        t.push(vec!["a".into(), "1.5".into()]);
        t.push(vec!["b".into(), f(0.000123456789)]);
        let back = Table::parse(&t.to_csv());
        assert_eq!(back, t);
        assert_eq!(back.num(0, "value"), 1.5);
        assert!((back.num(1, "value") - 0.000123456789).abs() < 1e-15);
        assert_eq!(back.text(1, "name"), "b");
    }

    #[test]
    fn extremum_refinement() {
        // Parabola with minimum -2 at x = 0.37, sampled unevenly.
        let g = |x: f64| 3.0 * (x - 0.37) * (x - 0.37) - 2.0;
        let p: Vec<(f64, f64)> = [0.0, 0.1, 0.3, 0.45, 0.7, 1.0].iter().map(|&x| (x, g(x))).collect();
        let (x, y) = refine_extremum(&p, 2);
        assert!((x - 0.37).abs() < 1e-12 && (y + 2.0).abs() < 1e-12);
    }
}
