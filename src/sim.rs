//! The solver: grid, streaming, boundaries, forces.
//!
//! # One time step
//!
//! Two copies of the populations are kept. `a` holds the post-collision populations of the
//! current time, `b` receives the next ones. For every fluid node the nine populations are
//! *pulled* from the upstream neighbours in `a`, collided, and written to `b`. A node's new
//! value therefore depends only on `a`, never on the order in which nodes are visited: any
//! split of the rows over threads gives bit-identical results.
//!
//! # Boundaries are links
//!
//! Every boundary condition here is expressed on *links*: a lattice link that leaves a fluid
//! node and ends in a non-fluid cell. Nodes owning at least one such link are handled in a
//! second, serial pass ([`Sim::link_pass`]) that replaces the population that would have come
//! out of the wall:
//!
//! * solid wall at fraction `q` along the link: interpolated bounce-back
//!   (Bouzidi, Firdaouss & Lallemand 2001), with the moving-wall term of Lallemand & Luo
//!   (2003); `q = 1/2` is the classic halfway bounce-back;
//! * prescribed velocity (inlet, lid, towing-tank walls): the same rule with a wall velocity;
//! * prescribed pressure (outlet): anti-bounce-back (Ginzburg, Verhaeghe & d'Humieres 2008),
//!   optionally with the boundary density adjusted so that plane sound waves leave
//!   ([`Sim::outflow_target`]).
//!
//! The same pass sums the momentum handed to each body (momentum-exchange method, Ladd 1994;
//! in the Galilean-invariant form of Wen et al. 2014), in a fixed link order.

use crate::geometry::{cut_fraction, Shape};
use crate::lattice::*;
use crate::pool::Pool;

/// Wall velocity as a function of the position of the wall point (lattice units).
pub type VelocityFn = Box<dyn Fn(f64, f64) -> [f64; 2] + Send + Sync>;

/// A solid wall: where it is (through the cells painted with it, refined by `shape`), how it
/// moves, and whether the force on it is recorded.
#[derive(Default)]
pub struct Wall {
    /// Wall velocity at unit scale; `None` for a wall at rest.
    pub velocity: Option<VelocityFn>,
    /// Index into [`Sim::vscale`]: the wall velocity is multiplied by that factor each step,
    /// which is how inlets are ramped up smoothly.
    pub vgroup: usize,
    /// Index into [`Sim::force`], if the hydrodynamic force on this wall should be summed.
    pub fgroup: Option<usize>,
    /// Exact outline. Without it the wall surface is assumed halfway along every cut link.
    pub shape: Option<Box<dyn Shape>>,
}

/// What a cell is.
pub enum Kind {
    Fluid,
    Wall(Wall),
    /// Open boundary held at this density (pressure = density / 3).
    Pressure(f64),
    /// Open boundary that is nearly a free-slip wall for the flow but lets sound out: its
    /// density is the given value plus u_n / c_s, with u_n the local outward normal velocity
    /// (the characteristic relation of an outgoing sound wave). A hydrodynamic pressure
    /// difference dp drives only u_n = dp / (rho c_s) through it, a fraction ~Mach of what a
    /// free boundary would pass, so at low Mach number the flow sees a wall.
    Radiating(f64),
}

pub const FLUID: u8 = 0;
/// Number of independent wall-velocity scale factors.
pub const VGROUPS: usize = 4;

/// Describes a domain before the solver is built.
pub struct Builder {
    pub nx: usize,
    pub ny: usize,
    pub periodic_x: bool,
    pub periodic_y: bool,
    pub tau: f64,
    pub collision: Collision,
    /// Uniform body acceleration.
    pub accel: [f64; 2],
    kinds: Vec<Kind>,
    cell: Vec<u8>,
}

impl Builder {
    pub fn new(nx: usize, ny: usize) -> Builder {
        assert!(nx >= 3 && ny >= 3, "grid must be at least 3 x 3");
        Builder {
            nx,
            ny,
            periodic_x: false,
            periodic_y: false,
            tau: 1.0,
            collision: Collision::Trt { magic: 3.0 / 16.0 },
            accel: [0.0, 0.0],
            kinds: vec![Kind::Fluid],
            cell: vec![FLUID; nx * ny],
        }
    }

