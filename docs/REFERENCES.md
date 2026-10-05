# References

Every entry was looked up on 2026-10-05 (bibliographic data through the Crossref / OpenAlex APIs; values from the
documents named). "Read" says what was actually read; where only a secondary source was available, that is stated.
Reference *values* live in `data/reference/*.csv`.

## Benchmarks

**[Ghia 1982]** U. Ghia, K. N. Ghia, C. T. Shin. "High-Re solutions for incompressible flow using the
Navier-Stokes equations and a multigrid method." *Journal of Computational Physics* 48 (1982) 387-411.
doi:10.1016/0021-9991(82)90058-4.
Read: Tables I and II (pp. 398-399) from a scan of the paper, compared digit by digit with three independent
transcriptions. One widely copied transcription has −0.51500 for Table II, Re = 1000, x = 0.9063; the paper prints
−0.51550. The Re = 400, 3200 and 10 000 columns contain entries that look like misprints; they are not used here.

**[Botella 1998]** O. Botella, R. Peyret. "Benchmark spectral results on the lid-driven cavity flow." *Computers &
Fluids* 27 (1998) 421-433. doi:10.1016/S0045-7930(98)00002-4.
Not read (paywalled). Its extrema are quoted second-hand from **[Marchi 2009]**.

**[Marchi 2009]** C. H. Marchi, R. Suero, L. K. Araki. "The lid-driven square cavity flow: numerical solution with
a 1024 × 1024 grid." *Journal of the Brazilian Society of Mechanical Sciences and Engineering* 31 (2009) 186-198.
Read: Tables 9 and 11-13, which list the Botella-Peyret and Ghia extrema next to their own.

**[Schäfer 1996]** M. Schäfer, S. Turek (with F. Durst, E. Krause, R. Rannacher). "Benchmark computations of
laminar flow around a cylinder." In E. H. Hirschel (ed.), *Flow Simulation with High-Performance Computers II*,
Notes on Numerical Fluid Mechanics 52, Vieweg, 1996, pp. 547-566. doi:10.1007/978-3-322-89849-4_39.
Read: the authors' preprint (archived copy of the TU Dortmund PDF): definitions in section 2, reference intervals
in the last rows of Tables 3 and 4. The volume number 52 is as given on the TU Dortmund benchmark pages; Crossref
lists none.

**[FeatFlow]** TU Dortmund, FeatFlow CFD benchmarking pages, "DFG flow around cylinder benchmark 2D-1, laminar
case Re = 20" and "2D-2, time-periodic case Re = 100".
2D-1: the page lists C_D = 5.57953523384, C_L = 0.010618948146, Δp = 0.11752016697, attributed to G. Nabh, "On
high order methods for the stationary incompressible Navier-Stokes equations", Univ. Heidelberg, Preprint 42/98,
1998 (not read). 2D-2: the page offers a "new reference results" data file (20141124_dfg_bench2_official.zip);
the values in `data/reference/schaefer_turek1996.csv` are the plain maxima/minima over its level-6, Δt = 1/1600 s
series (max C_D 3.227393, min C_D 3.164264, max C_L 0.986571, min C_L −1.021288), the pressure difference half a
period after the lift maximum (2.4848) and the Strouhal number 0.30184 from the period 0.3313 s. These were
extracted for this project, they are not printed on the page.

**[Sohankar 1998]** A. Sohankar, C. Norberg, L. Davidson. "Low-Reynolds-number flow around a square cylinder at
incidence: study of blockage, onset of vortex shedding and outlet boundary condition." *International Journal for
Numerical Methods in Fluids* 26 (1998) 39-56.
Read: Tables III and IV from the authors' copy of the paper.

## Wind engineering and Taipei 101

**[Irwin 2008]** P. A. Irwin. "Bluff body aerodynamics in wind engineering." *Journal of Wind Engineering and
Industrial Aerodynamics* 96 (2008) 701-712. doi:10.1016/j.jweia.2007.06.008.
Read in full. On Taipei 101 (pp. 702-704): "The wind tunnel tests … identified high across-wind loads, and
building accelerations beyond the applicable comfort criteria. The solution was developed in the form of
additional structure, modified shape and the installation of a large 740 ton tuned mass damper near the top of the
tower. The shape modification was to soften the building corners … This reduced the base wind-induced base bending
moments by approximately 25%". The paper gives no corner dimensions and no separate figures for drag, lift or
shedding frequency.

