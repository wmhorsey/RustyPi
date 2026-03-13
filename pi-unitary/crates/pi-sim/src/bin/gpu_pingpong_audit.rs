use std::env;
use std::mem;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use pi_sim::audit::{AuditRunConfig, AuditRunWriter, AuditSnapshotRef};
use pi_sim::choke_schema::{GPU_CHOKE_NODE_BYTES, TAU_TICKS_DEFAULT_U32};

const SHADER_SRC: &str = r#"
struct Params {
  tick: u32,
    node_count: u32,
    target_phase: u32,
    boundary_tension: u32,
}

struct NodeState {
    phase_tick: u32,
    coherence: i32,
    energy: i32,
    shell_ring_ticks: i32,
    spin_bias: i32,
    phase_id: u32,
    pathway_id: u32,
    drive: i32,
}

@group(0) @binding(0)
var<storage, read> src_nodes: array<NodeState>;

@group(0) @binding(1)
var<storage, read_write> dst_nodes: array<NodeState>;

@group(0) @binding(2)
var<uniform> params: Params;

fn clamp_nonneg(v: i32) -> i32 {
    return max(v, 0);
}

fn shortest_arc_4096(a: u32, b: u32) -> i32 {
    let d = abs(i32(a) - i32(b));
    return min(d, 4096 - d);
}

fn classify_phase(prev_phase: u32, coherence: i32, energy: i32, shell_ring_ticks: i32, spin_bias: i32) -> u32 {
    let tension = min(clamp_nonneg(coherence), clamp_nonneg(energy));
    if (tension <= 0) {
        if (prev_phase == 0u) {
            return 0u;
        }
        if (prev_phase == 2u) {
            return 3u;
        }
        if (prev_phase == 3u) {
            return 4u;
        }
        if (prev_phase == 5u) {
            return 0u;
        }
        return 5u;
    }

    switch prev_phase {
        case 0u: {
            if (coherence > 0 || energy > 0) {
                return 1u;
            }
            return 0u;
        }
        case 1u: {
            if (coherence >= 1 && shell_ring_ticks >= 3 && spin_bias >= 2) {
                return 2u;
            }
            return 1u;
        }
        case 2u: {
            if (coherence >= 8) {
                return 3u;
            }
            return 2u;
        }
        case 3u: {
            if (coherence <= 2) {
                return 4u;
            }
            return 3u;
        }
        case 4u: {
            let pressure = clamp_nonneg(energy) - clamp_nonneg(coherence);
            if (tension <= 0 || pressure > 2) {
                return 5u;
            }
            return 4u;
        }
        default: {
            if (tension <= 0) {
                return 0u;
            }
            return 5u;
        }
    }
}

fn pathway_from_phase(phase_id: u32, energy: i32, coherence: i32) -> u32 {
    switch phase_id {
        case 0u: {
            return 0u;
        }
        case 1u: {
            return 1u;
        }
        case 2u: {
            return 1u;
        }
        case 3u: {
            return 2u;
        }
        case 4u: {
            return 2u;
        }
        default: {
            if (energy > coherence) {
                return 4u;
            }
            return 3u;
        }
    }
}

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let i = gid.x;
    if (i >= params.node_count) {
    return;
  }

    let prev = src_nodes[i];
    var next = prev;

    let pulse_a = ((params.tick + i) % 29u) == 0u;
    let pulse_b = ((params.tick + i + i) % 37u) == 0u;
    let drive = select(0i, 1i, pulse_a || pulse_b);

    if (drive > 0) {
        next.energy = next.energy + drive;
    }

    let boundary_hold = max(1u, params.boundary_tension);
    let phase_step = select(0u, 1u, ((params.tick + i) % boundary_hold) == 0u);
    next.phase_tick = (prev.phase_tick + phase_step) & 4095u;
    let arc = shortest_arc_4096(next.phase_tick, params.target_phase);

    if (arc <= 24 && drive > 0) {
        next.coherence = next.coherence + 4;
    } else {
        next.coherence = max(0, next.coherence - 1);
    }

    let tension = min(clamp_nonneg(next.coherence), clamp_nonneg(next.energy));
    if (drive > 0 && arc <= 24 && tension > 0) {
        next.shell_ring_ticks = next.shell_ring_ticks + 1;
    } else {
        next.shell_ring_ticks = max(0, next.shell_ring_ticks - 1);
    }

    let asymmetry = clamp_nonneg(next.coherence) - clamp_nonneg(next.energy);
    if (drive > 0 && asymmetry > 0) {
        next.spin_bias = next.spin_bias + 1;
    } else {
        next.spin_bias = max(0, next.spin_bias - 1);
    }

    if (drive == 0 && next.energy > 0 && prev.phase_id == 0u) {
        next.energy = next.energy - 1;
    }

    let phase_id = classify_phase(
        prev.phase_id,
        next.coherence,
        next.energy,
        next.shell_ring_ticks,
        next.spin_bias,
    );
    next.phase_id = phase_id;
    if (phase_id == 5u && next.energy > 0) {
        next.energy = next.energy - 1;
    }

    next.pathway_id = pathway_from_phase(next.phase_id, next.energy, next.coherence);
    next.drive = drive;

    dst_nodes[i] = next;
}
"#;

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