    /// Registers a cell kind and returns its id for the `fill_*` calls.
    pub fn add(&mut self, kind: Kind) -> u8 {
        assert!(self.kinds.len() < 255);
        self.kinds.push(kind);
        (self.kinds.len() - 1) as u8
    }

    /// A wall at rest with no outline and no force bookkeeping.
    pub fn add_static_wall(&mut self) -> u8 {
        self.add(Kind::Wall(Wall::default()))
    }

    /// Paints the inclusive cell rectangle [x0, x1] x [y0, y1].
    pub fn fill_rect(&mut self, x0: usize, x1: usize, y0: usize, y1: usize, kind: u8) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.cell[y * self.nx + x] = kind;
            }
        }
    }

    /// Paints every cell whose centre lies inside the outline of wall `kind`.
    pub fn fill_shape(&mut self, kind: u8) {
        let Kind::Wall(Wall { shape: Some(shape), .. }) = &self.kinds[kind as usize] else {
            panic!("fill_shape needs a wall kind with an outline");
        };
        for y in 0..self.ny {
            for x in 0..self.nx {
                if shape.inside(x as f64, y as f64) {
                    self.cell[y * self.nx + x] = kind;
                }
            }
        }
    }

    pub fn build(self, threads: usize) -> Sim {
        Sim::from_builder(self, threads)
    }
}

const LINK_WALL: u8 = 0;
const LINK_PRESSURE: u8 = 1;
const NONE: usize = usize::MAX;

struct Link {
    /// Direction from the fluid node into the boundary.
    dir: u8,
    kind: u8,
    vgroup: u8,
    /// Fraction of the link in the fluid.
    q: f64,
    /// Node one step behind the fluid node (away from the wall), or `NONE` if not fluid.
    behind: usize,
    /// Force group, or `NONE`.
    fgroup: usize,
    /// Wall velocity at unit scale and the matching bounce-back correction 6 w (c . u_w).
    uw: [f64; 2],
    cu6: f64,
    /// Pressure links: boundary density.
    rho: f64,
    /// Pressure links: outward normal, and whether the density follows the local normal
    /// velocity (`Kind::Radiating`).
    normal: [i8; 2],
    radiating: bool,
}

struct BNode {
    idx: usize,
    /// Where each population is pulled from (unused for the cut directions).
    src: [usize; Q],
    links: std::ops::Range<usize>,
}

#[derive(Clone, Copy)]
struct ConstPtr(*const f64);
#[derive(Clone, Copy)]
struct MutPtr(*mut f64);
// SAFETY: used only inside `Sim::step`, where threads read `a` and write disjoint rows of `b`.
unsafe impl Sync for ConstPtr {}
unsafe impl Sync for MutPtr {}

pub struct Sim {
    pub nx: usize,
    pub ny: usize,
    n: usize,
    periodic: (bool, bool),
    a: Vec<f64>,
    b: Vec<f64>,
    /// 0 = interior fluid, 1 = fluid with at least one boundary link, 2 = not fluid.
    flag: Vec<u8>,
    bnodes: Vec<BNode>,
    links: Vec<Link>,
    omega: (f64, f64),
    collision: Collision,
    accel: [f64; 2],
    pool: Pool,
    /// Per-group multiplier applied to wall velocities; set by the caller before `step`.
    pub vscale: [f64; VGROUPS],
    /// Force on each force group during the last step (lattice units, momentum per step).
    pub force: Vec<[f64; 2]>,
    /// Makes pressure boundaries absorb plane sound waves instead of reflecting them.
    ///
    /// With `Some(u)`, the boundary density is raised by (<u_n> - u) / c_s each step, where
    /// <u_n> is the outward normal velocity averaged over all nodes next to the pressure
    /// boundary and `u` is the mean outflow velocity the incompressible flow must have (the
    /// inflow rate). The difference is non-zero only while a sound wave is passing, and
    /// rho' = u' / c_s is exactly the relation in a wave travelling outwards, so the wave
    /// leaves; vortices crossing the boundary do not change the average and are not affected.
    /// With `None` the boundary density is fixed and sound waves are reflected.
    pub outflow_target: Option<f64>,
    /// The density offset applied in the last step (diagnostic).
    pub outlet_density_offset: f64,
    /// Nodes next to a pressure boundary with the outward normal.
    pnodes: Vec<(usize, i8, i8)>,
    /// Steps taken.
    pub t: u64,
    fluid_nodes: usize,
}

