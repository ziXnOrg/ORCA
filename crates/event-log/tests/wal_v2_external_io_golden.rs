use event_log::v2::{
    to_jsonl_line, EventTypeV2, ExternalIOFinishedPayload, ExternalIOStartedPayload, RecordV2,
    WAL_VERSION_V2,
};
use serde_json::json;

#[test]
fn wal_v2_external_io_golden() {
    let started = RecordV2 {
        id: 10,
        ts_ms: 2000,
        version: WAL_VERSION_V2,
        event_type: EventTypeV2::ExternalIoStarted,
        run_id: "R2".into(),
        trace_id: "T2".into(),
        payload: ExternalIOStartedPayload {
            system: "grpc".into(),
            direction: "client".into(),
            scheme: "grpc".into(),
            host: "example.com".into(),
            port: 443,
            method: "orca.v1.Orchestrator/StartRun".into(),
            request_id: "REQ1".into(),
            headers: serde_json::Map::from_iter([(
                "authorization".into(),
                serde_json::Value::String("[REDACTED]".into()),
            )]),
            body_digest_sha256: "00".repeat(32),
        },
        attachments: None,
        metadata: json!({}),
    };

    let finished = RecordV2 {
        id: 11,
        ts_ms: 2003,
        version: WAL_VERSION_V2,
        event_type: EventTypeV2::ExternalIoFinished,
        run_id: "R2".into(),
        trace_id: "T2".into(),
        payload: ExternalIOFinishedPayload {
            request_id: "REQ1".into(),
            status: "ok".into(),
            duration_ms: 3,
        },
        attachments: None,
        metadata: json!({}),
    };

    let got = [to_jsonl_line(&started).unwrap(), to_jsonl_line(&finished).unwrap()].join("\n");
    let expected = std::fs::read_to_string("tests/golden/wal_v2_external_io_sample.jsonl").unwrap();
    assert_eq!(got, expected.trim_end_matches('\n'));
}
