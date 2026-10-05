//! Regression tests against the published numbers, on grids small enough for CI.
//!
//! The tolerances are fixed and deliberately tighter than "it runs": each was set a little
//! above the error measured at that resolution, so a change that degrades accuracy fails.
//! Full-resolution numbers are produced by `make validate` and shown in docs/report.

use vortexlab::cases::tunnel::Tunnel;
use vortexlab::cases::{cylinder, poiseuille};
use vortexlab::geometry::{section, Corner};
use vortexlab::lattice::Collision;
use vortexlab::reference::{cavity_extremum, st_interval, st_later};
use vortexlab::suite::{self, TRT};

fn within(v: f64, reference: f64, rel: f64) -> bool {
    (v - reference).abs() <= rel * reference.abs()
}

#[test]
fn poiseuille_is_second_order_and_exact_with_the_magic_parameter() {
    let run = |collision, q| -> Vec<poiseuille::Outcome> { [8, 16, 32].iter().map(|&h| poiseuille::run(h, 0.8, collision, q, 1)).collect() };
    // BGK and TRT(1/4) with halfway walls, TRT(3/16) with walls off the halfway position:
    // error falls with the square of the resolution.
    for (collision, q) in [(Collision::Bgk, 0.5), (Collision::Trt { magic: 0.25 }, 0.5), (TRT, 0.25), (TRT, 0.8)] {
        let out = run(collision, q);
        for order in poiseuille::observed_orders(&out) {
            assert!((order - 2.0).abs() < 0.05, "{collision:?} q={q}: observed order {order}");
        }
        assert!(out[2].l2 < 2.5e-3, "{collision:?} q={q}: error {}", out[2].l2);
    }
    // TRT with Lambda = 3/16 and halfway walls reproduces the parabola to rounding.
    for o in run(TRT, 0.5) {
        assert!(o.l2 < 1e-10, "rows {}: error {}", o.rows, o.l2);
        assert!((o.force_balance - 1.0).abs() < 1e-9, "wall drag does not balance the driving force");
    }
}

#[test]
fn cavity_re100_matches_ghia() {
    let r = suite::cavity_run(33, 100, 0.1, 2);
    let s = |k: &str| -> f64 { r.summary[suite::CAVITY_HEADER.iter().position(|h| *h == k).unwrap()].parse().unwrap() };
    assert!(r.outcome.converged);
    // Ghia's table has five decimals; measured deviations at this size: 0.0039 (u), 0.0052 (v).
    assert!(s("max_dev_u") < 0.006 && s("max_dev_v") < 0.008, "deviation from Ghia: {} {}", s("max_dev_u"), s("max_dev_v"));
    // Extrema against the spectral values (measured: u_min 1.0 % off on this 33 x 33 grid).
    for (q, tol) in [("u_min", 0.015), ("v_max", 0.02), ("v_min", 0.02)] {
        let reference = cavity_extremum("botella-peyret", 100, q).unwrap().0;
        assert!(within(s(q), reference, tol), "{q}: {} vs {reference}", s(q));
    }
}

#[test]
fn cavity_re1000_matches_ghia() {
    let r = suite::cavity_run(65, 1000, 0.1, 2);
    let s = |k: &str| -> f64 { r.summary[suite::CAVITY_HEADER.iter().position(|h| *h == k).unwrap()].parse().unwrap() };
    assert!(r.outcome.converged);
    // Measured at 65 x 65: 0.017 (u), 0.009 (v); u_min 2.2 % from the spectral value.
    assert!(s("max_dev_u") < 0.022 && s("max_dev_v") < 0.013, "deviation from Ghia: {} {}", s("max_dev_u"), s("max_dev_v"));
    for (q, tol) in [("u_min", 0.03), ("v_max", 0.035), ("v_min", 0.035)] {
        let reference = cavity_extremum("botella-peyret", 1000, q).unwrap().0;
        assert!(within(s(q), reference, tol), "{q}: {} vs {reference}", s(q));
    }
}

