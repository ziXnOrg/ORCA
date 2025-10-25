use event_log::v2::{
    to_jsonl_line, EventTypeV2, RecordV2, StartRunPayload, TaskEnqueuedPayload, UsageUpdatePayload,
    WAL_VERSION_V2,
};
use serde_json::json;

#[test]
fn wal_v2_sample_golden_red() {
    let rec1 = RecordV2 {
        id: 1,
        ts_ms: 1000,
        version: WAL_VERSION_V2,
        event_type: EventTypeV2::StartRun,
        run_id: "R1".to_string(),
        trace_id: "T1".to_string(),
        payload: StartRunPayload { workflow_id: "WF1".into() },
        metadata: json!({}),
    };
    let rec2 = RecordV2 {
        id: 2,
        ts_ms: 1001,
        version: WAL_VERSION_V2,
        event_type: EventTypeV2::TaskEnqueued,
        run_id: "R1".to_string(),
        trace_id: "T1".to_string(),
        payload: TaskEnqueuedPayload { envelope_id: "EV1".into(), agent: "a1".into() },
        metadata: json!({}),
    };
    let rec3 = RecordV2 {
        id: 3,
        ts_ms: 1002,
        version: WAL_VERSION_V2,
        event_type: EventTypeV2::UsageUpdate,
        run_id: "R1".to_string(),
        trace_id: "T1".to_string(),
        payload: UsageUpdatePayload { tokens: 123, cost_micros: 456789 },
        metadata: json!({}),
    };

    let got = [
        to_jsonl_line(&rec1).unwrap(),
        to_jsonl_line(&rec2).unwrap(),
        to_jsonl_line(&rec3).unwrap(),
    ]
    .join("\n");

    let expected = std::fs::read_to_string("tests/golden/wal_v2_sample.jsonl").unwrap();
    assert_eq!(got + "\n", expected);
}
