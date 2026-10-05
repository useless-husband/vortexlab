//! The experiments: each module sets up a domain, runs it and returns numbers.

pub mod cavity;
pub mod channel;
pub mod cylinder;
pub mod poiseuille;
pub mod tunnel;

/// Smooth 0 -> 1 ramp (cubic smoothstep) used to start inlets without a pressure shock.
pub fn ramp(t: u64, steps: u64) -> f64 {
    if steps == 0 || t >= steps {
        1.0
    } else {
        let s = t as f64 / steps as f64;
        s * s * (3.0 - 2.0 * s)
    }
}
