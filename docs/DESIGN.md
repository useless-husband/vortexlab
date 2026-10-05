# Design

This document explains how vortexlab is built, which problems turned out to be hard, and which alternatives were
considered and rejected. Citations in square brackets refer to [REFERENCES.md](REFERENCES.md).

## 1. What the solver computes

The lattice-Boltzmann method (LBM) does not discretise the Navier-Stokes equations directly. It tracks, at every
grid node, nine numbers `f_0 … f_8`: how much fluid is moving in each of nine directions (rest, four axis
directions, four diagonals: the "D2Q9" lattice [Qian 1992]). One time step is

1. **stream**: every `f_i` hops one cell in its direction;
2. **collide**: at every node the nine values relax towards a local equilibrium that has the same mass and
   momentum.

Density is `rho = sum f_i`, momentum `rho u = sum c_i f_i`, pressure `p = rho / 3`. For small Mach numbers this
reproduces the incompressible Navier-Stokes equations with kinematic viscosity `nu = (tau - 1/2) / 3`, where
`tau` is the relaxation time [Krüger 2017].

Two collision operators are implemented (`src/lattice.rs`):

* **BGK**: all populations relax at one rate `1/tau`.
* **TRT** (two relaxation times [Ginzburg 2008a]): the symmetric part `(f_i + f_opp)/2` relaxes at `1/tau`, which
  sets the viscosity; the antisymmetric part `(f_i - f_opp)/2` at a second rate chosen through the "magic
  parameter" `Lambda = (tau - 1/2)(tau_minus - 1/2)`. With `Lambda = 3/16` a halfway bounce-back wall sits
  *exactly* halfway between nodes for channel flow, independent of viscosity. All production runs use TRT with
  `Lambda = 3/16`.

A uniform body force uses the scheme of [Guo 2002], split into symmetric and antisymmetric parts so that it is
valid for TRT too.

### Units

Everything inside the solver is in lattice units (cell = 1, step = 1, reference density = 1). A physical flow is
mapped through two choices (`src/units.rs`):

| choice | meaning | constraint |
|---|---|---|
| `D` cells across the body | spatial resolution, `dx = D_phys / D` | cost grows like `D^3` in 2-D (cells x steps) |
| lattice velocity `U` of the stream | time step, `dt = dx U / U_phys` | Mach number `U sqrt(3)` must be small: error ~ Ma² |

The Reynolds number `Re = U D / nu` must equal the physical one, which fixes `nu` and hence
`tau = 3 U D / Re + 1/2`. `tau` must stay above 1/2; close to 1/2 (below about 0.51) the flow is under-resolved.
Force coefficients, Strouhal number and `dp / (rho U²)` are dimensionless and carry over unchanged; the CLI prints
the same flow for a 1 cm body in air and in water as an example of the conversion back.

## 2. Architecture

```
            Builder (cells: fluid / wall kinds / open boundaries, shapes)
                │ build()
                ▼
 ┌────────────────────────── Sim (src/sim.rs) ──────────────────────────┐
 │  a: 9 x N populations (current)      b: 9 x N (next)                 │
 │                                                                      │
 │  step():                                                             │
 │   1. bulk rows, in parallel (src/pool.rs):  pull 9 values from a,    │
 │      collide, write the node in b          ── interior fluid nodes   │
 │   2. link pass, serial:                    ── nodes touching a       │
 │      for every cut link build the incoming population by             │
 │      interpolated bounce-back / moving wall / pressure rule,         │
 │      collide, write the node in b, add the momentum handed           │
 │      to the wall to its force group                                  │
 │   3. swap a and b                                                    │
 └──────────────────────────────────────────────────────────────────────┘
        ▲                         ▲                          ▲
 cases/poiseuille, cavity   cases/channel (inlet,       cases/tunnel (uniform stream,
 (closed or periodic)       outlet, body, ramp)         sound-absorbing sides), custom
        │                         │                          │
        └────────── suite.rs (CSV tables) ── report.rs (HTML + SVG) ──────┘
 signal.rs (FFT, peaks)   png.rs / gif.rs (codecs)   render.rs (colour map)   svg.rs (charts)
```

There are no dependencies; `Cargo.lock` lists only this crate.

### Everything at a boundary is a link

A *link* is a lattice direction that leaves a fluid node and ends in a non-fluid cell. All boundary conditions
are rules for the one population that would have arrived along that link:

| boundary | rule | source |
|---|---|---|
| solid wall, surface at fraction `q` of the link | linear interpolated bounce-back; `q = 1/2` is plain halfway bounce-back | [Bouzidi 2001] |
| moving wall (lid, inlet, tunnel walls, test bodies) | the same plus `6 w rho (c·u_w)` | [Ladd 1994a], [Lallemand 2003] |
| pressure outlet | anti-bounce-back around the boundary density | [Ginzburg 2008a] |

`q` is found by bisection on the body's `inside(x, y)` predicate (`src/geometry.rs`), so any shape that can
answer "is this point inside?" works: circles, polygons, rounded squares, or the 0.5-contour of a bilinearly
interpolated picture.

