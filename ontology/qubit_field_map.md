# Qubit Field Map (Real-World Setup -> STE Interpretation)

## Purpose
Map how labs physically force qubits to behave and reinterpret each control channel in STE-native terms.

## Platform 1: Superconducting Qubits (Transmon/Fluxonium)
Real-world setup:
- Josephson-junction nonlinear circuits at ~GHz.
- Dilution refrigerator operation at ~mK.
- Microwave drive lines for single-qubit rotations.
- Couplers/resonators for two-qubit gates.
- Dispersive resonator readout.
- Heavy filtering/shielding to suppress quasiparticles and parasitic modes.

STE interpretation:
- Drive pulses impose a timed attraction-modulation pattern (the driving wave).
- Gate operations are controlled shell-elevation trajectories.
- Readout is forced collapse of driven wave structure into a detectable local event.
- Decoherence channels are uncontrolled collapse pathways (TLS defects, quasiparticles, radiation hits).

## Platform 2: Trapped-Ion Qubits
Real-world setup:
- RF Paul traps create effective confinement potentials.
- Laser cooling to near motional ground state.
- Optical pumping for initialization.
- Raman/optical transitions for gates.
- Fluorescence detection for state measurement.

STE interpretation:
- Trap fields shape local potential geometry where driving-wave structures remain coherent.
- Laser pulses drive coherent shell transitions.
- Measurement laser forces terminal collapse into bright/dark emission outcomes.
- Motional mode heating is field-noise injection into the relay channel.

## Platform 3: Neutral-Atom (Rydberg) Qubits
Real-world setup:
- Laser-cooled atom arrays in optical tweezers.
- Hyperfine-state encoding.
- Rydberg excitation pulses for strong, tunable interactions.
- Blockade-based entangling gates.
- Fluorescence/camera readout.

STE interpretation:
- Rydberg dressing amplifies interaction-range of the driving wave.
- Blockade behaves like local saturation front exclusion.
- Entangling gates are controlled differential collapse phases between neighboring nodes.
- Readout is forced collapse to emitting/non-emitting branches.

## Cross-Platform Invariant
All major platforms do the same meta-operation:
1. Isolate a local field region from ambient noise.
2. Inject a calibrated driving wave (EM/laser pulse).
3. Hold coherence long enough for a target phase trajectory.
4. Force collapse at measurement and classify outcomes.

This aligns with the ontology phrase:
- Light is collapse of the driving wave structure.

## Immediate STE Benchmark Implications
1. Add a relay-collapse counter to light propagation diagnostics.
2. Track shell overshoot and relaxation time per relay event.
3. Separate transient-collapse events from terminal-capture events.
4. Add a synthetic noise channel to emulate lab field contamination and map collapse fragility.

## Sources Used (high-level)
- Superconducting quantum computing overview and operation sections.
- Trapped-ion quantum computer setup and gate/measurement sections.
- Neutral atom quantum computer architecture and Rydberg gate sections.

These references were used for control-field setup patterns, not for importing governing assumptions into STE equations.
