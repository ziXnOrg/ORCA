use event_log::{EventRecord, JsonlEventLog};
use serde_json::Value;

#[test]
fn v1_roundtrip_still_works() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let log = JsonlEventLog::open(tmp.path()).unwrap();

    let _ =
        log.append(1, 1000, &serde_json::json!({"event":"start_run","workflow_id":"WF1"})).unwrap();

    let got: Vec<EventRecord<Value>> = log.read_range(1, 2).unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].id, 1);
    assert_eq!(got[0].ts_ms, 1000);
    assert_eq!(got[0].payload.get("event").and_then(|v| v.as_str()), Some("start_run"));
}

#[test]
fn v2_records_can_be_read_by_v1_record_type() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path();
    // Write the v2 golden lines directly
    std::fs::write(path, include_str!("golden/wal_v2_sample.jsonl")).unwrap();

    let log = JsonlEventLog::open(path).unwrap();
    let got: Vec<EventRecord<Value>> = log.read_range(1, 4).unwrap();

    assert_eq!(got.len(), 3);
    // Fields still mapped correctly
    assert_eq!(got[0].id, 1);
    assert_eq!(got[1].id, 2);
    assert_eq!(got[2].id, 3);

    // Payload remains accessible as JSON values
    assert_eq!(got[0].payload.get("workflow_id").and_then(|v| v.as_str()), Some("WF1"));
    assert_eq!(got[1].payload.get("envelope_id").and_then(|v| v.as_str()), Some("EV1"));
    assert_eq!(got[1].payload.get("agent").and_then(|v| v.as_str()), Some("a1"));
    assert_eq!(got[2].payload.get("tokens").and_then(|v| v.as_u64()), Some(123));
    assert_eq!(got[2].payload.get("cost_micros").and_then(|v| v.as_u64()), Some(456789));
}
