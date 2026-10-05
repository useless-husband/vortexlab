//! Solver-level tests: conservation, boundary conditions on flows with known solutions,
//! force evaluation, determinism.

use vortexlab::cases::channel::{Channel, Inflow, Spec};
use vortexlab::geometry::{section, Circle, Corner, Shape};
use vortexlab::lattice::{viscosity, Collision, CS2};
use vortexlab::sim::{Builder, Kind, Sim, Wall};

const TRT: Collision = Collision::Trt { magic: 3.0 / 16.0 };
const MODELS: [Collision; 3] = [Collision::Bgk, TRT, Collision::Trt { magic: 0.25 }];

fn swirl(nx: usize, ny: usize) -> impl Fn(usize, usize) -> (f64, f64, f64) {
    move |x, y| {
        let (a, b) = (2.0 * std::f64::consts::PI * x as f64 / nx as f64, 2.0 * std::f64::consts::PI * y as f64 / ny as f64);
        (1.0 + 0.02 * (a + 2.0 * b).sin(), 0.05 * b.sin() + 0.01, -0.04 * a.cos() + 0.02 * (a - b).sin())
    }
}

#[test]
fn periodic_box_conserves_mass_and_momentum() {
    for collision in MODELS {
        for threads in [1, 3] {
            let mut b = Builder::new(48, 36);
            (b.periodic_x, b.periodic_y, b.tau, b.collision) = (true, true, 0.62, collision);
            let mut sim = b.build(threads);
            sim.init(swirl(48, 36));
            let (m0, p0) = (sim.total_mass(), sim.total_momentum());
            for _ in 0..400 {
                sim.step();
            }
            let (m1, p1) = (sim.total_mass(), sim.total_momentum());
            println!("{collision:?} x{threads}: mass drift {:e}, momentum drift {:e} {:e}", m1 - m0, p1[0] - p0[0], p1[1] - p0[1]);
            // 1728 nodes x 9 populations x 400 steps of unbiased rounding: a few 1e-13.
            assert!((m1 - m0).abs() < 2e-12, "{collision:?}: mass drift {}", m1 - m0);
            assert!((p1[0] - p0[0]).abs() < 1e-12 && (p1[1] - p0[1]).abs() < 1e-12, "{collision:?}: momentum drift {:?}", [p1[0] - p0[0], p1[1] - p0[1]]);
            // The flow itself must have changed (viscous decay), or the test proves nothing.
            assert!(sim.max_speed() < 0.07);
        }
    }
}

#[test]
fn body_force_adds_exactly_its_impulse_in_a_periodic_box() {
    let mut b = Builder::new(20, 16);
    (b.periodic_x, b.periodic_y, b.tau, b.collision, b.accel) = (true, true, 0.7, TRT, [3e-5, -1e-5]);
    let mut sim = b.build(1);
    sim.init(swirl(20, 16));
    let (m, p0) = (sim.total_mass(), sim.total_momentum());
    for _ in 0..250 {
        sim.step();
    }
    let p1 = sim.total_momentum();
    assert!((p1[0] - p0[0] - 250.0 * 3e-5 * m).abs() < 1e-11);
    assert!((p1[1] - p0[1] + 250.0 * 1e-5 * m).abs() < 1e-11);
}

#[test]
fn closed_box_with_halfway_walls_conserves_mass() {
    for collision in MODELS {
        let mut b = Builder::new(34, 26);
        (b.tau, b.collision) = (0.58, collision);
        let wall = b.add_static_wall();
        b.fill_rect(0, 33, 0, 25, wall);
        b.fill_rect(1, 32, 1, 24, vortexlab::sim::FLUID);
        let mut sim = b.build(2);
        sim.init(swirl(34, 26));
        let m0 = sim.total_mass();
        for _ in 0..600 {
            sim.step();
        }
        assert!((sim.total_mass() - m0).abs() < 2e-12, "{collision:?}: {:e}", sim.total_mass() - m0);
        assert!(sim.max_speed() < 0.05, "walls must slow the flow down");
    }
}