**[Poon 2004]** D. C. K. Poon, S.-S. Shieh, L. M. Joseph, C.-C. Chang. "Structural design of Taipei 101, the
world's tallest building." *Proceedings of the CTBUH 2004 Seoul Conference*, pp. 271-278.
Read in full. Section 5: "RWDI demonstrated that a square tower with sharp corners creates large crosswind
excitation. Rounded and chamfered (45°) corners reduced lateral response, but a 'saw tooth' or 'double notch'
corner with 2.5 m (8.2 ft) notches achieved a dramatic reduction." Figure 2 caption: "'sawtooth' plans with two
2.5 m re-entrant corners at each building corner". No percentage and no plan width are stated.

A figure of "up to 40 %" reduction circulates in web articles; no primary source for it was found, and it is not
used here.

The following exist and are the standard citations for corner modification of square sections at high Reynolds
number, but their text could not be obtained, so no finding is attributed to them in this repository:

* K. C. S. Kwok, P. A. Wilhelm, B. G. Wilkie. "Effect of edge configuration on wind-induced response of tall
  buildings." *Engineering Structures* 10 (1988) 135-140. doi:10.1016/0141-0296(88)90039-9.
* H. Kawai. "Effect of corner modifications on aeroelastic instabilities of tall buildings." *J. Wind Eng. Ind.
  Aerodyn.* 74-76 (1998) 719-729. doi:10.1016/S0167-6105(98)00065-8.
* T. Tamura, T. Miyagi, T. Kitagishi. "Numerical prediction of unsteady pressures on a square cylinder with
  various corner shapes." *J. Wind Eng. Ind. Aerodyn.* 74-76 (1998) 531-542. doi:10.1016/S0167-6105(98)00048-8.
* T. Tamura, T. Miyagi. "The effect of turbulence on aerodynamic forces on a square cylinder with various corner
  shapes." *J. Wind Eng. Ind. Aerodyn.* 83 (1999) 135-145. doi:10.1016/S0167-6105(99)00067-7.
* K. T. Tse, P. A. Hitchcock, K. C. S. Kwok, S. Thepmongkorn, C. M. Chan. "Economic perspectives of aerodynamic
  treatments of square tall buildings." *J. Wind Eng. Ind. Aerodyn.* 97 (2009) 455-467.
  doi:10.1016/j.jweia.2009.07.005.

Read as abstracts or through a secondary source:

**[Miran 2015]** S. Miran, C. H. Sohn. "Numerical study of the rounded corners effect on flow past a square
cylinder." *International Journal of Numerical Methods for Heat & Fluid Flow* 25 (2015) 686-702.
doi:10.1108/HFF-12-2013-0339. Abstract (Re = 500): "as the corner radius ratio, R/D, increases, the Strouhal
number increases rapidly for R/D=0-0.2, and then gradually rises between R/D=0.2 and 0.5. The minimum values of the
mean drag coefficient and the RMS value of lift coefficient were found around R/D=0.2".

**[Dey 2016]** P. Dey, A. K. Das. *Journal of Applied Fluid Mechanics* 9 (2016) 1189-1199. Read for its summary
of Tamura & Miyagi (1999): "Chamfered and rounded corners decrease drag forces, as a result of reduction in wake
width".

No published 2-D laminar (Re ≈ 100-200) study of *recessed* corners was found; the recessed-corner results in this
repository have no direct published comparator.

## Lattice-Boltzmann method

**[Qian 1992]** Y. H. Qian, D. d'Humières, P. Lallemand. "Lattice BGK models for Navier-Stokes equation."
*Europhysics Letters* 17 (1992) 479-484. doi:10.1209/0295-5075/17/6/001.

**[Ladd 1994a]** A. J. C. Ladd. "Numerical simulations of particulate suspensions via a discretized Boltzmann
equation. Part 1. Theoretical foundation." *Journal of Fluid Mechanics* 271 (1994) 285-309.
doi:10.1017/S0022112094001771.

