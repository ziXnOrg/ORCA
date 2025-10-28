use event_log::v2::{to_jsonl_line, EventTypeV2, RecordV2, WAL_VERSION_V2, /* Attachment, BlobRef */};
use serde_json::json;

// RED: This test will fail to compile/run until attachments are added to v2::RecordV2
// and Attachment/BlobRef types are implemented with deterministic ordering.
#[test]
fn wal_v2_attachments_golden_red() {
    // TaskEnqueued with two attachments (intentionally out of order; expect sorted by digest)
    let rec = RecordV2 {
        id: 1,
        ts_ms: 1000,
        version: WAL_VERSION_V2,
        event_type: EventTypeV2::TaskEnqueued,
        run_id: "R1".to_string(),
        trace_id: "T1".to_string(),
        payload: json!({"envelope_id":"EV1","agent":"a1"}),
        attachments: vec![
            Attachment { // digest starting with "11" sorts after "00"
                digest_sha256: "11f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1f1".into(),
                size_bytes: 2048,
                mime: "image/png".into(),
                encoding: None,
                compression: "zstd".into(),
            },
            Attachment {
                digest_sha256: "00e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0".into(),
                size_bytes: 1024,
                mime: "text/plain".into(),
                encoding: Some("utf-8".into()),
                compression: "none".into(),
            },
        ],
        metadata: json!({}),
    };

    let got = to_jsonl_line(&rec).unwrap() + "\n";
    let expected = std::fs::read_to_string("tests/golden/wal_v2_attachments_sample.jsonl").unwrap();
    assert_eq!(got, expected);
}

