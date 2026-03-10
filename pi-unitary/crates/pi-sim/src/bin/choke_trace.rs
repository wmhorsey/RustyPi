use std::env;
use std::fs;
use std::path::PathBuf;

use pi_sim::{run_choke_scenario, ChokeScenarioConfig};

fn parse_usize_arg(value: Option<&String>, default_value: usize) -> usize {
    match value.and_then(|v| v.parse::<usize>().ok()) {
        Some(v) if v > 0 => v,
        _ => default_value,
    }
}

fn parse_u16_arg(value: Option<&String>, default_value: u16) -> u16 {
    match value.and_then(|v| v.parse::<u16>().ok()) {
        Some(v) => v,
        _ => default_value,
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut ticks = 256usize;
    let mut nodes = 4usize;
    let mut target = 0u16;
    let mut out: Option<PathBuf> = None;

    let mut i = 1usize;
    while i < args.len() {
        match args[i].as_str() {
            "--ticks" => {
                ticks = parse_usize_arg(args.get(i + 1), ticks);
                i += 1;
            }
            "--nodes" => {
                nodes = parse_usize_arg(args.get(i + 1), nodes);
                i += 1;
            }
            "--target" => {
                target = parse_u16_arg(args.get(i + 1), target);
                i += 1;
            }
            "--out" => {
                if let Some(path) = args.get(i + 1) {
                    out = Some(PathBuf::from(path));
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    let cfg = ChokeScenarioConfig {
        ticks,
        nodes,
        target_tick: target,
    };

    let rows = match run_choke_scenario(cfg) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("scenario error: {e}");
            std::process::exit(1);
        }
    };

    let mut csv = String::new();
    csv.push_str(pi_sim::ChokeTraceRow::csv_header());
    csv.push('\n');
    for row in rows {
        csv.push_str(&row.to_csv_line());
        csv.push('\n');
    }

    if let Some(path) = out {
        if let Err(e) = fs::write(&path, csv) {
            eprintln!("failed writing {}: {e}", path.display());
            std::process::exit(1);
        }
    } else {
        print!("{csv}");
    }
}
