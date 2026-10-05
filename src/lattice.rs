//! D2Q9 lattice: velocity set, equilibrium and the collision operators.
//!
//! Direction numbering (x to the right, y up):
//!
//! ```text
//!   6  2  5
//!    \ | /
//!   3--0--1
//!    / | \
//!   7  4  8
//! ```
//!
//! Lattice units throughout: dx = dt = 1, speed of sound squared `CS2` = 1/3,
//! kinematic viscosity nu = (tau - 1/2) / 3.

pub const Q: usize = 9;
pub const CX: [i32; Q] = [0, 1, 0, -1, 0, 1, -1, -1, 1];
pub const CY: [i32; Q] = [0, 0, 1, 0, -1, 1, 1, -1, -1];
pub const W: [f64; Q] = [4.0 / 9.0, 1.0 / 9.0, 1.0 / 9.0, 1.0 / 9.0, 1.0 / 9.0, 1.0 / 36.0, 1.0 / 36.0, 1.0 / 36.0, 1.0 / 36.0];
/// Index of the direction opposite to `i`.
pub const OPP: [usize; Q] = [0, 3, 4, 1, 2, 7, 8, 5, 6];
pub const CS2: f64 = 1.0 / 3.0;
/// The four (direction, opposite) pairs; TRT relaxes their sum and difference separately.
pub const PAIRS: [(usize, usize); 4] = [(1, 3), (2, 4), (5, 7), (6, 8)];

/// Collision model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Collision {
    /// Single relaxation time (Bhatnagar-Gross-Krook; Qian, d'Humieres & Lallemand 1992).
    Bgk,
    /// Two relaxation times (Ginzburg, Verhaeghe & d'Humieres 2008). `magic` is
    /// Lambda = (tau+ - 1/2)(tau- - 1/2); 3/16 puts a halfway bounce-back wall exactly
    /// midway between nodes for Poiseuille flow, 1/4 gives the best stability.
    Trt { magic: f64 },
}

impl Collision {
    /// (omega+, omega-) for a given tau = tau+ (the one that sets the viscosity).
    pub fn omegas(self, tau: f64) -> (f64, f64) {
        match self {
            Collision::Bgk => (1.0 / tau, 1.0 / tau),
            Collision::Trt { magic } => (1.0 / tau, 1.0 / (0.5 + magic / (tau - 0.5))),
        }
    }
}

/// Kinematic viscosity for a relaxation time.
pub fn viscosity(tau: f64) -> f64 {
    (tau - 0.5) * CS2
}

/// Relaxation time for a kinematic viscosity.
pub fn tau_for(nu: f64) -> f64 {
    nu / CS2 + 0.5
}

/// Density and momentum (rho, rho*ux, rho*uy) of a population set.
#[inline(always)]
pub fn moments(f: &[f64; Q]) -> (f64, f64, f64) {
    let rho = f[0] + f[1] + f[2] + f[3] + f[4] + f[5] + f[6] + f[7] + f[8];
    let jx = f[1] - f[3] + f[5] - f[6] - f[7] + f[8];
    let jy = f[2] - f[4] + f[5] + f[6] - f[7] - f[8];
    (rho, jx, jy)
}

/// Second-order (in velocity) Maxwell-Boltzmann equilibrium.
#[inline(always)]
pub fn equilibrium(rho: f64, ux: f64, uy: f64) -> [f64; Q] {
    let usq = 1.5 * (ux * ux + uy * uy);
    let mut out = [0.0; Q];
    for i in 0..Q {
        let cu = 3.0 * (CX[i] as f64 * ux + CY[i] as f64 * uy);
        out[i] = W[i] * rho * (1.0 + cu + 0.5 * cu * cu - usq);
    }
    out
}

/// BGK collision: every population relaxes towards equilibrium at rate `omega`.
#[inline(always)]
pub fn collide_bgk(f: &[f64; Q], omega: f64) -> [f64; Q] {
    let (rho, jx, jy) = moments(f);
    let inv = 1.0 / rho;
    let feq = equilibrium(rho, jx * inv, jy * inv);
    let mut out = [0.0; Q];
    for i in 0..Q {
        out[i] = f[i] - omega * (f[i] - feq[i]);
    }
    out
}