impl Sim {
    fn from_builder(b: Builder, threads: usize) -> Sim {
        let (nx, ny) = (b.nx, b.ny);
        let n = nx * ny;
        assert!(b.tau > 0.5, "tau must exceed 1/2 (viscosity must be positive)");
        let wrap = |x: isize, y: isize| -> Option<usize> {
            let x = if x < 0 || x >= nx as isize {
                if !b.periodic_x {
                    return None;
                }
                x.rem_euclid(nx as isize)
            } else {
                x
            };
            let y = if y < 0 || y >= ny as isize {
                if !b.periodic_y {
                    return None;
                }
                y.rem_euclid(ny as isize)
            } else {
                y
            };
            Some(y as usize * nx + x as usize)
        };

        let mut flag = vec![2u8; n];
        let mut bnodes = Vec::new();
        let mut links: Vec<Link> = Vec::new();
        let mut ngroups = 0usize;
        let mut pnodes: Vec<(usize, i8, i8)> = Vec::new();
        let mut fluid_nodes = 0usize;
        for y in 0..ny {
            for x in 0..nx {
                let idx = y * nx + x;
                if b.cell[idx] != FLUID {
                    continue;
                }
                fluid_nodes += 1;
                let first = links.len();
                let mut src = [idx; Q];
                for j in 1..Q {
                    let (cx, cy) = (CX[j] as isize, CY[j] as isize);
                    let target = wrap(x as isize + cx, y as isize + cy).unwrap_or_else(|| {
                        panic!("fluid cell ({x},{y}) touches a non-periodic edge; close the domain with wall or pressure cells")
                    });
                    // Population OPP[j] arrives from `target`.
                    src[OPP[j]] = target;
                    let kind = b.cell[target];
                    if kind == FLUID {
                        continue;
                    }
                    let behind = match wrap(x as isize - cx, y as isize - cy) {
                        Some(i) if b.cell[i] == FLUID => i,
                        _ => NONE,
                    };
                    let mut link =
                        Link { dir: j as u8, kind: LINK_WALL, vgroup: 0, q: 0.5, behind, fgroup: NONE, uw: [0.0, 0.0], cu6: 0.0, rho: 1.0, normal: [0, 0], radiating: false };
                    match &b.kinds[kind as usize] {
                        Kind::Fluid => unreachable!(),
                        Kind::Pressure(rho) | Kind::Radiating(rho) => {
                            link.kind = LINK_PRESSURE;
                            link.rho = *rho;
                            link.radiating = matches!(&b.kinds[kind as usize], Kind::Radiating(_));
                            // Outward normal: along x if the cell beside the node in x is open
                            // boundary too, otherwise along y.
                            let open = |i: Option<usize>| i.map(|i| matches!(b.kinds[b.cell[i] as usize], Kind::Pressure(_) | Kind::Radiating(_))) == Some(true);
                            link.normal = if open(wrap(x as isize + cx, y as isize)) { [cx as i8, 0] } else { [0, cy as i8] };
                            if !link.radiating && pnodes.last().map(|p: &(usize, i8, i8)| p.0) != Some(idx) {
                                pnodes.push((idx, link.normal[0], link.normal[1]));
                            }
                        }
                        Kind::Wall(wall) => {
                            assert!(wall.vgroup < VGROUPS);
                            link.vgroup = wall.vgroup as u8;
                            if let Some(shape) = &wall.shape {
                                link.q = cut_fraction(shape.as_ref(), x as f64, y as f64, cx as f64, cy as f64);
                            }
                            if let Some(vel) = &wall.velocity {
                                let (wx, wy) = (x as f64 + link.q * cx as f64, y as f64 + link.q * cy as f64);
                                link.uw = vel(wx, wy);
                                link.cu6 = 6.0 * W[j] * (cx as f64 * link.uw[0] + cy as f64 * link.uw[1]);
                            }
                            if let Some(g) = wall.fgroup {
                                link.fgroup = g;
                                ngroups = ngroups.max(g + 1);
                            }
                        }
                    }
                    links.push(link);
                }
                if links.len() > first {
                    flag[idx] = 1;
                    bnodes.push(BNode { idx, src, links: first..links.len() });
                } else {
                    flag[idx] = 0;
                }
            }
        }

        let mut sim = Sim {
            nx,
            ny,
            n,
            periodic: (b.periodic_x, b.periodic_y),
            a: vec![0.0; Q * n],
            b: vec![0.0; Q * n],
            flag,
            bnodes,
            links,
            omega: b.collision.omegas(b.tau),
            collision: b.collision,
            accel: b.accel,
            pool: Pool::new(threads),
            vscale: [1.0; VGROUPS],
            force: vec![[0.0, 0.0]; ngroups],
            outflow_target: None,
            outlet_density_offset: 0.0,
            pnodes,
            t: 0,
            fluid_nodes,
        };
        sim.init(|_, _| (1.0, 0.0, 0.0));
        sim
    }

