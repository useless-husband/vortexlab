# Changelog

## 0.1.0 - 2026-10-05

First version.

- D2Q9 lattice-Boltzmann solver with BGK and two-relaxation-time collision, Guo forcing, pull streaming on two
  arrays, persistent thread pool; results are bit-identical for any thread count.
- Link-based boundaries: interpolated bounce-back for curved bodies, velocity inlet, moving walls, pressure outlet
  that absorbs plane sound waves, sound-absorbing side boundaries.
- Momentum-exchange force evaluation; lift/drag histories; Strouhal number from the lift spectrum (own FFT) with a
  zero-crossing cross-check.
- Validation: Poiseuille flow (grid convergence), lid-driven cavity against Ghia, Ghia & Shin (1982), cylinder
  benchmark 2D-1 and 2D-2 of Schäfer & Turek (1996). Reference values with citations in `data/reference/`.
- Experiment: square section with sharp, chamfered, rounded, recessed and double-recessed corners.
- `vortexlab shape`: any dark-on-light PNG silhouette as a body; own PNG decoder/encoder and GIF encoder.
- Static HTML report (`docs/report/index.html`), generated from CSV results.
- Known limitations: 2-D, laminar, uniform grid, first-order open boundaries; case 2D-2 diverges at 10 cells per
  diameter. See README.
