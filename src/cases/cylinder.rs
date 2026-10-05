//! The DFG benchmark "flow around a cylinder" of Schafer & Turek (1996), cases 2D-1 (steady,
//! Re = 20) and 2D-2 (periodic, Re = 100).
//!
//! Physical set-up: channel 2.2 m x 0.41 m, cylinder of diameter D = 0.1 m centred at
//! (0.2, 0.2) m (slightly off the centreline, which makes the lift non-zero), kinematic
//! viscosity 1e-3 m^2/s, density 1 kg/m^3, parabolic inflow with mean velocity
//! U = 0.2 m/s (2D-1) or 1 m/s (2D-2). Coefficients are c = 2 F / (rho U^2 D), the pressure
//! difference is p(0.15, 0.2) - p(0.25, 0.2), and Re = U D / nu.
//!
//! Lattice set-up: `n` cells per diameter, so the fluid region is 22n x 4.1n cells (hence `n`
//! must be a multiple of 10) with the boundary planes half a cell outside the outermost nodes.

use crate::cases::channel::{Channel, Inflow, Series, Spec};
use crate::geometry::Circle;
use crate::lattice::{Collision, CS2};
use crate::signal;

#[derive(Clone, Copy, Debug)]
pub struct Setup {
    /// Cells per cylinder diameter (multiple of 10).
    pub n: usize,
    /// Mean inflow velocity in lattice units (sets the Mach number and the time step).
    pub u_mean: f64,
    pub collision: Collision,
    pub threads: usize,
}

struct Rig {
    ch: Channel,
    n: f64,
    xc: f64,
    yc: f64,
}

fn rig(s: Setup, re: f64, ramp_units: f64) -> Rig {
    assert!(s.n >= 10 && s.n % 10 == 0, "cells per diameter must be a multiple of 10");
    let n = s.n as f64;
    let (xc, yc) = (0.5 + 2.0 * n, 0.5 + 2.0 * n);
    let spec = Spec {
        nx: 22 * s.n + 2,
        ny: 41 * s.n / 10 + 2,
        inflow: Inflow::Parabolic { u_mean: s.u_mean },
        nu: s.u_mean * n / re,
        collision: s.collision,
        body: Box::new(Circle { cx: xc, cy: yc, r: 0.5 * n }),
        ramp_steps: (ramp_units * n / s.u_mean) as u64,
        kick: None,
    };
    Rig { ch: Channel::new(spec, s.threads), n, xc, yc }
}

impl Rig {
    /// Pressure difference front minus back, in units of rho U^2.
    fn pressure_drop(&self) -> f64 {
        let r = 0.5 * self.n;
        let front = self.ch.sim.probe_density_at_wall(self.xc - r, self.yc, -1.0, 0.0);
        let back = self.ch.sim.probe_density_at_wall(self.xc + r, self.yc, 1.0, 0.0);
        CS2 * (front - back) / (self.ch.u_ref * self.ch.u_ref)
    }

    /// Length of the recirculation zone behind the cylinder in diameters: distance from the
    /// rear of the cylinder to the point on the line y = y_c where u_x turns positive again.
    fn recirculation_length(&self) -> f64 {
        let sim = &self.ch.sim;
        let (j0, j1) = (self.yc.floor() as usize, self.yc.floor() as usize + 1);
        let u = |x: usize| 0.5 * (sim.macros(x, j0).1 + sim.macros(x, j1).1);
        let rear = self.xc + 0.5 * self.n;
        let start = rear.ceil() as usize + 1;
        if u(start) >= 0.0 {
            return 0.0;
        }
        for x in start..sim.nx - 2 {
            let (a, b) = (u(x), u(x + 1));
            if a < 0.0 && b >= 0.0 {
                return (x as f64 + a / (a - b) - rear) / self.n;
            }
        }
        f64::NAN
    }
}

#[derive(Clone, Debug)]
pub struct Steady {
    pub n: usize,
    pub cd: f64,
    pub cl: f64,
    /// Pressure difference in Pa (rho = 1 kg/m^3, U = 0.2 m/s).
    pub dp: f64,
    /// Recirculation length in metres.
    pub la: f64,
    /// Peak-to-peak variation of c_D over the last five convective time units, relative.
    pub residual: f64,
    pub steps: u64,
    pub converged: bool,
    pub seconds: f64,
}

/// Case 2D-1 (Re = 20). Runs until the drag varies by less than `tol` (relative) over five
/// convective time units D/U and its window mean has stopped drifting, or `max_units` passes.
pub fn run_2d1(s: Setup, max_units: f64, tol: f64) -> Steady {
    let start = std::time::Instant::now();
    let mut rig = rig(s, 20.0, 8.0);
    let unit = (rig.n / s.u_mean).round() as usize;
    let window = 5 * unit;
    let (mut prev_mean, mut residual, mut converged) = (f64::NAN, f64::NAN, false);
    let (mut cd, mut cl) = (0.0, 0.0);
    while (rig.ch.sim.t as f64) < max_units * unit as f64 {
        let (mut lo, mut hi, mut sum, mut sum_l) = (f64::MAX, f64::MIN, 0.0, 0.0);
        for _ in 0..window {
            rig.ch.step();
            let (d, l) = rig.ch.coefficients(rig.n);
            lo = lo.min(d);
            hi = hi.max(d);
            sum += d;
            sum_l += l;
        }
        cd = sum / window as f64;
        cl = sum_l / window as f64;
        residual = (hi - lo) / cd.abs();
        if !cd.is_finite() {
            break;
        }
        if residual < tol && ((cd - prev_mean) / cd).abs() < tol {
            converged = true;
            break;
        }
        prev_mean = cd;
    }
    Steady {
        n: s.n,
        cd,
        cl,
        dp: rig.pressure_drop() * 0.2 * 0.2,
        la: rig.recirculation_length() * 0.1,
        residual,
        steps: rig.ch.sim.t,
        converged,
        seconds: start.elapsed().as_secs_f64(),
    }
}

