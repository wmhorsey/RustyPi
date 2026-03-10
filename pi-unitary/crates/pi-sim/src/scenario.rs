use pi_core::MathError;

use crate::{AdditiveChokeKernel, ChokeNode, ResponseChannel};

#[derive(Debug, Clone)]
pub struct ChokeScenarioConfig {
    pub ticks: usize,
    pub nodes: usize,
    pub target_tick: u16,
    pub channel: ResponseChannel,
}

impl Default for ChokeScenarioConfig {
    fn default() -> Self {
        Self {
            ticks: 256,
            nodes: 4,
            target_tick: 0,
            channel: ResponseChannel::TrapBiased,
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
}

impl ChokeTraceRow {
    pub fn csv_header() -> &'static str {
        "tick,node,drive,phase_tick,coherence,energy,phase"
    }

    pub fn to_csv_line(self) -> String {
        format!(
            "{},{},{},{},{},{},{}",
            self.tick,
            self.node,
            self.drive,
            self.phase_tick,
            self.coherence,
            self.energy,
            self.phase,
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

pub fn run_choke_scenario(cfg: ChokeScenarioConfig) -> Result<Vec<ChokeTraceRow>, MathError> {
    if cfg.ticks == 0 {
        return Err(MathError::InvalidConfig("ticks must be > 0"));
    }
    if cfg.nodes == 0 {
        return Err(MathError::InvalidConfig("nodes must be > 0"));
    }

    let mut kernel = AdditiveChokeKernel::new_with_channel(cfg.nodes, cfg.channel)?;
    kernel.set_target_phase(cfg.target_tick)?;

    let mut rows = Vec::with_capacity(cfg.ticks * cfg.nodes);

    let mut drive = vec![0i64; cfg.nodes];
    for tick in 0..cfg.ticks {
        let mut i = 0usize;
        while i < cfg.nodes {
            drive[i] = drive_for(tick, i);
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
        };
        let rows = run_choke_scenario(cfg).expect("rows");
        assert_eq!(rows.len(), 20);
    }
}