    /// Sets every fluid node to the equilibrium of the given (rho, ux, uy) field.
    pub fn init(&mut self, field: impl Fn(usize, usize) -> (f64, f64, f64)) {
        let n = self.n;
        for y in 0..self.ny {
            for x in 0..self.nx {
                let idx = y * self.nx + x;
                let (rho, ux, uy) = if self.flag[idx] == 2 { (1.0, 0.0, 0.0) } else { field(x, y) };
                let feq = equilibrium(rho, ux, uy);
                for i in 0..Q {
                    self.a[i * n + idx] = feq[i];
                    self.b[i * n + idx] = feq[i];
                }
            }
        }
        self.t = 0;
    }

    pub fn threads(&self) -> usize {
        self.pool.threads()
    }

    pub fn fluid_nodes(&self) -> usize {
        self.fluid_nodes
    }

    pub fn is_fluid(&self, x: usize, y: usize) -> bool {
        self.flag[y * self.nx + x] != 2
    }

    /// Number of boundary links (all kinds).
    pub fn link_count(&self) -> usize {
        self.links.len()
    }

    /// Advances one time step.
    pub fn step(&mut self) {
        let (n, nx, ny) = (self.n, self.nx, self.ny);
        let (op, om) = self.omega;
        let [gx, gy] = self.accel;
        let forced = gx != 0.0 || gy != 0.0;
        let collision = self.collision;
        let a = ConstPtr(self.a.as_ptr());
        let b = MutPtr(self.b.as_mut_ptr());
        let flag = &self.flag[..];
        let nt = self.pool.threads();
        self.pool.run(&|t| {
            let (a, b) = (a, b);
            for y in t * ny / nt..(t + 1) * ny / nt {
                // SAFETY: `row` reads `a` and writes only row `y` of `b`; each thread owns a
                // distinct range of rows, and all indices are below 9 * n.
                unsafe {
                    if forced {
                        row(a.0, b.0, flag, n, nx, ny, y, &|f| collide_forced(f, op, om, gx, gy));
                    } else if collision == Collision::Bgk {
                        row(a.0, b.0, flag, n, nx, ny, y, &|f| collide_bgk(f, op));
                    } else {
                        row(a.0, b.0, flag, n, nx, ny, y, &|f| collide_trt(f, op, om));
                    }
                }
            }
        });
        self.link_pass();
        std::mem::swap(&mut self.a, &mut self.b);
        self.t += 1;
    }