#[derive(Clone, Debug)]
pub struct Unsteady {
    pub n: usize,
    pub cd_max: f64,
    pub cd_min: f64,
    pub cl_max: f64,
    pub cl_min: f64,
    /// Strouhal number D f / U from the lift spectrum, and from lift mean-crossings.
    pub st: f64,
    pub st_crossings: f64,
    /// Pressure difference (Pa; rho = 1, U = 1) half a period after a lift maximum.
    pub dp: f64,
    /// Relative spread (max - min) / mean of the lift maxima inside the analysis window:
    /// how periodic the flow had become.
    pub peak_spread: f64,
    pub periods: usize,
    pub steps: u64,
    pub seconds: f64,
    pub series: Series,
    /// Pressure-difference history, same sampling as `series`.
    pub dp_series: Vec<f64>,
    /// First sample of the analysis window.
    pub window_start: usize,
}

/// Case 2D-2 (Re = 100). Fails if the run diverges (it does at 10 cells per diameter). Runs `total_units` convective time units (D/U; 1 unit = 0.1 s of the
/// benchmark's physical time) and analyses the last `window_units`.
pub fn run_2d2(s: Setup, total_units: f64, window_units: f64) -> Result<Unsteady, String> {
    let start = std::time::Instant::now();
    let mut rig = rig(s, 100.0, 8.0);
    let unit = rig.n / s.u_mean;
    let steps = (total_units * unit) as usize;
    let mut series = Series { dt: 1.0 / unit, cd: Vec::with_capacity(steps), cl: Vec::with_capacity(steps) };
    let mut dp_series = Vec::with_capacity(steps);
    for _ in 0..steps {
        rig.ch.step();
        let (d, l) = rig.ch.coefficients(rig.n);
        if !d.is_finite() {
            return Err(format!("diverged after {} steps (tau = {:.4}): under-resolved at {} cells per diameter", rig.ch.sim.t, crate::lattice::tau_for(s.u_mean * rig.n / 100.0), s.n));
        }
        series.cd.push(d);
        series.cl.push(l);
        dp_series.push(rig.pressure_drop());
    }
    let i0 = steps - ((window_units * unit) as usize).min(steps);
    let (cd, cl, dp) = (&series.cd[i0..], &series.cl[i0..], &dp_series[i0..]);
    let (f, _) = signal::dominant_frequency(cl, series.dt);
    let period = 1.0 / (f * series.dt); // samples
    let pk = signal::peaks(cl);
    let tops: Vec<f64> = pk.iter().map(|p| p.1).collect();
    let fold = |v: &[f64], init: f64, g: fn(f64, f64) -> f64| v.iter().fold(init, |a, &b| g(a, b));
    let cl_max = fold(&tops, f64::MIN, f64::max);
    let spread = if tops.is_empty() { f64::NAN } else { (cl_max - fold(&tops, f64::MAX, f64::min)) / signal::mean(&tops) };
    // Pressure difference half a period after each lift maximum (the benchmark's definition).
    let half: Vec<f64> = pk.iter().map(|p| p.0 + 0.5 * period).filter(|&t| t < (dp.len() - 1) as f64).map(|t| signal::sample_at(dp, t)).collect();
    Ok(Unsteady {
        n: s.n,
        cd_max: fold(cd, f64::MIN, f64::max),
        cd_min: fold(cd, f64::MAX, f64::min),
        cl_max,
        cl_min: fold(cl, f64::MAX, f64::min),
        st: f,
        st_crossings: signal::crossing_frequency(cl, series.dt).unwrap_or(f64::NAN),
        dp: if half.is_empty() { f64::NAN } else { signal::mean(&half) },
        peak_spread: spread,
        periods: tops.len(),
        steps: steps as u64,
        seconds: start.elapsed().as_secs_f64(),
        series,
        dp_series,
        window_start: i0,
    })
}

/// Times `steps` steps of the 2D-2 set-up (forces evaluated every step, as in a real run).
/// Returns the channel (for its size and state hash) and the wall-clock seconds.
pub fn bench(s: Setup, steps: usize) -> (Channel, f64) {
    let mut rig = rig(s, 100.0, 8.0);
    let start = std::time::Instant::now();
    for _ in 0..steps {
        rig.ch.step();
    }
    let secs = start.elapsed().as_secs_f64();
    (rig.ch, secs)
}
