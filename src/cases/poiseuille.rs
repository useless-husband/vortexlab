//! Plane Poiseuille flow driven by a uniform body force, compared with the exact parabola
//! u(y) = g / (2 nu) * (y - y_bottom) * (y_top - y).
//!
//! The channel is periodic along x, so the only error sources are the collision model and the
//! wall rule: exactly what a grid-convergence study should isolate.

use crate::geometry::Shape;
use crate::lattice::{viscosity, Collision};
use crate::sim::{Builder, Kind, Wall};

/// Everything outside the band y0 < y < y1 is wall.
struct OutsideBand {
    y0: f64,
    y1: f64,
}

impl Shape for OutsideBand {
    fn inside(&self, _x: f64, y: f64) -> bool {
        y <= self.y0 || y >= self.y1
    }
}

#[derive(Clone, Debug)]
pub struct Outcome {
    /// Fluid rows across the channel.
    pub rows: usize,
    /// Distance from the outermost fluid row to the wall, in cells (0.5 = halfway).
    pub q: f64,
    /// Relative L2 error of the velocity profile.
    pub l2: f64,
    /// Steps until the profile stopped changing.
    pub steps: u64,
    /// Sum of the wall drag divided by the total driving force (1 when in balance).
    pub force_balance: f64,
    /// Computed and exact profile, for plotting: (y - y_bottom) / H, u / u_max.
    pub profile: Vec<(f64, f64, f64)>,
}

/// Runs to steady state. `q` positions both walls `q` cells beyond the outermost fluid rows;
/// with `q == 0.5` the classic halfway bounce-back is used, otherwise the interpolated rule.
pub fn run(rows: usize, tau: f64, collision: Collision, q: f64, threads: usize) -> Outcome {
    let nx = 4;
    let ny = rows + 2;
    let (y0, y1) = (1.0 - q, rows as f64 + q);
    let width = y1 - y0;
    let nu = viscosity(tau);
    // Peak velocity scaled like 1/rows (diffusive scaling) so the Mach number falls with h.
    let umax = 0.1 * 16.0 / rows as f64;
    let g = 8.0 * nu * umax / (width * width);

    let mut b = Builder::new(nx, ny);
    b.periodic_x = true;
    b.tau = tau;
    b.collision = collision;
    b.accel = [g, 0.0];
    let shape: Option<Box<dyn Shape>> = if q == 0.5 { None } else { Some(Box::new(OutsideBand { y0, y1 })) };
    let wall = b.add(Kind::Wall(Wall { shape, fgroup: Some(0), ..Wall::default() }));
    b.fill_rect(0, nx - 1, 0, 0, wall);
    b.fill_rect(0, nx - 1, ny - 1, ny - 1, wall);
    let mut sim = b.build(threads);

    let exact = |y: usize| g / (2.0 * nu) * (y as f64 - y0) * (y1 - y as f64);
    let mut prev = vec![0.0; ny];
    let check = 200;
    loop {
        for _ in 0..check {
            sim.step();
        }
        let mut change = 0.0f64;
        for y in 1..=rows {
            let u = sim.macros(1, y).1;
            change = change.max((u - prev[y]).abs());
            prev[y] = u;
        }
        if change < 1e-13 * umax || sim.t > 4_000_000 {
            break;
        }
    }
    let (mut num, mut den) = (0.0, 0.0);
    let mut profile = Vec::new();
    for y in 1..=rows {
        let (u, e) = (sim.macros(1, y).1, exact(y));
        num += (u - e) * (u - e);
        den += e * e;
        profile.push(((y as f64 - y0) / width, u / umax, e / umax));
    }
    let driving = g * sim.total_mass();
    Outcome { rows, q, l2: (num / den).sqrt(), steps: sim.t, force_balance: sim.force[0][0] / driving, profile }
}

/// Observed order of accuracy between successive resolutions: log2(e_coarse / e_fine) scaled
/// by the actual refinement ratio.
pub fn observed_orders(outcomes: &[Outcome]) -> Vec<f64> {
    outcomes
        .windows(2)
        .map(|w| {
            let h0 = w[0].rows as f64 - 1.0 + 2.0 * w[0].q;
            let h1 = w[1].rows as f64 - 1.0 + 2.0 * w[1].q;
            (w[0].l2 / w[1].l2).ln() / (h1 / h0).ln()
        })
        .collect()
}