    /// Boundary nodes: build the incoming populations with the boundary rules, collide, and
    /// accumulate the momentum exchanged with each force group. Serial and in a fixed order,
    /// so the force sums do not depend on the thread count.
    fn link_pass(&mut self) {
        let n = self.n;
        let (op, om) = self.omega;
        let [gx, gy] = self.accel;
        let collision = self.collision;
        let Sim { a, b, bnodes, links, force, vscale, pnodes, outflow_target, outlet_density_offset, .. } = self;
        let a = &a[..];
        let velocity = |idx: usize| -> (f64, f64) {
            let mut f = [0.0; Q];
            for i in 0..Q {
                f[i] = a[i * n + idx];
            }
            let (rho, jx, jy) = moments(&f);
            (jx / rho - 0.5 * gx, jy / rho - 0.5 * gy)
        };
        for f in force.iter_mut() {
            *f = [0.0, 0.0];
        }
        // Plane-wave absorbing outlet: see `outflow_target`.
        let drho = match *outflow_target {
            Some(target) if !pnodes.is_empty() => {
                let mut sum = 0.0;
                for &(idx, nx, ny) in pnodes.iter() {
                    let (ux, uy) = velocity(idx);
                    sum += nx as f64 * ux + ny as f64 * uy;
                }
                (sum / pnodes.len() as f64 - target) / CS2.sqrt()
            }
            _ => 0.0,
        };
        *outlet_density_offset = drho;
        for bn in bnodes.iter() {
            let idx = bn.idx;
            let mut f = [0.0; Q];
            for i in 0..Q {
                f[i] = a[i * n + bn.src[i]];
            }
            for l in &links[bn.links.clone()] {
                let j = l.dir as usize;
                let i = OPP[j];
                let out = a[j * n + idx];
                let scale = vscale[l.vgroup as usize];
                let back = if l.kind == LINK_WALL {
                    // Momentum given to the fluid by a moving wall: 6 w rho (c . u_w), with the
                    // local fluid density standing in for the density at the wall.
                    let corr = if l.cu6 != 0.0 { l.cu6 * scale * (f[0] + a[idx + n] + a[idx + 2 * n] + a[idx + 3 * n] + a[idx + 4 * n] + a[idx + 5 * n] + a[idx + 6 * n] + a[idx + 7 * n] + a[idx + 8 * n]) } else { 0.0 };
                    if l.q >= 0.5 {
                        let k = 0.5 / l.q;
                        k * (out - corr) + (1.0 - k) * a[i * n + idx]
                    } else if l.behind != NONE {
                        2.0 * l.q * out + (1.0 - 2.0 * l.q) * a[j * n + l.behind] - corr
                    } else {
                        // No fluid node behind (corner or gap one cell wide): plain bounce-back.
                        out - corr
                    }
                } else {
                    // Anti-bounce-back: reflects with opposite sign around the equilibrium of
                    // the boundary density, using the local velocity as the wall velocity.
                    let (ux, uy) = velocity(idx);
                    let cu = 3.0 * (CX[j] as f64 * ux + CY[j] as f64 * uy);
                    let rho_w = if l.radiating { l.rho + (l.normal[0] as f64 * ux + l.normal[1] as f64 * uy) / CS2.sqrt() } else { l.rho + drho };
                    -out + 2.0 * W[j] * rho_w * (1.0 + 0.5 * cu * cu - 1.5 * (ux * ux + uy * uy))
                };
                f[i] = back;
                if l.fgroup != NONE {
                    let (ux, uy) = (l.uw[0] * scale, l.uw[1] * scale);
                    let g = &mut force[l.fgroup];
                    g[0] += CX[j] as f64 * (out + back) - ux * (out - back);
                    g[1] += CY[j] as f64 * (out + back) - uy * (out - back);
                }
            }
            let o = if gx != 0.0 || gy != 0.0 {
                collide_forced(&f, op, om, gx, gy)
            } else if collision == Collision::Bgk {
                collide_bgk(&f, op)
            } else {
                collide_trt(&f, op, om)
            };
            for i in 0..Q {
                b[i * n + idx] = o[i];
            }
        }
    }

    fn pops(&self, idx: usize) -> [f64; Q] {
        let mut f = [0.0; Q];
        for i in 0..Q {
            f[i] = self.a[i * self.n + idx];
        }
        f
    }

