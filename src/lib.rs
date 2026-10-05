//! vortexlab: a small from-scratch 2-D lattice-Boltzmann wind tunnel.
//!
//! * [`lattice`]: D2Q9 constants, equilibrium, BGK / TRT collision.
//! * [`sim`]: grid, streaming, link-based boundary conditions, momentum-exchange forces.
//! * [`geometry`]: body shapes and link/surface intersection.
//! * [`pool`]: persistent worker threads.

// Index loops over the nine lattice directions (and over pixels) mirror the formulas they implement.
#![allow(clippy::needless_range_loop)]

pub mod cases;
pub mod geometry;
pub mod gif;
pub mod lattice;
pub mod png;
pub mod pool;
pub mod reference;
pub mod render;
pub mod report;
pub mod signal;
pub mod sim;
pub mod suite;
pub mod svg;
pub mod units;
