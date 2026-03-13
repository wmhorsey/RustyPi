use std::env;
use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use pi_sim::choke_schema::{phase_name, pathway_name, GPU_CHOKE_NODE_BYTES};

const FILE_MAGIC: &[u8; 8] = b"RPAUDIT1";
const RECORD_MAGIC: u32 = 0x5250_534E;
const RECORD_HEADER_BYTES: usize = 4 + 8 + 8 + 8 + 16 + 32;

fn parse_path_arg(value: Option<&String>, default_path: PathBuf) -> PathBuf {
    match value {
        Some(v) => PathBuf::from(v),
        None => default_path,
    }
}

fn read_u32_le(buf: &[u8], offset: usize) -> Result<u32, String> {
    let end = offset.saturating_add(4);
    if end > buf.len() {
        return Err("u32 out of bounds".to_string());
    }
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&buf[offset..end]);
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64_le(buf: &[u8], offset: usize) -> Result<u64, String> {
    let end = offset.saturating_add(8);
    if end > buf.len() {
        return Err("u64 out of bounds".to_string());
    }
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&buf[offset..end]);
    Ok(u64::from_le_bytes(bytes))
}

fn list_segments(run_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    let entries = fs::read_dir(run_dir).map_err(|e| format!("read_dir failed: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("dir entry failed: {e}"))?;
        let path = entry.path();
        let is_rpa = path
            .extension()
            .and_then(|v| v.to_str())
            .map(|v| v.eq_ignore_ascii_case("rpa"))
            .unwrap_or(false);
        if is_rpa {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

fn decode_payload_rows<W: Write>(
    mut writer: W,
    tick: u64,
    sequence: u64,
    payload: &[u8],
) -> Result<u64, String> {
    if payload.len() % GPU_CHOKE_NODE_BYTES != 0 {
        return Err(format!(
            "payload bytes {} not divisible by node record bytes {}",
            payload.len(),
            GPU_CHOKE_NODE_BYTES
        ));
    }

    let node_count = payload.len() / GPU_CHOKE_NODE_BYTES;
    for node_idx in 0..node_count {
        let base = node_idx * GPU_CHOKE_NODE_BYTES;
        let phase_tick = read_u32_le(payload, base)?;
        let coherence = read_u32_le(payload, base + 4)? as i32;
        let energy = read_u32_le(payload, base + 8)? as i32;
        let shell_ring_ticks = read_u32_le(payload, base + 12)? as i32;
        let spin_bias = read_u32_le(payload, base + 16)? as i32;
        let phase_id = read_u32_le(payload, base + 20)?;
        let pathway_id = read_u32_le(payload, base + 24)?;
        let drive = read_u32_le(payload, base + 28)? as i32;

        writeln!(
            writer,
            "{tick},{sequence},{node_idx},{phase_tick},{coherence},{energy},{shell_ring_ticks},{spin_bias},{phase_id},{},{pathway_id},{},{}",
            phase_name(phase_id),
            pathway_name(pathway_id),
            drive
        )
        .map_err(|e| format!("csv write failed: {e}"))?;
    }

    Ok(node_count as u64)
}

fn decode_segment<W: Write>(mut writer: W, segment_path: &Path) -> Result<(u64, u64), String> {
    let mut f = File::open(segment_path)
        .map_err(|e| format!("failed to open {}: {e}", segment_path.display()))?;
    let mut data = Vec::new();
    f.read_to_end(&mut data)
        .map_err(|e| format!("failed to read {}: {e}", segment_path.display()))?;

    if data.len() < FILE_MAGIC.len() || &data[0..FILE_MAGIC.len()] != FILE_MAGIC {
        return Err(format!("invalid segment magic in {}", segment_path.display()));
    }

    let mut cursor = FILE_MAGIC.len();
    let mut snapshots = 0u64;
    let mut rows = 0u64;

    while cursor < data.len() {
        if cursor + RECORD_HEADER_BYTES > data.len() {
            return Err(format!(
                "truncated record header in {} at byte {}",
                segment_path.display(),
                cursor
            ));
        }

        let magic = read_u32_le(&data, cursor)?;
        if magic != RECORD_MAGIC {
            return Err(format!(
                "invalid record magic in {} at byte {}",
                segment_path.display(),
                cursor
            ));
        }
        cursor += 4;

        let tick = read_u64_le(&data, cursor)?;
        cursor += 8;
        let sequence = read_u64_le(&data, cursor)?;
        cursor += 8;
        let payload_len = read_u64_le(&data, cursor)? as usize;
        cursor += 8;

        // Skip write timestamp and payload hash; both are preserved in manifest.
        cursor += 16;
        cursor += 32;

        if cursor + payload_len > data.len() {
            return Err(format!(
                "truncated payload in {} at byte {}",
                segment_path.display(),
                cursor
            ));
        }

        let payload = &data[cursor..cursor + payload_len];
        rows += decode_payload_rows(&mut writer, tick, sequence, payload)?;
        snapshots += 1;
        cursor += payload_len;
    }

    Ok((snapshots, rows))
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    let mut run_dir = PathBuf::from("audit_runs");
    let mut out_csv = PathBuf::from("decoded_choke_nodes.csv");

    let mut i = 1usize;
    while i < args.len() {
        match args[i].as_str() {
            "--run-dir" => {
                run_dir = parse_path_arg(args.get(i + 1), run_dir);
                i += 1;
            }
            "--out" => {
                out_csv = parse_path_arg(args.get(i + 1), out_csv);
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }

    if !run_dir.is_dir() {
        return Err(format!("run dir not found: {}", run_dir.display()));
    }

    if out_csv.is_relative() {
        out_csv = run_dir.join(out_csv);
    }

    let segments = list_segments(&run_dir)?;
    if segments.is_empty() {
        return Err(format!("no .rpa segments found in {}", run_dir.display()));
    }

    let file = File::create(&out_csv)
        .map_err(|e| format!("failed to create csv {}: {e}", out_csv.display()))?;
    let mut writer = BufWriter::new(file);

    writeln!(
        writer,
        "tick,sequence,node,phase_tick,coherence,energy,shell_ring_ticks,spin_bias,phase_id,phase,pathway_id,pathway,drive"
    )
    .map_err(|e| format!("csv header write failed: {e}"))?;

    let mut total_snapshots = 0u64;
    let mut total_rows = 0u64;
    for segment in segments {
        let (snapshots, rows) = decode_segment(&mut writer, &segment)?;
        total_snapshots += snapshots;
        total_rows += rows;
    }

    writer
        .flush()
        .map_err(|e| format!("csv flush failed: {e}"))?;

    println!("decoded choke audit complete");
    println!("snapshots={total_snapshots}");
    println!("rows={total_rows}");
    println!("csv={}", out_csv.display());

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("gpu_audit_decode_choke failed: {e}");
        std::process::exit(1);
    }
}