/// Plane Couette flow between two walls, seen from a frame in which both walls carry an
/// extra tangential velocity `frame`. Returns the shear force per unit length on the lower
/// and upper wall, the pressure force on the lower wall and the largest deviation of the
/// velocity profile from the exact linear one.
fn couette(frame: f64, shear: f64, collision: Collision) -> (f64, f64, f64, f64) {
    let (nx, rows) = (6, 20);
    let mut b = Builder::new(nx, rows + 2);
    (b.periodic_x, b.tau, b.collision) = (true, 0.9, collision);
    let low = b.add(Kind::Wall(Wall { velocity: Some(Box::new(move |_, _| [frame, 0.0])), fgroup: Some(0), ..Wall::default() }));
    let top = b.add(Kind::Wall(Wall { velocity: Some(Box::new(move |_, _| [frame + shear, 0.0])), fgroup: Some(1), ..Wall::default() }));
    b.fill_rect(0, nx - 1, 0, 0, low);
    b.fill_rect(0, nx - 1, rows + 1, rows + 1, top);
    let mut sim = b.build(1);
    let exact = |y: usize| frame + shear * (y as f64 - 0.5) / rows as f64;
    sim.init(|_, y| (1.0, exact(y), 0.0));
    for _ in 0..20_000 {
        sim.step();
    }
    let dev = (1..=rows).map(|y| (sim.macros(2, y).1 - exact(y)).abs()).fold(0.0, f64::max);
    (sim.force[0][0] / nx as f64, sim.force[1][0] / nx as f64, sim.force[0][1] / nx as f64, dev)
}

#[test]
fn wall_shear_is_galilean_invariant() {
    // The same shear flow observed from frames sliding along the walls at different speeds
    // must give the same wall forces: tau_wall = rho nu dU/dy, p = rho c_s^2.
    let shear = 0.04;
    let expected = viscosity(0.9) * shear / 20.0;
    for collision in [Collision::Bgk, TRT] {
        for frame in [0.0, 0.03, -0.05, 0.08] {
            let (low, top, normal, dev) = couette(frame, shear, collision);
            assert!(dev < 1e-12, "profile not linear in frame {frame}: {dev}");
            assert!((low - expected).abs() < 1e-12 * 1.0f64.max(1.0 / expected) * expected + 1e-13, "{collision:?} frame {frame}: lower wall shear {low} vs {expected}");
            assert!((top + expected).abs() < 1e-13, "{collision:?} frame {frame}: upper wall shear {top}");
            // Pressure pushes the lower wall down with rho c_s^2 per unit length.
            assert!((normal + CS2).abs() < 1e-12, "{collision:?} frame {frame}: normal force {normal}");
        }
    }
}

#[test]
fn body_moving_with_the_stream_feels_no_force() {
    // A body whose surface moves with a uniform stream does not disturb it: the stream stays
    // exactly uniform and the momentum-exchange force vanishes, whatever the stream's
    // direction and wherever the surface cuts the lattice links.
    let shapes: Vec<(&str, Box<dyn Fn() -> Box<dyn Shape>>)> = vec![
        ("circle", Box::new(|| Box::new(Circle { cx: 20.3, cy: 15.7, r: 6.4 }))),
        ("notched square", Box::new(|| section(Corner::DoubleRecessed(0.1), 20.5, 16.5, 12.0, 0.3))),
    ];
    for (name, make) in &shapes {
        for u in [[0.06, 0.0], [0.0, -0.05], [0.04, 0.03], [-0.07, 0.02]] {
            for collision in [Collision::Bgk, TRT] {
                let mut b = Builder::new(40, 32);
                (b.periodic_x, b.periodic_y, b.tau, b.collision) = (true, true, 0.6, collision);
                let body = b.add(Kind::Wall(Wall { velocity: Some(Box::new(move |_, _| u)), fgroup: Some(0), shape: Some(make()), ..Wall::default() }));
                b.fill_shape(body);
                let mut sim = b.build(1);
                sim.init(|_, _| (1.0, u[0], u[1]));
                for _ in 0..200 {
                    sim.step();
                }
                let f = sim.force[0];
                assert!(f[0].abs() < 1e-13 && f[1].abs() < 1e-13, "{name} in stream {u:?} ({collision:?}): force {f:?}");
                let (_, ux, uy) = sim.macros(3, 3);
                assert!((ux - u[0]).abs() < 1e-14 && (uy - u[1]).abs() < 1e-14, "{name}: stream disturbed");
            }
        }
    }
}

