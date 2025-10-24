//! Write-ahead event log prototype API (Phase 0).

#![deny(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use thiserror::Error;

/// Placeholder type for an event identifier.
pub type EventId = u64;

/// Errors emitted by the event log.
#[derive(Debug, Error)]
pub enum EventLogError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialize: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Minimal event record persisted to the log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord<T> {
    /// Monotonic event id assigned on append.
    pub id: EventId,
    /// Millis since epoch (caller supplies; Phase 0 keeps it simple).
    pub ts_ms: u64,
    /// Payload (schema defined elsewhere; Phase 0 uses generic T).
    pub payload: T,
}

/// A simple JSONL-backed append-only event log.
#[derive(Debug, Clone)]
pub struct JsonlEventLog {
    path: String,
}

impl JsonlEventLog {
    /// Create or open a log at `path`.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, EventLogError> {
        let p = path.as_ref();
        if !p.exists() {
            OpenOptions::new().create(true).write(true).truncate(true).open(p)?;
        }
        Ok(Self { path: p.to_string_lossy().into_owned() })
    }

    /// Append a payload; returns assigned EventId.
    pub fn append<T: Serialize>(
        &self,
        id: EventId,
        ts_ms: u64,
        payload: &T,
    ) -> Result<EventId, EventLogError> {
        let mut file = OpenOptions::new().append(true).open(&self.path)?;
        let rec = EventRecord { id, ts_ms, payload };
        let line = serde_json::to_string(&rec)?;
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
        file.flush()?;
        Ok(id)
    }

    /// Read events with id in [start, end) (half-open range).
    pub fn read_range<T: for<'de> Deserialize<'de>>(
        &self,
        start: EventId,
        end: EventId,
    ) -> Result<Vec<EventRecord<T>>, EventLogError> {
        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);
        let mut out = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }
            let rec: EventRecord<T> = serde_json::from_str(&line)?;
            if rec.id >= start && rec.id < end {
                out.push(rec);
            }
        }
        Ok(out)
    }
}

/// Example usage (doc test):
///
/// ```
/// use event_log::{JsonlEventLog, EventId};
/// use serde::{Serialize, Deserialize};
/// use std::time::{SystemTime, UNIX_EPOCH};
///
/// #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// struct P { v: u32 }
///
/// fn ts() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64 }
///
/// let path = tempfile::NamedTempFile::new().unwrap();
/// let log = JsonlEventLog::open(path.path()).unwrap();
///
/// let _ = log.append(1 as EventId, ts(), &P { v: 10 }).unwrap();
/// let _ = log.append(2 as EventId, ts(), &P { v: 20 }).unwrap();
///
/// let recs: Vec<event_log::EventRecord<P>> = log.read_range(1, 3).unwrap();
/// assert_eq!(recs.len(), 2);
/// assert_eq!(recs[0].payload, P { v: 10 });
/// assert_eq!(recs[1].payload, P { v: 20 });
/// ```
#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn append_and_read_roundtrip() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let log = JsonlEventLog::open(tmp.path()).unwrap();
        let _ = log.append(1, 1, &"hello").unwrap();
        let got: Vec<EventRecord<String>> = log.read_range(1, 2).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].payload, "hello");
    }
}


/// WAL v2 typed schema (skeleton for RED phase).
pub mod v2 {
    use serde::{Deserialize, Serialize};
    use serde_json::Value;

    pub const WAL_VERSION_V2: u8 = 2;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(rename_all = "snake_case")]
    pub enum EventTypeV2 {
        StartRun,
        TaskEnqueued,
        UsageUpdate,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RecordV2<T> {
        pub id: super::EventId,
        pub ts_ms: u64,
        pub version: u8,
        pub event_type: EventTypeV2,
        pub run_id: String,
        pub trace_id: String,
        pub payload: T,
        pub metadata: Value,
    }

    // Typed payloads to guarantee stable key ordering in serialization.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct StartRunPayload {
        pub workflow_id: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TaskEnqueuedPayload {
        pub envelope_id: String,
        pub agent: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct UsageUpdatePayload {
        pub tokens: u64,
        pub cost_micros: u64,
    }

    /// Serialize a V2 record to a JSON line with stable field ordering.
    pub fn to_jsonl_line<T: Serialize>(rec: &RecordV2<T>) -> Result<String, super::EventLogError> {
        let s = serde_json::to_string(rec)?;
        Ok(s)
    }
}
