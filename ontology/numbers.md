# STE — Numerical Ratios & Engine Calibration

*Dimensionless ratios extracted from published experiments, mapped to engine parameters.*

---

## Methodology

We treat two laboratory systems as windows into the STE substrate:

1. **Superfluid ⁴He** — substrate glimpsed through matter at minimum drag (T < 2.172 K)
2. **Quark-Gluon Plasma (QGP)** — substrate re-exposed by stripping matter at extreme energy (~156 MeV)

Both regimes reveal the same perfect fluid. Dimensionless ratios measured in these systems constrain the substrate's intrinsic proportions, independent of the arbitrary unit system used in the engine.

Two additional anomalous phenomena provide secondary validation:

3. **Time Crystals** — ground-state persistent motion (choked vortex maintaining circulation without dissipation)
4. **Quantum Mpemba Effect** — hotter quantum systems equilibrate faster (heat→heat concentration in bounded regime)

---

## Extracted Experimental Values

### QGP (Quark-Gluon Plasma)

| Quantity | Value | Source |
|---|---|---|
| Viscosity-to-entropy ratio η/s | ≈ 1/(4π) ≈ **0.0796** | RHIC/Brookhaven, conjectured KSS bound (Kovtun–Son–Starinets 2005) |
| Crossover temperature T_c | ~156 MeV (~1.8 × 10¹² K) | Lattice QCD consensus |
| Description | "Most perfect liquid ever observed" | RHIC experimental summary |

**What this tells us:** The substrate has an irreducible minimum viscosity. Even in its most fluid state, η/s cannot go below ~0.08 (in natural units). This is not matter-drag — it is the substrate's intrinsic attraction-mediated internal friction. The 1/(4π) form suggests a geometric origin: the solid angle 4π is the natural denominator for isotropic force distribution.

### Superfluid ⁴He

| Quantity | Value | Source |
|---|---|---|
| Lambda transition temperature T_λ | **2.172 K** | Kapitza 1937, standard reference |
| Liquid density ρ | **125 kg/m³** (at SVP, ~2 K) | Standard reference |
| Quantum of circulation κ | h/m₄ ≈ **9.97 × 10⁻⁸ m²/s** | Quantized vortex theory; Onsager 1949, Feynman 1955 |
| Film critical velocity v_film | ~**0.20 m/s** (20 cm/s) | Superfluid film flow measurements |
| Landau critical velocity v_c(Landau) | ~**58 m/s** (roton minimum) | Landau 1941 (theoretical); actual onset varies |
| First sound velocity c₁ | ~**238 m/s** (at T ≈ 0 K) | Standard He4 reference |
| Fountain pressure at T_λ | **0.692 bar** (= 56 m liquid He column) | Fountain effect measurements |
| Superfluid fraction ρ_s/ρ at T→0 | → **1.0** | Two-fluid model (Tisza, Landau) |
| Vortex core radius a₀ | ~**1 Å** (0.1 nm) | He4 vortex core measurements |
| Mean inter-particle spacing d | ~**3.6 Å** | From density: d ≈ (m/ρ)^(1/3) |

### Time Crystals

| Quantity | Value | Source |
|---|---|---|
| ³He time quasicrystal → continuous TC transition | ~**0.0001 K** (100 μK) | Autti et al. 2018, 2020 |
| InGaAs discrete time crystal lifetime | **40 minutes** | Feb 2024 experiment |
| Discrete TC oscillation period ratio | **2T** (period doubling) | Standard DTC observation (Google Sycamore 2021) |

**What this tells us:** Choked vortex structures can persist indefinitely in the ground state — "motion without energy." The period-doubling (2T) signature of discrete time crystals maps to the STE choke: the bypass flow oscillates around the depression at exactly half the driving frequency because each wrap-around takes two cycles to complete. The He³ continuous time crystal at 100 μK → persistent rotation at lowest achievable matter-drag.

### Quantum Mpemba Effect

| Quantity | Value | Source |
|---|---|---|
| Strong Mpemba speedup | **Exponential** at specific initial temperatures | Ares et al. 2023 (theory); Zhang et al. 2025 (trapped ion) |
| Natural occurrence | Appears spontaneously in nuclear spin cooling | Chatterjee et al. 2025 |
| Mechanism | "Memory of correlations" — initial quantum correlations speed equilibration | Nava & Fabrizio 2019; Ivander et al. 2023 |

**What this tells us:** In bounded phase space (pre-mass regime), higher energy → faster organization. This is heat→heat: the hotter system concentrates more efficiently because the STE self-attraction feedback is stronger at higher concentration. The "strong" Mpemba effect — exponential speedup at a specific temperature — maps to crossing the choke threshold, where concentration suddenly triggers a discrete organization event.

---

## Computed Dimensionless Ratios

These ratios are unit-free and therefore directly constraining for the engine.

### R1: Viscosity / Entropy Density — The Fluidity Bound

$$\frac{\eta}{s} \approx \frac{1}{4\pi} \approx 0.0796$$

