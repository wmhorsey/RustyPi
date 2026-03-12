use pi_core::MathError;

use crate::{AdditiveChokeKernel, ChokeNode, ResponseChannel};

#[derive(Debug, Clone)]
pub struct ChokeScenarioConfig {
    pub ticks: usize,
    pub nodes: usize,
    pub target_tick: u16,
    pub channel: ResponseChannel,
    pub generation_depth: u8,
    pub calm_factor_pct: u8,
}

impl Default for ChokeScenarioConfig {
    fn default() -> Self {
        Self {
            ticks: 256,
            nodes: 4,
            target_tick: 0,
            channel: ResponseChannel::TrapBiased,
            generation_depth: 0,
            calm_factor_pct: 100,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChokeTraceRow {
    pub tick: usize,
    pub node: usize,
    pub drive: i64,
    pub phase_tick: u16,
    pub coherence: i64,
    pub energy: i64,
    pub phase: &'static str,
    pub pathway: &'static str,
}

impl ChokeTraceRow {
    pub fn csv_header() -> &'static str {
        "tick,node,drive,phase_tick,coherence,energy,phase,pathway"
    }

    pub fn to_csv_line(self) -> String {
        format!(
            "{},{},{},{},{},{},{},{}",
            self.tick,
            self.node,
            self.drive,
            self.phase_tick,
            self.coherence,
            self.energy,
            self.phase,
            self.pathway,
        )
    }
}

fn phase_name(node: ChokeNode) -> &'static str {
    match node.phase {
        crate::ChokePhase::Free => "free",
        crate::ChokePhase::Formation => "formation",
        crate::ChokePhase::LiftOff => "liftoff",
        crate::ChokePhase::Coherence => "coherence",
        crate::ChokePhase::Drift => "drift",
        crate::ChokePhase::Dissolution => "dissolution",
    }
}

fn pathway_name(node: ChokeNode) -> &'static str {
    match node.phase {
        crate::ChokePhase::Free => "free_pool",
        crate::ChokePhase::Formation | crate::ChokePhase::LiftOff => "depression_consumption",
        crate::ChokePhase::Coherence | crate::ChokePhase::Drift => "choke_shell_structuring",
        crate::ChokePhase::Dissolution => {
            if node.energy > node.coherence {
                "catastrophic_collapse"
            } else {
                "radiative_release"
            }
        }
    }
}

/// Deterministic additive drive schedule for choke lifecycle testing.
fn drive_for(tick: usize, node: usize) -> i64 {
    // Multi-lane deterministic pulses across many nodes to produce
    // formation, drift, and dissolution occupancy in trace windows.
    if tick < 256 && (tick + node) % 29 == 0 {
        return 1;
    }
    if tick >= 96 && tick < 320 && (tick + node + node) % 37 == 0 {
        return 1;
    }
    if tick >= 224 && tick < 448 && (tick + node + node + node) % 43 == 0 {
        return 1;
    }
    0i64
}

fn baseline_load_stepup(target_tick: u16, generation_depth: u8, calm_factor_pct: u8) -> i64 {
    if target_tick < 128 {
        return 0;
    }

    // Step-up by powers of two above the first viable gate at 128.
    let mut step = 1i64;
    let mut v = target_tick / 128;
    while v >= 2 {
        step += 1;
        v /= 2;
    }

    let raw = step + i64::from(generation_depth);
    let calm = i64::from(calm_factor_pct.clamp(1, 100));
    let scaled = (raw * calm) / 100;
    if scaled > 0 {
        scaled
    } else {
        1
    }
}

fn baseline_pulse(tick: usize, node: usize, baseline: i64, calm_factor_pct: u8) -> i64 {
    if baseline <= 0 {
        return 0;
    }
    // Calmer environments inject baseline load less frequently.
    let period = 4usize + usize::from(calm_factor_pct.clamp(1, 100) / 10);
    if (tick + node) % period == 0 {
        baseline
    } else {
        0
    }
}

pub fn run_choke_scenario(cfg: ChokeScenarioConfig) -> Result<Vec<ChokeTraceRow>, MathError> {
    if cfg.ticks == 0 {
        return Err(MathError::InvalidConfig("ticks must be > 0"));
    }
    if cfg.nodes == 0 {
        return Err(MathError::InvalidConfig("nodes must be > 0"));
    }

    let mut kernel = AdditiveChokeKernel::new_with_channel(cfg.nodes, cfg.channel)?;
    kernel.set_target_phase(cfg.target_tick)?;
    let baseline = baseline_load_stepup(cfg.target_tick, cfg.generation_depth, cfg.calm_factor_pct);

    let mut rows = Vec::with_capacity(cfg.ticks * cfg.nodes);

    let mut drive = vec![0i64; cfg.nodes];
    for tick in 0..cfg.ticks {
        let mut i = 0usize;
        while i < cfg.nodes {
            drive[i] = drive_for(tick, i) + baseline_pulse(tick, i, baseline, cfg.calm_factor_pct);
            i += 1;
        }

        kernel.step(&drive)?;

        for (node_idx, node) in kernel.nodes().iter().enumerate() {
            rows.push(ChokeTraceRow {
                tick,
                node: node_idx,
                drive: drive[node_idx],
                phase_tick: node.phase_tick.tick(),
                coherence: node.coherence,
                energy: node.energy,
                phase: phase_name(*node),
                pathway: pathway_name(*node),
            });
        }
    }

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_is_deterministic() {
        let cfg = ChokeScenarioConfig {
            ticks: 32,
            nodes: 3,
            target_tick: 1,
            channel: ResponseChannel::TrapBiased,
            generation_depth: 0,
            calm_factor_pct: 100,
        };
        let a = run_choke_scenario(cfg.clone()).expect("run a");
        let b = run_choke_scenario(cfg).expect("run b");
        assert_eq!(a, b);
    }

    #[test]
    fn scenario_emits_expected_row_count() {
        let cfg = ChokeScenarioConfig {
            ticks: 10,
            nodes: 2,
            target_tick: 0,
            channel: ResponseChannel::TrapBiased,
            generation_depth: 0,
            calm_factor_pct: 100,
        };
        let rows = run_choke_scenario(cfg).expect("rows");
        assert_eq!(rows.len(), 20);
    }

    #[test]
    fn baseline_load_stepup_respects_threshold_and_nesting() {
        assert_eq!(baseline_load_stepup(64, 2, 100), 0);
        assert_eq!(baseline_load_stepup(128, 0, 100), 1);
        assert_eq!(baseline_load_stepup(128, 2, 100), 3);
        assert_eq!(baseline_load_stepup(256, 2, 50), 2);
    }
}