    #[inline]
    fn velocity_at(&self, idx: usize) -> (f64, f64) {
        let (rho, jx, jy) = moments(&self.pops(idx));
        // Stored populations are post-collision: with a body force they carry j + F, and the
        // physical velocity is (j + F/2) / rho.
        (jx / rho - 0.5 * self.accel[0], jy / rho - 0.5 * self.accel[1])
    }

    /// (rho, ux, uy) at a node; (1, 0, 0) for non-fluid cells.
    pub fn macros(&self, x: usize, y: usize) -> (f64, f64, f64) {
        let idx = y * self.nx + x;
        if self.flag[idx] == 2 {
            return (1.0, 0.0, 0.0);
        }
        let (rho, jx, jy) = moments(&self.pops(idx));
        (rho, jx / rho - 0.5 * self.accel[0], jy / rho - 0.5 * self.accel[1])
    }

    /// Sum of density over fluid nodes (compensated summation, so that conservation can be
    /// checked to rounding of the solver rather than of this sum).
    pub fn total_mass(&self) -> f64 {
        let mut m = Neumaier::default();
        for idx in 0..self.n {
            if self.flag[idx] != 2 {
                for i in 0..Q {
                    m.add(self.a[i * self.n + idx]);
                }
            }
        }
        m.value()
    }

    /// Sum of (post-collision) momentum over fluid nodes.
    pub fn total_momentum(&self) -> [f64; 2] {
        let (mut px, mut py) = (Neumaier::default(), Neumaier::default());
        for idx in 0..self.n {
            if self.flag[idx] != 2 {
                for i in 1..Q {
                    let f = self.a[i * self.n + idx];
                    px.add(CX[i] as f64 * f);
                    py.add(CY[i] as f64 * f);
                }
            }
        }
        [px.value(), py.value()]
    }

    /// Largest velocity magnitude in the fluid; NaN if the run has blown up.
    pub fn max_speed(&self) -> f64 {
        let mut m = 0.0f64;
        for idx in 0..self.n {
            if self.flag[idx] != 2 {
                let (ux, uy) = self.velocity_at(idx);
                let s = (ux * ux + uy * uy).sqrt();
                if s.is_nan() {
                    return f64::NAN;
                }
                m = m.max(s);
            }
        }
        m
    }

    /// Vorticity dv/dx - du/dy by central differences (one-sided next to walls, 0 in solids).
    pub fn vorticity(&self) -> Vec<f32> {
        let (nx, ny) = (self.nx, self.ny);
        let mut u = vec![(0.0f64, 0.0f64); self.n];
        for (idx, v) in u.iter_mut().enumerate() {
            if self.flag[idx] != 2 {
                *v = self.velocity_at(idx);
            }
        }
        let mut w = vec![0.0f32; self.n];
        let fluid =
            |x: isize, y: isize| x >= 0 && y >= 0 && x < nx as isize && y < ny as isize && self.flag[y as usize * nx + x as usize] != 2;
        for y in 0..ny as isize {
            for x in 0..nx as isize {
                if !fluid(x, y) {
                    continue;
                }
                // d(field)/d(axis) using whichever neighbours are fluid.
                let diff = |dx: isize, dy: isize, comp: usize| -> f64 {
                    let get = |x: isize, y: isize| {
                        let v = u[y as usize * nx + x as usize];
                        if comp == 0 {
                            v.0
                        } else {
                            v.1
                        }
                    };
                    let (p, m) = (fluid(x + dx, y + dy), fluid(x - dx, y - dy));
                    match (p, m) {
                        (true, true) => 0.5 * (get(x + dx, y + dy) - get(x - dx, y - dy)),
                        (true, false) => get(x + dx, y + dy) - get(x, y),
                        (false, true) => get(x, y) - get(x - dx, y - dy),
                        _ => 0.0,
                    }
                };
                w[y as usize * nx + x as usize] = (diff(1, 0, 1) - diff(0, 1, 0)) as f32;
            }
        }
        w
    }

