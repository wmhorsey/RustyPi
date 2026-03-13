use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const FILE_MAGIC: &[u8; 8] = b"RPAUDIT1";
const RECORD_MAGIC: u32 = 0x5250_534E; // "RPSN"

pub struct AuditRunConfig {
    pub out_dir: PathBuf,
    pub run_label: String,
    pub schema: String,
    pub lane: String,
    pub max_segment_bytes: u64,
}

impl AuditRunConfig {
    pub fn with_defaults<P: AsRef<Path>>(out_dir: P) -> Self {
        Self {
            out_dir: out_dir.as_ref().to_path_buf(),
            run_label: String::from("gpu-audit"),
            schema: String::from("pingpong-v1"),
            lane: String::from("full-state"),
            max_segment_bytes: 512 * 1024 * 1024,
        }
    }
}

pub struct AuditSnapshotRef<'a> {
    pub tick: u64,
    pub sequence: u64,
    pub payload: &'a [u8],
}

pub struct AuditRunWriter {
    run_dir: PathBuf,
    segment_id: u32,
    segment_path: PathBuf,
    segment_writer: BufWriter<File>,
    segment_bytes_written: u64,
    max_segment_bytes: u64,
    manifest_writer: BufWriter<File>,
    lane: String,
    schema: String,
    snapshots: u64,
    payload_bytes: u64,
    started_unix_ms: u128,
}

impl AuditRunWriter {
    pub fn create(config: AuditRunConfig) -> std::io::Result<Self> {
        let now_ms = unix_millis();
        let run_name = format!("{}-{}", sanitize_label(&config.run_label), now_ms);
        let run_dir = config.out_dir.join(run_name);
        fs::create_dir_all(&run_dir)?;

        let manifest_path = run_dir.join("manifest.jsonl");
        let manifest_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&manifest_path)?;
        let mut manifest_writer = BufWriter::new(manifest_file);

        // Header row for audit tooling compatibility.
        writeln!(
            manifest_writer,
            "{{\"kind\":\"run-start\",\"started_unix_ms\":{},\"schema\":\"{}\",\"lane\":\"{}\"}}",
            now_ms,
            json_escape(&config.schema),
            json_escape(&config.lane)
        )?;

        let (segment_path, segment_writer) = create_segment(&run_dir, 0)?;

        Ok(Self {
            run_dir,
            segment_id: 0,
            segment_path,
            segment_writer,
            segment_bytes_written: FILE_MAGIC.len() as u64,
            max_segment_bytes: config.max_segment_bytes.max(64 * 1024 * 1024),
            manifest_writer,
            lane: config.lane,
            schema: config.schema,
            snapshots: 0,
            payload_bytes: 0,
            started_unix_ms: now_ms,
        })
    }

    pub fn run_dir(&self) -> &Path {
        &self.run_dir
    }

    pub fn write_snapshot(&mut self, snap: AuditSnapshotRef<'_>) -> std::io::Result<()> {
        let payload_len = snap.payload.len() as u64;
        let record_len = record_overhead() as u64 + payload_len;
        if self.segment_bytes_written + record_len > self.max_segment_bytes {
            self.rotate_segment()?;
        }

        let offset = self.segment_bytes_written;
        let hash = blake3::hash(snap.payload);

        write_u32(&mut self.segment_writer, RECORD_MAGIC)?;
        write_u64(&mut self.segment_writer, snap.tick)?;
        write_u64(&mut self.segment_writer, snap.sequence)?;
        write_u64(&mut self.segment_writer, payload_len)?;
        write_u128(&mut self.segment_writer, unix_millis())?;
        self.segment_writer.write_all(hash.as_bytes())?;
        self.segment_writer.write_all(snap.payload)?;

        self.segment_bytes_written += record_len;
        self.snapshots += 1;
        self.payload_bytes += payload_len;

        writeln!(
            self.manifest_writer,
            "{{\"kind\":\"snapshot\",\"segment\":{},\"segment_file\":\"{}\",\"offset\":{},\"tick\":{},\"sequence\":{},\"payload_bytes\":{},\"blake3\":\"{}\",\"schema\":\"{}\",\"lane\":\"{}\"}}",
            self.segment_id,
            json_escape(file_name(&self.segment_path)),
            offset,
            snap.tick,
            snap.sequence,
            payload_len,
            hash.to_hex(),
            json_escape(&self.schema),
            json_escape(&self.lane)
        )?;

        Ok(())
    }

    pub fn finish(mut self) -> std::io::Result<()> {
        self.segment_writer.flush()?;
        self.manifest_writer.flush()?;

        let finished_ms = unix_millis();
        let summary_path = self.run_dir.join("run.json");
        let mut summary = BufWriter::new(File::create(summary_path)?);

        writeln!(
            summary,
            "{{\"kind\":\"run-summary\",\"started_unix_ms\":{},\"finished_unix_ms\":{},\"duration_ms\":{},\"snapshots\":{},\"payload_bytes\":{},\"schema\":\"{}\",\"lane\":\"{}\"}}",
            self.started_unix_ms,
            finished_ms,
            finished_ms.saturating_sub(self.started_unix_ms),
            self.snapshots,
            self.payload_bytes,
            json_escape(&self.schema),
            json_escape(&self.lane)
        )?;
        summary.flush()?;

        Ok(())
    }

    fn rotate_segment(&mut self) -> std::io::Result<()> {
        self.segment_writer.flush()?;
        self.segment_id = self.segment_id.saturating_add(1);
        let (path, writer) = create_segment(&self.run_dir, self.segment_id)?;
        self.segment_path = path;
        self.segment_writer = writer;
        self.segment_bytes_written = FILE_MAGIC.len() as u64;
        Ok(())
    }
}

fn create_segment(run_dir: &Path, segment_id: u32) -> std::io::Result<(PathBuf, BufWriter<File>)> {
    let name = format!("segment_{:05}.rpa", segment_id);
    let path = run_dir.join(name);
    let file = File::create(&path)?;
    let mut writer = BufWriter::new(file);
    writer.write_all(FILE_MAGIC)?;
    Ok((path, writer))
}

fn record_overhead() -> usize {
    // magic + tick + sequence + payload_len + ts + hash
    4 + 8 + 8 + 8 + 16 + 32
}

fn write_u32<W: Write>(w: &mut W, value: u32) -> std::io::Result<()> {
    w.write_all(&value.to_le_bytes())
}

fn write_u64<W: Write>(w: &mut W, value: u64) -> std::io::Result<()> {
    w.write_all(&value.to_le_bytes())
}

fn write_u128<W: Write>(w: &mut W, value: u128) -> std::io::Result<()> {
    w.write_all(&value.to_le_bytes())
}

fn unix_millis() -> u128 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_millis(),
        Err(_) => 0,
    }
}

fn file_name(path: &Path) -> &str {
    match path.file_name().and_then(|s| s.to_str()) {
        Some(v) => v,
        None => "segment_unknown.rpa",
    }
}

fn sanitize_label(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        String::from("run")
    } else {
        out
    }
}

fn json_escape(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}
