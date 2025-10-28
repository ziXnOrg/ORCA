use event_log::v2::{RecordV2, EventTypeV2, WAL_VERSION_V2, /* Attachment */};
use serde_json::json;

#[test]
#[should_panic]
fn invalid_digest_rejected_red() {
    let rec = RecordV2 {
        id: 42,
        ts_ms: 1234,
        version: WAL_VERSION_V2,
        event_type: EventTypeV2::TaskEnqueued,
        run_id: "R1".into(),
        trace_id: "T1".into(),
        payload: json!({"envelope_id":"EV1","agent":"a1"}),
        attachments: vec![Attachment { // digest is too short and should be rejected
            digest_sha256: "abc".into(),
            size_bytes: 1,
            mime: "application/octet-stream".into(),
            encoding: None,
            compression: "none".into(),
        }],
        metadata: json!({}),
    };
    // Expect panic or error during serialization/validation path in GREEN phase
    let _ = event_log::v2::to_jsonl_line(&rec).unwrap();
}