**Engine mapping:** This sets the **minimum damping coefficient** relative to local STE density. In the substrate's own units, no parcel should be damped below ~8% of its concentration-weighted inertia.

Current engine: `viscosity_base = 0.08`, `viscosity_exponent = 1.5` (in wasm.rs proton config).
The base value 0.08 already sits at the KSS bound — **this is correct by accident or intuition**.

### R2: Choke Threshold — Critical Velocity / Propagation Speed

$$\frac{v_c(\text{Landau})}{c_1} = \frac{58}{238} \approx 0.244$$

**Interpretation:** The substrate chokes when local flow velocity reaches ~24% of the local propagation speed (first sound / attraction-wave speed). Below this threshold, disturbances propagate away fast enough to prevent wrap-around. Above it, the flow outruns its own pressure signal and the depression gets enveloped.

This is directly analogous to the Mach number at which aerodynamic choking occurs (~0.3–0.4 in convergent nozzles). The substrate's value is lower because a self-attracting fluid has steeper gradients than air.

**Engine mapping:** `choke_vorticity_threshold` should scale with propagation speed. The ratio 0.24 sets the fraction of maximum local flux at which choke events trigger.

### R3: Film Threshold — Practical Choke in Constrained Geometry

$$\frac{v_{\text{film}}}{c_1} = \frac{0.20}{238} \approx 0.00084$$

**Interpretation:** In thin films (highly constrained geometry — analogous to bond channels between close parcels), choking occurs at a far lower absolute velocity. The *ratio* is much smaller because the constraint itself (thin film ≈ tight bond channel) amplifies the effective Mach number.

**Engine mapping:** Bonds at `shell_interaction_range` should have a lower effective choke threshold than open-field bonds.

### R4: Void Core / Interparticle Spacing

$$\frac{a_0}{d} = \frac{1.0\,\text{Å}}{3.6\,\text{Å}} \approx 0.28$$

**Interpretation:** The void depression at a vortex core occupies ~28% of the mean parcel spacing. This is the natural size ratio between "nothing" and "something" in a quiet substrate, where matter imposes gentle order.

**Engine mapping:** U-quark void parcels should have an effective radius ~0.28× the characteristic bond length. Currently U quarks have `shell_level = 0.8` — this ratio suggests they should occupy about 28% of the bond rest-length, not 80% of the shell range. This is a candidate for recalibration.

### R5: Superfluid Fraction at Ground State

$$\frac{\rho_s}{\rho}\bigg|_{T \to 0} \to 1.0$$

**Interpretation:** At zero matter-drag, 100% of the system is superfluid (substrate). This is the engine's initial condition: all parcels in free-flow, no chokes, no foam — pure attraction dynamics.

**Engine mapping:** At initialization, all field parcels should have `coherence = 0.0` (unchoked), `concentration = equilibrium_concentration`. No pre-seeded structure except the quark cores.

### R6: Quantum of Circulation → Minimum Parcel Angular Momentum

$$\kappa = \frac{h}{m_4} \approx 9.97 \times 10^{-8}\,\text{m}^2/\text{s}$$

In dimensionless form (using interparticle spacing d and first sound c₁):

$$\frac{\kappa}{d \cdot c_1} = \frac{9.97 \times 10^{-8}}{3.6 \times 10^{-10} \times 238} \approx 1.16$$

**Interpretation:** The circulation quantum is approximately **one interparticle spacing × one propagation speed** — i.e., the minimum vortex wraps at the speed of sound across one parcel width. The ratio being ~1.0 (not 0.01 or 100) confirms the STE picture: the choke event has a natural angular momentum quantum set by the substrate's own scales.

**Engine mapping:** Each choke parcel should carry angular momentum of order `bond_rest_length × propagation_speed_equivalent`. The `2π` phase winding in the Onsager-Feynman formula ∮v·dl = 2πℏn/m maps to one full wrap-around of the bypass flow.

### R7: Fountain Pressure — Chemical Potential Overwhelms Gravity

$$p_f(T_\lambda) = 0.692\,\text{bar} = 56\,\text{m liquid He column}$$

Ratio to hydrostatic pressure at the same depth:

$$\frac{p_f}{\rho g h} \gg 1 \quad \text{(fountain effect pushes He 56 m against gravity)}$$

**Interpretation:** The substrate's chemical potential gradient (STE seeking to equalize) overwhelms gravitational settling by a factor that produces 56 m of liquid column. In STE terms: internal attraction redistribution is far stronger than the residual (gravitational) attraction between bulk parcels. This quantifies the force hierarchy — internal budget dominates residual by a large factor.

**Engine mapping:** `diffusivity` (the heat/STE equalization rate) must be much stronger than `gravity_g` at short range. Currently diffusivity = 0.1, gravity_g = 0.02 — a factor of 5. The fountain effect suggests this ratio should be much larger for bonded (short-range) parcels.

---

## Summary: Engine Parameter Audit