#[test]
fn body_at_rest_in_fluid_at_rest_feels_no_force() {
    let mut b = Builder::new(40, 32);
    (b.periodic_x, b.periodic_y) = (true, true);
    let body = b.add(Kind::Wall(Wall { fgroup: Some(0), shape: Some(Box::new(Circle { cx: 19.2, cy: 16.9, r: 7.3 })), ..Wall::default() }));
    b.fill_shape(body);
    let mut sim = b.build(1);
    for _ in 0..50 {
        sim.step();
    }
    assert!(sim.force[0][0].abs() < 1e-14 && sim.force[0][1].abs() < 1e-14);
    assert!(sim.max_speed() < 1e-15);
}

/// An empty channel between a velocity inlet and a pressure outlet.
fn open_channel(nx: usize, rows: usize, inflow: Inflow, tau: f64) -> Sim {
    let mut b = Builder::new(nx, rows + 2);
    (b.tau, b.collision) = (tau, TRT);
    let h = rows as f64;
    let profile = move |y: f64| match inflow {
        Inflow::Uniform { u } => u,
        Inflow::Parabolic { u_mean } => 6.0 * u_mean * (y - 0.5) * (h - (y - 0.5)) / (h * h),
    };
    let inlet = b.add(Kind::Wall(Wall { velocity: Some(Box::new(move |_, y| [profile(y), 0.0])), ..Wall::default() }));
    let outlet = b.add(Kind::Pressure(1.0));
    let walls = match inflow {
        Inflow::Parabolic { .. } => b.add_static_wall(),
        Inflow::Uniform { u } => b.add(Kind::Wall(Wall { velocity: Some(Box::new(move |_, _| [u, 0.0])), ..Wall::default() })),
    };
    b.fill_rect(0, 0, 0, rows + 1, inlet);
    b.fill_rect(nx - 1, nx - 1, 0, rows + 1, outlet);
    b.fill_rect(0, nx - 1, 0, 0, walls);
    b.fill_rect(0, nx - 1, rows + 1, rows + 1, walls);
    let mut sim = b.build(2);
    sim.init(|_, y| (1.0, profile(y as f64), 0.0));
    sim
}

#[test]
fn uniform_stream_passes_through_inlet_and_outlet_unchanged() {
    // Inlet, moving walls and pressure outlet must all be transparent to a uniform stream.
    let mut sim = open_channel(40, 16, Inflow::Uniform { u: 0.07 }, 0.55);
    for _ in 0..500 {
        sim.step();
    }
    for (x, y) in [(1, 1), (20, 8), (38, 16), (38, 1)] {
        let (rho, ux, uy) = sim.macros(x, y);
        assert!((rho - 1.0).abs() < 1e-13 && (ux - 0.07).abs() < 1e-13 && uy.abs() < 1e-13, "at ({x},{y}): {rho} {ux} {uy}");
    }
}

