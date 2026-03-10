use pi_core::{MathError, PhaseTicks, TAU_TICKS_DEFAULT};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseChannel {
    TrapBiased,
    RadiativeBiased,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChokePhase {
    // Unconstrained background state with no trapped shell energy.
    Free,
    // Early shell accumulation around a forming choke.
    Formation,
    // Shell has enough constrained STE to detach from free background.
    LiftOff,
    // Shell-trapped choke is stable and strongly self-reinforcing.
    Coherence,
    // Stable choke begins losing constraint and slips toward release.
    Drift,
    // Constrained STE is being released back into free state.
    Dissolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChokeNode {
    pub phase_tick: PhaseTicks,
    pub coherence: i64,
    pub energy: i64,
    pub promoted: bool,
    pub phase: ChokePhase,
}

impl ChokeNode {
    pub fn new() -> Result<Self, MathError> {
        Ok(Self {
            phase_tick: PhaseTicks::new(0, TAU_TICKS_DEFAULT)?,
            coherence: 0,
            energy: 0,
            promoted: false,
            phase: ChokePhase::Free,
        })
    }
}

/// Additive-only choke lifecycle kernel.
///
/// Hot-path updates use only addition/subtraction/comparison and typed phase ring operations.
///
/// Ontology contract:
/// - Forward lifecycle (`Free -> ... -> Dissolution`) models choke formation and shell entrapment.
/// - `Dissolution` models constrained STE release and is the only late-stage path back to `Free`.
#[derive(Debug, Clone)]
pub struct AdditiveChokeKernel {
    nodes: Vec<ChokeNode>,
    target_phase: PhaseTicks,
    align_window_ticks: u16,
    phase_step: u16,
    coherence_gain: i64,
    coherence_loss: i64,
    lift_threshold: i64,
    coherent_threshold: i64,
    drift_threshold: i64,
    shell_tension_floor: i64,
    break_pressure_threshold: i64,
}

#[derive(Debug, Clone, Copy)]
struct ChannelProfile {
    coherence_gain: i64,
    coherence_loss: i64,
    lift_threshold: i64,
    coherent_threshold: i64,
    drift_threshold: i64,
    shell_tension_floor: i64,
    break_pressure_threshold: i64,
}

fn channel_profile(channel: ResponseChannel) -> ChannelProfile {
    match channel {
        // Trap-biased behavior prefers shell persistence.
        ResponseChannel::TrapBiased => ChannelProfile {
            coherence_gain: 4,
            coherence_loss: 1,
            lift_threshold: 1,
            coherent_threshold: 8,
            drift_threshold: 2,
            shell_tension_floor: 0,
            break_pressure_threshold: 2,
        },
        // Radiative-biased behavior favors quicker shell break and release.
        ResponseChannel::RadiativeBiased => ChannelProfile {
            coherence_gain: 3,
            coherence_loss: 2,
            lift_threshold: 1,
            coherent_threshold: 6,
            drift_threshold: 3,
            shell_tension_floor: 0,
            break_pressure_threshold: 0,
        },
    }
}

impl AdditiveChokeKernel {
    pub fn new(node_count: usize) -> Result<Self, MathError> {
        Self::new_with_channel(node_count, ResponseChannel::TrapBiased)
    }

    pub fn new_with_channel(
        node_count: usize,
        channel: ResponseChannel,
    ) -> Result<Self, MathError> {
        if node_count == 0 {
            return Err(MathError::InvalidConfig("node_count must be > 0"));
        }

        let mut nodes = Vec::with_capacity(node_count);
        for _ in 0..node_count {
            nodes.push(ChokeNode::new()?);
        }

        let profile = channel_profile(channel);

        Ok(Self {
            nodes,
            target_phase: PhaseTicks::new(0, TAU_TICKS_DEFAULT)?,
            align_window_ticks: 24,
            phase_step: 1,
            coherence_gain: profile.coherence_gain,
            coherence_loss: profile.coherence_loss,
            lift_threshold: profile.lift_threshold,
            coherent_threshold: profile.coherent_threshold,
            drift_threshold: profile.drift_threshold,
            shell_tension_floor: profile.shell_tension_floor,
            break_pressure_threshold: profile.break_pressure_threshold,
        })
    }

    #[inline]
    pub fn nodes(&self) -> &[ChokeNode] {
        &self.nodes
    }

    pub fn set_target_phase(&mut self, tick: u16) -> Result<(), MathError> {
        self.target_phase = PhaseTicks::new(tick, TAU_TICKS_DEFAULT)?;
        Ok(())
    }

    pub fn step(&mut self, drive: &[i64]) -> Result<(), MathError> {
        if drive.len() != self.nodes.len() {
            return Err(MathError::DomainViolation("drive length mismatch"));
        }

        for (idx, node) in self.nodes.iter_mut().enumerate() {
            let input = drive[idx];
            if input > 0 {
                node.energy += input;
            }

            node.phase_tick = node.phase_tick.add_ticks(self.phase_step);
            let arc = node.phase_tick.shortest_arc(self.target_phase)?;
            if arc <= self.align_window_ticks && input > 0 {
                node.coherence += self.coherence_gain;
            } else if node.coherence > 0 {
                node.coherence -= self.coherence_loss;
                if node.coherence < 0 {
                    node.coherence = 0;
                }
            }

            // Reservoir loss should be strongest in free pool; active shells retain contrast.
            if input == 0 && node.energy > 0 && node.phase == ChokePhase::Free {
                node.energy -= 1;
            }

            // Dissolution release: shell failure vents constrained STE back to free pool.
            if node.phase == ChokePhase::Dissolution && node.energy > 0 {
                node.energy -= 1;
                let pressure = break_pressure(node.coherence, node.energy);
                if pressure > self.break_pressure_threshold && node.energy > 0 {
                    node.energy -= 1;
                }
            }

            let next_phase = classify_phase(
                node.phase,
                node.coherence,
                node.energy,
                node.promoted,
                self.lift_threshold,
                self.coherent_threshold,
                self.drift_threshold,
                self.shell_tension_floor,
                self.break_pressure_threshold,
            );
            if next_phase == ChokePhase::LiftOff
                || next_phase == ChokePhase::Coherence
                || next_phase == ChokePhase::Drift
                || next_phase == ChokePhase::Dissolution
            {
                node.promoted = true;
            }
            if next_phase == ChokePhase::Free {
                node.promoted = false;
            }
            node.phase = next_phase;
        }

        Ok(())
    }
}

fn classify_phase(
    current: ChokePhase,
    coherence: i64,
    energy: i64,
    promoted: bool,
    lift_threshold: i64,
    coherent_threshold: i64,
    drift_threshold: i64,
    shell_tension_floor: i64,
    break_pressure_threshold: i64,
) -> ChokePhase {
    // Shell tension is the remaining capacity for an STE shell boundary to hold.
    // If tension is spent, dissolution can complete and return to free background.
    let tension = shell_tension(coherence, energy);
    let pressure = break_pressure(coherence, energy);
    if tension <= shell_tension_floor {
        if current == ChokePhase::Free {
            return ChokePhase::Free;
        }
        if !promoted {
            return ChokePhase::Free;
        }
        if current == ChokePhase::LiftOff {
            return ChokePhase::Coherence;
        }
        if current == ChokePhase::Coherence {
            return ChokePhase::Drift;
        }
        if current == ChokePhase::Dissolution {
            return ChokePhase::Free;
        }
        return ChokePhase::Dissolution;
    }

    match current {
        ChokePhase::Free => {
            if coherence > 0 || energy > 0 {
                ChokePhase::Formation
            } else {
                ChokePhase::Free
            }
        }
        ChokePhase::Formation => {
            if coherence >= lift_threshold {
                ChokePhase::LiftOff
            } else {
                ChokePhase::Formation
            }
        }
        ChokePhase::LiftOff => {
            if coherence >= coherent_threshold {
                ChokePhase::Coherence
            } else {
                ChokePhase::LiftOff
            }
        }
        ChokePhase::Coherence => {
            if coherence <= drift_threshold {
                ChokePhase::Drift
            } else {
                ChokePhase::Coherence
            }
        }
        ChokePhase::Drift => {
            if tension <= shell_tension_floor || pressure > break_pressure_threshold {
                ChokePhase::Dissolution
            } else {
                ChokePhase::Drift
            }
        }
        ChokePhase::Dissolution => {
            if tension <= shell_tension_floor {
                ChokePhase::Free
            } else {
                ChokePhase::Dissolution
            }
        }
    }
}

fn shell_tension(coherence: i64, energy: i64) -> i64 {
    let c = if coherence > 0 { coherence } else { 0 };
    let e = if energy > 0 { energy } else { 0 };
    if c < e {
        c
    } else {
        e
    }
}

fn break_pressure(coherence: i64, energy: i64) -> i64 {
    let support = shell_tension(coherence, energy);
    let refill = if energy > coherence {
        energy - coherence
    } else {
        0
    };
    refill - support
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choke_node_enters_lifecycle_with_sustained_drive() {
        let mut k = AdditiveChokeKernel::new(1).expect("kernel");
        k.set_target_phase(1).expect("target");

        for _ in 0..12 {
            k.step(&[1]).expect("step");
        }

        let node = k.nodes()[0];
        assert!(node.phase != ChokePhase::Free);
        assert!(node.coherence > 0);
    }

    #[test]
    fn coherence_decays_outside_alignment_window() {
        let mut k = AdditiveChokeKernel::new(1).expect("kernel");
        k.set_target_phase(0).expect("target");

        for _ in 0..8 {
            k.step(&[1]).expect("step");
        }
        let before = k.nodes()[0].coherence;

        k.set_target_phase(2048).expect("retarget");
        for _ in 0..8 {
            k.step(&[0]).expect("step");
        }

        let after = k.nodes()[0].coherence;
        assert!(after < before);
    }

    #[test]
    fn energy_never_negative() {
        let mut k = AdditiveChokeKernel::new(2).expect("kernel");
        for _ in 0..40 {
            k.step(&[0, 0]).expect("step");
        }
        for n in k.nodes() {
            assert!(n.energy >= 0);
        }
    }

    #[test]
    fn shell_tension_requires_both_channels() {
        assert_eq!(shell_tension(5, 3), 3);
        assert_eq!(shell_tension(3, 5), 3);
        assert_eq!(shell_tension(0, 5), 0);
        assert_eq!(shell_tension(5, 0), 0);
    }

    #[test]
    fn break_pressure_grows_when_refill_dominates() {
        assert_eq!(break_pressure(3, 3), -3);
        assert_eq!(break_pressure(1, 5), 3);
        assert_eq!(break_pressure(0, 5), 5);
    }
}
