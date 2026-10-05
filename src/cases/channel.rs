//! A body in a channel: the common set-up behind the Schafer-Turek benchmark, the
//! corner-modification experiment and "bring your own shape".
//!
//! ```text
//!   y = ny-1   wall cells (at rest, or moving with the stream for uniform inflow)
//!              +---------------------------------------------------+
//!   inlet      |  ->                                               |  outlet
//!   velocity   |  ->        (body)                                 |  constant
//!   cells x=0  |  ->                                               |  pressure
//!              +---------------------------------------------------+  cells x=nx-1
//!   y = 0      wall cells
//! ```
//!
//! All four boundary planes lie half a cell outside the outermost fluid nodes, so the fluid
//! region is exactly (nx-2) x (ny-2) cells.

use crate::cases::ramp;
use crate::geometry::Shape;
use crate::lattice::{tau_for, Collision};
use crate::sim::{Builder, Kind, Sim, Wall};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Inflow {
    /// Developed channel profile with the given mean velocity; channel walls at rest.
    Parabolic { u_mean: f64 },
    /// Uniform stream; the channel walls move with it, as for a body towed through a tank of
    /// still fluid, so no boundary layer grows on them.
    Uniform { u: f64 },
    /// Uniform stream between sound-absorbing side boundaries ([`Kind::Radiating`]): for the
    /// flow they act almost like the moving walls of `Uniform`, but the cross-stream sound
    /// wave that an oscillating lift force keeps exciting leaves instead of resonating.
    UniformOpen { u: f64 },
}

/// A short twist of the body about (cx, cy) that breaks the mirror symmetry of the set-up, so
/// vortex shedding does not have to wait for rounding errors to grow.
#[derive(Clone, Copy, Debug)]
pub struct Kick {
    pub cx: f64,
    pub cy: f64,
    /// Peak angular velocity (radians per step).
    pub omega: f64,
    pub start: u64,
    pub duration: u64,
}

pub struct Spec {
    pub nx: usize,
    pub ny: usize,
    pub inflow: Inflow,
    /// Lattice kinematic viscosity.
    pub nu: f64,
    pub collision: Collision,
    pub body: Box<dyn Shape>,
    /// Steps over which the inflow is raised from zero (0 = start from the developed flow).
    pub ramp_steps: u64,
    pub kick: Option<Kick>,
}

pub struct Channel {
    pub sim: Sim,
    ramp_steps: u64,
    kick: Option<Kick>,
    /// Mean inflow velocity (lattice units).
    pub u_ref: f64,
    /// Let plane sound waves leave through the outlet (default) instead of reflecting them.
    pub absorbing_outlet: bool,
}

impl Channel {
    pub fn new(spec: Spec, threads: usize) -> Channel {
        let Spec { nx, ny, inflow, nu, collision, body, ramp_steps, kick } = spec;
        let h = (ny - 2) as f64;
        let mut b = Builder::new(nx, ny);
        b.tau = tau_for(nu);
        b.collision = collision;
        let profile = move |y: f64| -> f64 {
            match inflow {
                Inflow::Uniform { u } | Inflow::UniformOpen { u } => u,
                Inflow::Parabolic { u_mean } => {
                    let s = (y - 0.5).clamp(0.0, h);
                    6.0 * u_mean * s * (h - s) / (h * h)
                }
            }
        };
        let inlet = b.add(Kind::Wall(Wall { velocity: Some(Box::new(move |_, y| [profile(y), 0.0])), vgroup: 1, ..Wall::default() }));
        let outlet = b.add(Kind::Pressure(1.0));
        let walls = match inflow {
            Inflow::Parabolic { .. } => b.add_static_wall(),
            Inflow::Uniform { u } => {
                b.add(Kind::Wall(Wall { velocity: Some(Box::new(move |_, _| [u, 0.0])), vgroup: 1, ..Wall::default() }))
            }
            Inflow::UniformOpen { .. } => b.add(Kind::Radiating(1.0)),
        };
        b.fill_rect(0, 0, 0, ny - 1, inlet);
        b.fill_rect(nx - 1, nx - 1, 0, ny - 1, outlet);
        b.fill_rect(1, nx - 2, 0, 0, walls);
        b.fill_rect(1, nx - 2, ny - 1, ny - 1, walls);
        if matches!(inflow, Inflow::Parabolic { .. } | Inflow::Uniform { .. }) {
            // Solid walls also own the four corner cells.
            b.fill_rect(0, nx - 1, 0, 0, walls);
            b.fill_rect(0, nx - 1, ny - 1, ny - 1, walls);
        }
        let spin = kick.map(|k| -> crate::sim::VelocityFn { Box::new(move |x, y| [-(y - k.cy), x - k.cx]) });
        let body = b.add(Kind::Wall(Wall { velocity: spin, vgroup: 2, fgroup: Some(0), shape: Some(body) }));
        b.fill_shape(body);
        let mut sim = b.build(threads);
        if ramp_steps == 0 {
            sim.init(|_, y| (1.0, profile(y as f64), 0.0));
        }
        let u_ref = match inflow {
            Inflow::Uniform { u } | Inflow::UniformOpen { u } => u,
            Inflow::Parabolic { u_mean } => u_mean,
        };
        Channel { sim, ramp_steps, kick, u_ref, absorbing_outlet: true }
    }