/// TRT collision: the symmetric part (f_i + f_opp)/2 relaxes at `op` (sets the viscosity),
/// the antisymmetric part (f_i - f_opp)/2 at `om` (free; tuned through the magic parameter).
#[inline(always)]
pub fn collide_trt(f: &[f64; Q], op: f64, om: f64) -> [f64; Q] {
    let (rho, jx, jy) = moments(f);
    let inv = 1.0 / rho;
    let (ux, uy) = (jx * inv, jy * inv);
    let usq = 1.5 * (ux * ux + uy * uy);
    let mut out = [0.0; Q];
    out[0] = f[0] - op * (f[0] - W[0] * rho * (1.0 - usq));
    for &(a, b) in &PAIRS {
        let cu = 3.0 * (CX[a] as f64 * ux + CY[a] as f64 * uy);
        let wr = W[a] * rho;
        let dp = op * (0.5 * (f[a] + f[b]) - wr * (1.0 + 0.5 * cu * cu - usq));
        let dm = om * (0.5 * (f[a] - f[b]) - wr * cu);
        out[a] = f[a] - dp - dm;
        out[b] = f[b] - dp + dm;
    }
    out
}

/// Collision with a uniform body acceleration (gx, gy), using the forcing of
/// Guo, Zheng & Shi (2002) split into symmetric/antisymmetric parts so that it is valid for
/// TRT as well (with `om == op` it is exactly Guo's BGK scheme).
///
/// The equilibrium is built on the half-step-corrected velocity u = (j + F/2)/rho, F = rho*g.
#[inline(always)]
pub fn collide_forced(f: &[f64; Q], op: f64, om: f64, gx: f64, gy: f64) -> [f64; Q] {
    let (rho, jx, jy) = moments(f);
    let inv = 1.0 / rho;
    let (ux, uy) = (jx * inv + 0.5 * gx, jy * inv + 0.5 * gy);
    let (fx, fy) = (rho * gx, rho * gy);
    let usq = 1.5 * (ux * ux + uy * uy);
    let uf = 3.0 * (ux * fx + uy * fy);
    let (kp, km) = (1.0 - 0.5 * op, 1.0 - 0.5 * om);
    let mut out = [0.0; Q];
    out[0] = f[0] - op * (f[0] - W[0] * rho * (1.0 - usq)) - kp * W[0] * uf;
    for &(a, b) in &PAIRS {
        let (cx, cy) = (CX[a] as f64, CY[a] as f64);
        let cu = 3.0 * (cx * ux + cy * uy);
        let cf = 3.0 * (cx * fx + cy * fy);
        let wr = W[a] * rho;
        let dp = op * (0.5 * (f[a] + f[b]) - wr * (1.0 + 0.5 * cu * cu - usq));
        let dm = om * (0.5 * (f[a] - f[b]) - wr * cu);
        let sp = kp * W[a] * (cu * cf - uf);
        let sm = km * W[a] * cf;
        out[a] = f[a] - dp - dm + sp + sm;
        out[b] = f[b] - dp + dm + sp - sm;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Small deterministic generator (xorshift64*) so tests need no dependency.
    pub struct Rng(pub u64);
    impl Rng {
        pub fn next(&mut self) -> f64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            (self.0.wrapping_mul(0x2545F4914F6CDD1D) >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    fn random_state(rng: &mut Rng) -> [f64; Q] {
        let rho = 0.8 + 0.4 * rng.next();
        let ux = 0.2 * (rng.next() - 0.5);
        let uy = 0.2 * (rng.next() - 0.5);
        let mut f = equilibrium(rho, ux, uy);
        for (i, v) in f.iter_mut().enumerate() {
            *v += 0.02 * W[i] * (rng.next() - 0.5); // non-equilibrium part
        }
        f
    }

    #[test]
    fn lattice_is_consistent() {
        assert!((W.iter().sum::<f64>() - 1.0).abs() < 1e-15);
        for i in 0..Q {
            assert_eq!(CX[OPP[i]], -CX[i]);
            assert_eq!(CY[OPP[i]], -CY[i]);
            assert_eq!(W[OPP[i]], W[i]);
        }
        // Isotropy up to fourth order: sum w c_a c_b = cs2 delta_ab, sum w cx^2 cy^2 = cs2^2.
        let s = |g: &dyn Fn(f64, f64) -> f64| -> f64 { (0..Q).map(|i| W[i] * g(CX[i] as f64, CY[i] as f64)).sum() };
        assert!(s(&|x, _| x).abs() < 1e-15);
        assert!((s(&|x, _| x * x) - CS2).abs() < 1e-15);
        assert!(s(&|x, y| x * y).abs() < 1e-15);
        assert!((s(&|x, y| x * x * y * y) - CS2 * CS2).abs() < 1e-15);
        assert!((s(&|x, _| x * x * x * x) - 3.0 * CS2 * CS2).abs() < 1e-15);
    }

    #[test]
    fn equilibrium_moments() {
        let mut rng = Rng(0x1234_5678_9abc_def1);
        for _ in 0..200 {
            let rho = 0.5 + rng.next();
            let ux = 0.3 * (rng.next() - 0.5);
            let uy = 0.3 * (rng.next() - 0.5);
            let f = equilibrium(rho, ux, uy);
            let (r, jx, jy) = moments(&f);
            assert!((r - rho).abs() < 1e-14);
            assert!((jx - rho * ux).abs() < 1e-14);
            assert!((jy - rho * uy).abs() < 1e-14);
            // Momentum flux: Pi_ab = rho cs2 delta_ab + rho u_a u_b.
            let pxx: f64 = (0..Q).map(|i| f[i] * (CX[i] * CX[i]) as f64).sum();
            let pxy: f64 = (0..Q).map(|i| f[i] * (CX[i] * CY[i]) as f64).sum();
            let pyy: f64 = (0..Q).map(|i| f[i] * (CY[i] * CY[i]) as f64).sum();
            assert!((pxx - rho * (CS2 + ux * ux)).abs() < 1e-14);
            assert!((pyy - rho * (CS2 + uy * uy)).abs() < 1e-14);
            assert!((pxy - rho * ux * uy).abs() < 1e-14);
        }
    }

    #[test]
    fn collisions_conserve_mass_and_momentum() {
        let mut rng = Rng(42);
        for k in 0..300 {
            let f = random_state(&mut rng);
            let tau = 0.51 + 1.5 * rng.next();
            let (op, om) = Collision::Trt { magic: 0.25 }.omegas(tau);
            let (r0, x0, y0) = moments(&f);
            for out in [collide_bgk(&f, op), collide_trt(&f, op, om)] {
                let (r, x, y) = moments(&out);
                assert!((r - r0).abs() < 1e-14, "mass, case {k}");
                assert!((x - x0).abs() < 1e-14 && (y - y0).abs() < 1e-14, "momentum, case {k}");
            }
        }
    }

    #[test]
    fn equilibrium_is_a_fixed_point() {
        let f = equilibrium(1.1, 0.07, -0.03);
        for out in [collide_bgk(&f, 1.7), collide_trt(&f, 1.7, 0.9)] {
            for i in 0..Q {
                assert!((out[i] - f[i]).abs() < 1e-15);
            }
        }
    }

    #[test]
    fn trt_with_equal_rates_is_bgk() {
        let mut rng = Rng(7);
        for _ in 0..200 {
            let f = random_state(&mut rng);
            let om = 0.6 + 1.3 * rng.next();
            let (a, b) = (collide_bgk(&f, om), collide_trt(&f, om, om));
            for i in 0..Q {
                assert!((a[i] - b[i]).abs() < 1e-15);
            }
        }
    }

    #[test]
    fn forced_collision_adds_exactly_the_force() {
        let mut rng = Rng(99);
        for _ in 0..200 {
            let f = random_state(&mut rng);
            let (gx, gy) = (1e-3 * (rng.next() - 0.5), 1e-3 * (rng.next() - 0.5));
            let (op, om) = Collision::Trt { magic: 3.0 / 16.0 }.omegas(0.5 + rng.next());
            let out = collide_forced(&f, op, om, gx, gy);
            let (r0, x0, y0) = moments(&f);
            let (r, x, y) = moments(&out);
            assert!((r - r0).abs() < 1e-14);
            assert!((x - x0 - r0 * gx).abs() < 1e-14);
            assert!((y - y0 - r0 * gy).abs() < 1e-14);
            // Zero force reduces to the plain operator.
            let (p, q) = (collide_forced(&f, op, om, 0.0, 0.0), collide_trt(&f, op, om));
            for i in 0..Q {
                assert!((p[i] - q[i]).abs() < 1e-15);
            }
        }
    }

    #[test]
    fn viscosity_round_trip() {
        assert!((viscosity(tau_for(0.0123)) - 0.0123).abs() < 1e-16);
        let (op, om) = Collision::Trt { magic: 0.1875 }.omegas(0.8);
        assert!(((1.0 / op - 0.5) * (1.0 / om - 0.5) - 0.1875).abs() < 1e-15);
    }
}