#[test]
fn inlet_and_outlet_drive_developed_channel_flow() {
    // Parabolic inflow + constant-pressure outflow: the flow must stay parabolic and the
    // pressure must fall linearly at the Poiseuille rate dp/dx = -12 rho nu u_mean / H^2.
    let (nx, rows, u_mean, tau) = (82, 20, 0.005, 0.8);
    let mut sim = open_channel(nx, rows, Inflow::Parabolic { u_mean }, tau);
    for _ in 0..40_000 {
        sim.step();
    }
    let h = rows as f64;
    // The density falls along the channel (that is the pressure gradient), so the velocity
    // rises slightly; what is conserved, and parabolic, is the mass flux rho * u.
    // `flux` is the midpoint-rule mean over the node rows, which overestimates the mean of a
    // parabola vanishing at the walls by the factor 1 + 1/(2 H^2).
    let flux = |x: usize| (1..=rows).map(|y| sim.macros(x, y).0 * sim.macros(x, y).1).sum::<f64>() / h / (1.0 + 0.5 / (h * h));
    let mean_flux = flux(40);
    let mut worst = 0.0f64;
    for y in 1..=rows {
        let exact = 6.0 * mean_flux * (y as f64 - 0.5) * (h - y as f64 + 0.5) / (h * h);
        let (rho, ux, uy) = sim.macros(40, y);
        worst = worst.max((rho * ux - exact).abs()).max(uy.abs());

    }
    println!("profile deviation / u_mean = {:e}", worst / u_mean);
    assert!(worst < 2e-4 * u_mean, "profile deviates by {worst}");
    let slope = CS2 * (sim.macros(60, 10).0 - sim.macros(20, 10).0) / 40.0;
    let exact = -12.0 * viscosity(tau) * mean_flux / (h * h);
    assert!((slope / exact - 1.0).abs() < 1e-3, "pressure gradient {slope} vs {exact}");
    // The outlet holds the density at 1, but only to within the dynamic pressure: the
    // anti-bounce-back rule cannot carry the shear of the profile through the boundary, so
    // the last few cells adjust and the pressure there is off by O(rho u^2).
    let rho_out = sim.macros(nx - 2, 10).0;
    let u_max = 1.5 * u_mean;
    assert!((rho_out - 1.0).abs() < 6.0 * u_max * u_max, "outlet density {rho_out}");
    // Mass flux in = mass flux out, and the inlet imposes the prescribed velocity.
    assert!((flux(2) / flux(nx - 3) - 1.0).abs() < 1e-6);
    let u_in = (1..=rows).map(|y| sim.macros(1, y).1).sum::<f64>() / h;
    assert!((u_in / u_mean - 1.0).abs() < 2e-3, "inlet velocity {u_in}");
}

fn cylinder_channel(threads: usize) -> Channel {
    let n = 10.0;
    let spec = Spec {
        nx: 222,
        ny: 43,
        inflow: Inflow::Parabolic { u_mean: 0.05 },
        nu: 0.05 * n / 100.0,
        collision: TRT,
        body: Box::new(Circle { cx: 20.5, cy: 20.5, r: 5.0 }),
        ramp_steps: 100,
        kick: None,
    };
    Channel::new(spec, threads)
}

#[test]
fn results_do_not_depend_on_the_thread_count() {
    let mut reference: Option<(u64, [u64; 2])> = None;
    for threads in [1, 1, 2, 3, 4, 7] {
        let mut ch = cylinder_channel(threads);
        assert_eq!(ch.sim.threads(), threads);
        for _ in 0..400 {
            ch.step();
        }
        let got = (ch.sim.state_hash(), [ch.sim.force[0][0].to_bits(), ch.sim.force[0][1].to_bits()]);
        match &reference {
            None => reference = Some(got),
            Some(r) => assert_eq!(&got, r, "run with {threads} threads differs bit-for-bit"),
        }
    }
    // And the flow is not trivial: there is a drag force.
    let mut ch = cylinder_channel(2);
    for _ in 0..400 {
        ch.step();
    }
    assert!(ch.coefficients(10.0).0 > 1.0);
}

#[test]
fn absorbing_outlet_removes_sound_waves() {
    // Start the channel impulsively (no ramp): this launches a strong plane sound wave.
    // With the reflecting outlet it keeps bouncing; with the absorbing outlet it leaves.
    let ripple = |absorbing: bool| -> f64 {
        let spec = Spec {
            nx: 302,
            ny: 42,
            inflow: Inflow::Uniform { u: 0.05 },
            nu: 0.02,
            collision: TRT,
            body: Box::new(Circle { cx: 60.5, cy: 20.5, r: 4.0 }),
            ramp_steps: 1, // inflow jumps from 0 to full speed while the fluid is at rest
            kick: None,
        };
        let mut ch = Channel::new(spec, 2);
        ch.sim.init(|_, _| (1.0, 0.0, 0.0));
        ch.absorbing_outlet = absorbing;
        for _ in 0..6000 {
            ch.step();
        }
        // Peak-to-peak drag over the next 1200 steps (more than one acoustic round trip).
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for _ in 0..1200 {
            ch.step();
            let cd = ch.coefficients(8.0).0;
            lo = lo.min(cd);
            hi = hi.max(cd);
        }
        (hi - lo) / hi
    };
    let (reflecting, absorbing) = (ripple(false), ripple(true));
    assert!(absorbing < 0.01, "drag still ripples by {absorbing} with the absorbing outlet");
    assert!(reflecting > 10.0 * absorbing, "reflecting {reflecting} vs absorbing {absorbing}");
}
