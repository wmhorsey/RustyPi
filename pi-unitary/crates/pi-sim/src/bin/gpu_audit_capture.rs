use std::env;
use std::path::PathBuf;
use std::time::Instant;

use pi_sim::audit::{AuditRunConfig, AuditRunWriter, AuditSnapshotRef};

fn parse_usize_arg(value: Option<&String>, default_value: usize) -> usize {
    match value.and_then(|v| v.parse::<usize>().ok()) {
        Some(v) if v > 0 => v,
        _ => default_value,
    }
}

fn parse_u64_arg(value: Option<&String>, default_value: u64) -> u64 {
    match value.and_then(|v| v.parse::<u64>().ok()) {
        Some(v) if v > 0 => v,
        _ => default_value,
    }
}

fn parse_u32_arg(value: Option<&String>, default_value: u32) -> u32 {
    match value.and_then(|v| v.parse::<u32>().ok()) {
        Some(v) if v > 0 => v,
        _ => default_value,
    }
}

fn xorshift64(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn fill_payload(buf: &mut [u8], tick: u64, seq: u64, state: &mut u64) {
    let mut i = 0usize;
    while i < buf.len() {
        let v = xorshift64(state) ^ tick.rotate_left(17) ^ seq.rotate_left(3);
        let bytes = v.to_le_bytes();
        let take = (buf.len() - i).min(8);
        buf[i..i + take].copy_from_slice(&bytes[..take]);
        i += take;
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut ticks: u64 = 10_000;
    let mut snapshot_every: u64 = 1;
    let mut state_bytes: usize = 64 * 1024 * 1024;
    let mut segment_mb: u32 = 512;
    let mut out_dir = PathBuf::from("audit_runs");
    let mut run_label = String::from("gpu-full-state");
    let mut lane = String::from("full-state");
    let mut schema = String::from("pingpong-v1");

    let mut i = 1usize;
    while i < args.len() {
        match args[i].as_str() {
            "--ticks" => {
                ticks = parse_u64_arg(args.get(i + 1), ticks);
                i += 1;
            }
            "--snapshot-every" => {
                snapshot_every = parse_u64_arg(args.get(i + 1), snapshot_every);
                i += 1;
            }
            "--state-bytes" => {
                state_bytes = parse_usize_arg(args.get(i + 1), state_bytes);
                i += 1;
            }
            "--segment-mb" => {
                segment_mb = parse_u32_arg(args.get(i + 1), segment_mb);
                i += 1;
            }
            "--out-dir" => {
                if let Some(v) = args.get(i + 1) {
                    out_dir = PathBuf::from(v);
                    i += 1;
                }
            }
            "--run-label" => {
                if let Some(v) = args.get(i + 1) {
                    run_label = v.clone();
                    i += 1;
                }
            }
            "--lane" => {
                if let Some(v) = args.get(i + 1) {
                    lane = v.clone();
                    i += 1;
                }
            }
            "--schema" => {
                if let Some(v) = args.get(i + 1) {
                    schema = v.clone();
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    let mut cfg = AuditRunConfig::with_defaults(out_dir);
    cfg.run_label = run_label;
    cfg.schema = schema;
    cfg.lane = lane;
    cfg.max_segment_bytes = (segment_mb as u64) * 1024 * 1024;

    let mut writer = match AuditRunWriter::create(cfg) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("failed to create audit run: {e}");
            std::process::exit(1);
        }
    };

    let mut seq: u64 = 0;
    let mut rng_state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut payload = vec![0u8; state_bytes];

    let start = Instant::now();
    let mut snapshots = 0u64;
    let mut payload_total = 0u128;

    for tick in 0..ticks {
        if tick % snapshot_every != 0 {
            continue;
        }

        fill_payload(&mut payload, tick, seq, &mut rng_state);
        let snap = AuditSnapshotRef {
            tick,
            sequence: seq,
            payload: &payload,
        };

        if let Err(e) = writer.write_snapshot(snap) {
            eprintln!("failed writing snapshot at tick {tick}: {e}");
            std::process::exit(1);
        }

        seq = seq.saturating_add(1);
        snapshots = snapshots.saturating_add(1);
        payload_total = payload_total.saturating_add(state_bytes as u128);
    }

    let run_dir = writer.run_dir().to_path_buf();
    if let Err(e) = writer.finish() {
        eprintln!("failed finalizing audit run: {e}");
        std::process::exit(1);
    }

    let elapsed = start.elapsed().as_secs_f64().max(1e-9);
    let gib = (payload_total as f64) / (1024.0 * 1024.0 * 1024.0);
    let gib_per_s = gib / elapsed;

    println!("audit run complete");
    println!("snapshots={snapshots}");
    println!("payload_bytes={payload_total}");
    println!("elapsed_s={elapsed:.3}");
    println!("throughput_gib_s={gib_per_s:.3}");
    println!("run_dir={}", run_dir.display());
}