#[test]
fn schaefer_turek_2d1_at_10_cells_per_diameter() {
    let o = cylinder::run_2d1(cylinder::Setup { n: 10, u_mean: 0.04, collision: TRT, threads: 2 }, 300.0, 1e-6);
    assert!(o.converged, "did not reach a steady state (residual {})", o.residual);
    // Measured: c_D 5.613 (+0.6 %), c_L 0.01050, dP 0.1133 (-3.6 %), L_a 0.0802 (-5.3 %).
    assert!(within(o.cd, st_later("2D-1", "cd").unwrap(), 0.01), "c_D {}", o.cd);
    assert!(within(o.cl, st_later("2D-1", "cl").unwrap(), 0.03), "c_L {}", o.cl);
    assert!(within(o.dp, st_later("2D-1", "dp").unwrap(), 0.045), "dP {}", o.dp);
    let (lo, hi) = st_interval("2D-1", "la").unwrap();
    assert!(within(o.la, 0.5 * (lo + hi), 0.065), "L_a {}", o.la);
}

#[test]
fn schaefer_turek_2d2_at_20_cells_per_diameter() {
    let o = cylinder::run_2d2(cylinder::Setup { n: 20, u_mean: 0.05, collision: TRT, threads: 2 }, 100.0, 20.0).unwrap();
    // Measured: c_Dmax 3.324 (+3.0 % from the later reference), c_Lmax 0.981 (-0.6 %),
    // St 0.2988 and dP 2.470 inside the 1996 intervals.
    assert!(within(o.cd_max, st_later("2D-2", "cd_max").unwrap(), 0.035), "c_Dmax {}", o.cd_max);
    assert!(within(o.cl_max, st_later("2D-2", "cl_max").unwrap(), 0.012), "c_Lmax {}", o.cl_max);
    assert!(within(o.cl_min, st_later("2D-2", "cl_min").unwrap(), 0.012), "c_Lmin {}", o.cl_min);
    let (lo, hi) = st_interval("2D-2", "st").unwrap();
    assert!(o.st > lo && o.st < hi, "St {}", o.st);
    assert!(within(o.st_crossings, o.st, 0.003), "the two Strouhal estimates disagree: {} {}", o.st, o.st_crossings);
    let (lo, hi) = st_interval("2D-2", "dp").unwrap();
    assert!(o.dp > lo && o.dp < hi, "dP {}", o.dp);
    // The flow has become periodic: lift maxima agree to 0.1 %.
    assert!(o.periods >= 5 && o.peak_spread < 1e-3, "{} periods, spread {}", o.periods, o.peak_spread);
    // 10 cells per diameter is under-resolved at Re = 100: the run must say so, not return numbers.
    let coarse = cylinder::run_2d2(cylinder::Setup { n: 10, u_mean: 0.05, collision: TRT, threads: 2 }, 30.0, 5.0);
    assert!(coarse.is_err_and(|e| e.contains("diverged")));
}

#[test]
fn rounding_the_corners_lowers_drag_and_raises_the_shedding_frequency() {
    // A small version of the corner experiment (20 cells per side, Re = 100, 10 % blockage).
    let tunnel = Tunnel { d: 20, re: 100.0, u: 0.1, upstream: 6.0, downstream: 12.0, height: 10.0, length: 1.0, collision: TRT, threads: 2, open_sides: true };
    let (cx, cy) = tunnel.centre();
    let run = |corner| tunnel.experiment(section(corner, cx, cy, 20.0, 0.0), 70.0, 40.0, false, &mut |_, _| {}).unwrap().stats;
    let (square, rounded) = (run(Corner::Sharp), run(Corner::Rounded(0.2)));
    println!("square {square:?}\nrounded {rounded:?}");
    // Plain square at Re = 100: published 2-D values are C_D ~ 1.46-1.53 and St ~ 0.145-0.150
    // at 5 % blockage or less; this run has 10 % blockage and a coarse grid.
    assert!(square.cd_mean > 1.45 && square.cd_mean < 1.75, "square C_D {}", square.cd_mean);
    assert!(square.st > 0.14 && square.st < 0.17, "square St {}", square.st);
    assert!(square.cl_rms > 0.1 && square.cl_rms < 0.35, "square C_L' {}", square.cl_rms);
    assert!(within(square.st_crossings, square.st, 0.01));
    assert!(square.cl_mean.abs() < 0.02, "mean lift of a symmetric body {}", square.cl_mean);
    assert!(rounded.cd_mean < 0.97 * square.cd_mean, "rounded C_D {} vs square {}", rounded.cd_mean, square.cd_mean);
    assert!(rounded.st > 1.02 * square.st, "rounded St {} vs square {}", rounded.st, square.st);
}
