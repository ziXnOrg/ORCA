//! WAL replay CLI for time-travel debugging (Phase 4).

#![deny(unsafe_code)]

use clap::{Parser, Subcommand};
use event_log::{EventRecord, JsonlEventLog};
use serde_json::{json, Value};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "orca-replay", about = "Replay ORCA WAL events for debugging")]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Show high-level stats for a WAL file
    Inspect {
        #[arg(short, long)]
        wal: PathBuf,
        #[arg(short = 'r', long)]
        run_id: Option<String>,
    },
    /// Replay events to stdout with filters
    Replay {
        #[arg(short, long)]
        wal: PathBuf,
        #[arg(short = 'r', long)]
        run_id: Option<String>,
        #[arg(long, default_value_t = 0)]
        from: u64,
        #[arg(long, default_value_t = u64::MAX)]
        to: u64,
        #[arg(long, default_value_t = 0)]
        since_ts_ms: u64,
        #[arg(long, default_value_t = 0)]
        max: u64,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        #[arg(short, long, default_value_t = false)]
        interactive: bool,
    },
    /// Convert events into a simple trace JSON for downstream tools
    ToTrace {
        #[arg(short, long)]
        wal: PathBuf,
        #[arg(short = 'r', long)]
        run_id: String,
        #[arg(long, default_value_t = 0)]
        from: u64,
        #[arg(long, default_value_t = u64::MAX)]
        to: u64,
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.cmd {
        Command::Inspect { wal, run_id } => cmd_inspect(&wal, run_id.as_deref())?,
        Command::Replay { wal, run_id, from, to, since_ts_ms, max, dry_run, interactive } => {
            cmd_replay(&wal, run_id.as_deref(), from, to, since_ts_ms, max, dry_run, interactive)?
        }
        Command::ToTrace { wal, run_id, from, to, out } => {
            cmd_to_trace(&wal, &run_id, from, to, out.as_ref().map(|p| p.as_path()))?
        }
    }
    Ok(())
}

fn load_events(
    wal: &PathBuf,
    run_id: Option<&str>,
    from: u64,
    to: u64,
    since_ts_ms: u64,
    max: u64,
) -> Result<Vec<EventRecord<Value>>, Box<dyn std::error::Error>> {
    let log = JsonlEventLog::open(wal)?;
    let mut recs: Vec<EventRecord<Value>> = log.read_range(from, to)?;
    if let Some(rid) = run_id {
        recs.retain(|rec| {
            let p = &rec.payload;
            let run = p
                .get("run_id")
                .and_then(|v| v.as_str())
                .or_else(|| p.get("workflow_id").and_then(|v| v.as_str()));
            run == Some(rid)
        });
    }
    if since_ts_ms > 0 {
        recs.retain(|rec| rec.ts_ms >= since_ts_ms);
    }
    if max > 0 && recs.len() as u64 > max {
        recs.truncate(max as usize);
    }
    Ok(recs)
}

