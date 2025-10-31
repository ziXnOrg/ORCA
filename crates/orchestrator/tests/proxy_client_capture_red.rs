// GREEN minimal integration test for non-capture path (Issue #84)
// Verifies redaction helper behavior when `capture` feature is disabled.

#[cfg(not(feature = "capture"))]
#[test]
fn metrics_stubs_feature_gated_and_no_op_when_disabled() {
    let mut headers = http::HeaderMap::new();
    assert!(orchestrator::proxy::redacted_headers_from_http(&headers).is_none());
    headers.insert("authorization", http::HeaderValue::from_static("Bearer x"));
    let red = orchestrator::proxy::redacted_headers_from_http(&headers).unwrap();
    assert!(red.contains_key("authorization"));
}
// EOF
