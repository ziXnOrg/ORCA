// RED phase tests for client-side capture wiring (Issue #84)
// These tests intentionally fail to drive the GREEN implementation.

#[test]
fn client_emits_external_io_started() {
    // TODO(GREEN): perform a unary call through captured client and assert a WAL ExternalIoStarted
    // event is emitted with direction="client".
    assert!(false, "Expected ExternalIoStarted with direction=\"client\" to be emitted by client layer");
}

#[test]
fn client_emits_external_io_finished_with_correlation() {
    // TODO(GREEN): perform a unary call through captured client and assert ExternalIoFinished emitted
    // with deterministic request_id matching the started event.
    assert!(false, "Expected ExternalIoFinished with correlated request_id to be emitted");
}

#[test]
fn client_redaction_only_when_sensitive_headers_present() {
    // TODO(GREEN): issue request without sensitive headers and assert headers map is empty/minimal.
    // Then issue request with {authorization,cookie,x-api-key} and assert redaction present.
    assert!(false, "Expected client capture to redact sensitive headers only when present");
}

#[test]
fn metrics_stubs_feature_gated_and_no_op_when_disabled() {
    // Under otel feature, metrics stubs should be callable; otherwise compile out / no-op.
    #[cfg(feature = "otel")]
    {
        // TODO(GREEN): invoke metrics histogram/counter via client capture path and ensure no panic.
        // Optionally, validate labels are low-cardinality on future implementation.
        assert!(false, "Expected metrics stubs to be invoked under `otel` feature");
    }
    #[cfg(not(feature = "otel"))]
    {
        // TODO(GREEN): ensure metrics path compiles out / is a no-op when `otel` is disabled.
        assert!(false, "Expected metrics to be no-ops when `otel` feature is disabled");
    }
}