fn cmd_inspect(wal: &PathBuf, run_id: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let recs = load_events(wal, run_id, 0, u64::MAX, 0, 0)?;
    let total = recs.len();
    let first_id = recs.first().map(|r| r.id).unwrap_or(0);
    let last_id = recs.last().map(|r| r.id).unwrap_or(0);
    let first_ts = recs.first().map(|r| r.ts_ms).unwrap_or(0);
    let last_ts = recs.last().map(|r| r.ts_ms).unwrap_or(0);
    let mut by_event = std::collections::BTreeMap::<String, usize>::new();
    for rec in &recs {
        let kind = rec
            .payload
            .get("event")
            .and_then(|v| v.as_str())
            .unwrap_or("event")
            .to_string();
        *by_event.entry(kind).or_default() += 1;
    }
    let out = json!({
        "total": total,
        "first_id": first_id,
        "last_id": last_id,
        "first_ts_ms": first_ts,
        "last_ts_ms": last_ts,
        "by_event": by_event,
    });
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

fn cmd_replay(
    wal: &PathBuf,
    run_id: Option<&str>,
    from: u64,
    to: u64,
    since_ts_ms: u64,
    max: u64,
    dry_run: bool,
    interactive: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let recs = load_events(wal, run_id, from, to, since_ts_ms, max)?;
    if dry_run {
        println!("events={}", recs.len());
        return Ok(());
    }
    println!("=== Replaying WAL: {:?} ===", wal);
    for (idx, rec) in recs.iter().enumerate() {
        let p: &Value = &rec.payload;
        println!(
            "[{}] id={} ts={} event={:?}",
            idx,
            rec.id,
            rec.ts_ms,
            p.get("event")
        );
        if interactive {
            println!("  payload: {}", serde_json::to_string_pretty(p)?);
            println!("Press Enter to continue...");
            let mut buf = String::new();
            std::io::stdin().read_line(&mut buf)?;
        }
    }
    println!("=== Replay complete ({}) ===", recs.len());
    Ok(())
}

fn cmd_to_trace(
    wal: &PathBuf,
    run_id: &str,
    from: u64,
    to: u64,
    out: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let recs = load_events(wal, Some(run_id), from, to, 0, 0)?;
    let mut items = Vec::with_capacity(recs.len());
    for rec in recs {
        items.push(json!({
            "run_id": run_id,
            "event": rec.payload.get("event").and_then(|v| v.as_str()).unwrap_or("event"),
            "ts_ms": rec.ts_ms,
            "record_id": rec.id,
            "payload": rec.payload,
        }));
    }
    let out_str = serde_json::to_string_pretty(&items)?;
    if let Some(path) = out {
        let mut f = File::create(path)?;
        f.write_all(out_str.as_bytes())?;
        println!("wrote trace JSON to {:?}", path);
    } else {
        println!("{}", out_str);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_sample_wal(dir: &std::path::Path) -> PathBuf {
        let wal = dir.join("log.jsonl");
        let log = JsonlEventLog::open(&wal).unwrap();
        let ts = orca_core::ids::now_ms();
        let _ = log.append(1, ts, &json!({"event":"start_run","workflow_id":"R1"})).unwrap();
        let _ = log.append(2, ts + 1, &json!({"event":"task_enqueued","run_id":"R1","envelope":{"id":"e1"}})).unwrap();
        let _ = log.append(3, ts + 2, &json!({"event":"usage_update","run_id":"R1","tokens":10,"cost_micros":1000})).unwrap();
        let _ = log.append(4, ts + 3, &json!({"event":"task_enqueued","run_id":"R2","envelope":{"id":"e2"}})).unwrap();
        wal
    }

    #[test]
    fn filter_by_run_and_range() {
        let dir = tempdir().unwrap();
        let wal = write_sample_wal(dir.path());
        let recs = load_events(&wal, Some("R1"), 2, 3, 0, 0).unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].id, 2);
    }

    #[test]
    fn since_ts_and_max() {
        let dir = tempdir().unwrap();
        let wal = write_sample_wal(dir.path());
        let all = load_events(&wal, None, 0, u64::MAX, 0, 0).unwrap();
        let since = load_events(&wal, None, 0, u64::MAX, all[1].ts_ms, 0).unwrap();
        assert!(since.len() <= all.len());
        let limited = load_events(&wal, None, 0, u64::MAX, 0, 2).unwrap();
        assert_eq!(limited.len(), 2);
    }

    #[test]
    fn to_trace_deterministic_output() {
        let dir = tempdir().unwrap();
        let wal = write_sample_wal(dir.path());
        let out1 = dir.path().join("trace1.json");
        let out2 = dir.path().join("trace2.json");
        cmd_to_trace(&wal, "R1", 0, u64::MAX, Some(&out1)).unwrap();
        cmd_to_trace(&wal, "R1", 0, u64::MAX, Some(&out2)).unwrap();
        let s1 = std::fs::read_to_string(out1).unwrap();
        let s2 = std::fs::read_to_string(out2).unwrap();
        assert_eq!(s1, s2);
    }
}