    /// Density interpolated bilinearly at an arbitrary point, using only fluid nodes of the
    /// surrounding cell (weights renormalised if some corners are solid).
    pub fn probe_density(&self, x: f64, y: f64) -> f64 {
        let (i, j) = (x.floor(), y.floor());
        let (fx, fy) = (x - i, y - j);
        let (mut sum, mut wsum) = (0.0, 0.0);
        for (di, dj, w) in [(0, 0, (1.0 - fx) * (1.0 - fy)), (1, 0, fx * (1.0 - fy)), (0, 1, (1.0 - fx) * fy), (1, 1, fx * fy)] {
            let (xi, yj) = (i as isize + di, j as isize + dj);
            if xi < 0 || yj < 0 || xi >= self.nx as isize || yj >= self.ny as isize {
                continue;
            }
            let idx = yj as usize * self.nx + xi as usize;
            if self.flag[idx] != 2 && w > 0.0 {
                sum += w * moments(&self.pops(idx)).0;
                wsum += w;
            }
        }
        if wsum > 0.0 {
            sum / wsum
        } else {
            f64::NAN
        }
    }

    /// Density extrapolated to a wall point (x, y) from three samples taken 1, 2 and 3 lattice
    /// units away along the outward unit normal (nx, ny) (quadratic extrapolation).
    pub fn probe_density_at_wall(&self, x: f64, y: f64, nx: f64, ny: f64) -> f64 {
        let p = |d: f64| self.probe_density(x + d * nx, y + d * ny);
        3.0 * p(1.0) - 3.0 * p(2.0) + p(3.0)
    }

    /// FNV-1a hash of the bit patterns of all fluid populations (for determinism checks).
    pub fn state_hash(&self) -> u64 {
        let mut h = 0xcbf29ce484222325u64;
        for idx in 0..self.n {
            if self.flag[idx] == 2 {
                continue;
            }
            for i in 0..Q {
                for byte in self.a[i * self.n + idx].to_bits().to_le_bytes() {
                    h ^= byte as u64;
                    h = h.wrapping_mul(0x100000001b3);
                }
            }
        }
        h
    }

    pub fn periodic(&self) -> (bool, bool) {
        self.periodic
    }
}

/// Kahan-Babuska-Neumaier compensated summation.
#[derive(Default)]
struct Neumaier {
    sum: f64,
    comp: f64,
}

impl Neumaier {
    fn add(&mut self, x: f64) {
        let t = self.sum + x;
        self.comp += if self.sum.abs() >= x.abs() { (self.sum - t) + x } else { (x - t) + self.sum };
        self.sum = t;
    }
    fn value(&self) -> f64 {
        self.sum + self.comp
    }
}

/// Stream-and-collide for the interior fluid nodes of one row.
///
/// # Safety
/// `a` and `b` must point to `9 * n` values, `n = nx * ny`, `y < ny`, and no other thread may
/// write row `y` of `b`.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
unsafe fn row<C: Fn(&[f64; Q]) -> [f64; Q]>(
    a: *const f64,
    b: *mut f64,
    flag: &[u8],
    n: usize,
    nx: usize,
    ny: usize,
    y: usize,
    collide: &C,
) {
    let ym = if y == 0 { ny - 1 } else { y - 1 };
    let yp = if y == ny - 1 { 0 } else { y + 1 };
    let (base, bm, bp) = (y * nx, ym * nx, yp * nx);
    for x in 0..nx {
        if *flag.get_unchecked(base + x) != 0 {
            continue;
        }
        let xm = if x == 0 { nx - 1 } else { x - 1 };
        let xp = if x == nx - 1 { 0 } else { x + 1 };
        let f = [
            *a.add(base + x),
            *a.add(n + base + xm),
            *a.add(2 * n + bm + x),
            *a.add(3 * n + base + xp),
            *a.add(4 * n + bp + x),
            *a.add(5 * n + bm + xm),
            *a.add(6 * n + bm + xp),
            *a.add(7 * n + bp + xp),
            *a.add(8 * n + bp + xm),
        ];
        let o = collide(&f);
        for (i, v) in o.iter().enumerate() {
            *b.add(i * n + base + x) = *v;
        }
    }
}
