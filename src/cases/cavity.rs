//! Lid-driven square cavity: all four walls no-slip, the top one sliding at constant speed.
//! Compared with the centreline velocities tabulated by Ghia, Ghia & Shin (1982).

use crate::lattice::Collision;
use crate::sim::{Builder, Kind, Wall};

#[derive(Clone, Debug)]
pub struct Outcome {
    pub n: usize,
    pub re: f64,
    pub steps: u64,
    pub converged: bool,
    pub seconds: f64,
    /// u / U_lid along the vertical line through the centre, as (y, u) with y in [0, 1].
    pub u_vertical: Vec<(f64, f64)>,
    /// v / U_lid along the horizontal line through the centre, as (x, v).
    pub v_horizontal: Vec<(f64, f64)>,
}

/// `n` x `n` fluid cells; the walls sit half a cell outside the outermost nodes, so node `i`
/// (1-based) is at (i - 1/2)/n. Runs until the largest velocity change over 1000 steps drops
/// below `tol` times the lid speed, or `max_steps`.
pub fn run(n: usize, re: f64, u_lid: f64, collision: Collision, tol: f64, max_steps: u64, threads: usize) -> Outcome {
    let start = std::time::Instant::now();
    let mut b = Builder::new(n + 2, n + 2);
    b.tau = crate::lattice::tau_for(u_lid * n as f64 / re);
    b.collision = collision;
    let wall = b.add_static_wall();
    let lid = b.add(Kind::Wall(Wall { velocity: Some(Box::new(move |_, _| [u_lid, 0.0])), ..Wall::default() }));
    b.fill_rect(0, n + 1, 0, n + 1, wall);
    b.fill_rect(1, n, n + 1, n + 1, lid);
    b.fill_rect(1, n, 1, n, crate::sim::FLUID);
    let mut sim = b.build(threads);

    let snapshot = |sim: &crate::sim::Sim| -> Vec<(f64, f64)> {
        let mut v = Vec::with_capacity(n * n);
        for y in 1..=n {
            for x in 1..=n {
                let (_, ux, uy) = sim.macros(x, y);
                v.push((ux, uy));
            }
        }
        v
    };
    let mut prev = snapshot(&sim);
    let mut converged = false;
    while sim.t < max_steps {
        for _ in 0..1000 {
            sim.step();
        }
        let now = snapshot(&sim);
        let change = now.iter().zip(&prev).map(|(a, b)| (a.0 - b.0).abs().max((a.1 - b.1).abs())).fold(0.0, f64::max);
        prev = now;
        if change.is_nan() {
            break;
        }
        if change < tol * u_lid {
            converged = true;
            break;
        }
    }

    // Centrelines: a node line if n is odd, the mean of the two neighbouring lines if even.
    let (c0, c1) = if n % 2 == 1 { (n.div_ceil(2), n.div_ceil(2)) } else { (n / 2, n / 2 + 1) };
    let mut u_vertical = vec![(0.0, 0.0)];
    let mut v_horizontal = vec![(0.0, 0.0)];
    for k in 1..=n {
        let s = (k as f64 - 0.5) / n as f64;
        u_vertical.push((s, 0.5 * (sim.macros(c0, k).1 + sim.macros(c1, k).1) / u_lid));
        v_horizontal.push((s, 0.5 * (sim.macros(k, c0).2 + sim.macros(k, c1).2) / u_lid));
    }
    u_vertical.push((1.0, 1.0));
    v_horizontal.push((1.0, 0.0));
    Outcome { n, re, steps: sim.t, converged, seconds: start.elapsed().as_secs_f64(), u_vertical, v_horizontal }
}

/// Value of a tabulated profile at `s` by cubic Lagrange interpolation through the four
/// nearest samples (the profile includes both wall points).
pub fn interpolate(profile: &[(f64, f64)], s: f64) -> f64 {
    let n = profile.len();
    let mut k = profile.partition_point(|p| p.0 < s);
    k = k.clamp(2, n - 2);
    let pts = &profile[k - 2..k + 2];
    let mut out = 0.0;
    for i in 0..4 {
        let mut w = 1.0;
        for j in 0..4 {
            if i != j {
                w *= (s - pts[j].0) / (pts[i].0 - pts[j].0);
            }
        }
        out += w * pts[i].1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::interpolate;

    #[test]
    fn cubic_interpolation_is_exact_for_cubics() {
        let f = |x: f64| 1.0 - 2.0 * x + 0.5 * x * x * x;
        let prof: Vec<(f64, f64)> = [0.0, 0.07, 0.2, 0.31, 0.5, 0.74, 0.9, 1.0].iter().map(|&x| (x, f(x))).collect();
        for s in [0.0, 0.01, 0.25, 0.5, 0.62, 0.95, 1.0] {
            assert!((interpolate(&prof, s) - f(s)).abs() < 1e-12);
        }
    }
}
