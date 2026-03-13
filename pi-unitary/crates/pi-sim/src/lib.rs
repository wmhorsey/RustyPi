pub mod choke_additive;
pub mod choke_schema;
pub mod coherence;
pub mod recursive;
pub mod scenario;
pub mod audit;

pub use choke_additive::{AdditiveChokeKernel, ChokeNode, ChokePhase, ResponseChannel};
pub use coherence::{coherence_gate, phase_delta};
pub use recursive::AdditiveFractalKernel;
pub use scenario::{run_choke_scenario, ChokeScenarioConfig, ChokeTraceRow};
