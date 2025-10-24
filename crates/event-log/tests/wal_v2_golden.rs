use event_log::v2::{to_jsonl_line, EventTypeV2, RecordV2, WAL_VERSION_V2};
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
        payload: json!({"workflow_id": "WF1"}),
        metadata: json!({}),
    };
    let rec2 = RecordV2 {
        id: 2,
        ts_ms: 1001,
        version: WAL_VERSION_V2,
        event_type: EventTypeV2::TaskEnqueued,
        run_id: "R1".to_string(),
        trace_id: "T1".to_string(),
        payload: json!({"envelope_id": "EV1", "agent": "a1"}),
        metadata: json!({}),
    };
    let rec3 = RecordV2 {
        id: 3,
        ts_ms: 1002,
        version: WAL_VERSION_V2,
        event_type: EventTypeV2::UsageUpdate,
        run_id: "R1".to_string(),
        trace_id: "T1".to_string(),
        payload: json!({"tokens": 123, "cost_micros": 456789}),
        metadata: json!({}),
    };

    let got = vec![rec1, rec2, rec3]
        .into_iter()
        .map(|r| to_jsonl_line(&r).unwrap())
        .collect::<Vec<_>>()
        .join("\n");

    let expected = std::fs::read_to_string("tests/golden/wal_v2_sample.jsonl").unwrap();
    assert_eq!(got + "\n", expected);
}

