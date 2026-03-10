use pi_core::{MathError, PhaseTicks, TAU_TICKS_DEFAULT};

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
    pub dissolution_ticks: i64,
    pub promoted: bool,
    pub phase: ChokePhase,
}

impl ChokeNode {
    pub fn new() -> Result<Self, MathError> {
        Ok(Self {
            phase_tick: PhaseTicks::new(0, TAU_TICKS_DEFAULT)?,
            coherence: 0,
            energy: 0,
            dissolution_ticks: 0,
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
    dissolution_hold_ticks: i64,
}

impl AdditiveChokeKernel {
    pub fn new(node_count: usize) -> Result<Self, MathError> {
        if node_count == 0 {
            return Err(MathError::InvalidConfig("node_count must be > 0"));
        }

        let mut nodes = Vec::with_capacity(node_count);
        for _ in 0..node_count {
            nodes.push(ChokeNode::new()?);
        }

        Ok(Self {
            nodes,
            target_phase: PhaseTicks::new(0, TAU_TICKS_DEFAULT)?,
            align_window_ticks: 24,
            phase_step: 1,
            coherence_gain: 4,
            coherence_loss: 2,
            lift_threshold: 1,
            coherent_threshold: 48,
            drift_threshold: 10,
            dissolution_hold_ticks: 6,
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

            // Background energy relaxation when no fresh drive is present.
            if input == 0 && node.energy > 0 {
                node.energy -= 1;
            }

            // Energy leak when coherence is low; additive, one quantum per tick.
            if node.phase == ChokePhase::Dissolution && node.energy > 0 {
                node.energy -= 1;
            }

            if node.phase == ChokePhase::Dissolution && node.dissolution_ticks > 0 {
                node.dissolution_ticks -= 1;
            }

            let next_phase = classify_phase(
                node.phase,
                node.coherence,
                node.energy,
                node.dissolution_ticks,
                node.promoted,
                self.lift_threshold,
                self.coherent_threshold,
                self.drift_threshold,
            );
            if next_phase == ChokePhase::Dissolution && node.phase != ChokePhase::Dissolution {
                node.dissolution_ticks = self.dissolution_hold_ticks;
            }
            if next_phase == ChokePhase::LiftOff
                || next_phase == ChokePhase::Coherence
                || next_phase == ChokePhase::Drift
                || next_phase == ChokePhase::Dissolution
            {
                node.promoted = true;
            }
            if next_phase == ChokePhase::Free {
                node.promoted = false;
                node.dissolution_ticks = 0;
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
    dissolution_ticks: i64,
    promoted: bool,
    lift_threshold: i64,
    coherent_threshold: i64,
    drift_threshold: i64,
) -> ChokePhase {
    // Hard collapse rule: once both coherence and energy are depleted,
    // only previously promoted nodes are allowed a bounded dissolution release.
    if energy <= 0 && coherence <= 0 {
        if current == ChokePhase::Free {
            return ChokePhase::Free;
        }
        if !promoted {
            return ChokePhase::Free;
        }
        if current == ChokePhase::Dissolution && dissolution_ticks <= 0 {
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
            if coherence <= 0 {
                ChokePhase::Dissolution
            } else {
                ChokePhase::Drift
            }
        }
        ChokePhase::Dissolution => {
            if energy <= 0 && dissolution_ticks <= 0 {
                ChokePhase::Free
            } else {
                ChokePhase::Dissolution
            }
        }
    }
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
}