fn u32_words_to_le_bytes(words: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(words.len() * 4);
    for &w in words {
        out.extend_from_slice(&w.to_le_bytes());
    }
    out
}

fn main() {
    if let Err(e) = pollster::block_on(run()) {
        eprintln!("gpu_pingpong_audit failed: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    let mut ticks: u64 = 10_000;
    let mut snapshot_every: u64 = 1;
    let mut state_bytes: usize = 64 * 1024 * 1024;
    let mut nodes: usize = 0;
    let mut segment_mb: u32 = 512;
    let mut target_phase: u32 = 0;
    let mut boundary_tension: u32 = 1;
    let mut out_dir = PathBuf::from("audit_runs");
    let mut run_label = String::from("gpu-pingpong");
    let mut lane = String::from("full-state-gpu");
    let mut schema = String::from("choke-node-v1-gpu");

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
            "--nodes" => {
                nodes = parse_usize_arg(args.get(i + 1), nodes);
                i += 1;
            }
            "--segment-mb" => {
                segment_mb = parse_u32_arg(args.get(i + 1), segment_mb);
                i += 1;
            }
            "--target-phase" => {
                target_phase = parse_u32_arg(args.get(i + 1), target_phase) % TAU_TICKS_DEFAULT_U32;
                i += 1;
            }
            "--boundary-tension" => {
                boundary_tension = parse_u32_arg(args.get(i + 1), boundary_tension);
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

    // Node layout is fixed-width so snapshots are schema-stable across runs.
    let node_count = if nodes > 0 {
        nodes
    } else {
        (state_bytes / GPU_CHOKE_NODE_BYTES).max(1)
    };
    let state_bytes = node_count * GPU_CHOKE_NODE_BYTES;

    let mut cfg = AuditRunConfig::with_defaults(out_dir);
    cfg.run_label = run_label;
    cfg.schema = schema;
    cfg.lane = lane;
    cfg.max_segment_bytes = (segment_mb as u64) * 1024 * 1024;

    let mut writer = AuditRunWriter::create(cfg).map_err(|e| e.to_string())?;

    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .ok_or_else(|| String::from("no suitable GPU adapter found"))?;

    let adapter_info = adapter.get_info();
    println!(
        "gpu adapter: {} ({:?})",
        adapter_info.name,
        adapter_info.backend
    );

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("pi-sim-gpu-audit-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            },
            None,
        )
        .await
        .map_err(|e| format!("request_device failed: {e}"))?;

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("pi-sim-gpu-audit-shader"),
        source: wgpu::ShaderSource::Wgsl(SHADER_SRC.into()),
    });

    let storage_usage =
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST;
    let ping = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ping-buffer"),
        size: state_bytes as u64,
        usage: storage_usage,
        mapped_at_creation: false,
    });
    let pong = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pong-buffer"),
        size: state_bytes as u64,
        usage: storage_usage,
        mapped_at_creation: false,
    });

    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("snapshot-readback"),
        size: state_bytes as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let params = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("compute-params"),
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("pingpong-bind-layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let bg_ping_to_pong = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("bg-ping-to-pong"),
        layout: &bind_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: ping.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: pong.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: params.as_entire_binding(),
            },
        ],
    });

    let bg_pong_to_ping = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("bg-pong-to-ping"),
        layout: &bind_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: pong.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: ping.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: params.as_entire_binding(),
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("pingpong-pipeline-layout"),
        bind_group_layouts: &[&bind_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("pingpong-pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: "main",
    });

    // Deterministic seed initialization for audit reproducibility.
    let mut seed_state: u64 = 0x0123_4567_89AB_CDEF;
    let mut init_words = vec![0u32; (state_bytes / mem::size_of::<u32>()).max(1)];
    for idx in 0..node_count {
        let base = idx * 8;
        let v = xorshift64(&mut seed_state);
        init_words[base] = (idx as u32) % TAU_TICKS_DEFAULT_U32; // phase_tick
        init_words[base + 1] = ((v as u32) & 0x3) as i32 as u32; // coherence
        init_words[base + 2] = (((v >> 16) as u32) & 0x3) as i32 as u32; // energy
        init_words[base + 3] = 0; // shell_ring_ticks
        init_words[base + 4] = 0; // spin_bias
        init_words[base + 5] = 0; // phase_id (free)
        init_words[base + 6] = 0; // pathway_id (free_pool)
        init_words[base + 7] = 0; // drive
    }
    let init_bytes = u32_words_to_le_bytes(&init_words);
    queue.write_buffer(&ping, 0, &init_bytes);

    let start = Instant::now();
    let mut snapshots = 0u64;
    let mut payload_total = 0u128;
    let mut active_is_ping = true;
    let boundary_tension = boundary_tension.max(1);

    for tick in 0..ticks {
        let params_words = [
            (tick as u32).wrapping_add(1),
            node_count as u32,
            target_phase,
            boundary_tension,
        ];
        let params_bytes = u32_words_to_le_bytes(&params_words);
        queue.write_buffer(&params, 0, &params_bytes);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("pingpong-encoder"),
        });

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("pingpong-compute-pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&pipeline);
            if active_is_ping {
                cpass.set_bind_group(0, &bg_ping_to_pong, &[]);
            } else {
                cpass.set_bind_group(0, &bg_pong_to_ping, &[]);
            }
            let groups = ((node_count as u32) + 63) / 64;
            cpass.dispatch_workgroups(groups.max(1), 1, 1);
        }

        let snapshot_now = tick % snapshot_every == 0;
        if snapshot_now {
            let src = if active_is_ping { &pong } else { &ping };
            encoder.copy_buffer_to_buffer(src, 0, &staging, 0, state_bytes as u64);
        }

        queue.submit(Some(encoder.finish()));
        active_is_ping = !active_is_ping;

        if snapshot_now {
            let slice = staging.slice(..);
            let (tx, rx) = mpsc::channel();
            slice.map_async(wgpu::MapMode::Read, move |res| {
                let _ = tx.send(res);
            });

            device.poll(wgpu::Maintain::Wait);
            match rx.recv() {
                Ok(Ok(())) => {}
                Ok(Err(e)) => return Err(format!("readback map failed at tick {tick}: {e}")),
                Err(e) => return Err(format!("readback channel failed at tick {tick}: {e}")),
            }

            {
                let mapped = slice.get_mapped_range();
                let snap = AuditSnapshotRef {
                    tick,
                    sequence: snapshots,
                    payload: &mapped,
                };
                writer
                    .write_snapshot(snap)
                    .map_err(|e| format!("audit write failed at tick {tick}: {e}"))?;
            }
            staging.unmap();

            snapshots = snapshots.saturating_add(1);
            payload_total = payload_total.saturating_add(state_bytes as u128);
        }
    }

    let run_dir = writer.run_dir().to_path_buf();
    writer.finish().map_err(|e| format!("finish failed: {e}"))?;

    let elapsed = start.elapsed().as_secs_f64().max(1e-9);
    let gib = (payload_total as f64) / (1024.0 * 1024.0 * 1024.0);
    let gib_per_s = gib / elapsed;

    println!("gpu ping-pong audit complete");
    println!("snapshots={snapshots}");
    println!("payload_bytes={payload_total}");
    println!("elapsed_s={elapsed:.3}");
    println!("throughput_gib_s={gib_per_s:.3}");
    println!("node_count={node_count}");
    println!("node_state_bytes={GPU_CHOKE_NODE_BYTES}");
    println!("run_dir={}", run_dir.display());

    Ok(())
}
