//! Lattice units <-> physical units.
//!
//! The solver works in lattice units: cell size 1, time step 1, reference density 1. A
//! simulation is tied to a physical flow by two choices,
//!
//! * the cell size `dx` (metres per cell), i.e. how many cells resolve the body, and
//! * the lattice velocity `u_lat` that represents the physical reference velocity,
//!
//! which fix the time step `dt = dx * u_lat / u_phys`. What must match between the two worlds
//! is the Reynolds number `Re = U L / nu`; what must stay small is the Mach number
//! `Ma = u_lat / c_s` with `c_s = 1/sqrt(3)`, because the method is only weakly compressible
//! and its error grows like Ma^2 (staying below about 0.1-0.17 is customary). The remaining
//! knob, the relaxation time `tau = 3 nu_lat + 1/2`, follows; it must stay above 1/2, and close
//! to 1/2 (below about 0.51) the scheme loses robustness.

use crate::lattice::{tau_for, CS2};

#[derive(Clone, Copy, Debug)]
pub struct Units {
    /// Metres per cell.
    pub dx: f64,
    /// Seconds per step.
    pub dt: f64,
    /// Physical density (kg/m^3) of lattice density 1.
    pub rho: f64,
}

impl Units {
    /// `l_cells` cells span the physical length `l_phys`; lattice speed `u_lat` is `u_phys`.
    pub fn new(l_phys: f64, l_cells: f64, u_phys: f64, u_lat: f64, rho: f64) -> Units {
        let dx = l_phys / l_cells;
        Units { dx, dt: dx * u_lat / u_phys, rho }
    }
    pub fn length(&self, cells: f64) -> f64 {
        cells * self.dx
    }
    pub fn time(&self, steps: f64) -> f64 {
        steps * self.dt
    }
    pub fn velocity(&self, u_lat: f64) -> f64 {
        u_lat * self.dx / self.dt
    }
    pub fn viscosity(&self, nu_lat: f64) -> f64 {
        nu_lat * self.dx * self.dx / self.dt
    }
    pub fn lattice_viscosity(&self, nu_phys: f64) -> f64 {
        nu_phys * self.dt / (self.dx * self.dx)
    }
    /// Pressure difference in Pa for a lattice density difference (p = c_s^2 rho).
    pub fn pressure(&self, drho_lat: f64) -> f64 {
        CS2 * drho_lat * self.rho * (self.dx / self.dt).powi(2)
    }
    /// Force per unit span (N/m) for a 2-D lattice force (momentum per step).
    pub fn force_per_span(&self, f_lat: f64) -> f64 {
        f_lat * self.rho * self.dx.powi(3) / (self.dt * self.dt)
    }
}

pub fn reynolds(u: f64, l: f64, nu: f64) -> f64 {
    u * l / nu
}

pub fn mach(u_lat: f64) -> f64 {
    u_lat / CS2.sqrt()
}

/// Relaxation time needed for Reynolds number `re` with lattice velocity `u_lat` and a body of
/// `l_cells` cells.
pub fn tau_for_reynolds(re: f64, u_lat: f64, l_cells: f64) -> f64 {
    tau_for(u_lat * l_cells / re)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schaefer_turek_2d2_numbers() {
        // D = 0.1 m resolved by 40 cells, mean velocity 1 m/s represented by 0.05.
        let u = Units::new(0.1, 40.0, 1.0, 0.05, 1.0);
        assert!((u.dx - 0.0025).abs() < 1e-15);
        assert!((u.dt - 1.25e-4).abs() < 1e-15);
        assert!((u.velocity(0.05) - 1.0).abs() < 1e-12);
        let nu_lat = u.lattice_viscosity(1e-3);
        assert!((nu_lat - 0.02).abs() < 1e-12);
        assert!((reynolds(0.05, 40.0, nu_lat) - 100.0).abs() < 1e-9);
        assert!((tau_for_reynolds(100.0, 0.05, 40.0) - 0.56).abs() < 1e-12);
        assert!((u.viscosity(nu_lat) - 1e-3).abs() < 1e-15);
        // Dimensionless groups survive the conversion: Cd from lattice or physical numbers.
        let f_lat = 0.123;
        let cd_lat = 2.0 * f_lat / (0.05 * 0.05 * 40.0);
        let cd_phys = 2.0 * u.force_per_span(f_lat) / (1.0 * 1.0 * 1.0 * 0.1);
        assert!((cd_lat - cd_phys).abs() < 1e-12);
        let dp_lat = 0.004 * CS2;
        assert!((u.pressure(0.004) - dp_lat / (0.05 * 0.05)).abs() < 1e-12);
        assert!((mach(0.05) - 0.0866).abs() < 1e-4);
    }
}