The inlet is a wall whose velocity is the inflow profile ("velocity bounce-back"). For a parabolic profile the
three links of an inlet node sample the profile with weights 1/6, 2/3, 1/6: Simpson's rule, so the discrete mass
flux equals the integral of the parabola exactly.

### Forces

The force on a body is the momentum its links hand over per step (momentum exchange [Ladd 1994a], [Mei 2002]):
for each link, `c (f_out + f_back)`. For walls that move, the Galilean-invariant form of [Wen 2014] is used,
`(c - u_w) f_out - (-c - u_w) f_back`. Tests: Couette flow observed from frames sliding along the walls at four
different speeds gives the same wall shear, equal to `rho nu dU/dy` to 1e-12; a body whose surface moves with a
uniform stream feels zero force to 1e-13 for oblique streams and arbitrary cut fractions. What is *not* tested is
a body that actually travels across the lattice; the solver has no moving-geometry support.

## 3. Hard problems

### 3.1 Bit-identical results for any thread count

Floating-point addition is not associative, so a parallel solver easily produces results that depend on the number
of threads. Here the design removes the possibility rather than managing it:

* Streaming is a *pull* from the old array into the new one. A node's new value is a pure function of the old
  array, so the order in which rows are processed, and which thread processes them, cannot matter.
* Everything that accumulates (forces, the outlet's mean velocity) is computed in the serial link pass, in the
  fixed order in which links were created.

`tests/solver.rs::results_do_not_depend_on_the_thread_count` runs the cylinder benchmark with 1, 2, 3, 4 and 7
threads and compares a hash of every population bit and the force bits; `vortexlab bench` prints the same hash.

### 3.2 Sound waves that would not die

LBM is a weakly *compressible* method, so the domain is also an acoustic cavity. The first tunnel runs showed the
drag of a square cylinder wandering by ±10 % with a period of about 17 convective time units: a plane sound wave
bouncing between the velocity inlet (a rigid wall for sound) and the constant-pressure outlet (a pressure-release
surface), with nothing but the tiny bulk viscosity to damp it.

The fix is a boundary that is matched to the wave instead of reflecting it. In a plane wave travelling towards the
outlet, density and velocity perturbations are tied by `rho' = rho u' / c_s`. Incompressibility says the
cross-section average of the outflow velocity must equal the inflow rate, so any difference between the two *is*
the acoustic `u'`. Each step the outlet density is therefore set to `1 + (<u_n> - U_in) / c_s`
(`Sim::outflow_target`). Vortices crossing the outlet do not change the average, so they are not affected. With
this, the same run settles to a periodic drag signal; `tests/solver.rs::absorbing_outlet_removes_sound_waves`
starts a channel impulsively and checks that the residual drag ripple is below 1 % and at least ten times smaller
than with the reflecting outlet. For the steady Schäfer-Turek case it cut the time to converge from 65 000 to
17 500 steps at 20 cells per diameter without changing the converged values.

A second resonance appeared in the corner study. An oscillating lift force drives a sound wave *across* the
tunnel. With solid side walls 20 D apart and a stream velocity of 0.1, the lowest cross mode has a period of 6.9
convective units and vortex shedding from a square at Re = 200 a period of 6.0: the lift amplitude beat between
0.31 and 0.76 (period about 60 units) and the RMS lift moved by 20 % from run to run. Halving the Mach number
removed it (constant amplitude) but doubles the cost. Instead the side boundaries are now open boundaries whose
density follows the *local* normal velocity, `1 + u_n / c_s` (`Kind::Radiating`). For sound this is a matched
termination; for the flow it is almost a wall, because a hydrodynamic pressure difference `dp` only drives
`u_n = dp / (rho c_s)` through it, a fraction of order Mach of what a free boundary would pass. The same run now
shows a lift amplitude constant to three digits.

Both are low-order characteristic boundary conditions. They are not perfect absorbers for oblique waves, and the
side boundaries are slightly leaky to the mean flow (of order Ma), which is one reason the blockage correction of
these runs is not exactly that of a solid-walled tunnel.

### 3.3 An unstable outlet

The textbook anti-bounce-back outlet extrapolates the wall velocity from two interior nodes. In the tunnel that
version blew up as soon as the first vortices reached the outlet (checkerboard noise spreading upstream at the
speed of sound). Using the velocity of the boundary node itself is first-order but stable; that is what is
implemented. A known side effect, covered by a test: the rule cannot carry shear through the boundary, so the
profile adjusts over the last few cells and the pressure level there is off by `O(rho u²)`. All measurements are
taken far from the outlet (at least 16 D).

### 3.4 Conservation to the last bit

A periodic box initially lost mass at about 1e-16 per node per step with BGK. That is "rounding", but it was
systematic: the weights 4/9, 1/9, 1/36 are not exact in binary, so the computed equilibrium did not sum to `rho`.
The rest population's equilibrium is now computed as the remainder `rho - sum(moving equilibria)`, which is
algebraically identical and makes the sum exact; the drift is now a few 1e-13 after 400 steps on 1728 nodes
(the test sums with compensated summation, otherwise the measurement would be noisier than the thing measured).

### 3.5 What "the inlet velocity" means in a compressible method

A velocity bounce-back inlet needs a density in its momentum term `6 w rho (c·u_w)`. With the reference density
`rho_0 = 1` the inlet prescribes the *mass flux* `rho_0 u`; with the local fluid density it prescribes the
*velocity*. The two differ because the density at the inlet is above 1 by the pressure drop along the channel.
Both were run on the Schäfer-Turek cases at 20 cells per diameter:

| | c_D (2D-1), U = 0.04 | U = 0.02 | extrapolated to Ma = 0 | c_Dmax (2D-2), U = 0.05 | U = 0.025 | extrapolated |
|---|---|---|---|---|---|---|
| local density | 5.667 | 5.612 | 5.593 | 3.362 | 3.335 | 3.326 |
| reference density | 5.589 | 5.582 | 5.580 | 3.324 | 3.321 | 3.320 |

(Extrapolation assumes an error proportional to Ma².) Both variants head for the same incompressible limit, but
prescribing the mass flux has a several times smaller compressibility error at a given Mach number: forces scale
with `rho u²`, and with the mass flux fixed the higher inlet density is partly offset by a lower velocity. The
reference-density form is the one implemented. The 10-cell 2D-2 run diverges with either.

### 3.6 Reading a frequency more finely than the FFT bin

A run long enough to hold 20 shedding periods has a frequency resolution of 5 %, useless against a reference
interval of ±1.7 %. The lift history is windowed (Hann), zero-padded eight-fold and the peak located by fitting a
parabola to the log-amplitude of the three highest bins (`signal::dominant_frequency`); a test recovers a
frequency placed between bins to 2e-4. As a check that shares no code with the FFT, the Strouhal number is also
computed from the mean spacing of upward mean-crossings and reported next to it.

### 3.7 Codecs without libraries

* PNG reading needs a full DEFLATE inflater (stored, fixed and dynamic Huffman blocks), CRC-32, Adler-32, five row
  filters, bit depths 1-16, palettes and transparency. The decoder is tested against files written by Python's
  zlib (`tools/make_png_fixtures.py`), i.e. against an independent encoder, and fed truncated and random input.
* PNG writing uses LZ77 with hash chains and the *fixed* Huffman code. Vorticity pictures are stored as palette
  indices, and because the colour map is ordered, the "Sub" row filter turns smooth regions into runs of small
  numbers; that recovers most of what dynamic Huffman coding would give for these pictures.
* GIF writing implements variable-width LZW with dictionary resets. Files were checked with ffmpeg once by hand;
  the automated test uses the crate's own decoder.

## 4. Trade-offs considered and rejected

| alternative | why not |
|---|---|
| MRT collision | TRT already fixes the wall-location error and stabilises low viscosity with one free parameter that has a clear meaning; MRT adds several more to tune and document. |
| Zou-He (non-equilibrium bounce-back) inlets/outlets | need special corner treatment and are less robust at low viscosity; expressing every boundary as a link rule keeps one code path and makes corners trivial. |
| in-place streaming (AA pattern, swap) | halves memory traffic, but makes the update order-dependent and determinism across threads much harder to argue. Memory is not the constraint at these sizes. |
| hand-written SIMD | the scalar kernel reaches about 100 million node updates per second per core on the development machine, already more than a published production code reports per core for its 3-D lattice (see README, Performance); not worth the unsafe surface. |
| convective (Orlanski) outlet | passes vortices well but pins no pressure level and does not absorb plane sound waves, which were the actual problem. |
| sponge layers | absorb short waves; the troublesome modes here have wavelengths several times the domain, where a thin sponge acts as a (reflecting) pressure-release surface. |
| incompressible (He-Luo) equilibrium | removes part of the Ma² error in steady flow but changes the meaning of density and pressure; a Mach-number sensitivity row in the validation tables is more transparent. |
| grid refinement / body-fitted meshes | the single largest accuracy lever left (the Schäfer-Turek runs spend most cells far from the cylinder), and the largest complexity jump. Out of scope. |
| WebAssembly front end | the target browser runs with WebAssembly disabled; the report is static files instead. |
| 3-D, turbulence models, GPU | future work; a validated 2-D core was the priority. |

## 5. Known limitations of the method as implemented

* Uniform grid, 2-D, laminar. No turbulence model.
* Linear interpolated bounce-back is second-order but does not conserve mass exactly; the outlet makes up the
  (tiny) difference. Where a link has no fluid node behind it (inside a notch one cell wide, in concave corners)
  the rule falls back to halfway bounce-back, which is locally first-order.
* Open boundaries are first-order and described in 3.2-3.3.
* The compressibility error is O(Ma²); every validation table has a row that halves the Mach number to show its
  size.
* PNG input must be non-interlaced. GIF output uses a fixed 202-colour palette.
