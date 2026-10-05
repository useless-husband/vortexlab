//! The "towing tank": a body in a uniform stream between side boundaries that move with the
//! stream (solid walls, or sound-absorbing boundaries that behave almost like them).
//! Used for the corner-modification experiment and for user-supplied shapes.

use crate::cases::channel::{Channel, Inflow, Kick, Series, Spec, Stats};
use crate::geometry::Shape;
use crate::lattice::Collision;
use crate::render::{self, View};
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
    /// Streamwise length of the body in D (1 for the compact sections); widens the pictures.
    pub length: f64,
    pub collision: Collision,
    pub threads: usize,
    /// Side boundaries: `true` = sound-absorbing ([`Inflow::UniformOpen`]), `false` = solid
    /// walls moving with the stream.
    pub open_sides: bool,
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
            inflow: if self.open_sides { Inflow::UniformOpen { u: self.u } } else { Inflow::Uniform { u: self.u } },
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

/// Everything one tunnel run produces.
pub struct Outcome {
    pub series: Series,
    /// Statistics of the part after the discarded start-up.
    pub stats: Stats,
    /// Vorticity at the end of the run, as palette indices (see [`render::palette`]).
    pub picture: Vec<u8>,
    /// One shedding period of vorticity frames (empty if not requested or nothing sheds).
    pub frames: Vec<Vec<u8>>,
    /// Width and height of the picture and of every frame.
    pub size: (usize, usize),
    pub steps: u64,
    pub seconds: f64,
    /// Million lattice (fluid node) updates per second, wall-clock, including analysis.
    pub mlups: f64,
}

impl Outcome {
    pub fn png(&self) -> Vec<u8> {
        crate::png::encode(self.size.0, self.size.1, crate::png::Pixels::Indexed(&self.picture, &render::palette()))
    }

    /// Looping animation of one shedding period; `None` if there are no frames.
    pub fn gif(&self) -> Option<Vec<u8>> {
        if self.frames.is_empty() {
            return None;
        }
        let mut g = crate::gif::Writer::new(self.size.0, self.size.1, &render::palette());
        for f in &self.frames {
            g.frame(f, 7);
        }
        Some(g.finish())
    }
}

/// Vorticity drawn in the darkest colour, in units of U/D.
pub const VORTICITY_FULL_SCALE: f64 = 3.0;
/// Frames per shedding period in the animations.
pub const FRAMES_PER_PERIOD: usize = 20;

impl Tunnel {
    /// The part of the tunnel shown in pictures: from 2 D upstream of the body's nose to
    /// 11.5 D downstream of its tail, 3.2 D to either side, at most 480 pixels wide.
    pub fn view(&self) -> View {
        let (nx, ny) = self.grid();
        let (cx, cy) = self.centre();
        let d = self.d as f64;
        let clampx = |v: f64| (v.round().max(1.0) as usize).min(nx - 1);
        let clampy = |v: f64| (v.round().max(1.0) as usize).min(ny - 1);
        let (x0, x1) = (clampx(cx - (2.0 + 0.5 * self.length) * d), clampx(cx + (11.5 + 0.5 * self.length) * d));
        View { x0, x1, y0: clampy(cy - 3.2 * d), y1: clampy(cy + 3.2 * d), zoom: (480.0 / (x1 - x0) as f64).min(2.0) }
    }

    /// Full experiment: run `units` convective time units, compute statistics after the
    /// first `discard` units, draw the final vorticity field and, if `animate`, run one more
    /// shedding period to record a seamless loop. `progress` is called 20 times with the
    /// fraction done and the history so far.
    pub fn experiment(
        &self,
        body: Box<dyn Shape>,
        units: f64,
        discard: f64,
        animate: bool,
        progress: &mut dyn FnMut(f64, &Series),
    ) -> Result<Outcome, String> {
        let start = std::time::Instant::now();
        let mut ch = self.build(body);
        let mut series = Series { dt: 1.0 / self.steps_per_unit(), cd: Vec::new(), cl: Vec::new() };
        for k in 0..20 {
            let part = self.run(&mut ch, units / 20.0, 0, &mut |_| {})?;
            series.cd.extend(part.cd);
            series.cl.extend(part.cl);
            progress((k + 1) as f64 / 20.0, &series);
        }
        let stats = series.stats(discard);
        let view = self.view();
        let scale = VORTICITY_FULL_SCALE * self.u / self.d as f64;
        let picture = render::vorticity_frame(&ch.sim, view, scale);
        let mut frames = Vec::new();
        if animate && stats.st > 0.0 {
            let every = (self.steps_per_unit() / stats.st / FRAMES_PER_PERIOD as f64).round().max(1.0) as usize;
            let extra = (every * FRAMES_PER_PERIOD) as f64 / self.steps_per_unit();
            self.run(&mut ch, extra, every, &mut |sim| frames.push(render::vorticity_frame(sim, view, scale)))?;
            frames.truncate(FRAMES_PER_PERIOD);
        }
        let seconds = start.elapsed().as_secs_f64();
        let steps = ch.sim.t;
        Ok(Outcome {
            series,
            stats,
            picture,
            frames,
            size: view.size(),
            steps,
            seconds,
            mlups: ch.sim.fluid_nodes() as f64 * steps as f64 / seconds / 1e6,
        })
    }
}