**[Zou 1997]** Q. Zou, X. He. "On pressure and velocity boundary conditions for the lattice Boltzmann BGK model."
*Physics of Fluids* 9 (1997) 1591-1598. doi:10.1063/1.869307. (Considered, not used; see DESIGN.md.)

**[Bouzidi 2001]** M. Bouzidi, M. Firdaouss, P. Lallemand. "Momentum transfer of a Boltzmann-lattice fluid with
boundaries." *Physics of Fluids* 13 (2001) 3452-3459. doi:10.1063/1.1399290.

**[Guo 2002]** Z. Guo, C. Zheng, B. Shi. "Discrete lattice effects on the forcing term in the lattice Boltzmann
method." *Physical Review E* 65 (2002) 046308. doi:10.1103/PhysRevE.65.046308.

**[Mei 2002]** R. Mei, D. Yu, W. Shyy, L.-S. Luo. "Force evaluation in the lattice Boltzmann method involving
curved geometry." *Physical Review E* 65 (2002) 041203. doi:10.1103/PhysRevE.65.041203.

**[Lallemand 2003]** P. Lallemand, L.-S. Luo. "Lattice Boltzmann method for moving boundaries." *Journal of
Computational Physics* 184 (2003) 406-421. doi:10.1016/S0021-9991(02)00022-0.

**[Ginzburg 2008a]** I. Ginzburg, F. Verhaeghe, D. d'Humières. "Two-relaxation-time lattice Boltzmann scheme:
about parametrization, velocity, pressure and mixed boundary conditions." *Communications in Computational
Physics* 3 (2008) 427-478.

**[Wen 2014]** B. Wen, C. Zhang, Y. Tu, C. Wang, H. Fang. "Galilean invariant fluid-solid interfacial dynamics in
lattice Boltzmann simulations." *Journal of Computational Physics* 266 (2014) 161-170.
doi:10.1016/j.jcp.2014.02.018.

**[Krüger 2017]** T. Krüger, H. Kusumaatmaja, A. Kuzmin, O. Shardt, G. Silva, E. M. Viggen. *The Lattice Boltzmann
Method: Principles and Practice.* Springer, 2017. doi:10.1007/978-3-319-44649-3.

The method papers above were cited from their bibliographic records; the formulas implemented follow the standard
forms as presented in [Krüger 2017] and were verified by the tests in this repository rather than against the
original texts.

## Lattice-Boltzmann software (related work)

**[Palabos]** J. Latt et al. "Palabos: parallel lattice Boltzmann solver." *Computers & Mathematics with
Applications* 81 (2021) 334-350. doi:10.1016/j.camwa.2020.03.022.

**[OpenLB]** M. J. Krause et al. "OpenLB — open source lattice Boltzmann code." *Computers & Mathematics with
Applications* 81 (2021) 258-288. doi:10.1016/j.camwa.2020.04.033.

**[waLBerla]** M. Bauer et al. "waLBerla: a block-structured high-performance framework for multiphysics
simulations." *Computers & Mathematics with Applications* 81 (2021) 478-501. doi:10.1016/j.camwa.2020.01.007.

**[Sailfish]** M. Januszewski, M. Kostur. "Sailfish: a flexible multi-GPU implementation of the lattice Boltzmann
method." *Computer Physics Communications* 185 (2014) 2350-2368. doi:10.1016/j.cpc.2014.04.018.

**[lbmpy]** M. Bauer, H. Köstler, U. Rüde. "lbmpy: automatic code generation for efficient parallel lattice
Boltzmann methods." *Journal of Computational Science* 49 (2021) 101269. doi:10.1016/j.jocs.2020.101269.
Read (arXiv:2001.11806), section V.A: "Activating NT-stores results in the expected performance of about 300
MLUP/s on this system, very close to the maximal 304 MLUP/s predicted by the roofline estimate" (D3Q19, single
relaxation time, double precision, 20-core Xeon Gold 6148 socket).

## Colour

The vorticity colour map is built in the Oklab colour space (B. Ottosson, "A perceptual color space for image
processing", 2020, https://bottosson.github.io/posts/oklab/); the conversion in `src/render.rs` follows the reference
implementation given there and is checked by a round-trip test and a white-point test.
