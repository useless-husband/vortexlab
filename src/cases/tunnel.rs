//! The "towing tank": a body in a uniform stream between walls that move with the stream.
//! Used for the corner-modification experiment and for user-supplied shapes.

use crate::cases::channel::{Channel, Inflow, Kick, Series, Spec};
use crate::geometry::Shape;
use crate::lattice::Collision;
use crate::sim::Sim;

#[derive(Clone, Copy, Debug)]
pub struct Tunnel {
    /// Cells across the body's frontal width (the reference length D).
    pub d: usize,
    /// Reynolds number U D / nu.
    pub re: f64,
    /// Stream velocity in lattice units.
    pub u: f64,
    /// Distance from the inlet to the body centre, in D.
    pub upstream: f64,
    /// Distance from the body centre to the outlet, in D.
    pub downstream: f64,
    /// Channel height in D; the blockage ratio is 1 / height.
    pub height: f64,
    pub collision: Collision,
    pub threads: usize,
}

impl Tunnel {
    pub fn grid(&self) -> (usize, usize) {
        let d = self.d as f64;
        (((self.upstream + self.downstream) * d).round() as usize + 2, (self.height * d).round() as usize + 2)
    }

    /// Where the body centre goes (lattice coordinates).
    pub fn centre(&self) -> (f64, f64) {
        let d = self.d as f64;
        (0.5 + (self.upstream * d).round(), 0.5 + 0.5 * (self.height * d).round())
    }

    pub fn steps_per_unit(&self) -> f64 {
        self.d as f64 / self.u
    }

    /// Inflow ramp of 5 convective units, then a 2-unit twist of the body (peak surface speed
    /// 0.2 U) to trigger vortex shedding.
    pub fn build(&self, body: Box<dyn Shape>) -> Channel {
        let (nx, ny) = self.grid();
        let (cx, cy) = self.centre();
        let unit = self.steps_per_unit();
        let spec = Spec {
            nx,
            ny,
            inflow: Inflow::Uniform { u: self.u },
            nu: self.u * self.d as f64 / self.re,
            collision: self.collision,
            body,
            ramp_steps: (5.0 * unit) as u64,
            kick: Some(Kick { cx, cy, omega: 0.4 * self.u / self.d as f64, start: (5.0 * unit) as u64, duration: (2.0 * unit) as u64 }),
        };
        Channel::new(spec, self.threads)
    }

    /// Runs `units` convective time units, recording the force coefficients every step and
    /// calling `on_frame` every `frame_every` steps (0 = never). Fails if the run blows up.
    pub fn run(&self, ch: &mut Channel, units: f64, frame_every: usize, on_frame: &mut dyn FnMut(&Sim)) -> Result<Series, String> {
        let unit = self.steps_per_unit();
        let steps = (units * unit) as usize;
        let mut series = Series { dt: 1.0 / unit, cd: Vec::with_capacity(steps), cl: Vec::with_capacity(steps) };
        for i in 0..steps {
            ch.step();
            let (cd, cl) = ch.coefficients(self.d as f64);
            if !cd.is_finite() {
                return Err(format!(
                    "the simulation became unstable after {} steps (tau = {:.4}); use more cells per body width, a lower Reynolds number or a lower velocity",
                    i,
                    crate::lattice::tau_for(self.u * self.d as f64 / self.re)
                ));
            }
            series.cd.push(cd);
            series.cl.push(cl);
            if frame_every != 0 && (i + 1) % frame_every == 0 {
                on_frame(&ch.sim);
            }
        }
        Ok(series)
    }
}
