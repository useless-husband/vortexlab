//! vortexlab: a small from-scratch 2-D lattice-Boltzmann wind tunnel.
//!
//! * [`lattice`]: D2Q9 constants, equilibrium, BGK / TRT collision.
//! * [`sim`]: grid, streaming, link-based boundary conditions, momentum-exchange forces.
//! * [`geometry`]: body shapes and link/surface intersection.
//! * [`pool`]: persistent worker threads.

pub mod cases;
pub mod geometry;
pub mod gif;
pub mod lattice;
pub mod png;
pub mod pool;
pub mod render;
pub mod signal;
pub mod sim;
pub mod units;
