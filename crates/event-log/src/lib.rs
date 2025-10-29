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
    #[error("invalid: {0}")]
    Invalid(String),
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

/// WAL v2 typed schema with deterministic serialization and golden-tested stable ordering.
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
        ExternalIoStarted,
        ExternalIoFinished,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub struct Attachment {
        pub digest_sha256: String,
        pub size_bytes: u64,
        pub mime: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub encoding: Option<String>,
        pub compression: String, // "zstd" | "none"
    }

    impl Ord for Attachment {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            self.digest_sha256.cmp(&other.digest_sha256)
        }
    }
    impl PartialOrd for Attachment {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub attachments: Option<Vec<Attachment>>, // new field; serialized after payload
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

    // External I/O capture payloads
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ExternalIOStartedPayload {
        pub system: String,    // "grpc" | "http"
        pub direction: String, // "client" | "server"
        pub scheme: String,    // e.g., "grpc"
        pub host: String,
        pub port: u16,
        pub method: String,     // rpc.service + "/" + rpc.method
        pub request_id: String, // deterministic correlation id
        pub headers: serde_json::Map<String, serde_json::Value>, // redacted map
        pub body_digest_sha256: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ExternalIOFinishedPayload {
        pub request_id: String,
        pub status: String, // e.g., "ok" | grpc status code
        pub duration_ms: u64,
    }

    const ATTACH_MAX_COUNT: usize = 8;
    const STR_MAX_LEN: usize = 128;
    const TOTAL_ATTACH_JSON_MAX: usize = 8 * 1024; // bytes

    fn is_hex_sha256(s: &str) -> bool {
        s.len() == 64 && s.as_bytes().iter().all(|b| b.is_ascii_hexdigit())
    }

    /// Serialize a V2 record to a JSON line with stable field ordering and deterministic attachment ordering.
    pub fn to_jsonl_line<T: Serialize>(rec: &RecordV2<T>) -> Result<String, super::EventLogError> {
        // Validate + sort attachments deterministically by digest
        let mut sorted: Option<Vec<Attachment>> = None;
        if let Some(att) = &rec.attachments {
            if att.len() > ATTACH_MAX_COUNT {
                return Err(super::EventLogError::Invalid(format!(
                    "attachments count {} exceeds max {}",
                    att.len(),
                    ATTACH_MAX_COUNT
                )));
            }
            let mut a = att.clone();
            for x in &a {
                if !is_hex_sha256(&x.digest_sha256) {
                    return Err(super::EventLogError::Invalid("invalid digest".into()));
                }
                if x.mime.len() > STR_MAX_LEN
                    || x.encoding.as_deref().map(|e| e.len()).unwrap_or(0) > STR_MAX_LEN
                    || x.compression.len() > STR_MAX_LEN
                {
                    return Err(super::EventLogError::Invalid(
                        "oversized attachment string field".into(),
                    ));
                }
            }
            a.sort();
            // Rough size cap via JSON length of attachments only
            let approx = serde_json::to_string(&a).map_err(super::EventLogError::Serde)?.len();
            if approx > TOTAL_ATTACH_JSON_MAX {
                return Err(super::EventLogError::Invalid("attachments too large".into()));
            }
            sorted = Some(a);
        }

        // Build a serialization wrapper to control field order explicitly.
        #[derive(Serialize)]
        struct RecordV2Ser<'a, T: Serialize> {
            id: super::EventId,
            ts_ms: u64,
            version: u8,
            event_type: &'a EventTypeV2,
            run_id: &'a str,
            trace_id: &'a str,
            payload: &'a T,
            #[serde(skip_serializing_if = "Option::is_none")]
            attachments: Option<&'a [Attachment]>,
            metadata: &'a Value,
        }

        let ser = RecordV2Ser {
            id: rec.id,
            ts_ms: rec.ts_ms,
            version: rec.version,
            event_type: &rec.event_type,
            run_id: &rec.run_id,
            trace_id: &rec.trace_id,
            payload: &rec.payload,
            attachments: sorted.as_deref(),
            metadata: &rec.metadata,
        };

        let s = serde_json::to_string(&ser)?;
        Ok(s)
    }
}