| Engine Parameter | Current Value | Experimental Constraint | Status |
|---|---|---|---|
| `viscosity_base` | 0.08 | η/s ≈ 1/4π ≈ 0.08 (KSS bound) | ✅ **Correct** |
| `viscosity_exponent` | 1.5 | Superlinear — dense regions thicken sharply | ✅ Reasonable (QGP shows sharp crossover) |
| `gravity_g` | 0.02 | Residual after internal budget | ✅ Small relative to bond forces |
| `diffusivity` | 0.1 | Fountain: 56 m column → internal ≫ gravity | ⚠️ May need increase relative to gravity (currently 5×, should be larger) |
| `choke_vorticity_threshold` | 0.5 | R2: choke at ~24% of propagation speed | ⚠️ Needs mapping to local flux speed |
| `shell_interaction_range` | 1.5 | R4: void ≈ 0.28× spacing | ⚠️ Void parcels may be too large |
| `void_threshold` | 1e-10 | U quarks at STE ≈ 0.05 | ⚠️ Gap between "void" definition and U-quark implementation |
| `dt` | 0.001 | R6: κ/(d·c₁) ≈ 1.16 → one wrap per sound-crossing time | ✅ Small enough to resolve |
| `max_bond_distance` | 15.0 | — | Unchanged (geometric, not substrate-constrained) |
| `foam_annihilation_range` | 2.0 | — | No direct experimental constraint yet |

### Priority Recalibrations

1. **Diffusivity vs. gravity ratio** — The fountain effect tells us that STE equalization along bonds dominates gravitational settling by at least an order of magnitude more than current 5× ratio. Consider `diffusivity = 0.5` or introduce distance-dependent scaling (stronger at short range).

2. **Void size ratio** — U quarks with `shell_level = 0.8` occupy 80% of shell range; R4 says voids should be ~28% of spacing. Reducing U-quark effective size would let the QGP shell be thicker relative to the void — closer to the He4 vortex geometry.

3. **Choke threshold context-sensitivity** — R2 (24% in bulk) vs. R3 (0.08% in thin films) show that choke threshold depends strongly on local geometry. The engine currently uses a single `choke_vorticity_threshold`. A bond-width-dependent threshold would capture this: narrow channels choke earlier.

---

## Canonical Mapping (Boundary-State Terminology)

For consistency with `overview.md`, `model.md`, `glossary.md`, and RustyPi/PrimalPi docs, numeric calibration should be interpreted through the same mass boundary-state triad:

- transient spike state (ring-down dominated)
- depressed-core shell state (metastable)
- persistent void-core shell state (shell-borne stress around STE-zero core)

Wave-channel interpretation is likewise canonical: higher-frequency swells couple harder to shell boundaries and are reabsorbed faster.

---

## Falsifiability

The key test of the STE model is **cross-scale consistency**: the dimensionless ratios extracted from QGP should predict the behavior of superfluid He4, and vice versa, because both are windows into the same substrate.

| Ratio | QGP prediction | He4 measurement | Consistent? |
|---|---|---|---|
| Minimum η/s | 1/4π ≈ 0.08 | Superfluid: η → 0 *for the superfluid component*; normal component carries the viscosity | ✅ — the two-fluid split means the substrate fraction (superfluid) has η=0 while the matter fraction (normal) obeys KSS-like bounds |
| Vortex formation at flow threshold | Expected (QGP shows collective flow) | Confirmed: quantized vortices above critical v | ✅ |
| Structure formation at high concentration | QGP produces hadrons as it cools | He4 vortex lattices form at high rotation | ✅ — same bounded-phase-space ordering |
| Void-as-depression | Confinement: quarks cannot be isolated | Vortex core: singular depression in superfluid density | ✅ — same topology |

If future measurements of QGP viscosity fall significantly below 1/4π, or if He4 vortex core radios deviate strongly from ~0.28× spacing under new conditions, the model would need revision. The ratios are **rigid predictions**, not adjustable parameters.

---

## New Experimental Anchors (additions to model.md)

| Prediction / Mechanism | Experimental confirmation |
|---|---|
| Choked vortex persists without dissipation | Time crystals: persistent periodic structure in ground state — "motion without energy" (Wilczek 2012, realized 2016–2024) |
| He³ superfluid at ~100 μK: time quasicrystal → continuous time crystal | Autti et al. 2018, 2020 — substrate organizes when matter-drag minimized to micro-kelvin level |
| Heat→heat in bounded regime → faster equilibration | Quantum Mpemba Effect: hotter quantum system reaches equilibrium faster (Zhang et al. 2025, Chatterjee et al. 2025) |
| Choke threshold crossing → exponential speedup | Strong Mpemba Effect: exponential equilibration speedup at specific initial temperature (Ares et al. 2023) |

---

*Document created during numerical calibration phase. All ratios are extracted from published experimental data and mapped to engine parameters via the STE ontology. No free parameters were introduced — the engine either matches the experimental ratio or it doesn't.*