    pub fn step(&mut self) {
        let t = self.sim.t;
        self.sim.vscale[1] = ramp(t, self.ramp_steps);
        self.sim.outflow_target = self.absorbing_outlet.then_some(self.u_ref * self.sim.vscale[1]);
        self.sim.vscale[2] = match self.kick {
            Some(k) if t >= k.start && t < k.start + k.duration => {
                let s = (t - k.start) as f64 / k.duration as f64;
                k.omega * (std::f64::consts::PI * s).sin().powi(2) * (2.0 * std::f64::consts::PI * s).sin().signum()
            }
            _ => 0.0,
        };
        self.sim.step();
    }

    /// Drag and lift coefficients of the body for the last step, 2 F / (rho u_ref^2 d).
    pub fn coefficients(&self, d: f64) -> (f64, f64) {
        let k = 2.0 / (self.u_ref * self.u_ref * d);
        (k * self.sim.force[0][0], k * self.sim.force[0][1])
    }
}

/// Force-coefficient history of a run, sampled every step.
#[derive(Clone, Debug, Default)]
pub struct Series {
    /// Convective time units (d / u_ref) per sample.
    pub dt: f64,
    pub cd: Vec<f64>,
    pub cl: Vec<f64>,
}

/// Statistics of the developed part of a force history.
#[derive(Clone, Copy, Debug)]
pub struct Stats {
    pub cd_mean: f64,
    pub cd_rms: f64,
    pub cl_mean: f64,
    pub cl_rms: f64,
    /// Strouhal number f d / u_ref from the lift spectrum; 0 if the lift does not oscillate.
    pub st: f64,
    /// Strouhal number from mean-crossings of the lift (independent cross-check).
    pub st_crossings: f64,
    /// Whether the lift oscillates regularly in the analysed window: at least three periods
    /// and the two frequency estimates within 5 % of each other. If not, `st` is 0.
    pub periodic: bool,
}

impl Series {
    /// Statistics over the samples from time `from` (in convective units) to the end.
    pub fn stats(&self, from: f64) -> Stats {
        let i0 = ((from / self.dt) as usize).min(self.cd.len().saturating_sub(2));
        let (cd, cl) = (&self.cd[i0..], &self.cl[i0..]);
        let cl_rms = crate::signal::rms(cl);
        let (mut st, mut st_crossings, mut periodic) = (0.0, 0.0, false);
        if cl_rms > 1e-6 {
            let f = crate::signal::dominant_frequency(cl, self.dt).0;
            st_crossings = crate::signal::crossing_frequency(cl, self.dt).unwrap_or(0.0);
            periodic = f * cl.len() as f64 * self.dt >= 3.0 && (st_crossings / f - 1.0).abs() < 0.05;
            if periodic {
                st = f;
            }
        }
        Stats {
            cd_mean: crate::signal::mean(cd),
            cd_rms: crate::signal::rms(cd),
            cl_mean: crate::signal::mean(cl),
            cl_rms,
            st,
            st_crossings,
            periodic,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Series;

    #[test]
    fn strouhal_is_reported_only_for_a_regular_oscillation() {
        let dt = 0.01;
        let wave = |f: f64, n: usize| -> Series {
            let cl: Vec<f64> = (0..n).map(|i| 0.4 * (2.0 * std::f64::consts::PI * f * i as f64 * dt).sin()).collect();
            Series { dt, cd: vec![1.5; n], cl }
        };
        // 0.16 cycles per time unit over 100 units, statistics from t = 50: eight periods.
        let s = wave(0.16, 10_000).stats(50.0);
        assert!(s.periodic && (s.st - 0.16).abs() < 1e-3 && (s.st_crossings - 0.16).abs() < 2e-3);
        assert!((s.cl_rms - 0.4 / 2f64.sqrt()).abs() < 5e-3 && (s.cd_mean - 1.5).abs() < 1e-12);
        // Only one and a half periods in the window: not enough to call it periodic.
        let s = wave(0.03, 10_000).stats(50.0);
        assert!(!s.periodic && s.st == 0.0);
        // A drifting, non-oscillating lift.
        let n = 5000;
        let drift = Series { dt, cd: vec![1.0; n], cl: (0..n).map(|i| (i as f64 / n as f64).powi(2)).collect() };
        assert!(!drift.stats(10.0).periodic);
        // No lift at all.
        let flat = Series { dt, cd: vec![1.0; n], cl: vec![0.0; n] };
        let s = flat.stats(10.0);
        assert!(!s.periodic && s.st == 0.0 && s.cl_rms == 0.0);
    }
}
