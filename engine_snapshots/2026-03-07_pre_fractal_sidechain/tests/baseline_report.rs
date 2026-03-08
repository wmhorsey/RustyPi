//! Baseline diagnostics report validation.
//!
//! Reads the persisted diagnostics summary artifact and enforces
//! sanity thresholds, printing a compact report line.

use std::fs;
use std::path::PathBuf;

use serde_json::Value;

fn read_summary_json() -> Value {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target");
    path.push("diagnostics");
    path.push("latest_run_summary.json");

    let raw = fs::read_to_string(&path).expect(
        "missing diagnostics summary artifact. Run test diagnostics_artifacts first",
    );

    serde_json::from_str(&raw).expect("invalid diagnostics summary json")
}

fn f64_field(root: &Value, path: &[&str]) -> f64 {
    let mut v = root;
    for key in path {
        v = &v[*key];
    }
    v.as_f64().unwrap_or(f64::NAN)
}

fn u64_field(root: &Value, path: &[&str]) -> u64 {
    let mut v = root;
    for key in path {
        v = &v[*key];
    }
    v.as_u64().unwrap_or(0)
}

#[test]
fn baseline_report_sanity() {
    let summary = read_summary_json();

    let ticks = u64_field(&summary, &["run", "ticks"]);
    let parcels_final = u64_field(&summary, &["run", "parcels_final"]);

    let ste = f64_field(&summary, &["final", "total_ste"]);
    let ar_res = f64_field(&summary, &["final", "action_reaction_residual"]);
    let chi = f64_field(&summary, &["final", "chirality"]);
    let chi_abs = f64_field(&summary, &["final", "chirality_abs"]);
    let zero_x = u64_field(&summary, &["final", "zero_crossings"]);
    let lock_ticks = u64_field(&summary, &["final", "lock_ticks"]);
    let pairs = u64_field(&summary, &["final", "compound_pairs"]);
    let dwell = f64_field(&summary, &["final", "compound_dwell"]);
    let potential = f64_field(&summary, &["final", "compound_potential"]);
    let yield_now = f64_field(&summary, &["final", "yield"]);

    let max_pairs = u64_field(&summary, &["aggregates", "max_pairs"]);
    let max_yield = f64_field(&summary, &["aggregates", "max_yield"]);
    let mean_chi_abs = f64_field(&summary, &["aggregates", "mean_chirality_abs"]);

    println!(
        "\nBASELINE | ticks={} parcels={} ste={:.6} ar={:.3e} chi={:.4} |chi|={:.4} zc={} lock={} pairs={} dwell={:.2} pot={:.4} y_now={:.4} y_max={:.4} mean|chi|={:.4}",
        ticks,
        parcels_final,
        ste,
        ar_res,
        chi,
        chi_abs,
        zero_x,
        lock_ticks,
        pairs,
        dwell,
        potential,
        yield_now,
        max_yield,
        mean_chi_abs,
    );

    // Core sanity checks
    assert!(ticks >= 500, "artifact run too short");
    assert!(parcels_final > 0, "no parcels left in artifact run");

    assert!(ste.is_finite() && ste > 0.0, "invalid total STE");
    assert!(ar_res.is_finite() && ar_res < 1e-6, "AR residual too large: {ar_res}");

    assert!(chi.is_finite(), "chirality must be finite");
    assert!(chi_abs.is_finite() && (0.0..=1.0 + 1e-12).contains(&chi_abs), "|chi| out of range: {chi_abs}");
    assert!(mean_chi_abs.is_finite() && (0.0..=1.0 + 1e-12).contains(&mean_chi_abs), "mean |chi| out of range: {mean_chi_abs}");

    assert!(dwell.is_finite(), "compound dwell must be finite");
    assert!(potential.is_finite(), "compound potential must be finite");
    assert!(yield_now.is_finite(), "current yield must be finite");
    assert!(max_yield.is_finite(), "max yield must be finite");

    // If pairs ever formed, max_pairs should reflect it.
    assert!(max_pairs >= pairs, "max_pairs must be >= final pairs");

    // Lock counter cannot exceed total ticks.
    assert!(lock_ticks <= ticks, "lock ticks cannot exceed total ticks");
}
