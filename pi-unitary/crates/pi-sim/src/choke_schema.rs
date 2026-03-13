use crate::ChokePhase;

pub const TAU_TICKS_DEFAULT_U32: u32 = 4096;
pub const GPU_CHOKE_NODE_WORDS: usize = 8;
pub const GPU_CHOKE_NODE_BYTES: usize = GPU_CHOKE_NODE_WORDS * 4;

pub const PHASE_FREE: u32 = 0;
pub const PHASE_FORMATION: u32 = 1;
pub const PHASE_LIFTOFF: u32 = 2;
pub const PHASE_COHERENCE: u32 = 3;
pub const PHASE_DRIFT: u32 = 4;
pub const PHASE_DISSOLUTION: u32 = 5;

pub const PATHWAY_FREE_POOL: u32 = 0;
pub const PATHWAY_DEPRESSION_CONSUMPTION: u32 = 1;
pub const PATHWAY_CHOKE_SHELL_STRUCTURING: u32 = 2;
pub const PATHWAY_RADIATIVE_RELEASE: u32 = 3;
pub const PATHWAY_CATASTROPHIC_COLLAPSE: u32 = 4;

pub fn phase_id_from_phase(phase: ChokePhase) -> u32 {
    match phase {
        ChokePhase::Free => PHASE_FREE,
        ChokePhase::Formation => PHASE_FORMATION,
        ChokePhase::LiftOff => PHASE_LIFTOFF,
        ChokePhase::Coherence => PHASE_COHERENCE,
        ChokePhase::Drift => PHASE_DRIFT,
        ChokePhase::Dissolution => PHASE_DISSOLUTION,
    }
}

pub fn phase_name(phase_id: u32) -> &'static str {
    match phase_id {
        PHASE_FREE => "free",
        PHASE_FORMATION => "formation",
        PHASE_LIFTOFF => "liftoff",
        PHASE_COHERENCE => "coherence",
        PHASE_DRIFT => "drift",
        PHASE_DISSOLUTION => "dissolution",
        _ => "unknown",
    }
}

pub fn pathway_id_for(phase_id: u32, energy: i64, coherence: i64) -> u32 {
    match phase_id {
        PHASE_FREE => PATHWAY_FREE_POOL,
        PHASE_FORMATION | PHASE_LIFTOFF => PATHWAY_DEPRESSION_CONSUMPTION,
        PHASE_COHERENCE | PHASE_DRIFT => PATHWAY_CHOKE_SHELL_STRUCTURING,
        PHASE_DISSOLUTION => {
            if energy > coherence {
                PATHWAY_CATASTROPHIC_COLLAPSE
            } else {
                PATHWAY_RADIATIVE_RELEASE
            }
        }
        _ => PATHWAY_FREE_POOL,
    }
}

pub fn pathway_name(pathway_id: u32) -> &'static str {
    match pathway_id {
        PATHWAY_FREE_POOL => "free_pool",
        PATHWAY_DEPRESSION_CONSUMPTION => "depression_consumption",
        PATHWAY_CHOKE_SHELL_STRUCTURING => "choke_shell_structuring",
        PATHWAY_RADIATIVE_RELEASE => "radiative_release",
        PATHWAY_CATASTROPHIC_COLLAPSE => "catastrophic_collapse",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_names_match_contract() {
        assert_eq!(phase_name(PHASE_FREE), "free");
        assert_eq!(phase_name(PHASE_FORMATION), "formation");
        assert_eq!(phase_name(PHASE_LIFTOFF), "liftoff");
        assert_eq!(phase_name(PHASE_COHERENCE), "coherence");
        assert_eq!(phase_name(PHASE_DRIFT), "drift");
        assert_eq!(phase_name(PHASE_DISSOLUTION), "dissolution");
    }

    #[test]
    fn dissolution_pathway_obeys_energy_coherence_relation() {
        assert_eq!(
            pathway_id_for(PHASE_DISSOLUTION, 10, 5),
            PATHWAY_CATASTROPHIC_COLLAPSE
        );
        assert_eq!(
            pathway_id_for(PHASE_DISSOLUTION, 4, 5),
            PATHWAY_RADIATIVE_RELEASE
        );
    }
}
